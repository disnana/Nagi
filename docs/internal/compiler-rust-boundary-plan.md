# コンパイラとRust境界の段階計画

状態: PR0は#77でmainへ反映済み。Q-001は2026-10-05に承認済み。Phase 1は#78のhead `08199bf`で4 OS・editor CIまで成功し、その後ユーザーがmain `0107f37`へマージした。tree一致を確認済みで、エージェントはマージ操作を行っていない。Phase 2は[ADR 007](adr/007-build-generations.md)に基づき実装し、#79 head `27c8bf4`の4 OS・editor/package・Docs・merge gate CIが成功。その後ユーザーがmain `2f2c93d`へマージし、成功headとのtree一致を確認した。Phase 3は#80 head `35038940`の4 OS・editor・site・merge gate CIが成功し、main `f10cb64`へ反映済み。tree一致を確認した。Phase 4のQ002は2026-10-06に承認済みで、[ADR 010](adr/010-sqlite-transaction-boundary.md)に従い一接続・一Txの検証から進める。Q004でgeneric deadpool比較試作とcapability初版値も別に承認済み。adapterの検証と公開配線は未完了、Phase 5は未実装。[進捗](progress.md)を参照。

基点はmain `8f6cc6cf7d7c08811736325263618cbea19314b8`。PR #76のhead `13b59aa`とtreeは同じであり、#74・#76のchecked facts、Low互換性、Rust backendを維持する。本計画は2026-10-05の依頼に基づく。実装済みの保証と、後続Phaseで追加する予定の保証を分ける。

## 設計原則

Rustを隠すためにRustを再実装しない。Nagiの名前・型・move・view/origin・公開resourceの規則をNagiで検査し、その決定を生成まで運ぶ。外部crateのAPI、trait、Send/Sync、native Rust、target/linkと最終borrow/memory safetyはrustcへ委譲する。

Nagiが保証する規則を満たすと判定した後、その規則に関わる生成ミスでRustが拒否されるのはコンパイラの不具合である。rustcに最終検査を任せることを、この不具合の免責に使わない。

全面HIR/SSA、独自borrow checker、Rustの型・trait体系の再実装は行わない。暗黙clone、借用の長寿命化、独自寿命runtimeで未証明の条件を埋めない。各Phaseを文書・テスト・実装・CIの順で完了し、次へ進む。

## 現在の経路と変更後の境界

現在のCLIは[emit.rs](../../compiler/src/emit.rs)にある。

```text
High → parse / module・name resolution → 初回check
     → 型付きLow textと行対応 → Low parse / 行復元
     → native Low / @replace統合 → 最終check
     → Rust生成 → Cargo / rustc

User Low → parse / module・name resolution
         → native Low / @replace統合 → 最終check → Rust生成
```

High→Low text→再parseは維持する。保存Lowを別コマンドで開く場合はそのLowが入力ソースであり、失われたHigh位置やchecked proofを復活させない。

Phase 1で最終境界だけを変える。

```text
最終Low AST + native + source provenance
    → integrate / Nagi semantics check / codegen plan確定
    → CheckedProgram
    → Rust codegen
    → Cargo / rustc
```

初回High check、editorの回復AST、parser成功だけではCheckedProgramを作れない。optionalな型を持つProgramを全面的なTyped IRへ作り替える計画ではない。

## Guarantee Register

「現在」は既存契約を維持する項目。「予定」は指定Phaseでテスト・実装を揃えてから提供する項目であり、現在の保証として宣伝しない。ここにない保証の追加・削除・owner変更はStop条件とする。

| ID・状態 | guarantee | owner・検査場所 | codegenへ渡すもの | 継続検査・違反時の分類 |
|---|---|---|---|---|
| G-NAME・現在 | module/定義のidentityに基づく名前・型の解決。表示名からbuiltinを推測しない | Nagi checker / modules | canonical module/definition ID、解決済み型・参照 | `symbols`, `stdlib_imports`, `module_emission`。受理後のidentity破綻はcompiler defect |
| G-TYPE・現在 | Nagiの対応型、引数・戻り値、Result/nullable、制御構造の一致 | Nagi checker / check | 式型、binding型、解決済み呼出し・署名 | `typed_errors`, `option_match`, `nullable_roundtrip`, conformance。不正入力はuser error、受理後の生成型不一致はcompiler defect |
| G-MOVE・現在 | move/copy、部分move、shared親からの非Copy move禁止、分岐・loopの継続状態 | Nagi checker / check | operand use、BindingId、flow facts、生成用Copy決定 | `ownership_boundaries`, `shared_field_moves`, `loop_ownership`, `copy_capabilities`。Nagiの受理判断と生成が違えばcompiler defect |
| G-VIEW・現在 | view origin、reborrow、sequential borrow、許可されたreturn、static view、関数値由来の借用 | Nagi checker / check | origin/use facts、storage/cleanup plan、parameter/statement lowering決定 | `static_callback_views`, `view_flow_completion`, `view_container_drop`, `scope_runtime_contract`。origin欠落・不正な長寿命化はcompiler defect |
| G-RESULT・現在 | expected errorとnullableを区別し、Resultを裸の式で捨てない | Nagi checker / check | resolved Result型、error出口、match/tryの決定 | `result_discard`, `result_match`, `typed_errors`。受理後の誤った伝播はcompiler defect |
| G-CONST・現在 | 対応する8整数幅の静的ゼロ除算とsigned MIN/-1・MIN%-1を拒否。profile依存overflowをKnownへ格上げしない | Nagi checker / constant_eval | 型と検査結果。codegenで別の定数foldをしない | `constant_validation`, `integer_arithmetic`, corpus。静的失敗を後段へ通すのはcompiler defect |
| G-RESOURCE・現在 | 登録済みresourceのcopy/shared/field storage、debug/serialization、実payloadとphantomの区別 | Nagi checker / stdlib・capabilities | 登録済みresource/operation identity、Passing、payload role、生成用capability決定 | `auth_boundaries`, `actor_stdlib`, `actor_codegen`, `http_codegen`。Nagi宣言違反はuser error、生成側だけの追加条件はcompiler defect |
| G-SQL・現在 | 明示schemaを指定したSQLite対象literalだけ、列名・必要返却列・bind数等を検査 | Nagi compiler / sql_check | 検査済みASTとschema検査結果。実データの型・NULLを静的保証にしない | `sql_check`, `nullable_database`。schema指定違反はuser error、実DB差はruntime error |
| G-AUTH・現在 | 宣言された保護APIへ正しいGrant[P]を渡し、偽造/copy/shared/禁止field storage/move後使用を拒否 | Nagi checker / check・capabilities | nominal permission identityと所有権・payloadの決定 | `auth_boundaries`、auth-boundary実HTTP。trusted issuer/policyの内容は保証外 |
| G-RUST・現在 | 外部API、trait、最終Send/Sync/Clone、native本体、target/link、最終borrow/memory safety | rustc / Cargo build | Nagiが選んだ正しい型・所有形態とRust adapter署名 | `rust_dependencies`, `build_diagnostics`, real native tests。extern実装不一致はdelegated error。Nagi保証済みの生成ミスはcompiler defectへ戻す |
| G-LIFECYCLE・現在 | Nagiが選ぶ評価順・cleanup anchorを保持。Future dropは既完了/受理済み副作用のrollback完了を保証しない | Nagi compilerのlowering + Rust Drop/Future | checked cleanup/error出口、通常Rustの所有構造 | `view_container_drop`, `scope_runtime_contract`, runtime adversarial tests。選択した構造の誤生成はcompiler defect、任意destructorの正しさは保証外 |
| G-ARTIFACT・現在 | 別canonical source/outのアプリを既定/明示の共通targetへ置いても、互いのexeを上書きしない。mainの既存保証はこの範囲で、追加の世代隔離はG-GENERATIONに記録する | Nagi compiler / build CLI | canonical app identityとpackage/executable名 | `shared_target`, `project`。別アプリの取り違えはcompiler defect。短いhashは権限・暗号学的隔離ではない |
| G-SEALED・現在・Phase 1 main反映、CI成功 | Rust codegenの入力は最終統合・check済みで、外部から可変化できない | Nagi checker / finalizer | ProgramをmoveしたCheckedProgramと確定plan・provenance | API compile-fail、facts completeness、決定性、既存High/Low/native conformance。[ADR 006](adr/006-sealed-codegen-input.md)。欠落factsはICE候補。#78でmain反映済み、正式releaseは未実施 |
| G-GENERATION・現在・Phase 2 main反映・CI成功・Q-001承認済み | 実行するgenerationを他buildで上書きせず、成功generationのみpublish。dependency cacheは共有 | Nagi compiler / build CLI、OS advisory lock | app/generation identity、成功artifact metadata、生成Rust provenance | 並行build、失敗publish、Windows実行中exe。取り違えはcompiler defect、OS/file/lock失敗はinfra error。#79 head `27c8bf4`で4 OS・package/editor CI成功、main `2f2c93d`へ反映済み |
| G-TX・Phase 4予定 | Transactionはaffine、nonCopy/nonshared、永続格納・task transfer禁止。commit/rollbackがconsumeしResultで完了を観測 | Nagi checker / ResourceContract・transfer検査。native完了はDB adapter | Tx capability、nested payload/transfer決定、明示終端操作 | normal/Err/unwind/cancellation、nested Option/Result、spawn拒否。Nagi保証の抜けはcompiler defect。応答未受信のCOMMITは結果不明になり得る |
| G-POOL・Phase 4予定 | Txが接続を専有し、cleanup成功を確認する前に再利用しない。rollback失敗接続を再利用しない | Nagi DB worker/adapter。checkerがDB完了を静的証明するとはしない | leaseとcleanup状態を保つnative APIへの確定呼出し | acquire/begin応答喪失、取消、cleanup失敗、close/worker終了とpermit解放。native Drop実行だけをrollback成功の証拠にしない |
| G-AUTH-SCOPE・Phase 5以降の方向 | Grantが実際のScope値を保持し、保護操作は別bare resource IDを取らない | Nagi checkerの型/所有規則 + trusted Rust issuer/adapter | permission/scope identityとGrantの所有形態 | 未実装。期限・失効・全routeの認可漏れ・request regionを型で保証しない |

ownerをNagi compilerとしたbuild/SQL項目はcheckerの能力とは区別する。Nagi checkerはRustのtraitソルバーや一般的なSend/Sync判定を追加しない。登録済みresourceの禁止task transferはNagi自身の契約なので、RustのSendに丸投げしない。

## CheckedProgramとemitterの入力

構築はchecker側の最終factoryに閉じ、Programをmoveする。native統合、@replace署名、通常check、定数・route条件、codegen前提の検証が成功した後にのみ構築する。public unchecked constructor、mutable accessor、`DerefMut`、内部Programを取り出すAPIを設けない。初回checkをするAPIは残しても、そこから最終codegenへ直接通さない。

保持するのはcanonical AST/identity、checked operand/origin/flow facts、私有storage/cleanup plan、生成用に確定したcapability・呼出しpassing・source provenance。Rust名の変換がplanより先に必要な既存順序は保つ。canonicalな情報と私有Rust名を区別する。

現在emitterにあるCopy/Serde/Debug/ChargeとFromRow eligibility、view storage plan、parameter rebindingやstatementのborrow lowering決定を封印時へ移す。Phase 1では現在の判定関数と判定順を再利用し、Phase 3のResourceContract共通化や意味論変更を先取りしない。

emitterは型のRust表記、quote/escape、確定したaction・derive・call・field accessorの出力を担当する。checkerの環境を呼び直したり、変数名/builtin一覧/型形からmove/copy/borrow・resource capabilityを再推測したりしない。facts欠落をdefaultやcloneで埋めない。

crate分割は行わない。必要と判明した場合はStop。専用Typed HIRや全CFGへの移行もこの計画には含めない。

## Source provenanceと診断

source identity/positionと生成由来は別の軸で保持する。

| 由来 | 追跡する内容 |
|---|---|
| Generated Low | 同じコンパイルで生成したLow。元Highのcanonical path/module IDと文・定義行、lowering lineage |
| User Low | 入力として読んだLow。保存されたgenerated.lowでも再読込時はこの扱い。元High位置を推測しない |
| Native | 読み込んだnative Lowと手書きRustを区別。native Lowはそのファイル位置、Rust本体はRust位置 |
| @replace | 置換した本体の由来と置換対象のcanonical definition IDの両方。attrsから後で推測しない |
| Synthetic | main bridge、derive等の合成glue。確実な元位置がなければ無対応とする |

Sourcesのappend offset、Low行復元、@replace統合、Rust名変換を通して保持する。[ADR 003](adr/003-diagnostic-boundary.md)のstatement-line精度を維持し、Highの式column/full span/edit対応を実装済みとしない。

Rust diagnosticsはprimaryと関連causeを保持し、`--rust-diagnostics`で元のRust診断を参照できるようにする。未対応span、依存、native、syntheticを近くのHigh位置へ推測変換しない。error/warningを消したり、追加allowで受理させたりしない。

| 分類 | 判定の根拠 |
|---|---|
| user error | Nagi入力の契約違反、無効なUser/Native Low・@replace署名。sourceと原因が対応する |
| compiler defect / ICE候補 | 欠落checked facts、不正plan、Generated LowのNagi保証破綻、受理後のNagi生成Rustの契約違反 |
| rustc delegated error | 実crate/API/trait/native署名/target等の不一致と原因を確認できる |
| infrastructure error | toolchain、dependency取得、権限、lock、file、link環境の失敗 |
| unclassified | 由来と保証ownerから安全に決められない。元診断を保ち、conformance corpusなら失敗artifactを保存してStop |

Generated Lowにエラーが出ただけでICEと断定しない。Native/@replaceによる影響も見る。逆にUser Lowでも最終check後のNagi生成ミスはuser errorへ格下げしない。RustのE0308/Send等のcodeだけで分類しない。

## ResourceContract

Phase 3は最初に現在のpass/fail、generated derives/accessors/Passingのcharacterization・golden・conformanceを保存する。ResourceInfo、capabilities、checker、sealed planで使うcopy/shared/storage/debug/serializationとinline/shared/phantom payload roleを共通根拠へ集める。

`storage`の現判定はclass等の所有field格納であり、永続DB格納や一般的な資源lifetimeの証明ではない。この意味を変えない。enumの現在のSerde判定、function signature/phantomを実payloadと数えない条件、App/Supervisorの内部共有payloadも固定する。

Phase 3のlifecycleは`Unspecified`等の不活性な状態にする。既存判定には影響させない。利用者が任意Rust型へ自由に安全契約を宣言する仕組みは追加しない。

具体構造と先行テストは[ADR 008](adr/008-resource-contracts.md)に記録した。公開ResourceInfoを内包する単一descriptorから、用途別のlegacy queryとgeneric roleを導く。受理意味論・公開structのfield集合・native APIは変えない。

## Lifecycle / panic / cancellation

Future cancellationは所有Futureがdropされること。source上のawaitだけがcancel地点であるとは定義しない。明示moveの移動先、通常Rust Drop、cleanup anchorが資源の破棄責任を持つ。

Resultの業務Err、unwind panic、取消、compile/delegated/infra errorを区別する。handler panicの500変換は状態・DBのrollbackではない。受理済みblocking/DB操作や外部副作用はcaller取消後も完了し得る。

生成アプリは既存のpanic=unwind設定を保つ。async Drop、Drop中の独自panic契約、取消時に勝手にbackground taskへcommitを移す仕組みは追加しない。panic=abort、OOM、process kill、OS crashからの資源回復は保証外。

## Build generation isolation

app identityはcanonicalな入口sourceと利用者の論理outの組で、generation identityとは別にする。source内容だけのhashを依存全体のcache keyと呼ばない。generationは新しいsnapshotを識別するものであり、完全content-addressed cacheにはしない。

成功済みgenerationをbuild/runで上書き・削除・killしない。互換出力を共有する**canonical out単位**のOS advisory lockでsource生成→Cargo→publishを直列化し、run中は保持しない。同じoutを使う別sourceも調停する。generationとlatest metadataはapp identityごとのnamespaceへ置く。異なるoutのbuildは独立で、dependency cacheは引き続きCargoに共有させる。lockの対象をapp IDだけにすると、別source・同一outの互換ファイルが競合するため採らない。

同じoutへ書くcheck/lower/--costも、このwrite lockを使う方針とする。非書込みcheck/map等に不要なbuild lockを要求しない。独自のlockを複数持ち込み、取得順に依存する設計を初版には入れない。

生成・Cargo途中のsnapshotはstagingとして扱い、成功後のみtemp→publishでimmutableな実行artifactを用意し、最新generationのmetadataをatomic更新する。更新失敗なら以前の成功metadataを維持し、新generationをrunしない。atomic性・Windows挙動を実テストせずに保証しない。lock unsupportedやfile失敗を「ロックなしで成功」へ変えない。

互換出力として既存outの`generated.low`、`src/main.rs`、`Cargo.toml`と既存`Cargo.lock`を維持する案を優先する。Cargo実行には世代snapshotを使い、既存lockを継承する。check/lowerが生成Lowを書き出す現在の用途も残す。したがって互換出力は調査・直接Cargo用の出力であり、成功generationの証拠とは扱わない。

buildではgeneration内の生成・Cargo成功とartifact publish、互換出力の更新を終えてからlatest metadataを更新する。互換更新の途中で失敗した場合、buildは失敗と報告して以前の成功metadataを維持する。互換ファイルの一部が更新済みの場合はその事実を診断し、完全な旧版へ戻ったと主張しない。runは互換ファイルを読まず、成功metadataで選んだgenerationを使う。複数ファイル全体のatomic置換や、非協調の外部Cargo実行までの調停は保証しない。

固定するsnapshotはcompilerが生成するLow/Rust/manifest、読み取り済みの埋込み入力とsource/provenance、および引き継ぐlockである。外部path crate/runtime/native Rustの参照先を世代化するだけで全Rust module/includeを固定できたとはしない。外部依存・nativeの同時編集までを含むworkspace全体の原子的snapshotは対象外。その範囲を必要とする場合は別設計にする。成功後のbinaryはこれらの外部source編集で置き換わらない。

実行ファイルは`native:`に実際の成功generationのpathを示す。同一app再buildで同一pathを求める内部testは、[Q-001](open-questions.md#q-001-同じアプリの識別と実行ファイルの世代を分ける)の承認に基づき、app identity同一とgeneration相違を別々に検査する形へ変更する。旧generationをbuild時に上書き・削除・killしない。

依存追加はしない。`std::fs::File::lock`はRust 1.89以降の候補で、監査環境はRust 1.98.1、CIはstable。MSRVを新たな公開契約として勝手に変更しない。lockとatomic publishの4 OS実証をPhase 2の条件にする。

## DB Pool / Transactionの予定契約

Phase 4までは新APIを実装しない。SQLiteとPostgreSQLは別module・別接続型という既存方針を保つ。最初の候補は現在依存のrusqliteを使うSQLiteで、SQLx/Scylla等のdriver追加を既成事実にしない。

Poolは接続取得・lease返却・閉鎖を扱う。Transactionは一接続を専有するaffine resourceで、Copy/shared/永続格納/task transferを禁止し、commit(tx)/rollback(tx)がconsumeしてResultを返す。nested Option/Result等から禁止を迂回できないことを検査する。任意Rust本体のtransferまでNagiが検出するとは保証しない。

Future[return]の戻り値型だけでは、Future内部に捕捉したTxを見落とす。`pending = use_tx(tx); spawn pending`、alias、Future返却やOption経由、標準task起動境界をnegative対象にする。最小のprivate capture/transfer factsで追跡する案を検証し、一般region/effect解析や計画外のRust解析が必要ならStopする。現Nagiには汎用spawn_blocking APIがないため、新構文を追加して検査したことにはしない。native Rust内のspawn_blockingはtrusted境界である。

現在のDbはworker内でConnectionを持ち、replyはSend + 'staticである。借用付きrusqlite Transactionをそのままreplyへ返せない。既存DbへBEGIN/COMMITを別々に送る方式やunsafeな自己参照型で済ませない。worker内のnative Txと外側のowned session/leaseを分ける案を先に検証する。既存DbのAPI・取消保証を変更しない。

明示commit/rollbackは完了を観測する。Dropはrollback完了のAPIではない。cleanupを確認するまでは接続をPoolの再利用対象にしない。rollback失敗の接続を成功扱いで返却しない。native destructorとworker終了の責任を観測する。

COMMITがDBへ送信された後、応答受信前にcaller Futureがdropされた場合は結果不明になり得る。commit済みとrollback済みをどちらも断定しない。normal return、Result error、Future cancellation、unwind panicをテストし、acquire途中取消、transaction進行中取消、commit応答喪失を別ケースにする。

pool容量・acquire/busy timeoutはOptionsの必須指定、transaction開始modeはBeginModeの必須指定、module/API名はQ002と[ADR 010](adr/010-sqlite-transaction-boundary.md)で採用した。既存Dbのqueue 64/busy 500msを新Poolの既定値へ流用しない。新しい公開policy値を決める必要が出たらStop。nested transaction/savepointは対象外。

依存の監査基点はCargo.lockのrusqlite 0.40.2、Tokio 1.53.1、Axum 0.8.9。manifestのversion要求と実際の解決版を混同しない。

## Auth Scopeの将来方針

Phase 4完了前には実装しない。`Grant<Permission, Scope>`の内部にScopeの実値を保持し、protected operationは別resource IDを受けず、その値を使う方向とする。偽造/Copy/serialize/shared/永続格納を禁止する。

型だけで現在の有効性、expiry、revocation、全routeの保護を証明しない。trusted verifier/issuer/policy/adapterの責任は残る。request region/effect systemは追加しない。

現在のGrant[P]を即座に置換しない。arity・既存APIの互換性は別の設計判断が必要であり、この計画で削除・破壊を許可したとは扱わない。

## Phaseごとのacceptance criteria

| 段階 | 最初の差分 | 完了の観測 | 次へ進む条件 |
|---|---|---|---|
| PR0 | 本文、progress、open questions、DESIGN日英・agent入口。実装変更なし | 既存契約・tests・ADRとの差を明記、links/site/change detector確認、文書PR CI | Stop未解決なら停止。回答を反映した計画とCIが揃ってからPhase 1 |
| Phase 1 | 最小failing API/facts/provenance tests → sealingとemitter入力変更 | Program→emit、外部構築/可変化がcompile-fail。同入力のLow/factsが決定的。High/保存Low/UserLow/Native/@replaceの位置と由来を保持。既存pass/fail/runtime期待を維持 | fmt/clippy/full tests、High→Low→Rust build/run、bounded property/fuzz、対応4 OS・editor CI成功 |
| Phase 2 | 実Cargoの競合・実行中旧exe・失敗publish回帰 → generation isolation | dependency cache・project cwd・既存out/lockを維持。別generationは別artifact、旧binary継続、新build失敗でlatest不変。同一outの別app、check/lower並行、projection途中失敗も観測。Windowsでも実証 | Q-001反映済み。4 OS、既定/明示target、parallel、process失敗、package/install/editor CI成功 |
| Phase 3 | 実装を変えないcharacterization/golden/conformance commit | 集約前後の既存pass/fail・生成結果・診断・runtimeが一致。payload roleとcapabilityの単一根拠。lifecycle不活性 | characterization commitのCI成功後に集約。全関連CI・bounded生成探索成功 |
| Phase 4 | Pool/Tx APIとlifecycle ADR、negative/実DB tests | affine/transfer禁止（Future capture含む）、明示consume、normal/Err/cancel/unwind、begin応答喪失、cleanup失敗、worker/permit解放、再利用前cleanup、結果不明の区別 | 未決policy/driver/unsafe/契約衝突があればStop。採用契約の4 OS/runtime/conformance/fuzzと比較測定成功 |
| Phase 5以降 | 方向だけ。今回自律実装対象外 | Scope型/実値・旧Grant互換性・trusted境界の次期設計 | Phase 4完了と必要な設計判断後 |

同時に複数Phaseの実装をしない。各Phaseは明確なcommit/PR単位とし、前Phaseが未マージなら依存とbaseを明示する。mainへのmerge、版更新・releaseはこの計画の実行に含めない。

## テストと性能

既存[compiler-testing](compiler-testing.md)のtaxonomyと[regression corpus](../../tests/conformance/)を使う。追加の失敗は再現→最小化→恒久回帰→修正の順に扱う。pass/failの対、拒否段階・source位置・primary/関連causeを残す。公開意味論・利用者契約・High/Low互換性・Guarantee Register・security/lifecycleの期待を変える必要があればStopする。承認済み設計の内部file/path等は、before/afterと理由を記録して更新できる。assert削除で不具合を隠さない。

High→初回check→Low→最終check→CheckedProgram→Rust→build/runを継続検証する。frontend mutationと狭い生成propertyを使い、後段Rust buildは限定したgrammar/corpusへ絞る。単なるseed保存で終わらず縮小ソース、失敗段階、toolchain/targetを保存する。

Phase 1は同入力のLow・canonical facts・planの決定性を検査し、意味的snapshotをHashMapの偶然の順番へ依存させない。既存Drop/取消/Scope検査を保つ。Phase 3は固定したgoldenだけでなく既存native conformanceを両版で比較する。

測定はRust/Axum同条件とNagi生成版を比較する。check/Low parse/check時間、plan保持量、native compile時間、binary、Future frame、allocation、memoryを変更に応じて観測する。clone/allocationやDrop順を変えて数値だけを改善しない。共有hostの短期throughputを本番SLOと呼ばない。性能上の意味論変更が必要ならStop。

PRごとは既存fmt/clippy/unit/compile-pass/fail/regression/conformanceを維持する。コード変更には4 OS native・editor/packageゲートを使う。週次/手動は拡大生成・mutation/fuzzを使う。文書のみのPR0ではRust全suiteを起動せず、site/links/change detection/gateを確認する。スキップされたRust jobを新しい実装検証の成功に数えない。

## Security guaranteeとcompatibility

CheckedProgramは内部の受理状態を表し、プログラムのsandboxではない。source/manifest/native/dependencyを信頼する既存build前提は変えない。advisory lockは協調するNagi buildの調停であり、悪意ある別process、任意Rust、directory差し替えへの隔離保証ではない。

Authは宣言したtyped保護APIの境界、SQLは明示schemaでの限定検査。暗号の正しさ、business policy、任意Rustの安全契約、実DBの型/NULL、panic後rollbackを追加保証しない。

High/Low構文、@replace、保存Low、既存APIは維持する。内部Rust library APIのemit入力をCheckedProgramへ変えることはPhase 1の明示変更。生成ファイルのpaths/lock/再build手順とbinary generationはQ-001に従い、公開Docs日英、scripts、editors、distributionを相互確認する。

## 未決事項とStop

実装前の未決事項は[open-questions](open-questions.md)へ集める。Q-001は解決済み。app identityを維持し、generationごとにside-by-side生成し、build成功後にlatest metadataをatomic更新する。旧binary pathの同一性は公開仕様ではなく内部実装上の期待であり、承認済みgeneration設計に合わせて更新できる。

次もStop条件として維持する: 計画外の公開意味論・syntax・High/Low破壊、Rustへのowner変更、security/Guarantee Registerの追加・縮小、性能目的の意味論変更、acceptanceやnormative ADRの衝突、公開意味論/利用者契約/High・Low互換性/登録保証/security・lifecycleに関わるtest期待変更、分類不能なconformance失敗、unsafe、crate分割/想定外rewrite、依存追加/更新、新policy値の必要。承認済み設計の内部生成先・file名・path等のtestは、根拠を記録した更新を認める。

Stopでは問題・根拠・選択肢・互換性・推奨案を記録し、実装を先行させない。文書の陳腐化や既存Phaseの範囲外という説明だけを将来変更の禁止と誤読しないが、利用者契約・test期待を黙って変更しない。

## 採用案と見送る案

| 案 | 判断と理由 |
|---|---|
| 最終checkのsealed境界と既存planの移送 | 推奨。#74/#76を保持し、責任とmodule依存を小さく固定できる |
| Programを包むだけでemitter再推論を残す | 不採用。APIだけが変わり、二重の意味論が残る |
| finalizerでRust textを先に生成して保存するだけ | 不採用。checked facts/planの契約を文字列へ置き換え、checker→codegenの分離にならない |
| Typed HIR/SSA全面rewrite、独自Rust checker | 不採用。費用と互換性リスクが大きく、今の問題に必要な証拠がない |
| side-by-side generation + success metadata | Q-001で採用。旧exeを上書きせずrun中のlockも不要 |
| stable executableを更新しrun終了までlock | 不採用候補。長期サーバー中のbuildを止める。generation隔離の目的に合わない |
| source-only content hashをbuild cache keyにする | 不採用。native/依存/features/target等を含まない |
| 任意Rust resourceの安全Contractを自己申告で受理 | 不採用。現在の検査で安全と証明できない |

## 用語

- **Program**: 構文・解決・検査途中にも使う共通AST。存在だけで受理を意味しない。
- **CheckedProgram**: 最終Low/native統合・checkと生成plan確定後の封印状態。
- **facts / plan**: Nagi checkerの判断と、それに基づくRust-onlyの格納・cleanup・出力決定。公開型とは別。
- **provenance**: 元source identity/positionと生成・native・置換・合成の由来。
- **app identity / generation identity**: 論理アプリの識別と、個々のbuild snapshotの識別。
- **ResourceContract**: 登録済み資源の既存capability/payload規則と、段階的に追加するlifecycle規則。
- **affine**: 最大1回消費できる所有資源。明示終端へ必ず到達するという保証とは別。
- **delegated error**: Rust/環境へ明示委譲した条件の不一致。Nagi生成ミスを含めない。
- **compiler defect / ICE候補**: Nagiが保証した判断・内部状態・生成の契約違反。全backend拒否を機械的にICEへ分類しない。

## 根拠

現実装: [check](../../compiler/src/check.rs)、[emit](../../compiler/src/emit.rs)、[view flow](../../compiler/src/view_flow.rs)、[source](../../compiler/src/source.rs)、[diagnostics](../../compiler/src/diagnostics.rs)、[ResourceInfo](../../compiler/src/stdlib.rs)、[capabilities](../../compiler/src/capabilities.rs)、[database](../../runtime/src/database.rs)。

既存判断: [language invariants](language-invariants.md)、[pipeline](compiler-pipeline.md)、[ADR 001](adr/001-backend-boundaries.md)、[ADR 003](adr/003-diagnostic-boundary.md)、[ADR 004](adr/004-checked-boundaries.md)、[ADR 005](adr/005-native-artifact-identity.md)、[#76結果](boundary-foundation-results.md)。

外部資料: [Rust File::lock](https://doc.rust-lang.org/std/fs/struct.File.html#method.lock)、[Rust rename](https://doc.rust-lang.org/std/fs/fn.rename.html)、[Rust Drop](https://doc.rust-lang.org/std/ops/trait.Drop.html)、[rusqlite 0.40.2 Transaction](https://docs.rs/rusqlite/0.40.2/rusqlite/struct.Transaction.html)、[Tokio 1.53.1 cancellation](https://docs.rs/tokio/1.53.1/tokio/task/index.html#cancellation)、[既存compiler research](compiler-testing-research.md)。API版・保証範囲を参照し、他言語の意味論をそのままNagiへコピーしない。
