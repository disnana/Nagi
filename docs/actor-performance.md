# actorの測定

`std.actor`の公開前に、Supervisorに登録したactorへ一件ずつ送り、返信を待つ経路を測定しました。生成Nagiと同じ処理のRustを一つの実行ファイルに入れ、同じruntime・async呼び出しwrapper・依存・release設定で比較しています。測定したソースと条件は生ログに記録しています。現在の型付きAPIは[`std.actor`](actor-reference.md)を参照してください。

Rust側も同じSupervisorを使います。この測定で分かるのは、指定した経路での生成コードと手書きRustの差です。Supervisor自体の追加コストや、別の並行実行基盤の優劣は測っていません。

## 呼び出しと返信

AMD EPYC 9V74の共有ホストで、Tokioのcurrent_threadをCPU 0へ固定しました。環境のCPU quotaは4コア相当で、CPUを占有していません。受付処理の修正前→後→後→前の順に測り、各条件14回、各回100,000件の中央値を使います。

修正後の結果です。M件/秒は100万件/秒、p99は呼び出しの99%が完了する時間です。

| mailbox_ms | 入力 | Nagi M件/秒 | Rust M件/秒 | Nagi p99 µs | Rust p99 µs |
| --- | --- | ---: | ---: | ---: | ---: |
| 0 | i64 | 1.513 | 1.493 | 0.711 | 0.716 |
| 0 | str 64 bytes | 1.356 | 1.325 | 0.791 | 0.806 |
| 0 | str 4096 bytes | 1.260 | 1.244 | 0.842 | 0.862 |
| 5000 | i64 | 1.255 | 1.254 | 0.922 | 0.912 |
| 5000 | str 64 bytes | 1.129 | 1.086 | 1.027 | 1.021 |
| 5000 | str 4096 bytes | 1.017 | 1.033 | 1.122 | 1.096 |

`mailbox_ms=5000`でも、容量が空いていれば即座に受け入れます。不要な待機登録を避ける修正で、Nagiのi64は0.6954→1.2551 M件/秒、p99は1.582→0.922 µsになりました。0msは1.447→1.513 M件/秒でした。[生ログと修正前のpatch](../benchmarks/results/actor-stdlib/README.md)で条件とばらつきを確認できます。

時間には入力の生成、返信の破棄、各呼び出しの時刻取得を含みます。起動、warmup、測定後の並べ替えは含みません。順番に一件ずつ送るclosed-loopで、混雑時の受付待ち時間や固定の最大処理量を示す測定ではありません。

## 確保・待機・再起動

確保は時間測定とは別に10,000件を数えました。NagiとRustはどちらも、i64が1件につき1回・104 bytes、文字列は入力生成を含め2回・文字列容量+104 bytesでした。これは累計の確保要求で、常駐量ではありません。

この経路ではメッセージごとのBoxFutureやtask追加はありません。子の処理loopには起動ごとに一つの型消去されたfutureがあり、pollはvtableを通ります。

起動済みのactorを四つ、5秒間待機させたところ、プロセスのCPU tick増加は0でした。分解能は0.01秒なので、観測したCPU時間はその未満です。RSSは約2.6 MiBで変わりませんでした。warmup、測定用配列、allocatorも含むプロセス全体の値で、actor一つのメモリ量ではありません。

制御したhandlerの失敗では、既定の再起動間隔10msを含め、generation 2の返信まで約11msでした。再起動は新しい状態を作り、失敗したメッセージを自動で再配送しません。

## 再現する

以下は現在のソースを測る手順です。記録済みの結果と同じ実装を再現する場合は、生ログに記載されたソースのhashも確認してください。release compilerをビルドした後、リポジトリのルートで実行します。probeはofflineでビルドするため、依存を先に取得してください。`--cpu`は利用可能なCPUに合わせてください。

```sh
python scripts/actor_probe.py --nagic target/release/nagic --cpu 0 --iterations 100000 --rounds 7 --allocation-iterations 10000 --idle-ms 5000 --mailbox-ms 0 --output build/actor-probe-0.jsonl
python scripts/actor_probe.py --nagic target/release/nagic --cpu 0 --iterations 100000 --rounds 7 --allocation-iterations 10000 --idle-ms 5000 --mailbox-ms 5000 --output build/actor-probe-5000.jsonl
```

`--skip-build`はruntimeと入力の署名が一致するときだけ既存バイナリを使います。修正前とのABBA比較は[生ログの手順](../benchmarks/results/actor-stdlib/README.md)を参照してください。

HTTP、DB、複数コア、Elixir、Supervisorなしのmpscとの比較はしていません。容量・キャンセル・再試行の条件は[API](actor-reference.md)と[Supervisor](supervisor.md)を参照してください。
