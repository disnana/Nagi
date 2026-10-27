# Go private lifecycle kernel: 最初の独立slice

基点は#186の`3c0ad0d`。`compiler/go-runtime/internal/lifecycle`はstd-onlyの
私有Go moduleで、生成Go applicationへまだ組み込まない。公開`std.actor`を
早く受理するための簡易代替ではない。Rustの公開契約は
[actor reference](../actor-reference.md)、[Supervisor](../supervisor.md)、
`runtime/src/actor.rs`と`actor/lifecycle.rs`を根拠とする。

## このsliceで観測する境界

- 登録は実行しない。Startは一度だけ。名前重複・上限・停止後登録を拒否する。
- managerはRegistered→Running→Stopping→Cleaning→Completeを管理する。
  子ごとのgeneration、active join record、pending restartを保持する。
- Factory(ctx)は世代ごとの子goroutine内で呼ぶ。panic回復境界とrecordを
  factory呼出しの前に用意し、子の正常return・Error・panicとdefer完了を観測する。
- one-for-oneで失敗した子だけを再起動する。TEMPORARYは再起動せず失敗を記録し、
  健全な兄弟は続く。TRANSIENTはworker Error/panic、PERMANENTは正常終了も対象。
- 再起動予算はgroup全体の試行回数。初回起動を含めず、窓は(now-window, now]。
  正確な境界の古い試行は失効する。上限超過は兄弟も停止し、terminal failureを返す。
- RequestStopは冪等。pending restartを取り消す。明示取消は再起動理由にしない。
  本物のworker failureが停止と競合した場合は失敗を失わない。
- StopAndWait成功は子callbackとdefer、明示cleanup hookの終了後だけ。
  deadline/cancelで待ち切れなければIncompleteを返し、live recordを残す。
  別waiterが後で実終了を確認できる。
- cleanup hookは全子join後に一度だけ、lock外で呼ぶ。hookがblockしている間は
  Cleaningのobligationを残す。hookのpanicも終了原因へ記録する。
- 完了後はfactory/cancel/cleanup callbackと再起動historyを解放する。
  Snapshotはlogical obligationの観測で、GCによるphysical memory回収の証拠ではない。

Factoryのerrorはworker failure専用。business Errを返信として処理してFactoryが
成功するnative例は再起動しないが、これはTurn/mailbox/CallErrorの実装証拠ではない。
native errorの任意objectをterminalへ保持せず、Error()の結果を1024 bytesへ限定して
copyする。Error()もretention/panic境界の内側でlock外に呼ぶ。panic payloadはformatせず、
固定したfailureへ変換する。

このkernelのcleanup hookは私有native契約であり、Nagi ownerのDrop、shared[C]の
最後の所有者、任意native resourceのclose完了を実装したものではない。これらを
publicへつなぐには、checked ownership factsに基づく決定的release計画が別途必要。

## まだ受理しない範囲と依存順

Go backendのpublic std.actor操作、async/await、view/shared、scope/taskは拒否を維持する。
Go toolchainを欠損させたHigh・手書きLowでもactor.default_optionsの未対応診断と元行を
要求する。Rustで動いたtestやbenchmarkをGo保証へ転用しない。

次の独立sliceはcold async（引数captureと本体開始を分離）、borrow/callback identity、
owner cleanup、scope cancellationと実joinの順に封印factsへ接続する。operationは
canonical Operation IDを保持し、Rust pathや関数名文字列から再判定しない。
これらのgate後にscalar限定のpublic actorを検討する。mailbox/Turn、READY/events、
count/byte charge、reply期限、accepted-workの扱いは別sliceである。

M/R/Eの所有、処理中を含む容量、call受理後のtimeoutはrollbackでないこと、
reply生成時刻による期限判定、古いgenerationへ送ったmessageの非再配送を維持する。
RustのString/Vec capacityとGoのstring/slice保持量・native envelope sizeは同じとは
限らない。容量計量を長さだけで代替せず、backend固有layoutと共通の許否契約を
設計してから対応型を増やす。

## failureの限界

同じ子goroutineのrecoverable panicとworker Errorだけを隔離する。
OOM/abort/os.Exit/runtime fatal/Goexit、別goroutineのpanic、任意native障害を回復できるとは
扱わない。panic捕捉は外部副作用のrollbackではない。Go schedulerがpreemptできても
goroutineの安全な強制終了APIはなく、非協調callbackの資源を回収済みとは報告しない。
finalizer/GCやcancel通知をcleanup成功の代わりにしない。

## 検査と有限測定

12 native testをGoのJSON test結果で名前集合まで照合する。zero/skip/fail/重複/欠落を
成功にせず、既存Go integrationと同じ3 OS gateへ接続する。Linux/macOSは27件、
WindowsはUnix symlink固有1件を除く26件。raceは対応C compilerがある環境で別実行し、
CGO無効の通常実行と混同しない。既存Rust actor/lifecycle testは削除しない。

native testはone-for-one、global budget、boundary、正常PERMANENT、business/worker
failure、stop/restart race32回、blocked worker/defer/cleanup、sticky failure、
最終record/callback解放をbarrier付きで確認する。非協調例もtestのreleaseで有限に戻す。
go testの30秒と外側process-tree deadlineで無限実行を防ぐ。

benchmark入口はREADMEのBenchmarkLifecycle。64/1024 childの起動→待機→完全停止に
限定し、回数は1、process timeoutは30秒。性能の数値やRust/Go優劣はまだ主張しない。
共有cache・低並列を使い、workspace空き8GiBを下回る測定は始めない。

後続のlocalhost probeを追加するときは127.0.0.1のephemeral portだけにbindし、
controller接続数16、child上限1024、wall time60秒、warmup2秒、測定5秒、drain5秒、
request上限10,000、target process RSS上限1GiBを既定の有限条件にする。上限到達は
停止理由を保存して打ち切る。これは私有測定controlであり、Go HTTP対応ではない。
kernel段階では起動/停止/再起動/idleとlogical recordsだけを測り、mailbox throughputを
測ったとは言わない。1/2 vCPUを分け、CPU quota、toolchain、GOMAXPROCS=1/2、GC設定、
開始/最大RSS、再起動数、実join結果、raw log/source hashを記録する。1GiBはtarget processの
範囲で、load generatorとcontainer全体の使用量も別に監視し、共有CPUの影響を記録する。
実成功/s、CPU/成功、peak RSS、p99、故障中成功率とallocationを区別する。
Rust側も同じ入力と停止条件で再測定して初めて比較し、既存Rust公開値を流用しない。
短時間の成功から長時間安定性を断定しない。外部actor libraryや独自schedulerを先に導入せず、
実測したbottleneckを根拠に採否を判断する。

## ローカル観測（最終CIとは別）

Linuxで実Go integration27件、compiler library192件、Go native12件、native race、
go vet、nagic all-targets Clippy、fmtが成功した。64/1024 childの有限benchmarkは
入口のsmokeとして実行できたが、固定CPU/GCやRust同条件比較の性能値ではない。
CI policy223件の初回は古い25/24件の手書きmatrixを持つ15 assertionが失敗した。
matrix期待を27/26へ同期し、検査の緩和はしていない。同期後の再検査と独立最終review、
3 OS exact-head CIを完了条件とする。
