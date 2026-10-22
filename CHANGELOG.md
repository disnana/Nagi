# Changelog

## 未リリース / Unreleased

### JetBrains editing assistance

- Add compiler-backed hover/Quick Documentation for exact function/class targets, including available arguments, return types and fields. Render diagnostic tooltips with escaped readable framing while preserving original descriptions, unknown details and hints; retain cold/current-snapshot and trusted-project restrictions.

- Preserve manually chosen High indentation on Enter, and add a Nagi-specific consecutive-Enter gesture to leave one block at a time. Keep delimiter continuations, Low brace context, smart-indent settings and undo behavior.
- Align supported High `else`/`case` headers on colon input and standalone High/Low closing delimiters with their known owner. Use bounded lexical context and leading-whitespace edits; preserve ordinary input when context is unknown.
- Document the source-based comparison with PyCharm Python editing, applicable improvements and the compiler/formatter work needed for broader support. Do not claim full Python editor parity.

### Runtime dependency maintenance

- Require Tokio 1.54.1 or newer for the runtime and update the active workspace/fixture locks to 1.54.1, incorporating the timer-driver synchronization fix in [Tokio #8692](https://github.com/tokio-rs/tokio/pull/8692). The previous tracked locks used 1.53.1/1.53.2, not the reported affected 1.54.0. Generated applications have independent locks: rebuild them with the updated runtime; updating Nagi's root lock alone does not update existing binaries. This change is unreleased.

### Nagi application development tools

- Add opt-in `--profile dev --incremental=units` to group closed primitive synchronous functions into at most eight dependency-ordered Cargo libraries. Keep unsupported functions, async and user types in a facade; rebuild affected units and reverse dependents while Cargo can reuse independent units. Full checks and immutable generations remain mandatory. Receipt schema 3 verifies the complete source-unit closure; existing modes and schemas remain supported.

- Add opt-in `--profile dev --incremental` to reuse the generated Rust body as an immutable content-addressed Cargo library. Identical bodies reuse Cargo artifacts while each generation compiles its own entry shim. Keep full checks, Cargo validation and old executables; reject corrupt source caches. Adapters, additional Rust dependencies and managed runs retain monolithic builds. Receipt schema 2 verifies the body reference; schema 1 remains supported. Released 0.2.0 and default release builds are unchanged.

- Add opt-in `build/run --profile dev|release`, preserving release as the default and the existing debug/release overflow behavior. Keep complete Nagi checks, Cargo dependency validation, generation isolation and immutable successful executables.
- Add opt-in `--build-stats` with typed declaration/function change observations, dependency graphs, Cargo artifact counts and phase timings. Record and verify published artifact hashes; semantic observations do not authorize native reuse or skipped checks/tests.
- Bundle standard-library-only Python CI-generation and receipt-verification tools in future distributions. Generate workflows compatible with released 0.2.0, isolate cache writes to trusted default-branch pushes, and document fork cache readability. Reject corrupt records, nonregular artifacts and linked/reparse paths.
- Document the existing security API guarantees, unsupported transports and staged build roadmap, with measured application build results and their limits. These features are unreleased; published 0.2.0 remains unchanged.

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
