# Published 0.1.10 → S1 frozen head public API差分

Base: `815b7d554dba3ac6b11ddad2089b782cbed46eb1` (`nagi-v0.1.10`, public published_at 2026-10-04T17:08:09Z)。Head: `08e90c6984e689f7d026b3ff40277b9898ab4c68`, tree `bc62c787471d9e4481e36c0cca6e2ed294cebbc1`。

根拠はtemporary bare repositoryから取得した両immutable treeのsource diff (`11-public-source.diff`) と `16-public-api-evidence.json`。宣言抽出だけではRustのeffective visibilityやenum arm変更を証明できないため、module/reexport/構造体fieldsと実署名を読む。正式semver-checkerによる結果ではない。後続S2/version headはこの表へ再diffする。

| Surface | 観測した差・source | Compatibility / 移行 |
|---|---|---|
| Nagi通常代入 | `docs/ownership.md:23`, `compiler/src/check.rs`、`stdlib.rs:1085` | 既存owned non-Copy localの裸RHS代入を拒否。標準move importを加える。Copy/fresh値/引数return等の既存consumeは維持。採用済み#87の狭いsource互換変更 |
| Task構文・型 | `ast.rs:135` SpawnBind追加、`docs/task-handles.md:25`, `stdlib.rs:76` | 新しいscope-local結果handle。既存statement Spawn variantと旧unit/Result[unit,Error]故障契約は維持。裸ユーザー名Task/moveをbuiltin化しない |
| Task標準module | `stdlib.rs:15`, `:1033` | std.taskのdiscardはMove入力・unit返却、kindはview入力・Copy Kind返却、messageはview入力・borrow_owner=0のview[str]。opaque Failure、4定数、非Clone/非shared。close/handle個別cancelはない |
| compiler emit::rust | `emit.rs:1161` | `(&Program) -> Result<String,String>`から`(&CheckedProgram) -> Result<String,String>`。戻り値は維持、外部Rust callerはfinalizeを経由する |
| compiler emit::rust_with_lines | `emit.rs:1185` | `&Program`から`&CheckedProgram`。Result<Generated,String>は維持 |
| check::finalize / checked facade | `check.rs:4370`, `checked.rs:3`, `check/checked.rs:26` | `finalize(Program, Program, SourceProvenance) -> Result<CheckedProgram, FinalizeError>`追加。CheckedProgram constructor/fieldsは私有、program/provenance getterはread-only。check/check_editor/integrateの既存署名は維持 |
| FinalizeError分類 | `check/checked.rs:66` | UserError/CompilerDefect/Unclassifiedとkind()/Display/Error追加。生成Low/native replacementとuser Lowのlineageを分離し、未知原因を推測しない |
| ast::Stmt | `ast.rs:201` | 旧public fieldsにprivate flow/taskを追加。外部struct literalがsource互換変更。parserで取得。private factsを利用者が偽造しない |
| ast::S / ast::Type | `ast.rs:135`, `:4` | SへSpawnBind variant追加で外部exhaustive match更新が必要。TypeのOrd/PartialOrd deriveは追加。Type tuple fieldsは維持 |
| stdlib public enum | `stdlib.rs:19/:76/:104` | StandardModuleにAuth/Ownership/Task、ResourceにPrincipal/Grant/Task/TaskFailure/TaskFailureKind、OperationにOwnershipMove/TaskDiscard/TaskKind/TaskMessage追加。旧variantは残り、exhaustive matchesへ影響 |
| metadata functions/fields | `stdlib.rs:220`等 | 旧ResourceInfo/OperationInfo/FieldInfo等の公開field構造・旧function署名は維持。役割を集約したResourceContract/TypeArgumentRole/operation emission semanticsはprivateで新public constructorではない |
| provenance/diagnostics | `source.rs:26/:35/:52/:147`, `diagnostics.rs:54/:182/:209` | SourceProvenance/SourceOrigin/LoweringKindとgetter、新diagnostic renderer追加。cargo_messageの旧署名と詳細表示は維持。Generated/Sourcesは以前からprivate fieldを持つので、新規struct-literal破壊として数えない |
| CLI/options | `project.rs:31/:116`, `docs/error-handling.md:196` | build/run --rust-diagnostics追加、mapped表示が既定で簡潔になる。Optionsは以前からprivate manifest_pathがあり、rust_diagnostics field追加を新たなstruct-literal破壊として数えない |
| Native build location | `generation.rs`, `emit.rs:2025`, `docs/projects.md:126` | 固定cache exe探索からnative:成功世代path読取へ移行。旧成功exe保持/共有依存cache/同out lockを維持。内部path自体は公開固定契約ではない |
| Runtime Task types | `runtime/src/lib.rs:30`, `task.rs:24/:64/:99/:147` | runtime rootにTask/TaskFailure/TaskFailureKind/TaskScopeを追加reexport。task module自身はprivate。TaskにはCloneがなく!Sync。Nagiのscope/義務/escape検査をRustのconsumeだけで代用しない |
| Runtime TaskScope methods | `task.rs:164/:218/:267/:424/:475/:485/:491` | new/default、spawn_value<T:Send+'static>(Future<Output=T>+Send+'static)->Task<T>、legacy spawn(Future<Output=Result<(),Error>>)、receive(Task<T>)->Future<Result<T,TaskFailure>>、discard->unit、async join/cancel->Result<(),Error>を追加。旧Scope::cancelのasync unit署名は維持 |
| Runtime fault lifecycle | `task.rs:424/:485`, `docs/task-handles.md:35` | receiveはactual joinを確認、faultはsticky、join/cancelは全actual join後Err。Task handle Dropはreceipt放棄、TaskScope Dropはabort要求。同期Dropはjoin完了を返さない。body Err保持はsealed generated scope planの責務 |
| Auth proof API | `runtime/src/lib.rs:6`, `auth.rs:23/:56`, `stdlib.rs:11` | public auth::Principal/Grant<P>とtrusted Rust issuance/getter/consume methods追加。Nagiからproof構築/JSON/Clone/sharedは不可。experimental認証境界であり公開SQLite Pool/Txではない |
| Dependency lock | `Cargo.lock`, `runtime/Cargo.toml:12` | deadpool=0.13.1とdeadpool-runtime=0.3.1だけ追加。旧external packages削除/版更新0。rusqlite0.40.2/Tokio1.53.1を維持、rusqlite hooks feature追加。以前のprivate SQLite prototypeに伴う依存でTask public接続が新scheduler/crateを追加したわけではない |
| Existing runtime/install contracts | concurrent.rs/actor.rs/http_server.rs/http.rs/database.rs/result.rs、4installer/uninstaller scripts | 0.1.10 treeからsource bytesが同じ。旧Scope/Supervisor/HTTP/public DBとinstaller/uninstallerのAPI変更をTask releaseに混ぜて主張しない |

Rust embeddingの最小new API例は固定08e90c6からのsnapshot依存を使う別packageで`cargo run --locked --offline`し、成功した（`13-embedding-api-run.log`）。これはRust caller compile/runとemitter string結果の確認であり、生成コードnative buildの代用ではない。Task High/保存Low/手書きLow nativeは専用compiler契約と実配布verifierで別に検証される。

公開版更新対象はworkspace versionとCargo.lockのnagic/nagi-runtime二つ。現在のheadはなお0.1.10を名乗るが公開0.1.10の同sourceではない。Tag/version表示だけで導入済みと判断せずsource commit/treeとpackaged release.jsonを読む。VS Code package version0.1.13は独立で差分がない。
