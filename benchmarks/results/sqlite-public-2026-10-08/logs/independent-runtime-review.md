# SQLite公開runtime 独立設計・コードレビュー

2026-10-07 23:18 UTC。対象は immutable commit `e78f35a1d4272b1475e9763fc36870e04c46e2be`、base `676576724829e45b077b58628bfe2417e6cf3673`。先行runner commit `8cd703f`のcompiler実装・その後のworking diffは評価対象外。レビュー担当は実装者から独立し、source/docは読み取りだけで確認した。実装、test、期待値、dependency、version、release、mergeを変更していない。

**確認済みのP0/P1/P2 findingはない。承認済み契約の範囲で、専用lazy adapterへの移行を妨げるruntime blockerは見つからなかった。** これはruntimeの限定評価であり、compiler三構文・46入力・生成Rust・4 OS・最終immutable配布物のacceptance完了を意味しない。

確認した根拠はAGENTS.md、固定commitのmod.rs / adapter.rs / session.rs、73のSQLite unit oracle、外部public API integration、公開runtime判断、ADR 010、capacity一次調査・採用判断、close回帰、取得予算設計、Q002 API契約、および下記raw logである。実装者やagentの成功報告は実行証拠に使っていない。

## 主要な評価

| 評価対象 | 一次sourceとoracle | 判断・条件 |
| --- | --- | --- |
| lazy capacity・依存 | mod.rs:151 / 193、adapter.rs:80 / 518 / 560、runtime/Cargo.toml、Cargo.lock | Optionsは正数/usize/Semaphore上限と時間範囲/busy i32を検査するscalar値。openは全capacity slotを予約せずnativeを起動しない。ledger/idle増分のtry_reserveがALLOCATION境界。deadpool/deadpool-runtimeだけを削除し、新crate・版・feature変更はない。 |
| logical/native容量の分離 | adapter.rs:100 / 134 / 521 / 550 / 566 / 325、adapter_tests.rs:632 / 672 / 720 / 804 / 894 | permitはTokio owned permit、live native recordは実join/起動不成立まで保持。starting・取消・private detach時にもcapacity checkと登録を同ledger lockで行い、permit返却だけからreplacementを起動できない。healthy Aの終了までBのjoinを待たせる全worker fenceにもしていない。 |
| 取得予算 | mod.rs:231、adapter.rs:101 / 539 / 575 / 594、acquire_tests.rs:142 / 194 / 254 / 315 / 337 / 389 | 最初のpollで一度作った絶対deadlineをlogical待ちから新native登録まで渡す。Immediateは期限切れDeadlineと別。ready/open/BEGIN/busyには持ち越さず、通常timeoutだけでPoolを故障させない。健康なidle再貸出にはTokio TimeoutのReady優先という既存の限界があり、厳密な壁時計上限ではない。 |
| FIFO・permit refund | adapter.rs:96 / 141、public_tests.rs:317、Tokio 1.53.1 semaphore.rs:19 / 796、batch_semaphore.rs:686 | logical permit待ちをTokioに委譲し、取消はwaiter除去と予約済みpermit返却をTokioが実施する。native fence後のadmission順・BEGIN完了順までのFIFOは保証/実証していない。 |
| EOF cleanup・checkout所有 | session.rs:278 / 424 / 597 / 769 / 880 / 910 / 949、adapter.rs:693 / 698、tests.rs:406 / 435 / 452 / 481 / 573 | BeginRequestがcheckout ownerを保持し、worker lexical session末尾だけで返す。Txはsession senderのみ。満杯inboxにDropからrollbackをtry_sendする設計ではなく、EOFまで受理済みcommandsを処理し、native終端/cleanup後にpermitを返す。unpolled finish、送信前取消、begin reply喪失、commit後reply取消を別に扱う。 |
| close・最後のPool Drop | adapter.rs:51 / 87 / 291 / 712 / 808 / 844、adapter_tests.rs:166 / 299 / 325 / 347 / 450 / 480 / 599 / 1062、public_tests.rs:257 | closing共有化とBEGIN admissionは同ledgerで直列化。closeはsemaphoreを閉じ、permitの予約状態に依存せずidleを取り出してlock外でDropする。checkoutはWeak pool参照なので最後のowner Dropでclose要求でき、active Txは終端を続ける。close timeout/取消ではclosingを戻さない。Drop自体はjoin完了APIではない。 |
| native closeとactual join | adapter.rs:440 / 451 / 488 / 502 / 506 / 339 / 352、session.rs:469、adapter_tests.rs:565 / 981 / 1126 / 1172 | observerが先に起動し、そのclosureだけがnative JoinHandleを持つ。native join後にterminal causeをledgerへ公開し、それからcompletion counter/live record除去を通知する。ready reply、handle Drop、native close、actual joinを同じ事象にしていない。spawn失敗はstart_failedに分離しfake joinを数えない。observer自身のthread exitをjoinするAPIではない。 |
| lock/Drop順序 | adapter.rs:60 / 89 / 294 / 300 / 325 / 698 / 811 / 831、session.rs:220 / 229 | idle drain、WorkerHandle Drop、rejected BeginRequest Dropはidle/ledger lockの外。ledger→State.stats順に逆向きのState.stats→ledger呼出は見つからない。callback unwindもlockを保持したまま実行しない。確認対象の通常経路にlock循環は見つからなかった。 |
| poison・再利用・cause | adapter.rs:267 / 300 / 654 / 698、session.rs:229 / 630 / 744 / 901 / 943、tests.rs:382 / 504 / 525 / 542 / 603、public_tests.rs:200 / 225 / 275 / 295 | cleanup失敗・callback panic・recycle異常はretire/取得停止へ進み、補充retryを追加していない。普通のSQL/bind/decode Errでactiveが確認できる場合は継続する。primary/cleanupを別に保持し、COMMITTEDとcleanup Errを両立できる。新acquire/closeのFailureへ過去Tx outcomeを転用しない。 |
| SQL・typed row境界 | session.rs:54 / 663 / 702 / 711 / 776 / 845、tests.rs:31 / 79 / 169 / 219 / 279 / 318 / 361 | 常設authorizerはprepare/step/reprepare/finalizeまで残り、管理guardはhardcoded BEGIN/COMMIT/ROLLBACKだけに限定される。FromRowは管理権限下で実行しない。一文・匿名bind・readonly/column shapeはrusqlite metadataで検査する。Sql::Static保持、Sql::Owned/typed values移動、標準FromRow境界は公開本体に接続されている。 |
| production/public API・配布 | lib.rs:13、mod.rs:193 / 269 / 282、runtime/tests/sqlite_public_api.rs:25、scripts/releases/package.py:31 / 43、scripts/releases/test_release.py:215 / 279 | 公開本体はcfg(test)に隠れておらずprivate prototypeとの二重実装もない。openはpathをFuture構築時に所有化しSend+'static、query/all/execは&Txを借用、commit/rollbackはFuture構築時からTxをconsumeする。外部consumerはtest seams無しでbuild/run成功。配布predicateは本体を含めtest moduleを除外できる。既存Db sourceは本commitで未変更。 |

常設authorizerの検査は、禁止actionのstatic whitelistとnative testを照合した防御的契約確認である。包括的な悪意schema/SQL function sandbox、第三者targetでの再現やexploitationの評価はしていない。

## 非blockingの文書・保守メモ

1. **P3: FIFOの範囲を公開説明で限定する。** adapter.rs:1 / 549、public_tests.rs:317はlogical Semaphore slot待ちの公平性を示す。capacity>1ではstartup/open/BEGINとnative fenceの完了順が異なり得るため、全begin完了順のFIFO保証と説明しない。新schedulerを足す必要は見つからなかった。親担当から、公開Docsではlogical permit待ちだけへ限定する旨を確認済み。
2. **P3: ReplyLostのretired=falseの読解を明示する。** session.rs:525 / 593、mod.rs:105ではReplyLostをUnknown/retired=falseとして作る。callback panicの後はsession.rs:901 / 943がretireを記録する一方、当該callはoneshot喪失を先に観測し得る。tests.rs:617とadapter_tests.rs:526はpanic→Pool停止・close causeを検査するが、falseをhealthyの証明とする契約はない。retired=falseを「そのFailureに退役確定情報がない」と説明し、ReplyLost/Unknownから再利用・rollback済み・retry安全を推論させない。これは確認済みの安全性欠陥やStop契約違反としては分類していない。親担当から、この公開説明を追加する旨を確認済み。
3. **保守責任:** pool crateを外したため、idle pop/return、close drain、fallible reserve、Weak checkout、ledgerとの接続はNagiが保守する。idle expiry/background replacement/resize/min-idle/汎用hookを後で足す場合は、今回の小さな責任分界をそのまま成立済みと扱わず、同じclose/cancellation/native capacity/lock順oracleを再評価する。2thread/started connectionの費用は実測・宣言されており、公開直前に別のpool/observer構造を追加する根拠は見つからなかった。

## 独立実行とraw evidence

レビュー時に `git diff --exit-code e78f35a -- runtime Cargo.lock` が差分無しだった。runtimeを変更しないまま、以下をoffline/lockedで独立再実行した。compilerの変動中working diffをbuild/acceptanceとして評価していない。

```text
cargo test --offline --locked -p nagi-runtime --lib sqlite::
73 passed; 0 failed; 0 ignored; 147 filtered out; finished in 1.72s

cargo test --offline --locked -p nagi-runtime --test sqlite_public_api
1 passed; 0 failed; 0 ignored; finished in 0.00s
```

環境はLinux、PATH `/workspace/toolchains/cargo/bin`、RUSTUP_HOME `/workspace/toolchains/rustup`、CARGO_HOME `/workspace/toolchains/cargo`、CARGO_TARGET_DIR `/workspace/nagi-build-cache/check`、NAGI_NATIVE_TARGET_DIR `/workspace/nagi-build-cache/native`、dev/test debug=0、incremental=0、jobs=2。新しい巨大capacity確保、資源枯渇実験、独自fault/PoCは追加していない。比較testには性能thresholdがなく、今回の独立成功も性能保証には使わない。

実装側の下記raw log本文も確認した。以下は独立再実行とは別の証拠である。

| raw log | 読み取った結果 | SHA-256 |
| --- | --- | --- |
| runtime-full-tests-final.log | runtime lib219 pass/1既存ignored、SQLite public integration1、task integration3、doctest10 pass | `30d1886146d4045761f8cd97c2a33369172098e5dd2f8bf41851fda06f477ece` |
| runtime-clippy-final.log | runtime all-targets clippy warnings deny成功 | `16f9521e2c2c7fde238d20b8d5dc0b2156a05b61e084a8676df224f09fdbe36a` |
| release-package-tests.log | 37 tests、OK | `0f515f79a08d14b1a3f063ce12e4400ab2c25fc5630355afd544df1b4e45d727` |
| distribution-projection-tests.log | cfg(test)なしの独立runtime/外部consumer build、public integration1成功 | `898035a6b8ad921b24cd037d6dc0322db32cbe2858e86173e6fa890893f7bbfa` |
| runtime-comparison.log | 比較3 pass、数値threshold無し | `0497f3a89eaf25710bdca306f56d18cb95298bebf55df9f215767b24991cd552` |

artifact directoryは本記録と同じ `/workspace/nagi-sqlite-public-2026-10-08/`。distribution-projection/dependency-diff.jsonは新registry版0を記録する。runtime-public-cost.jsonはopen時native_created=0、通常cap2のnative_workers=2、close後joined=2/pending=0とworker＋observer各接続2threadモデルに整合する。Options/Pool/Txの48/24/8 bytesやwarm比較値を全heap量・性能threshold・他OSの予測に使うべきではない。

## 残るacceptanceと宣言済み限界

- Linuxのruntime契約を確認した。Windows/macOS各target、compiler三構文・46入力・生成app、最終head全suite/CIは本レビューでは未確認。4 OSの実行結果を別に受け取る必要がある。
- packaging predicate/unit fixtureとproduction投影は確認した。将来のrelease archiveや未レビューcompilerを含むimmutable配布物は作成・検証していない。
- source/contract/oracle照合と通常小さい既存testで評価した。全thread interleavingの形式検証、Miri/Loom、全OOM/allocator abort、process kill、unbounded blocking SQLの完了保証はしていない。
- close成功はnative close確認とnative workerのactual joinを待つ。最後のPool Dropの復帰やobserver thread自身の全終了は同じ保証ではない。
- SQLのErrを変更ゼロと扱わない。INSERT OR FAIL・AFTER triggerでの先行変更や受理済みSQLは取消だけでは戻らず、明示rollback/EOF cleanupの観測が必要。
- 原設計との違いはユーザー承認済みの専用adapter/依存除去/ALLOCATION/lazy open責務である。未採用vendor案のLayout上限を現在のscalar capacity契約へ再適用していない。

本レビューの結論は、上の限定scopeと2件の公開説明を維持した場合にruntimeの移行を続けられるというもの。merge/release/version変更を承認する記録ではない。
