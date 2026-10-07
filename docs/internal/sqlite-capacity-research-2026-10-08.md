# SQLite capacityの一次調査と採用判断

2026-10-08の作業記録（一次資料取得は2026-10-07 UTC）。対象baseは`676576724829e45b077b58628bfe2417e6cf3673`。初回は未承認候補を提示し、その後ユーザーが「正式化前に長期的に妥当な設計」を優先してSQLite公開に必要な依存・API・Failure分類変更と実装検証を承認した。巨大allocation/resource exhaustionの実験は行っていない。

## 現在の採用判断

**既存Tokioの公平なSemaphoreと、小さな専用adapterを採用する。** `runtime::sqlite`のidle queueは空で始め、使われた接続だけを保持する。deadpool/deadpool-runtimeを削除し、新crate・vendor・版更新は追加しない。native容量・独立join observer・native session・常設authorizerは既存prototypeから一度だけ移し、二重実装を残さない。

Tokio 1.53.1 `sync/semaphore.rs`のFIFO待機と`acquire_owned`/`close`、`sync/batch_semaphore.rs`のwaiter取消を使うため、独自waiter queue・公平性・取消schedulerを実装しない。Nagiが保守する追加部分は、idle FIFOのpop/return、permitを持つcheckout、weak pool参照、create/recycleと既存ledgerの接続である。background retry、idle expiry、resize、min-idle、汎用manager/hook APIは公開しない。この責務は「stock wrapperを維持する」より増えるが、bb8/mobcへ別のclose/drain/予算layerを足す場合より直接に契約を表せる。

限定vendorは実現可能だが、全capacity予約を続ける構造と第三者patch/配布保守が残る。bb8/mobcはlazyだが、下記のcloseと取得予算の差を補う必要がある。専用adapterは全capacityのcontainerを作らないため、connectionsにprivate slot全体のLayout上限を課す根拠がなくなる。Optionsは正数・usize・Semaphore上限、時間表現とbusy_msのi32境界を検査する。実containerの増分予約は`try_reserve(1)`で検査し、`ALLOCATION`を返す／記録する。最大値のOptions検査は算術だけであり、同じ値のSemaphoreやcontainerやworkerを作る実験ではない。

openはpathの所有化・組合せ検査と小さい管理構造の作成まで。native open/startはbegin時のlazy起動で、取得timerはrecord登録で終了する。Options/open成功が将来の確保成功を証明するとは宣言しない。idle返却の予約失敗はDropからResultを返せないため、causeを共有ledgerへ記録し新取得を停止、idleと該当workerを退役させ、実joinまでrecordを保持する。未起動record予約失敗は`NOT_APPLICABLE / false`、返却で接続を退役させた場合は`NOT_APPLICABLE / true`。allocation以外のArc・String・Tokio内部等のheapまでfallibleにしたとは扱わず、OOM普遍回復は保証しない。

現在の実装・分類・検証証拠は[公開runtime判断](sqlite-public-runtime-decision.md)へまとめる。以下のvendor案と全slot Layoutの受理範囲は**以前の未適用候補**として残す。現在の契約ではない。

## 初回に提示した具体案（未採用・未適用）

今回の公開縦切りを完成させる候補として、**deadpool 0.13.1の限定vendor patchを承認する案を推奨する**。crate自身がprivate slotの正確なLayoutを無確保で検査し、その同じslot queueをfallibleに予約してPoolへ渡す。既存のslot選択・Semaphore待機・取消・recycle・close algorithmは維持する。Nagi内へprivate `ObjectInner`やpool algorithmを複製しない。

承認対象は、(1)固定release sourceのvendor同梱と以下の限定API、(2)公開`FailureKind.ALLOCATION`の追加、(3)Options/openの責務、(4)独立runtimeと配布物へのvendor同梱・検証である。新しい数値capやOOM普遍回復保証は含めない。代替は依存/Failure集合を維持し、SQLite公開配線を保留して既存Docs/先行REDだけを扱うこと。upstreamへの投稿・push・PR・merge・releaseはこの承認に含めない。

## 最新deadpoolの一次source

crates.ioの[metadata](https://crates.io/api/v1/crates/deadpool)で最新stableは**0.13.1**、deadpool-runtimeは**0.3.1**。0.13.1 release archiveはversion metadataのchecksumへSHA-256照合した。ローカルCargo cacheの同版とも照合する。現在のfeatureは`default-features=false, managed, rt_tokio_1`で、版/featureを変えるだけでfallible allocation APIが得られる候補は確認できない。

[builder.rs](https://docs.rs/crate/deadpool/0.13.1/source/src/managed/builder.rs)の`build`（90行）は`Result`を返すが、`BuildError`は`NoRuntimeSpecified`だけ（14–17行）。allocation失敗を返すbuilderではない。[pool.rs](https://docs.rs/crate/deadpool/0.13.1/source/src/managed/pool.rs)の73行は`VecDeque::with_capacity(max_size)`、grow/resizeの277行は`reserve_exact`。0でbuild→resizeでも回避できない。[object.rs](https://docs.rs/crate/deadpool/0.13.1/source/src/managed/object.rs)の`ObjectInner<M>`（43行）は`pub(crate)`で、`M::Type`に加えid/metricsを含む。公開ObjectのsizeofやWorkerHandleだけで正確なslot Layoutを検証したことにはならない。

upstream mainもcommit [`f4efd12e65b2cc9347f6ca0ee42800cd376ea658`](https://github.com/deadpool-rs/deadpool/commit/f4efd12e65b2cc9347f6ca0ee42800cd376ea658)の[builder](https://github.com/deadpool-rs/deadpool/blob/f4efd12e65b2cc9347f6ca0ee42800cd376ea658/crates/deadpool/src/managed/builder.rs)と[pool](https://github.com/deadpool-rs/deadpool/blob/f4efd12e65b2cc9347f6ca0ee42800cd376ea658/crates/deadpool/src/managed/pool.rs)を固定して取得した。同じinfallible確保で`try_build`/`try_reserve`はない。releaseと未releaseのmainを区別する。

## 限定vendor案のレビュー可能な差分

以下は**未適用・未buildの擬似diff**。crate内のmanaged exports/builder/poolだけを対象とし、既存`BuildError`/`build`の互換性を保つ。新errorは既存BuildErrorをnonCopy化せず別型で表す。

```diff
 // managed module exports（名称は実装時に確定）
+ pub enum CapacityValidationError { SemaphoreLimit, SlotLayout(LayoutError) }
+ pub enum TryBuildError {
+     Configuration(BuildError),
+     Capacity(CapacityValidationError),
+     Allocation(TryReserveError),
+ }

 // Pool<M,W>内。ObjectInnerはcrate外へ公開しない。
+ pub fn validate_max_size(n: usize) -> Result<(), CapacityValidationError> {
+     if n > Semaphore::MAX_PERMITS {
+         return Err(CapacityValidationError::SemaphoreLimit);
+     }
+     Layout::array::<ObjectInner<M>>(n)
+         .map(|_| ())
+         .map_err(CapacityValidationError::SlotLayout)
+ }

 // 既存from_builderは新しい共通constructorへ自分のqueueを渡すだけ。
  pub(crate) fn from_builder(builder: PoolBuilder<M,W>) -> Self {
-     // PoolInner中でVecDeque::with_capacity(...)
+     let slots = VecDeque::with_capacity(builder.config.max_size);
+     Self::from_builder_with_slots(builder, slots)
  }
+ fn from_builder_with_slots(builder: PoolBuilder<M,W>,
+                            slots: VecDeque<ObjectInner<M>>) -> Self {
+     // 既存PoolInner初期化をここへ一度だけ移す。
+     // Slots.vec=slots。max_size/size/manager/semaphore/hooks/runtimeは既存のまま。
+ }
+ pub(crate) fn try_from_builder(builder: PoolBuilder<M,W>) -> Result<Self,TryBuildError> {
+     Self::validate_max_size(builder.config.max_size).map_err(TryBuildError::Capacity)?;
+     let mut slots = VecDeque::new();
+     slots.try_reserve_exact(builder.config.max_size).map_err(TryBuildError::Allocation)?;
+     Ok(Self::from_builder_with_slots(builder, slots))
+ }

 // PoolBuilder内。runtime/timeouts整合検査は既存buildと共通化する。
+ pub fn try_build(self) -> Result<Pool<M,W>,TryBuildError> {
+     self.validate_configuration().map_err(TryBuildError::Configuration)?;
+     Pool::try_from_builder(self)
+ }
```

crate内部なので`Layout::array::<ObjectInner<M>>`は実型へ適用できる。[Layout::array](https://doc.rust-lang.org/std/alloc/struct.Layout.html#method.array)は計算検査であり確保しない。[VecDeque::try_reserve_exact](https://doc.rust-lang.org/std/collections/struct.VecDeque.html#method.try_reserve_exact)が成功した**同じqueueを保持する**。試しの予約を捨ててstockで再確保する案ではない。stableの`TryReserveError::kind`は使用しない。事前Layout検査と予約失敗のerror経路を分離する。

実行環境のRustは`rustc 1.99.0 (b940084d7 2026-09-28)`。同commitの[VecDeque source](https://github.com/rust-lang/rust/blob/b940084d7/library/alloc/src/collections/vec_deque/mod.rs#L1159)、[RawVec source](https://github.com/rust-lang/rust/blob/b940084d7/library/alloc/src/raw_vec/mod.rs#L782)、[Layout source](https://github.com/rust-lang/rust/blob/b940084d7/library/core/src/alloc/layout.rs#L565)も取得した。空queueのexact予約はlen+additionalを検査し、その要素Layoutをgrowへ渡す。Layout::arrayも同じ実型のbyte表現不能を拒否する。allocatorが要求以上を返し得ることと、要求分の予約失敗を返すことを区別する。これはsource照合でありallocator faultの実行観測ではない。

公開Nagiからresize/growを出さず、constructor以外の確保方式をこのpatchで全面交換しない。`close()->resize(0)`と既存`retain`は別の終了経路であり、限定予約APIで全heapをfallible化したとは書かない。Arc・String・thread/Tokio内部の小容量確保なども残る。OOM/allocator abortの普遍的回復は保証しない。

## 受理範囲とOptions/openの責務

Optionsは全slotの確保を行わない設定検査とする。connections/queue_capacityは`i64>0`→`usize::try_from`→Tokio `Semaphore::MAX_PERMITS`以下、connectionsにはvendor `Pool::<NativeManager>::validate_max_size`による**実slot型のLayout**を追加する。msは非負、Duration変換可能性、必要なInstant加算可能性を検査する。busy_msはさらに`i32::try_from`を検査する。[rusqlite 0.40.2 busy.rs](https://docs.rs/crate/rusqlite/0.40.2/source/src/busy.rs)の26–32行はDurationをi32 millisecondsへ変換し、範囲外で`expect("too big")`を実行するためである。これは任意capではなく既存safe native APIの受理境界。0msの即時成功を維持する。任意の数値default/capは追加しない。Layout受理域はtargetとWorkerHandle/依存実型の配置に従うため、単一の固定上限値を全OSへ宣言しない。

queue_capacityはTokio bounded mpscのpermit上限である。Tokio 1.53.1 `bounded.rs:160`は正数検査、`batch_semaphore.rs:130–144`は`usize::MAX >> 3`上限。`mpsc/list.rs:53`は初期block一つを作り、必要時にblockを増やす。指定queue_capacity個のCommandをconstructorで一括確保する実装ではない。全caller/全heapの上限でもない。

openはpathとOptionsの組合せを検査し、`try_build`で実slot queueを予約する。pathなしのOptions検査で将来の確保成功を証明しない。Options成功後にも資源不足でopen失敗し得る。lazy native起動を維持し、slotの確保とnative Connection/worker起動を区別する。path`:memory:`とconnections>1、空path、URI拒否は承認済み契約のまま。

| 失敗 | 場所 | 公開分類候補 | outcome / retired |
| --- | --- | --- | --- |
| 正数/変換/Tokio上限/slot Layout不正 | Options constructor | 既存`Error`のvalidation Err | Txなし |
| path/Options組合せ不正、open再検査の不正 | open | `FailureKind.INVALID` | `NOT_APPLICABLE / false` |
| 検査を通ったslotの`try_reserve_exact`がErr | open | **新`ALLOCATION`を推奨** | `NOT_APPLICABLE / false` |
| worker/observer起動不成立 | native起動 | 既存`WORKER`を基本にnative起動/終了policyへ対応 | native未起動をfake joinへ変えない |
| 内部でruntime/timeouts設定不整合 | 内部construction | 実装不具合。利用者capacity不正と混ぜない | 公開前に検出 |

`ALLOCATION`はこのfallible予約から実際に返されたErrの分類であり、全allocationの失敗が必ずこの値になる保証ではない。SQL/native接続が未起動なのでretiredはfalse、Tx outcomeはNOT_APPLICABLE。既存`INVALID`へ環境依存の予約失敗を押し込むと有効な設定を不正と扱い、既存`WORKER`へ押し込むと未起動のworker故障に見える。新kindを避けるなら分類意味の拡張を明示承認する必要があり、推奨しない。

## 既存wrapperとの比較

release archiveをchecksum照合した[bb8 0.9.1](https://crates.io/api/v1/crates/bb8/0.9.1)、[mobc 0.9.0](https://crates.io/api/v1/crates/mobc/0.9.0)、[async-resource 0.1.0](https://crates.io/api/v1/crates/async-resource/0.1.0)をsourceで確認した。以下は実行検証や採用判断ではない。

| 候補 | allocation/所有 | 取得/取消/公平性 | close/native joinとの適合 |
| --- | --- | --- | --- |
| deadpool限定vendor | private slot Layout検査＋同queueのfallible全予約。既存owned Object維持 | 現行Semaphore/FIFOとtask-local予算を維持。既存barrier oracleを再実行 | 現行close/drain＋Nagi ledger/observer維持。crate constructor成功だけではnative joinを保証しない |
| bb8 0.9.1 | `internals.rs:180`の`VecDeque::new`でlazy、owned `get_owned`あり。max_sizeは**u32** | `inner.rs:83–134`のget timeoutはcheckout検査を含み、0 Durationはbuilderで拒否。connectは背景spawnなので現在の取得task-localが届かない。FIFOはidle選択でwaiter公平性の同等証明ではない | 公開close/idle drainなし。active owned checkoutはpool強参照を保持。close・失敗時補充停止・独立joinを新たに接続する費用が大きい |
| mobc 0.9.0 | `lib.rs:318–346`の`Vec::new`＋Semaphoreでlazy、owned checkoutあり | Semaphore待機はFIFO、connectはcaller内。ただし`get_timeout`はready/check込み、設定0ms拒否。checkout Dropは背景taskへ回収を委譲しruntime終了を再検証要 | 公開close/drainなし。`max_idle(0)`/setterの0は**unlimited**でidle破棄に使えない。共有closing後のidle解放に公開safe APIが不足。Connectionの強Pool参照も扱う必要あり |
| async-resource 0.1.0 | max_count=usize、unbounded queueと動的record、owned Managed、公開drainあり | 管理thread/独自executorを持つ。acquireのResolve timer等に未解決FIXME、0msはtimeout無効。既存予算/取消 oracleを満たした資料なし | drainはresource管理完了でnative joinの証拠ではない。固定古い版の管理thread/unsafe内部/追加依存を新規評価する費用に見合う根拠なし |

bb8には`retry_connection(false)`, `min_idle(None)`, `idle_timeout(None)`, `max_lifetime(None)`が必要だが、それだけで承認済み取得予算/closeへ適合するわけではない。mobcにも`get_timeout(None)`, native終了ledger、closing checkと失敗後create拒否が必要。wrapper全体を差し替えてcapacityだけの問題が終わったとは扱わない。両候補のlazy container成長はinfallibleなので、fallible build/全OOM回復の代替でもない。

依存候補は`bb8 = { version="=0.9.1", default-features=false }`（MSRV1.75）と`mobc = { version="=0.9.0", default-features=false, features=["tokio"] }`。以下はrepoのcompiler/runtimeをpath参照する外部scratchのlock解決だけを行った結果。crate build/実行は行っていない。既存crateの更新を依存追加の費用へ混ぜないため、fresh解決で更新された7既存crateをscratch manifest内だけで既存版へ固定して再解決した。repoへのpin/依存更新ではない。

| 候補 | base lockにないpackage/version | 既存feature/edge差 |
| --- | --- | --- |
| bb8、parking_lotなし | bb8 0.9.1、portable-atomic 1.15.0 | 既存crateの版/依存edge変更なし。portable-atomicは64-bit atomicなしtarget用でlockには載るが現在の4 OS x64/arm64の通常buildでは対象外 |
| mobc、tokioのみ | mobc 0.9.0、async-trait 0.1.92、futures-timer 3.0.4、metrics 0.24.6、portable-atomic 1.15.0、rapidhash 4.5.1、lazy_static 1.5.1、nu-ansi-term 0.50.3、sharded-slab 0.1.7、thiserror/thiserror-impl 1.0.69、thread_local 1.1.10、tracing-attributes 0.1.31、tracing-log 0.2.0、tracing-subscriber 0.3.23、valuable 0.1.1 | futures-channelのsink、tracingのattributes等が合成される。既存thiserror 2.0.21を維持し1.0.69が併存。Tokio/rusqlite/既存crate版更新なし。lock全target集合なのでvaluable等のoptional/target条件と実4 OS buildを区別する |

mobcのtokio以外のexecutor/unstableは有効化しない。scratchはdeadpoolも残して新規差分を測るため、実際に置換する場合はdeadpool/deadpool-runtimeの削除も別途確認する。完全な依存edgeとchecksumは`bb8-dependency-diff.json`/`mobc-dependency-diff.json`、具体lockは`resolve-*/Cargo.lock`に保存した。async-resourceはfirst-choiceから外したためlock解決は行わず、一次Cargo.tomlでconcurrent-queue 1.1/option-lock 0.1/futures-util、既定multitask 0.2の追加境界まで確認した。

## vendorと独立runtime/配布への費用

直接path dependencyとして`runtime/vendor/deadpool`を固定releaseから作り、version0.13.1、managed/rt_tokio_1、deadpool-runtime0.3.1、Tokio/rusqliteの版とfeatureを維持する候補。追加transitive crateは不要。registry版とはCargo source identityが変わるのでlockのdeadpool source/checksum、生成app lock/cacheの扱いを検証する。upstream checksumとローカルpatch hashは別々に保存する。

workspace rootだけの`[patch.crates-io]`は、生成appからpath依存するruntimeや配布済みruntimeへ自動で伝播しないため採らない。runtimeの相対pathにvendorを置き、独立runtime manifestでも同じpathへ解決する。crateのMIT/Apache-2.0 license、README/必要manifest/srcを同梱し、第三者sourceのprovenanceと限定patchを記録する。

Cargoはworkspace内のpath依存を自動member化する。dummy packageだけの`cargo metadata --offline --no-deps`で、`runtime/vendor/deadpool`がmemberになることを確認した。このCargo1.99ではrootのexcludeを指定するだけではruntime子配置のvendorはmemberから外れず、nested `[workspace]`もmultiple rootsで拒否された。**子配置を維持する推奨案はvendor manifestをruntime専用に正規化し、upstream dev-dependencies/test/bench metadataを除くこと**。これも第三者sourceとの差として記録し、root `default-members=["compiler","runtime"]`を明記する。dummy metadataで子vendorがmemberに残ってもdefault-membersから外れること、配布runtimeを独立package＋`[workspace]`にした場合のdefault memberがruntimeだけになることを確認した。限定patchの検査をNagi側から行い、上流test harnessを動かす場合は検証専用scratchで原manifest/必要sourceを使う。存在しないtest pathを宣言したmanifestを配布しない。

代替配置はrepo/配布rootの兄弟`vendor/deadpool`と`runtime`、runtimeから`../vendor/deadpool`を参照しroot excludeを使うこと。dummy metadataでこの兄弟配置はmemberから外れることを確認した。ただしruntimeだけをcopyした独立buildではvendor兄弟も必要になり、既存runtime単独搬送の条件を変える。その費用から本候補では子配置＋runtime専用manifestを推奨する。これらは空のdummy metadata検査であり、実deadpool全suiteを実行した結果ではない。

現在の[`scripts/releases/package.py`](../../scripts/releases/package.py)は44–45行の許可集合がruntime/Cargo.tomlとruntime/srcだけ、82行のgit archive pathも同じ範囲なので、**現在の配布scriptではvendorは落ちる**。同梱許可集合とarchive inputの双方を明示追加し、package/install/verifyの独立runtime・展開後native buildを4 OSで検査する必要がある。版を上げたりreleaseを実行する承認ではなく、将来の配布に欠落を残さない準備である。

upstream追従は自動ではない。将来のdeadpool更新ではchecksum/source・slot型・constructor/resize/return/recycle/closeの差を再照合し、patch不要な公式APIが出れば依存選択を再判断する。Nagiがpool algorithmを保守する案ではないが、限定third-party patchの保守責任は発生する。

## 承認後の検証条件と調査ログ

静的境界の検査は巨大確保なしで正数/usize/Semaphore/実slot Layoutを確認する。予約失敗の分類は小さいfault injectionで確認し、実OOMを起こさない。try_build成功後のget/return/取消/closeは既存oracleを維持する。特にnative cap、startup取消、detach permit gap、別worker healthy中の独立join、cleanup失敗後create停止、logical→nativeの同じ取得予算、0ms、close取消を再実行する。Runtime/registry/native生成・46入力はその後の縦切りacceptanceであり、この調査の成功ではない。

source/metadata/archive/checksumログは作業directory`/workspace/nagi-sqlite-public-2026-10-08/capacity/`に保存した。dependency probeはこのdirectory内だけで解決し、repo manifest/source/lockを変更しない。候補crateの第三者codeはbuild/実行していない。
