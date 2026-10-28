# ローカル過負荷・回復・Supervisor検証（2026-10-27）

Nagiの現行`main` `0d1884cb5a24235a440811a9f5d16be1bf5a664a`を基点とする本変更候補を、
同じ管理環境内のloopbackだけで検証した記録です。各`environment.json`には実行binaryとharnessの
SHA-256があります。外部サービスや公開ホストへ負荷を送っていません。この結果は
インターネット上のDDoS耐性、回線容量、TLS性能、複数processや複数hostの能力を示しません。

## 有限条件

- HTTP/1.1、`127/8`、Nagi server 1 worker。serverとclientを別CPUへ固定。
- cgroup上限は4 CPU / 16 GiB。HTTP測定はserver RSS 2 GiB、cgroup 12 GiB、50,000 request、
  client 512 worker / 256 connection、各phaseの有限時間で停止します。
- [Vegeta v12.13.0](https://github.com/tsenart/vegeta/releases/tag/v12.13.0)の公式Linux binaryを
  記録済みSHA-256と照合しました。
- `examples/crud.nagi`の既定route制限（burst 1,000、以降1 token / 1,000 ms）を変更していません。
- 3回のHTTP試行は別process、別DBから開始しました。各試行内では通常phaseと過負荷phaseに
  異なるloopback source addressを使い、3試行では同じ2 addressを再利用しました。

## 通常通信と過負荷

最初に`/health`へ100 request/sを3秒、続けて10,000 request/sを3秒送信しました。過負荷中は
別bucketの`/small`を0.1秒間隔で確認し、終了後は`/health`が再び200になるまで有限時間だけ確認しました。

| 観測 | 中央値 | 3回の範囲 |
| --- | ---: | ---: |
| 通常phase HTTP 200 | 300 / 300 | 全試行同じ |
| 通常phase p99 | 1.492 ms | 0.976–1.528 ms |
| 過負荷phaseの実送信 | 30,000 | 全試行同じ |
| 過負荷phase HTTP 200 | 705 | 全試行同じ |
| 過負荷phase HTTP 429 | 29,295 | 全試行同じ |
| 過負荷phase HTTP 500 / 503 | 0 / 0 | 全試行同じ |
| transport failure (`status 0`) | 0 | 全試行同じ |
| 過負荷phase p99（200と429を含む） | 58.042 ms | 37.358–79.128 ms |
| 過負荷中の別route HTTP 200 | 32件 | 30–33件、全件200 |
| 別route p99 | 2.505 ms | 2.067–3.909 ms |
| server CPU（1 core比） | 43.75% | 40.99–47.99% |
| server peak RSS | 10,168 KiB | 8,740–10,828 KiB |
| server peak FD | 150 | 141–156 |
| 回復確認後のFD | 10 | 10–11 |

429は安全なrate load sheddingとして数え、成功responseへ混ぜていません。応答容量が一時的に
枯渇した場合の503も許容対象ですが、修正後の3試行では発生しませんでした。過負荷phaseのp99は429を含む
全responseの値で、business requestだけの処理latencyではありません。終了後、同じrouteは3試行とも
有限期限内に200へ戻りました。回復確認後のRSSは8,672–9,732 KiBでした。allocatorの保持を含むため、
この短時間のRSSだけでmemory leakの有無を断定しません。公開queue長は取得できません。

修正前の追加試行では、同じ過負荷中に200が655件、429が29,295件、500が50件発生しました。
`http-resilience-pre-fix-500/`にその失敗証拠を残しています。原因は通常応答枠の容量不足が`Busy`になった後、
既定error mapperも同じ通常枠を使って再び`Busy`となり、mapper失敗が一律500へ変換されていたことです。
修正では`Busy`だけを独立fallback枠の503へ保ち、それ以外のmapper失敗は引き続き詳細を伏せた500とします。
固定回帰では通常応答枠を意図的に使い切って503を確認し、解放後に通常応答を再作成できることも確認しました。

## 低速接続、途中送信、異常切断

無送信、途中header、途中body、未使用keep-aliveを各25本、合計100接続作り、15秒間、別routeの
通常通信を毎秒1件確認しました。最後の観測では全100 clientがEOFを確認し、途中body 25件には
HTTP 408が返りました。通常通信15件はすべてHTTP 200、median 0.644 ms、p95 0.855 ms、最大
0.885 msでした。

serverはbaseline 10 FD / 4,976 KiB、観測中最大111 FD / 7,472 KiBでした。client close後は
10 FD / 7,476 KiB、追加の200回のRST後は10 FD / 7,492 KiBでした。Linux TCPにはTIME_WAITが
残るため、process FDの回収をTCP state全消滅と同一視しません。短時間のRSS値だけでmemory leakの
有無を断定しません。

## ActorとSupervisor

公開Nagi actor経路と同じruntimeを使う手書きRust経路の故障注入では、どちらもmailbox満杯を拒否し、
故障したreplyを再配送せず、generation 2でstate 1から再開しました。故障から回復responseまでNagi
11.167 ms、比較Rust 11.191 msでした。1回の共有host観測であり、性能優位は主張しません。

runtimeの合成restart probeでは次を確認しました。このprobeは`std.actor`の公開Supervisor実装を通らず、
Tokio taskを直接spawnして再起動規則と兄弟taskの進行を測る補助試験です。

- controlled panic 3回を3回再起動。intensity停止なし。兄弟workerは11回進行。再起動観測2.042–2.153 ms。
- 1秒window / 上限2のcrash loopは2回再起動後に停止。兄弟workerは9回進行。
- queueは1,000 job中909完了、91 dead-letter、312 retry、peak in-flight 8。91件は設定した失敗policyの
  結果で、909 + 91 = 1,000件をすべて最終状態まで追跡しており、未記録の取りこぼしではありません。
- 100,000 taskのcreate/joinを3回、順番に実行し、各回completed 100,000、unfinished 0。同時に
  300,000 taskを保持した試験ではありません。

最後のtask試行後もprocess RSSは約39 MiBでした。allocator、測定配列、runtimeを含むprocess値であり、
taskごとの解放量やmemory leak不在の証明ではありません。panicは同process内の協調的なRust taskです。
この合成probeではphaseごとのCPU使用率とFD推移を別々に記録していないため、HTTP測定のCPU／FD値を
Supervisorの結果へ流用しません。
SIGSEGV、abort、OOM、process kill、yieldしない処理、native blocking処理の隔離は検証しておらず、
BEAM/Elixir process相当とは扱いません。

Go backendはこの時点でHTTP、actor、Supervisorに未対応なため、Rust/Go比較は実施していません。

## 測定基盤で修正した問題

検証開始時に、測定基盤そのものの問題を4件と、runtimeの負荷時status分類の問題を1件再現しました。

1. `http_capacity.py`がVegeta v12.13.0にない`report -buckets`を渡し、attack結果を読む前に終了していた。
   JSON reportに元から含まれるpercentileを使い、未対応flagを削除しました。
2. loopback clientが環境のHTTP proxy設定を継承し得た。PythonとVegetaのproxy継承を明示的に無効化し、
   `127/8`の自己管理対象へだけ接続するよう固定しました。
3. keep-aliveのraw readerが期待外response後のEOFを検出せずCPU loopになった。有限header/body、
   `Content-Length`、EOF検査へ置き換え、現行authorityに合わせて`Host: localhost:8080`を送ります。
4. 途中body fixtureだけportなしHostを送り、body期限ではなく入口authority拒否の400を測っていた。
   許可済み`localhost:8080`へ直し、25件すべてがbody待ち期限の408になることを再測定しました。
5. 応答容量不足から既定mapperも容量不足になった場合、`Busy`が500に変わっていた。fallbackの503へ
   分類を保ち、500を内部障害だけに戻しました。

開始時点で共有cgroup memoryが指定ceilingを超えている場合も、Vegeta失敗へ誤分類せず明示的に拒否します。

## 再現

```sh
cargo build --locked -p nagic
NAGI_NATIVE_TARGET_DIR=/path/to/native-target \
  target/debug/nagic build examples/crud.nagi --out build/local-resilience/crud

python scripts/http_resilience.py \
  --binary /path/to/generated/crud-binary \
  --vegeta /path/to/verified/vegeta-12.13.0 \
  --out build/http-resilience-run

python scripts/http_connection_lifecycle.py \
  --binary /path/to/generated/crud-binary \
  --out build/http-lifecycle --seconds 15 --count 25

python scripts/actor_probe.py --nagic target/debug/nagic \
  --phase lifecycle --iterations 2000 --rounds 1 \
  --allocation-iterations 100 --idle-ms 500 --mailbox-ms 5000 \
  --output build/actor-lifecycle.jsonl

cargo run --locked --release -p nagi-runtime --example concurrency_bench
```

HTTP各試行のbinary、harness、load generator hash、resource snapshotは各`environment.json`と
`result.json`にあります。`actor-lifecycle.jsonl`は生成Nagi / Rust比較のsourceとbinary hashを、
`concurrency-environment.json`はruntime source hashと実行環境を記録します。
