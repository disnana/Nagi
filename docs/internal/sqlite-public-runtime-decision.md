# SQLite公開runtime: 採用実装と検証

> SQL表現・旧API併存に関する記述は当時の記録。SF00 #100の採用設計に基づき、SF05では旧Db/db_*とpublic Sqlを削除し、literal Queryへ統一する。現在の契約は[SF05契約](security-foundation/sf05-contract.md)、維持するPool/Tx lifecycleは[公開リファレンス](../sqlite-pool.md)を参照。過去の測定をSF05の検証結果として扱わない。

2026-10-08。正式化前の長期的妥当性を優先するユーザー承認に基づく実装判断。対象baseは`676576724829e45b077b58628bfe2417e6cf3673`。commit、push、release、版変更を行っていない。compiler/三構文/46入力/4 OSのacceptanceは別の検証で、以下のruntime成功だけでは完成扱いしない。

## Pool選択と保守責任

| 選択 | 容量と予算 | 終了と保守 | 結論 |
|---|---|---|---|
| deadpool 0.13.1 stock | 全capacityのinfallible予約。fallible builderなし | 既存prototypeとの接続済みだが公開capacity条件を満たさない | 不採用 |
| deadpool限定vendor | 実slot Layout＋fallible全予約APIは実現可能。同じqueueを保持できる | algorithmは維持できるが全capacity予約、upstream patch、runtime独立同梱が残る | 初回候補から不採用へ変更 |
| bb8 0.9.1 / mobc 0.9.0 | lazy。timeoutにready/checkまで含む等、同一取得予算との調整が必要 | close/idle drain safe APIがなく、strong checkout・背景回収・補充を別に制御する必要がある | 不採用 |
| 既存Tokio Semaphore＋専用adapter | scalar容量、idle/native ledgerの実増分だけfallible予約。同じAcquireBudgetを直接渡す | waiter公平性・取消はTokio。Nagiはidle/checkout/returnと既存native ledger/observerを保守 | **採用** |

Tokio 1.53.1 [Semaphore](https://docs.rs/crate/tokio/1.53.1/source/src/sync/semaphore.rs)はFIFO取得、owned permit、closeを提供する。[batch semaphore](https://docs.rs/crate/tokio/1.53.1/source/src/sync/batch_semaphore.rs)は取消時のwaiter除去・permit返却を扱う。native ledgerは既存のstarting/healthy/取消/detachedの容量fenceであり、新しいwaiter schedulerではない。close・実join・causeの責任はpool crateだけでは満たせず、どの候補にも既存ledger/observerが必要だった。lazy idle queueとpermit checkoutを小さく直接接続する費用を選ぶ。

独自のidle expiry、background replacement/retry、min-idle、resize、汎用hook/manager APIは加えない。依存はdeadpool/deadpool-runtimeを削除するだけで、既存Tokio/rusqlite/crate版・featureを変更しない。一次sourceと候補の具体transitive差は[capacity調査](sqlite-capacity-research-2026-10-08.md)を参照する。

## 公開契約と実装位置

`runtime::sqlite`へsessionとadapterを移し、private prototypeの二重実装を残さない。既存native oracleは公開本体を実行する。テスト用Driver、観測、gate、fault injectionは公開APIへ出さない。旧`Db`を維持する。

| Rust API | 所有・戻り値 |
|---|---|
| `options(i64, i64, i64, i64)` | `Result<Options, Error>`。順はconnections/queue_capacity/acquire_ms/busy_ms |
| `open(&str, Options)` | pathをFuture作成時に所有化、`Future<Result<Pool, Failure>> + Send + 'static` |
| `clone_pool(&Pool)` | `Pool`。clone共通のclosing状態 |
| `begin(&Pool, BeginMode)` | async `Result<Tx, Failure>` |
| `parameters()` | owned `Parameters` |
| `bind_i64(Parameters, i64)` / `bind_text(Parameters, String)` / `bind_bytes(Parameters, Vec<u8>)` / `bind_null(Parameters)` | owned `Parameters`を返す |
| `bind_f64(Parameters, f64)` | 非finiteを拒否する`Result<Parameters, Error>` |
| `query<T: FromRow>(&Tx, Sql, Parameters)` | async `Result<Option<T>, Failure>` |
| `all<T: FromRow>(&Tx, Sql, Parameters)` | async `Result<Vec<T>, Failure>` |
| `exec(&Tx, Sql, Parameters)` | async `Result<i64, Failure>` |
| `commit(Tx)` / `rollback(Tx)` | Future作成時からTxをconsume、async `Result<(), Failure>` |
| `close(&Pool, i64)` | async `Result<(), Failure>`。msは非負native時間範囲 |
| `copy_primary_error(&Failure)` / `copy_cleanup_error(&Failure)` | `Option<Error>`。causeのkind/messageを明示コピー |

BeginModeはDeferred/Immediate/Exclusive、OutcomeはNotApplicable/Active/Committed/RolledBack/Unknown。FailureKindはInvalid/Closed/AcquireTimeout/Busy/Sql/Bind/Decode/Aborted/Cleanup/Worker/ReplyLost/CloseTimeout/**Allocation**。Failure fieldはkind/outcome/retired/message、causeはprivate。Debugはkind/outcome/retiredだけでSQL・path・cause/messageを表示しない。Options/PoolのDebugも値・metadataだけ。Tx/Parameters/Failureのpublic Clone/Copyを設けず、Txのpublic Debugを設けない。

Optionsは正数・usize・Semaphore上限とnative時間、busy_msのi32を検査する。全capacity slot配列がなくなったので実slot Layout境界は適用しない。openは空path/URI/複数connectionの`:memory:`を拒否する。native Connection/workerはbeginでlazy起動し、filesystem/native open失敗はWORKERでDatabase causeを保持する。URI flagをnative openに渡さない。

logical permitからnative record登録まで同じ絶対取得予算を使う。登録後のready/BEGIN/busyは期限外。0msは空き条件成立なら成功する。recycle失敗で補充/retryへ進まず、Poolを止めてcauseを保持する。Sql::Staticはstatic参照を保ち、Sql::Ownedと文字/bytes Parametersは所有を移す。

user SQLは一文・匿名bind・typed rowdecode・操作形をnative metadataで検査する。Transaction/Savepoint/Pragma/Attach/Detach/Unknownを常設authorizerで拒否し、管理BEGIN/COMMIT/ROLLBACKだけがprivate権限を持つ。prepare/step/reprepare/finalizeまでhookを保ち、FromRow実行を管理権限へ入れない。SQLite Busy/LockedはBusy cause、その他native ErrorはDatabase causeを保持する。

## 予約失敗とDrop

native record追加前の`try_reserve(1)`失敗はALLOCATION/NOT_APPLICABLE/retired=falseで、workerを起動しない。idle returnの`try_reserve(1)`失敗はResultを返せないため、ALLOCATION causeをledgerに保存してretired=true、新取得停止、idleをlock外で破棄する。worker senderの解放からnative cleanup/close、独立observerの実joinへ責任が続き、cause公開後にのみlive recordを除く。Dropが戻ったことを回復や終了完了と数えない。

最後のPool owner Dropでもcheckout/TxはPoolを強参照しない。native request内のcheckoutはTx/session EOF cleanupを終えてから返る。active Txは終端を続けられ、close取消/timeoutで共有closingを解除しない。close/acquireはTxがないためNotApplicableを返し、過去のTx terminal outcomeを新取得へ転用しない。

小さいfault injectionだけを使い、巨大allocation/資源枯渇実験は行わない。Arc/String/Tokio内部等のallocationがすべてResultになるとは保証せず、OOM/allocator abortの普遍回復を保証しない。

## 実行証拠と配布

証拠directoryは`/workspace/nagi-sqlite-public-2026-10-08/`。Linux実行環境で次を確認した。先の71 oracleにpublic FIFO/取消と通常cap2 costを追加し、SQLite unit oracleは73本になった。

| 検証 | 結果 / artifact |
|---|---|
| runtime全suite、既存Db/HTTP/taskを含む | lib219 pass、1既存ignored、SQLite公開integration1、task integration3、doctest10 pass。`runtime-full-tests-final.log` |
| runtime clippy all-targets、warnings deny | pass。`runtime-clippy-final.log` |
| release packaging unit | 37 pass。`release-package-tests.log` |
| 独立production runtime＋外部public API consumer | pass。`distribution-projection-tests.log` |
| warm比較・budget比較・cap2費用 | 3 pass、数値thresholdなし。`runtime-comparison.log`と下記JSON |

最初の全suiteではsandbox socket bind EPERMによる既存HTTP35失敗があり、network許可付きで全suiteを再実行して成功した。SQLite契約失敗として数えない。4 OSはCIの実行結果を待ち、Linux成功を他targetの証明にしない。

配布は新vendorがなく既存`runtime/src`許可集合に収まる。release package unit fixtureへsqlite/mod.rs・adapter.rs・session.rsを加え、`*_tests.rs`除外を確認する。未commit sourceを実release archiveにしたとは報告せず、`package.included`と`runtime_manifest`を用いた独立production投影をscratchへ作り、runtime cfg(test)を有効化しない外部consumerをbuild/testした。base lockの既存registry版を維持し、投影で新registry版がないことを`distribution-projection/dependency-diff.json`へ保存した。実release archiveは将来のimmutable commitから作る。

debug profile/current-thread、cap1 warm serial、32×128 transaction/backendの一回観測で、direct p50/p95は114611/314969 ns、専用adapterは117056/391033 ns。取得budgetありは115781/298179 ns、private無期限fixtureは116441/379032 ns。未poll begin futureのsize_of_valはbudgetあり784 bytes/なし768 bytesで、heap全量や生成Nagi futureの測定ではない。raw sampleは`runtime-adapter-comparison.json`/`runtime-budget-comparison.json`へ保存した。性能保証・合否threshold・他OS予測に用いない。

通常cap2の`runtime-public-cost.json`ではopen時native_created=0、active native_workers=2、close後native_joined=2/pending=0。Linux process threadは2→6、worker＋独立observerの2thread/started connectionと整合した。Options/Pool/Txは48/24/8 bytes、openは38281 nsだった。指定容量に比例する全heapを測った値ではない。
