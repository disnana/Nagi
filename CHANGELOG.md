# Changelog

## 未リリース / Unreleased

- VS Codeでclass/record呼び出しの名前付きフィールド名を補完し、引数ヒントは入力順ではなくフィールド名に合わせます。既存の`=`、供給済みフィールド、shadowing、保存済み宣言の警告を維持します。公開0.1.13には未収録です。
- Complete named class/record field labels in VS Code and select signature parameters by field name. Preserve existing equals signs, supplied fields, shadowing guards and saved-declaration warnings. This is not included in published extension 0.1.13.
- HTTP応答容量不足が既定error mapperでも容量不足になった場合、内部障害500へ変換せず、独立fallback枠から503を返します。他のmapper失敗の非公開500、既存認証・期限・応答上限を維持します。有限loopback過負荷、低速接続、異常切断、回復、actor/Supervisor補助検証の再現可能な証拠を追加します。
- Preserve typed HTTP load shedding when both a handler response and its error mapper exhaust the ordinary response pool: return a bounded fallback 503 while keeping other mapper failures as opaque 500 responses. Add finite loopback-only overload, slow-connection, disconnect, recovery and actor/supervision evidence without claiming Internet DDoS or BEAM equivalence.
- 関数・組み込み・標準APIの呼び出しで、全引数を`name=value`として宣言順に指定できるようにする。既存の位置引数とclass/enum構築を維持し、混在、重複、未知、不足、順序違いはbuild時に拒否する。checked frontendで既存の位置呼び出しへ正規化し、評価順・所有権検査・backend出力を変えない。
- Allow function, builtin, and registered standard-API calls to name every argument in declaration order with `name=value`. Preserve positional calls and class/enum construction; reject mixed, duplicate, unknown, missing, and out-of-order arguments at build time. Normalize through the checked frontend so source-order evaluation, ownership checks, and backend lowering stay unchanged.
- High構文にPythonと同じ`elif condition:`を追加し、既存の入れ子`else:` + `if`と同じ検査済み`if`へloweringします。Low構文と従来の`elif`識別子は維持し、VS Code / JetBrainsの補完・色・字下げも文脈に限定します。
- Add contextual High `elif condition:` branches, lowered through the existing checked `if` representation. Preserve Low syntax and existing `elif` identifiers, with context-aware completion, coloring and indentation in both editor plugins.

## Nagi 0.2.2 — 2026-10-27

- 0.2.1との互換性、既定のfull runtime・release build、完全なHigh/Low・型・所有権・Policy検査、Cargoのnative再判定、旧成功成果物の保護を維持しながら、小さな変更の開発待ち時間と初回依存解決の再現性を改善します。IDEプラグインは別の版・公開工程です。
- Preserve 0.2.1 compatibility, the default full runtime and release build, complete High/Low/type/ownership/Policy checks, Cargo's native freshness decisions, and previous successful artifacts while improving supported edit loops and reproducible first dependency resolution. Editor plugins remain separately versioned and released.

### Build and dependency selection

- `build/run --runtime-profile full|auto`を明示opt-inで追加。既定fullと既存Rust API（default-features無効時を含む）を保持し、完全検査済みの小さな同期プログラムだけ共有ソースの軽量packageを選択します。unknown/async/adapter/追加Rust依存/incrementalはfullへ戻し、Cargoのnative再判定と旧成功世代の保護を維持します。[計測条件と限界](benchmarks/results/runtime-profiles-2026-10-24/README.md)。
- Add opt-in runtime dependency selection from checked IR, preserving complete High/Low/type/ownership/Policy checks and the existing full runtime API. Include the shared-source core package and its dependency notices in distributions. Keep release settings and receipt/CAS compatibility; the measured gains apply to supported small synchronous programs, not arbitrary HTTP/SQL backends.
- Add explicit `--seed-lock` for `build` and `run`. A supporting distribution can provide the initial Cargo lock when the output has none; existing or invalid output locks are never replaced. The seed is an integrity-checked resolution input, not a native artifact cache or signature. Cold full builds were slower in the recorded comparison, so the option remains off by default.
- `build`/`run`へ明示`--seed-lock`を追加。出力にlockがない場合だけ対応配布物の初期lockを使い、既存・破損lockは置換しません。seedは整合性付きの依存解決入力であり、native成果物cacheや署名ではありません。記録したcold full比較では遅かったため既定化しません。
- Update `rustls`, `rustls-webpki`, and `webpki-roots` to fixed releases and require the corrected WebPKI dependency when old application locks are re-resolved. Existing generated locks and executables are not changed automatically; rebuild deployed applications with the new runtime.

### Development and feedback

- Keep changes limited to discoverable root-level website tests on the existing documentation CI path. Preserve full validation for unknown files and mixed compiler, runtime, security or dependency changes. This optimizes Nagi's repository CI; application build behavior is unchanged.
- 利用者向けCI generatorに`--build-profile dev|release`を追加。既定releaseと完全検査を維持し、Nagi 0.2.1以上の開発用buildをWindows/Unixで選択できます。profile別cacheと0.2.0向けrelease互換を維持し、未対応の組合せは出力前に拒否します。
- Add explicit development/release profile selection to the next user-project CI generator, defaulting to release and targeting published Nagi 0.2.1 by default. Keep full checks, trusted-branch cache writes and older CLI release compatibility; separate profile caches under a new key namespace. Published 0.2.1 archives are unchanged.
- Add backward-compatible `read_line(prompt)` alongside `read_line()`. The prompt is borrowed, written and flushed before input, and write/flush/read failures remain explicit `Result` errors. Compiler-owned editor metadata now includes `read_line` and the supported builtin vocabulary; compatible editor releases consume that metadata independently.
- Add runnable Japanese/English Result/`try` lessons and an end-to-end task-management JSON/SQLite API tutorial, including a concrete FastAPI/Quart comparison with tested versions and stated limits.

### Stability and release limits

- Improve bounded CI diagnostics after BrowserLogin and Session deadline failures without changing the failing request's result or relaxing the test. An intermittent valid-request 504 observed in release-level CI is not proven fixed; a later retry succeeded, and the retained diagnostics remain investigation evidence rather than a runtime remedy.
- Keep runtime-profile and lock-seed modes explicit. They do not skip Nagi checks, restore native artifacts, make every backend use the core runtime, or establish general build/runtime performance gains. Measurements, unsupported cases and fallback reasons are documented with the respective features.

## Nagi 0.2.1

- Publish the backward-compatible build and stability improvements below. For faster application development, opt into `nagic run app.nagi --profile dev`; ordinary CLI builds retain the release default. Initial native builds still compile Rust dependencies. Incremental modes are optional and do not skip type, ownership or capability checks.
- 0.2.0互換のビルド・検査・安定性改善を公開。開発用profile、差分ビルド、利用者向けCI生成、読み取り専用cache調査を追加し、既定release設定と旧成功成果物の保護を維持します。IDEプラグインの版と公開は別です。

### Runtime dependency maintenance

- Require Tokio 1.54.1 or newer for the runtime and update the active workspace/fixture locks to 1.54.1, incorporating the timer-driver synchronization fix in [Tokio #8692](https://github.com/tokio-rs/tokio/pull/8692). The previous tracked locks used 1.53.1/1.53.2, not the reported affected 1.54.0. Generated applications have independent locks: rebuild them with the updated runtime; updating Nagi's root lock alone does not update existing binaries.

### Supervisor shutdown stability

- Keep the Supervisor event stream open until the first stop request has published SHUTDOWN. Prevent a concurrent completed cleanup from closing the stream before that notification; preserve child joins, cancellation, STOPPED events and shutdown deadlines. Add a deterministic regression and retain the supervised-worker sample's exact event counts.

### Nagi application development tools

- Add bounded, read-only `nagic cache inspect` for explicitly selected generation and Cargo-cache roots. Report logical sizes, incomplete scans and lossless OS paths; do not delete caches, claim reclaimable space or use observations to authorize native artifact reuse.

- Reuse the parser's original physical-module tokens during source loading, avoiding a second lex while preserving full High/Low checks and diagnostics. Share Cargo dependency targets across frontend regression fixtures and remove an unused duplicate CI release-compiler build; keep all tests and distribution builds.
- Add repeatable full-check and eight-case ordinary-versus-units build benchmarks, including runtime edits, corrupt-lock rejection and old-executable preservation. Report slow cases and cache conditions separately; do not change the default release profile.

- Add opt-in `--profile dev --incremental=units` to group closed primitive synchronous functions into at most eight dependency-ordered Cargo libraries. Keep unsupported functions, async and user types in a facade; rebuild affected units and reverse dependents while Cargo can reuse independent units. Full checks and immutable generations remain mandatory. Receipt schema 3 verifies the complete source-unit closure; existing modes and schemas remain supported.

- Add opt-in `--profile dev --incremental` to reuse the generated Rust body as an immutable content-addressed Cargo library. Identical bodies reuse Cargo artifacts while each generation compiles its own entry shim. Keep full checks, Cargo validation and old executables; reject corrupt source caches. Adapters, additional Rust dependencies and managed runs retain monolithic builds. Receipt schema 2 verifies the body reference; schema 1 remains supported. Released 0.2.0 and default release builds are unchanged.

- Add opt-in `build/run --profile dev|release`, preserving release as the default and the existing debug/release overflow behavior. Keep complete Nagi checks, Cargo dependency validation, generation isolation and immutable successful executables.
- Add opt-in `--build-stats` with typed declaration/function change observations, dependency graphs, Cargo artifact counts and phase timings. Record and verify published artifact hashes; semantic observations do not authorize native reuse or skipped checks/tests.
- Bundle standard-library-only Python CI-generation and receipt-verification tools. Generate workflows compatible with released 0.2.0, isolate cache writes to trusted default-branch pushes, and document fork cache readability. Reject corrupt records, nonregular artifacts and linked/reparse paths.
- Document the existing security API guarantees, unsupported transports and staged build roadmap, with measured application build results and their limits. Previously distributed 0.2.0 binaries remain unchanged. Function-unit partitioning improved unchanged rebuilds in the measured fixture but slowed edited cases, so it remains opt-in; see the [measurements](benchmarks/results/release-0.2.1-build-stability-2026-10-22/README.md).

## JetBrains 0.1.6

- 入力・改行・解析パスの差し替えや取消で、未変更tokenの意味色が一瞬消える経路を修正。表示色を診断の注釈から分離し、同じ完全tokenの色を新しい描画まで保持します。編集後の字句照合・完全な新indexの空表示・hard invalidationと保持上限を維持し、型・説明・診断・定義移動は現在のsnapshotだけを使います。
- keep unchanged semantic colors visible through typing, Enter, and rejected or coalesced analysis passes. Own the visual highlighters separately from diagnostics, revalidate complete tokens after edits, and replace colors from complete current results, including empty results. Preserve invalidation, storage limits, theme keys and current-snapshot requirements for types, documentation, diagnostics and navigation.
- 連続Enterで空行の字下げを1段戻す独自動作を撤去。空行の字下げを維持し、手動Backspace・undo/redo・括弧内・Low・設定幅の動作を保ちます。公開済み0.1.5のZIPは変更しません。
- remove the custom consecutive-Enter block-exit gesture. Keep the current blank-line indentation while preserving manual Backspace, undo/redo, delimiter continuations, Low braces and configured indent widths. Published 0.1.5 assets remain unchanged.
- 入力途中の式・call・文字列や型エラーでは、字句確認できる未変更名の色だけを維持し、現在のpartial情報を優先します。旧型・説明・診断・参照は再利用しません。
- Retain only lexical-checked visual styles while input is incomplete, prioritizing provided current partial colors. Keep the plugin ID, common ZIP, Java 21 bytecode and supported IDE minimum. The compiler remains a separate installation; this plugin release does not publish Nagi 0.2.2.

## JetBrains 0.1.5 — 2026-10-24

- 正常なHigh/Lowソースの末尾で改行した際、公開Nagi 0.2.1のカーソル回復解析がシンボル情報を欠落させても、同じ未保存スナップショットから情報を取り直し、色付け・ホバー説明・定義移動を維持します。`Point`の名前付きコンストラクターと末尾の字下げなし空行を含むケースを検証しています。公開済みplugin 0.1.4にはこの修正は含まれません。
- Recover current symbol facts when published Nagi 0.2.1 omits its index after cursor recovery at a final unindented blank line. Preserve semantic colors, documentation and navigation for the same unsaved buffer, including the typed Point record case. This fix was merged after the 0.1.4 distribution was published.
- Keep the original frontend verdict, diagnostics and completion candidates. Retry at most once within the existing shared deadline, cancellation, snapshot-identity and trusted-project checks; fixed compilers do not require the compatibility retry.
- プラグインIDとIDEA/PyCharm共通ZIP、対応IDE下限、既存のDevelopment/Release Run設定を維持します。Nagi本体とMarketplaceは別の公開工程です。
- Publish a fresh 0.1.5 common plugin ZIP with the existing plugin ID and supported IDE builds. The compiler remains a separate installation; this plugin patch does not release Nagi 0.2.2 or submit to JetBrains Marketplace.

## JetBrains 0.1.4

- Add compiler-backed hover/Quick Documentation for exact function/class/variable targets, including available arguments, return types and fields. Complete the same initial documentation request after background analysis, without waiting on the UI thread or starting a Rust build. Keep bounded waits, cancellation, current-snapshot and trusted-project restrictions; do not invent missing builtin signatures.
- Keep colors for untouched identifiers visible during background analysis, independently of invalidated types, diagnostics and navigation. Discard touched tokens and hard-invalidated presentation data with bounded ownership.
- Render diagnostic tooltips with escaped readable framing while preserving original descriptions, unknown details and hints. Show compiler-provided return types in Parameter Info.
- Preserve manually chosen High indentation on Enter, and add a consecutive-Enter gesture to leave one block at a time. Keep delimiter continuations, Low brace context, smart-indent settings and undo behavior. Align supported High `else`/`case` headers and standalone High/Low closing delimiters with their known owner; preserve ordinary input when context is unknown.
- Default IDE Run to Development when the compiler advertises support; keep explicit Release available in Run Configuration. Validate bounded compiler capabilities off the UI thread. Known older compilers retain Release with an explanatory notice; unknown or failed probes do not silently run. CLI defaults and Check remain unchanged.
- Fix a standard Run handoff race that could dispose an already accepted new Run descriptor when the old environment closes. Preserve cancellation before handoff and process cleanup when the project closes. The reported Windows green console still requires validation with the new candidate.
- 関数・class・変数のホバー説明、初回解析待ち、入力中の色保持、診断表示と編集補助を改善。IDEの開発用Runと画面引き渡しの修正も追加し、既存plugin IDとIDEA/PyCharm共通ZIPを維持します。先行候補のホバー受入を追加修正のGUI確認へ流用しません。
- Prepare version 0.1.4 for publication. The user confirmed the hover/color candidate works; this is not exhaustive GUI coverage. Publish only after the final version's required CI and four IDE test/Verifier paths succeed. GitHub Release and Marketplace submission/approval are separate; this entry does not claim either has already happened. Full PyCharm Python parity, whole-file formatting, Rename and automatic imports remain out of scope.

## JetBrains 0.1.3

- Prepare the common IntelliJ IDEA/PyCharm plugin for the public Nagi 0.2.0 compiler, preserving plugin ID `com.disnana.nagi`, display name **Nagi**, and version 0.1.3. Compiler-backed High/Low completion, structured diagnostics, exact-source definition navigation, semantic coloring and Parameter Info use 0.2.0. Compiler 0.1.11 supports ordinary Check/Run but has no assist protocol.
- Add the standard Nagi Run Configuration with Run/Stop, save-before-run and untrusted-project rejection. Include IDE-setting-aware bracket/quote pairing, bounded indentation and smart Backspace, explicit function live templates, and bounded top-level lexical `def`/`fn` completion. Previously saved local files may supply unsaved analysis buffers; new unsaved files and unresolved definition targets remain unsupported. Editor assistance does not prove full build success.
- Enable `NAGI_RUN_RETENTION=latest` only for IDE-managed runs with compiler 0.2.0. Reclaim eligible generations while protecting recorded successful latest, active and input-referenced generations, last-good runs, and unknown metadata. Leave ordinary CLI generations and the shared Cargo cache alone. No strict disk-capacity limit, arbitrary Rust dependency discovery, or power-loss durability guarantee is added.
- 公開Nagi 0.2.0に対応するJetBrains 0.1.3の正式公開を準備。High/Low補完・診断・正確な定義位置への移動、semantic色分け・引数情報、標準Run/Stop、基本編集補助とIDE管理runの世代回収を含む。0.1.11は通常Check/Runのみでassist未対応。制約とcompiler対応表は[日英の導入案内](editors/jetbrains-nagi/README.md)を参照。
- This entry prepares publication; it does not claim that 0.1.3 assets already exist. After merge to main and successful required IDE tests/Plugin Verifier checks, the existing workflow can create the GitHub Release. Marketplace submission and approval remain separate owner actions. The implementation passed all four IDEA/PyCharm stable/EAP paths in [main CI 39422234923](https://github.com/disnana/Nagi/actions/runs/39422234923); EAP Verifier reports existing deprecated/experimental API warnings. Final GUI acceptance for the release HEAD remains unconfirmed.

## Nagi 0.2.0

- Require an explicit HTTP authentication policy, bounded Host/authority configuration, finite response/rate budgets and route rate rules. Response factories return Result and borrow the actual Request's ResponseContext; error mappers receive that context explicitly. Missing budgets are rejected before listen. See the [migration guide](docs/en/migration-0.2.0.md).
- Connect durable Session issuance/rotation/revocation, bounded reusable authenticated CSRF tokens, separate finite one-use pre-login state, CORS finalization and the fixed Ed25519 verification-only profile. Keep business errors separate from authentication/runtime failures and preserve original deadlines, revocation gates and actual cancellation joins.
- Connect typed HTML/SQL and registered HTTPS operations. Outbound requests use checked DNS candidates, TCP-only finite failover, one-operation TTL=0 results, ordinary TLS verification and finite bodies; no automatic resend after request sending begins or implicit decompression. Compression remains a separate 0.2.1 task.
- HTTP方針・Host・応答/rate予算を明示する安全基盤を候補実装へ接続。ResponseContextは実Requestから借用し、factory/mapperはResultで失敗を返す。Session/CSRF/CORS・Ed25519・typed HTML/SQL・用途別HTTPS操作の保証と移行方法は[日本語ガイド](docs/migration-0.2.0.md)を参照。process-local制限は複数台の全体制限ではなく、OOMからの普遍的回復も保証しない。

### SQLite Pool / Transaction and SQL

- Publish the canonical `std.db.sqlite` Pool and affine Tx API with owned Parameters, opaque direct-literal Query values, structured Failure/Outcome and observed close. Acquisition budgets remain separate from SQL execution and busy handling; cancellation does not undo already committed work.
- Remove legacy Db/db_* execution without a downgrade path. Preserve original-source migration diagnostics, parameter binding, transaction cleanup and actual native joins. See the bilingual migration guide for replacement code and guarantees.
- Use the existing Tokio semaphore and lazy SQLite adapter instead of deadpool. Checked capacity and recoverable reservation refusal do not guarantee universal recovery from OOM.

## JetBrains 0.1.2

- Prepare Nagi for JetBrains as one common IntelliJ IDEA/PyCharm ZIP, preserving plugin ID com.disnana.nagi and the existing Marketplace listing. Use version 0.1.2 so GUI testers and Marketplace can distinguish it from published 0.1.1; the repository owner performs Marketplace upload. This PR does not publish the package.
- Require IDEA 2025.1.1 build 251.25410.109 or PyCharm 2025.1.1 build 251.25410.122 for the public project-trust API. Older IDE users can retain published 0.1.1 or upgrade their IDE. Replace deprecated ProcessAdapter and project-trust calls while keeping untrusted-project execution blocked.
- Verify the same candidate ZIP with independent stable/EAP tests and Plugin Verifier for both IDEs, then promote those exact bytes to the CI distribution artifact. Resolve EAP test dependencies with the official Gradle plugin and keep the common plugin's Java API and bytecode at 21.
- Keep the English/Japanese Marketplace Description in plugin.xml and unify README, documentation, and website installation guidance. The compiler remains a separate installation; compiler-backed completion, definition navigation, and live diagnostics belong to the next PR.

## JetBrains 0.1.1 — 2026-10-08

- Publish the first formal GitHub Release of the IntelliJ IDEA and PyCharm plugin. Include the existing High/Low highlighting, indentation, folding, check/run commands, and gutter Run buttons. Install the matching IC or PC ZIP from disk; the compiler remains a separate installation. This release does not publish to JetBrains Marketplace.
- Publish separately verified IntelliJ IDEA and PyCharm plugin ZIPs to GitHub Releases when the JetBrains plugin version is updated on main. Verify the installed plugin id and version from each packaged JAR, preserve SHA-256 readback and immutable release assets, and keep current-version CI changes out of published releases.

## Nagi 0.1.11 — 2026-10-07

- Add scope-local Task result handles through High, saved Low, and handwritten Low. Require one await or explicit discard for every result type; move transfers both the handle and obligation. Separate inner business Results from sticky TaskFailure, request sibling cancellation on faults, and join direct children before normal/error scope exit. Preserve legacy spawn behavior, original body/legacy Errors, and synchronous Drop limits. Provide typed Supervisor monitor migration using parent try, runnable Task examples, and isolated-distribution Task validation without changing HTTP failure propagation or adding a fault escalation API.
- Add canonical `std.ownership.move` with one inferred input, preserving nested view origins, async function alias provenance, constant validation, and cleanup responsibility. Generate Rust's by-value `std::convert::identity` without a new runtime helper, clone, or allocation. Reject bare owned non-Copy local assignment; fresh values and existing argument, return, field/index, try, and match consumption remain unchanged. Keep current Copy rules and the existing Future restrictions.
- Separate stable application identity, shared dependency caches, and successful native build generations. Serialize writers to the same generated directory with an OS lock, preserve earlier successful executables, and update latest only after a successful build. Use the emitted `native:` path; `NAGI_NATIVE_TARGET_DIR` selects the dependency cache rather than the executable location.
- Seal Rust generation behind `check::finalize` and immutable `check::checked::CheckedProgram`. Rust embedding callers pass the finalized value to `emit::rust` and `emit::rust_with_lines`; mutable Program inputs are no longer accepted. Parser construction replaces external `ast::Stmt` struct literals, and public AST/standard metadata enums gain ownership, Task, and auth variants. See the Japanese and English 0.1.11 migration guides.
- Add a common typed constant validator for compound/alias zero divisors and signed MIN division/remainder across eight integer widths. Preserve debug/release overflow behavior; unreachable constant failures are rejected as documented. Print signed MIN leaves without generating out-of-range Rust literals.
- Add experimental `std.auth.Principal` and nominal `Grant[P]` proof boundaries with no Nagi construction, JSON decoding, copying, or sharing. Demonstrate custom Nagi authorization through a trusted Rust/Axum adapter; the example uses fixed credentials and does not implement JWS verification or general authorization analysis.
- Keep mapped Rust errors concise by default; `build/run --rust-diagnostics` retains generated Rust details. Native, dependency, and unmapped errors stay visible. Extend the conformance corpus to 42 sources and bounded generation to 18 grammars, including explicit moves, function values, multiple borrow sources, and pure async Result/Option.
- Lower view-containing List, Result, and Option restorations through one private checked-flow plan, including aliases, nested values, branches, loops, and async. Preserve source cleanup positions, RHS-before-replacement evaluation, and cancellation without cloning payloads. Use the checker's move/borrow facts and final loop facts rather than reconstructing them from builtin names.
- Keep scope bodies in the same coroutine so local views do not cross an extra async boundary, including functions that use but do not return views. Verify nested body/join errors, completed sibling cancellation on normal errors, parent Future destruction, and body panic with the real runtime through High, saved Low, and handwritten Low.
- Generate lazy `env` fallbacks without a closure so source-level `try` and `await` keep their error and async context. Preserve Task receipt obligations on skipped short-circuit operands and env fallbacks; retain always-evaluated left/key receipts without forcing evaluation. Verify present/missing values, fallback failures, and conditional consumption in all three source forms.
- Add repository-development AGENTS.md, model-specific custom agents, explicit language contracts, compiler pipeline/research notes, bounded program generation with failure artifacts and shrinking, and staged fuzz smoke. Run the bounded corpus in PR CI and larger fixed-seed explorations weekly.
- Give sequential direct-view assignments in return-terminated blocks separate inferred lifetimes. Compile and execute restored input views and earlier nested returns through High, saved Low, and handwritten Low; preserve continuing branch and loop updates.
- Verify Rust extern argument and sync/async mismatches with source-mapped build diagnostics through High, saved Low, and handwritten Low.
- Exercise generated scope code against the real runtime for child errors, child panics, body error propagation, and completed sibling cancellation through all three source forms.

## Nagi 0.1.10

- Infer local lifetimes for view-containing function parameters so checked, non-escaping reassignments and list appends compile through High and saved Low. Preserve public signatures, return-origin checks, Rust adapter wrappers, and owning-parameter destruction order.
- Give direct views reassigned in returning branches separate local lifetimes. Preserve updates on continuing paths, loop and match bindings, and rejection of escaping local views.
- Check byte-view iteration and indexing as `u8`, matching the generated Rust slice. Reject string-view iteration before Rust generation; preserve ordinary and owned-element slices.
- Reject explicit integer division and remainder by literal zero during `check`, with diagnostics at the original source line. Leave runtime-dependent division and floating-point arithmetic unchanged.
- Return HTTP 504 when a handler finishes successfully after its deadline, including synchronous work that does not yield. This does not interrupt synchronous work or roll back application state.
- Add a database-free byte-inspector HTTP example and verify binary inputs, nullable JSON output, and route behavior through High and saved Low.
- Add an Axum quote API with Rust HTTP transport and Nagi async business logic. Verify typed JSON inputs, custom Result errors, invalid configuration, and shutdown through High and saved Low.
- Make Rust-library integration a central design goal in DESIGN, README, and the Japanese/English Docs. Document current adapter boundaries and remaining limitations.
- Add separate coding-agent guides and a reusable development skill under `ai/`, with compiler-checked examples and instructions for validating Rust adapters.
- Add Bash and PowerShell uninstallers with preview modes. Remove verified, unchanged distributions and managed command/PATH entries; preserve modified or unverifiable files.
- Add system, light, and dark appearance settings to the Japanese/English site. Align the header, Docs navigation, and footer across desktop and mobile layouts.
- Run JetBrains integration tests with the current compiler in CI, alongside IntelliJ IDEA and PyCharm plugin compatibility checks.
- Give view-returning function values with no borrowing input a static output lifetime. Preserve input-bound callbacks, nested function lifetime scopes, and static origins through calls in High and Low.
- Diagnose non-Copy field moves through shared values and borrowed resources before Rust generation, including outer `owned` wrappers and temporary call results. Preserve Copy reads, explicit copies, and moves from ordinary owned records.
- Reject bare expressions that discard a Result wrapped in `owned`, including nested ownership wrappers. Preserve explicit bindings and existing `try`/`match` restrictions for wrapped values.
- Add opt-in offline SQLite checks with `check --sql-schema FILE --sql-dialect sqlite`. Validate literal query names, required row columns, and bind counts in a bounded worker; keep dynamic SQL and value/NULL checks at runtime.
- Generate SQLite row decoding for supported scalar fields wrapped in `owned`, including nullable fields. Remove manual `FromRow` workarounds for these fields to avoid duplicate implementations.
- Accept direct borrow-free return values such as `None`, empty lists, and owned error results in view-containing return types. Preserve checks on local views, aliases, and opaque function results.
- Add an initial IntelliJ IDEA/PyCharm plugin for High/Low highlighting, indentation, folding, and explicit check/run commands. Verify IDEA and PyCharm 2025.1.1, with compatibility checks for the supported 2024.3 builds.
- Add gutter Run buttons for top-level High and Low entry points in the JetBrains plugin, preserving project selection, saving, and trust checks.
- Preserve failing exit codes for owned Result entry points and avoid string allocations when reading `error_kind` without taking ownership.
- Give applications distinct executable names when explicitly sharing a native build cache. Keep default names and shared dependency caching unchanged.
- Recover installer updates after a read-only shell profile rejects PATH registration, and reject PATH entries containing colons before changing the installation.
- Add cross-platform TCP regressions for oversized HTTP bodies, incomplete uploads, and connection capacity recovery. Clarify Supervisor reply errors, worker failures, and scope cancellation in both Docs languages.
- Return a generic 500 and close the connection when an HTTP handler unwinds with a panic, including standard HTTP error mappers and the compatibility router. Preserve ordinary Result handling and request deadlines; this does not roll back application state.

## VS Code 0.1.13

- Stop tasks when project inputs or settings change during saving.
- Keep standard-library definition views separate for different compiler contents, including updates that retain the same version string.

## Nagi 0.1.9

- Allow read-only `for` loops over non-Copy List elements, with ownership and lifetime checks. Keep Copy iteration unchanged.
- Add `std.result.map_error` for explicit, typed error conversion through synchronous functions. Preserve `try` error identity and borrowed success values.
- Add explicit task readiness and typed, bounded Supervisor event waits. Reject duplicate, stale, and inactive readiness signals.
- Add borrowed JSON Content-Type validation. The quote API sample retains request IDs in its handler wrapper without intermediate copies.
- Accept multiline calls, constructors, lists, indexing, and function types in handwritten Low, preserving statement boundaries and source diagnostics.
- Reject overlapping generated output before writing, including aliases to source files, embedded assets, Rust adapters, and manifests.
- Catch additional moves that conflict with borrows in indexing, comparisons, and stored views. Keep supported immediate reads, independent fields, copies, and loop reassignment valid.
- Decode nullable SQLite columns for supported scalar row types, including bool, floats, and bytes. Preserve nested Option types through saved Low.
- Keep HTTP response send deadlines active during graceful shutdown while healthy in-flight requests finish.
- Map Rust backend cause notes to Nagi source locations and show readable imported names. Preserve native Rust diagnostics and suggestions.
- Preserve release wrapping and debug overflow checks for constant integer arithmetic. Treat local callbacks named `include_text` as ordinary calls and report the actual Clone requirement for copied list views.
- Keep cross-module edges inside the HTML map viewport and label group counts accurately.
- Add runnable stock-report, device-settings, seat-reservations, file-json, quote-api, supervised-worker, and handwritten Low order-quote projects. Verify their native behavior through High, saved Low, and direct Low.
- Send compiler progress to stderr and keep `nagic run` stdout for application output, including when cost reporting is enabled.
- Make distribution archives reproducible for identical inputs so a resumed draft release can reuse verified assets.

## VS Code 0.1.12

- Save reachable dirty imports before Lower, Build, and Run, including imports outside the project. Stop when saving fails, aliases conflict, or the entry changes while inputs are being saved.
- Show mapped build and run failures in Problems without duplicating unchanged-source checks. Refresh imported-file diagnostics across closed buffers, symlinks, and Windows path casing.
- Keep diagnostics and pending checks for unrelated projects when their dependency snapshots remain valid.
- Complete shadowed local names without inserting constructor or call arguments. Show read-only borrow hints for borrowed loop variables.
- Insert bare declaration names in quoted from-import completion, add Result Ok/Err pattern assistance, and resolve file aliases consistently for completion and definition lookup.
- Add completion, hover, signatures, and definition coverage for the new Result, HTTP, and Supervisor operations.
- Improve Low editing with brace-based folding, language-specific declaration help, import completion, and entry-point/import snippets.
- Scaffold async entry points without requiring a database, and add a database-free HTTP App snippet. Check the High snippets with the Nagi compiler.
- Make VSIX packaging reproducible and keep installation commands independent of a fixed version filename.

## VS Code 0.1.11

- Publish the extension to Visual Studio Marketplace as 0.1.11. There are no functionality changes from 0.1.10.

## Nagi 0.1.8 / VS Code 0.1.10

- Add static `map types`, `map modules`, and `map calls` through a shared Graph IR, with Mermaid, D2, JSON, and standalone HTML renderers, focused views, and optional D2 SVG/PNG export.
- Include the committed version's change summary, previous component release and commit comparison, pull requests, and contributor information in release notes.
- Diagnose moves conflicting with earlier temporary borrows in calls and lists, and consume standalone owned-value expressions during checking. Keep scalar `owned[T]` class fields consistent with generated Rust Copy behavior.
- Keep Docs API references and measurements under HTTP and actor topics, show the current page, preserve sidebar scrolling, and remember the mobile contents menu.
- Serve Docs assets and links from `nagi.disnana.com/`, while preserving project/user Pages defaults for forks. Skip full Rust checks for changes limited to the site's domain and Pages workflow; keep full checks for CI configuration changes.
- Add `std.actor` with typed messages and replies, owned state transitions, named async factories and handlers, restart policies, lifecycle events, and tracked shutdown. Keep business errors separate from handler and call failures.
- Check actor message, reply, and error payloads for owned-capacity accounting; reject unsupported Map, shared, borrowed, and opaque payload graphs before native generation. Preserve actor identities and generic arguments in independently loaded Low.
- Support multiple registered standard modules in completion, signatures, and read-only definition navigation. Add a Supervisor/HTTP sample with state updates, business failures, and explicit shutdown.
- Add `std.http.server`: DB-free apps, native Method/Status, request headers, typed shared state, async handlers, and app/route error mapping. Bound headers, bodies, admission, response sending, and shutdown.
- Support registered standard-module imports, comma-separated from imports, and exhaustive Option matching with Some/None. Preserve resource identity and borrowed lifetimes through saved Low.
- Extend VS Code completion, hover, signature help, and read-only definition navigation to standard resources, constants, and operations.
- Preserve checked numeric types in generated Rust, including integer literals printed or compared without an assignment.
- Restore parser nesting depth after speculative indexing, retain parentheses around try/await field and index receivers, and reject unsupported class type arguments instead of discarding them.
- Preserve string bytes, including control characters, through High-to-Low and Rust generation.
- Diagnose routes colliding with builtin GET endpoints or equivalent capture paths before the HTTP router is constructed.
- Count only changes made by the current db_exec SQL batch, including trigger changes, rather than returning a previous statement's count.
- Insert class names in VS Code type annotations without constructor arguments, and retain hover types for local function values.

## Nagi 0.1.7 / VS Code 0.1.9

- Distinguish a normal class named Future from an async result. Keep synchronous function aliases replaceable when they return that class, and diagnose unsupported changes between different async function aliases before Rust generation.
- Support shared fields in class JSON encoding and decoding. Report class fields with unsupported serialization or built-in Map key types at their Nagi source line. Verify these paths in extracted distributions on all four target platforms.
- Add VS Code indentation support for High block headers, else/case alignment, multiline calls and lists, and Low braces. Respect editor indentation settings and ignore delimiters in strings/comments. Verify real typing, snippets, and Undo in the Extension Host without requiring the compiler.
- Reject stored views of temporary owners and track borrowed JSON inputs before Rust generation. Keep immediate views, owned copies, and match-local borrows available.
- Keep VS Code keyword/type completion, builtin hover, and signature help available in new or incomplete buffers without requiring the compiler. Avoid guessed builtin explanations for shadowed names or unresolved imports.
- Diagnose unsupported JSON value types and non-class database row types at the built-in call's source line. Preserve shared data, borrowed JSON strings/bytes, and Rust bridge implementations for user classes.

- Reject non-printable values in `print` and `write` during type checking, including `unit`, function values, byte/list views, and timestamps. Keep numeric, boolean, string, borrowed string, and UUID output supported.
- Check numeric negation, UUID/timestamp ordering, and borrowed element comparisons before Rust generation. Reject unsupported async function signatures, containers, and stored futures while keeping local async aliases working. Remove the unrelated blanket copy/ownership hint from source diagnostics.
- Detect recursive class layouts through nullable, Result, and owned wrappers. Cache validated class dependencies and report invalid field types at the field's line.

- Use the Marketplace publisher `Disnana` and the extension name as the VSIX manifest identity. The extension ID is `Disnana.nagi-lang`. Include the MIT license and limit Marketplace packages to files needed by the extension. Verify the identity and build a VSIX artifact for extension changes without publishing unchanged versions.
- Keep the Docs sidebar's scroll position when changing pages. Separate introductory guides from language references, list built-in argument types, and clarify explanations and runnable examples in both languages.
- Distinguish missing Cargo from other launch failures. Preserve build diagnostics without treating every Cargo failure as a rejected Nagi program.
- Evaluate spawned task arguments in the parent so owned copies keep their source usable, and reject views retained by the future. Diagnose unsupported scope error types while keeping custom Rust error conversions. Stop supervisor test workers when their parent is dropped.
- Separate installer success, next steps, and app build prerequisites. Use terminal colors where available and keep plain-text labels when colors are disabled.
- Re-running the installer updates to the latest published Nagi release. Keep a fixed command on PATH, restore the previous command if activation fails, and remove unchanged older distributions only after a successful update. Explicit version selection remains available.
- Check entry-point and HTTP handler signatures before invoking Rust, with diagnostics pointing to the original Nagi or Low file. Reject duplicate HTTP routes and multiple request-body parameters.
- Read an `id` query parameter when the route has no path capture, instead of returning an HTTP 500 response.
- Build source files whose names contain punctuation, emoji, or decomposed accents.
- Accept Rust dependency tables with local paths, package aliases, and Cargo feature selection while retaining version strings and complete CLI overrides. Add a local Rust library example and verify it from another directory and all four distribution platforms.
- Use the installed `nagic` command throughout the introductory language and HTTP guides.
- Cache native dependencies during distribution verification and check HTTP route parameters on all four target platforms.

- Consume the Result operand of `try` during ownership checking. Check the first inferred list element once so moving constructors and function calls remain valid.
- Build HTTP servers with no user routes when the builtin `serve` is called. Keep ordinary and locally aliased functions named `serve` independent of HTTP runtime generation.
- Complete the VS Code catalog for all existing builtins and their generic arguments. Preserve type arguments and call parentheses already present when accepting completion, and offer fn/Option type annotations.

Published versions and downloads are listed in [GitHub Releases](https://github.com/disnana/Nagi/releases).
