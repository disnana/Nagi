# CI coverage levels (post 0.2.0 candidate)

This change is separate from the frozen 0.2.0 release. No measured speedup is claimed. Existing comprehensive command bodies and diagnostic parsers remain unchanged; only their selection and matrix wiring change. The new wiring has an exact inverse to the original workflow before its historical source guards run.

| Coverage | L1 ordinary PR | L2 main / weekly / escalated PR | L3 explicit release / RC |
| --- | --- | --- | --- |
| Change, source, release-planning and policy checks | Required | Required | Required |
| Critical Linux compiler/runtime library tests, two front-end integration targets, fmt/clippy, doctests | Required for code | Comprehensive suite includes these checks | Comprehensive suite includes these checks |
| Original Linux comprehensive checks and Rust 1.88 profile | Deferred | Required for selected Nagi work | Required for selected Nagi work |
| Original package and CLI/generated/native regression steps | Deferred | Linux, Windows, macOS ARM | All four platforms including macOS Intel |
| Browser trust/E2E | Existing PR selection retained | Required for selected Nagi work | Required for selected Nagi work |
| IDE checks and VSIX/plugin packaging | Existing component selectors | Existing component selectors | Required for every releasing component |
| Install, upgrade, uninstall, distribution extraction, security and compatibility contracts | Comprehensive stages | Original selected-platform commands | Original commands on all four Nagi platforms |
| Stable Ready to merge aggregate | Required | Required | Required |

The L1 Rust allowlist is limited to `compiler/src/lexer.rs`, `compiler/src/parser.rs` and `compiler/tests/enum_parser.rs`, plus existing presentation/plugin paths. Checker, emitter, lowering, every runtime/task/ownership/auth/storage/network boundary, dependencies, build, workflow and unknown paths escalate to L2. A missing, malformed, empty or incomplete Git range escalates. There is no empty filter or arbitrary thread limit. The critical inventory guard compiles all four fixed test targets and requires each executable to list nonempty unique cases before tests run.

Any validated planned component release forces L3. A manual `release` coverage request also selects L3 but does not authorize publishing. IDE-only release plans retain their component gates without inventing Nagi work. Release flags, matrices, booleans and actual selected job outcomes must match; skipped, missing, cancelled or failed selected jobs cannot pass Ready. Publication depends on Ready, ci-plan and release-plan success, L3, a main push and an actual approved component release. Before downloads or publication, publication_gate.py independently rechecks every selected job, release/package consistency, the exact matrix, and release-plan SHA against this workflow commit. A failed, cancelled, skipped or incomplete selected job refuses publication.

The weekly schedule is Monday 03:23 UTC. Version/CHANGELOG approval policy stays in the existing release planner; no PR, tag or manual action publishes automatically. Main release runs remain immutable and are not cancelled by later main commits.

Nextest remains an opt-in comparison, not the runner for mandatory tests. It executes cases in separate processes, so nested Cargo/native builds, global state, resource contention and libtest-dependent diagnostic parsers need separate evidence before conversion. Doctests stay with Cargo. No release/test profile, timeout, assertion, test seed or feature setting changes. Actions are pinned to reviewed commit SHAs. Heavy Rust caches restore with existing keys but save only after all ordinary steps succeed on main; the L1 Rust build/dependency cache is separately scoped to a successful PR and cannot feed publication producers. See [audit and measurements](ci-optimization.en.md) and [nextest experiment](nextest-evaluation.en.md).
