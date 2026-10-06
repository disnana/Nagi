# SQLite private多接続の検証記録

2026-10-06。[設計](sqlite-multiconnection-design.md)に沿ったPhase 4の内部縦切り。公開Pool/Tx、取得期限、compiler capture検査は対象外。既存一接続の43件を維持して多接続の7件を追加した。

## 先行テストと失敗段階

設計commit `ed89794`、tests-only `e0384e3`を保存した。#83のmain反映も取り込んだsource `c4dfa04`では、Config.pathやwith_capacity等が未実装で、Cargoは14個のE0560/E0599を報告してexit 101となった。これは実行時の不具合再現とは別のcompile REDである。

容量だけを接続して単一reaperを残した中間source `3a9824d`では、次の試験を実行した。

```sh
cargo test --locked -p nagi-runtime \
  sqlite_prototype::adapter_tests::multi_cancelled_b_joins_and_c_begins_before_healthy_a_finishes \
  -- --nocapture
```

buildは成功し、接続AがactiveなままではBのpost-join publication barrierへ到達できず、10秒のfixture watchdogで失敗した。Aの終了とbarrier解放によるcleanupを行ってから、`B join was queued behind active A`のassertでexit 101。timeoutを成功・skipへ変えていない。中間実装は公開完成版にしない。

ローカル原本は`/tmp/nagi-sqlite-multiconnection-proof/`。compile REDログのSHA-256は`85aa64b862cd4e8fa4d179c58c40f42efb1cc4f920a4e93a47969d381dcf84d6`、runtime REDは`bfe45cf658c2e63f4cf8d4784ec074790ea7fa9c88cd6ca4634202c2f20a6351`。commitを残したため中間sourceから再実行できる。ログのhashだけを契約成立の根拠にはしない。

独立レビューでnative起動失敗も別分岐として検査する必要が分かった。先行test source `24fb95a`ではseam未定義のE0560 1件でexit 101。ログSHA-256は`1133c9ce77be30ba42aeac82f982a017d989001484bb290a60d5b24fc6159f12`。seamは実OS spawn失敗と同じcause→start_failed→reply経路へ接続した。これらのsource commitはローカルの観測IDで、公開PRのcommitと同一とは限らない。公開時には同じtree/実装bytesであることを確認する。

## 継続検査する範囲

| ケース | 観測する条件 |
|---|---|
| filesystemの2接続 | AのTxを保持したままBでINSERT/COMMITでき、Aが同じDBの行を読める |
| B startup取消 | Bの実native close/joinがAの終端前に観測され、次のCを開始できる |
| stock detachのpermit先行返却 | Bのhandleが生存する間もnative capacityを超えてCを起動しない |
| Bのterminal cause | Bの終了結果をAに待たせず公開し、新規取得はWorker error。既存Aの終端は続ける |
| closeとstartup | close FutureのDropでもclosingとjoin責任が残る。遅れたBからBEGINを受理しない |
| observer起動不成立 | native Bを開始せず、実join/native closeを捏造しない。failed-startは別counter |
| native起動不成立 | observer起動後もcauseをreply喪失に取り違えず公開する。起動していないBのjoin/close成功を数えない |

terminal cause試験は、filesystemの実native close後にprivate seamでcauseを注入する。実SQLite close errorそのものの多接続再現ではない。既存in-memoryのcleanup/close失敗試験は維持する。

## 共通原因と変更

一接続のnative終了fenceを複数接続へ広げる際、logical permit返却とnative終了が別時点であることに加え、終了観測も独立させる必要がある。容量の数値変更だけでは、単一reaperのblocking joinが健康な接続の後ろで別workerを止める。今回の反例はprivate多接続への拡張でP1となる条件で、公開版に存在する多接続Poolの障害とはしない。

nativeのlive登録とclosing/failed確認は同lockに揃えた。observerが先に起動し、自分のclosure内でnative JoinHandleを取得して実joinする。cause公開の後にcompletionを通知する。起動不成立はstart_failedへ分け、close成功やjoinを偽装しない。待機順とhealthy recycleはstock deadpool、SQL/cleanupは既存session coreを維持した。

fixtureにもP2の失敗時cleanupと観測不足があった。directoryを取得直後にRAII化し、setup失敗はgate解放・Tx終端・closeを試してから失敗とする。publication中にCを再pollしてPendingとlive登録保持を直接確認する。未joinの場合はDBを削除せず診断付きで保存し、Windowsの削除失敗による二重panicを避ける。成功ケースでも未joinを許す変更ではない。

## 成立した範囲

ローカル実装source `711ed4b`でruntime179 unit＋5 doctestが成功した。追加前の43件と新7件はすべて実行し、failed/ignoredは0。Solの独立sourceレビューではJoinHandleの所有、cause/accountingの順、lock順、strong-reference cycleにブロッカーは見つからなかった。

compiler、生成Rust backend、cfg(test)を除くruntime、依存、CI設定のbytesはmainから変更していない。公開checkの保証を広げた変更ではなく、予定High/Low 46入力をsemantic受理まで昇格したものでもない。既存2件はparser検査のまま。任意Rust・untrusted programの隔離や、SQL/COMMITの外部副作用rollbackを追加していない。

| ローカル検査 | 実行結果 |
|---|---|
| private native22＋adapter27＋比較1 | 50件成功、元43件の期待値を保持 |
| runtime全体 | 179 unit＋5 doctest成功 |
| 全workspace | 92 suite、881成功、failed/ignored 0。conformanceと既存生成探索も含む |
| fmt／all-target clippy | 成功、警告をerrorとして検査 |
| 既存fuzz smoke | 1000 mutation、95 checked Low emit、16 bounded native case、panic 0。coverage-guided fuzzの実施ではない |
| CI変更判定のPython試験 | 52成功 |
| website | 90ページ、local links/anchors/assets成功 |

原ログ・command/exit/hashと測定sampleは[検証artifact](../../benchmarks/results/sqlite-independent-observer-2026-10-06/README.md)に保存した。4 OS CIの実行結果は公開PRの最終headで確認し、ローカルLinuxの結果から成功を推測しない。

## costの観測

同じnative core、cap1、warm直列、memory DB、test/debug profileで各4096 Txを単独測定した。direct p50/p95は65.629/138.899µs、adapterは113.731/159.469µs。この条件ではadapter側が遅く、一般的なoverhead上限や高速化の証明にはしない。約2thread/connectionとconnection生成時のready Arc/Mutexを使うが、thread起動・allocationの費用は測定区間外。多接続のthroughput、本番性能、allocation count、Future size、memory、binary/compile timeは未測定。

## 完了判定と未確認

private縦切りの終了・容量契約を上の有限ケースで継続検査できる状態にした。実native close errorそのものの多接続回帰、observer自身の内部panic/OS abortの回復、公開APIは成立範囲に含めない。4 OS CIは公開後の確認対象。

| 残課題 | 分類・理由 | 次の観測／判断 |
|---|---|---|
| logical/native予約待ちの予算 | P1の予防。公開取得期限は未配線 | 0ms、残予算、同task別scope、取消、予約後startupのprivate先行テスト |
| 巨大capacity | P2の公開設計阻害。stock infallible allocationの扱い未確定 | allocation前の可表現性と受理範囲を決める。任意上限やOOM回復保証を勝手に追加しない |
| public Options/Failure/Parametersとcapture/registry/sealed SQL | Phase 4の未実装 | 予定46入力をsemantic/pass-fail/元位置/Rust buildへ配線し、実DBと4 OSで検証 |
| observer内部failureの観測 | P2の残る内部検証範囲 | 公開化前に内部panicがclose timeout等で観測される条件を点検。caller取消との混同を避ける |

Phase 4全体のacceptance、G-TX/G-POOLの公開保証、Phase 5、版更新、releaseの成功とは報告しない。

## #84のCIとmain反映

公開head `ada363eecdd59d231ea04157e2bc3570ffffa990`は、ローカル最終sourceと同じtree `117533295a2bbd16d12d413cb00f72076d17133e`であることを確認した。[checks run 37417583234](https://github.com/disnana/Nagi/actions/runs/37417583234)と[website run 37417583024](https://github.com/disnana/Nagi/actions/runs/37417583024)は成功。Windows x64、Linux x64、macOS Intel／Apple Siliconの各ログで、sqlite_prototype stepの50件（native22＋adapter27＋比較1）と、予定構文parser2件の実行・成功を確認した。別stepのpanic filter再実行を新しいcase数へ加算していない。

Linux全検査、PyCharm／IntelliJ、merge gateも成功。VSIX／releaseはこのPRではskip。外部の承認レビューはなく、SolのsourceレビューやCIと同一扱いしない。ユーザーが2026-10-06 05:47 UTCに[PR #84](https://github.com/disnana/Nagi/pull/84)をマージし、main `e7aff1da0a36503d239d70cf5dbcf892655978e0`を読み戻した。エージェントはmerge操作を行っていない。

後続の[取得予算](sqlite-acquire-budget-design.md)と[公開capacity](sqlite-capacity-decision.md)は別の縦切り・判断で、#84の50件からその成功を推測しない。
