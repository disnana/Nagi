# Changelog

## Unreleased

- Reject index expressions that move their borrowed container, and views saved outside the scope of their owner. Preserve immediate reads, copies, outer owners, static views and returning paths.
- Decode nullable SQLite columns for every already-supported scalar row type, including bool, floats and bytes.
- Preserve nested Option types in generated Low and avoid redundant Rust enum field-pattern warnings.
- Keep HTTP response send deadlines active during graceful shutdown and finish healthy in-flight requests.
- Insert bare declaration names in quoted from-import completion, and add Result Ok/Err pattern assistance in VS Code.
- Add stock-report, device-settings and reservation-worker projects, with native behavior checks for both High and independently loaded Low in CI. Skip Rust checks for sample README translations alone.
- Reject comparison operands that move a value while its left-hand operand still borrows it, including aggregate fields and indexed values. Keep scalar comparisons and independent fields valid.
- Allow loop variables to be reassigned in generated Rust, matching Nagi's type checking.
- Show Nagi locations for Rust backend cause notes and readable names for imported declarations. Keep native Rust diagnostics and suggestions available, and prevent Cargo progress from corrupting diagnostic lines.
- Show mapped build and run failures in VS Code Problems without duplicating unchanged-source checks. Refresh diagnostics when unsaved imported files close, and preserve source locations across symlinks and Windows path casing.

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
