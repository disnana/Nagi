# Nagi's purpose and design decisions

[日本語](DESIGN.md)

Nagi aims to use Rust's performance and libraries while making HTTP, authentication, authorization, validation, database, and data-processing boundaries concise to write. Application logic belongs in readable High code; library-specific configuration and advanced features use Rust adapters. Replacing Rust or Go, or rebuilding HTTP and database infrastructure, is not the goal.

Priorities are reducing integration work and making failures understandable from Nagi code. Direct access to arbitrary Rust APIs, or needing no Rust knowledge at all, is not a claim about current capabilities.

This document covers the current implementation and design decisions adopted or left open. Main, work in a PR, and published releases are separate states. See the [reference](docs/en/README.md) for usage, the [roadmap](docs/en/roadmap.md) for priorities, and [CHANGELOG](CHANGELOG.md) for changes in published releases.

## The development we target

We target both developers building APIs without Rust knowledge and developers combining Rust libraries or their own infrastructure. Ordinary operations should not require Rust adapters; advanced integrations should remain possible.

The syntax resembles Python, but Nagi does not have Python's behavior. Users still need to understand Nagi types, move, view, and Result. Standard PostgreSQL APIs, outgoing HTTP, a database-agnostic DB parameter API, and user-defined opaque resource types are unimplemented. The development source has a SQLite-specific Pool/Tx API with typed Parameters. Even ordinary applications can still require Rust code today.

The name comes from the Japanese word 凪, meaning calm. The idea is that the surface can remain calm while the internals are busy. This guides the design; it is not a promise to hide ownership or failures from users.

## Current decisions

| Topic | Decision | Relationship to implementation |
|---|---|---|
| Primary application syntax | Put High first | Indentation syntax and type/ownership checks are implemented |
| Common operations | Provide Nagi APIs using existing libraries | Implemented for HTTP, JSON, SQLite, and other operations; API gaps remain |
| Advanced operations | Connect through Rust adapters and extern | Sync/async integration exists; arbitrary Rust types are not directly usable |
| Coexistence with Rust assets | Write application logic in High and combine it with existing frameworks, drivers, and custom infrastructure | Bidirectional async integration with Axum is verified in a sample; general async function types are unsupported |
| Low | Limit its scope to compatibility, brace syntax, generated-code inspection, and function replacement | Implemented; expansion into an independent systems language is paused |
| Executable generation | Keep the current Rust backend | Native generation through Rust/Cargo is implemented |
| DB expansion | Separate SQLite and PostgreSQL types; align operation, row, and error conventions | The SQLite Pool/Tx API is merged into main through #99. It is absent from 0.1.11; main 62bbda9 passed four-platform CI. PostgreSQL design follows later |
| Standard HTTP foundation | Evaluate Axum/Tower as the first candidate | Adoption is undecided; a comparison under equal conditions has not been run |
| Independent backend, VM, self-hosting | Leave future adoption open | Unimplemented; these are not current features or next-release promises |

## What High should make consistent

High prioritizes consistent ways to read types, data transfer, and failures over adding more ways to write everyday code. Classes/enums, nullable values, Result, and move/view/shared express these choices and distinguish business failures from execution failures.

Owned arguments generally move; view supports read-only borrowing, and copy can preserve the original value. This does not mean every runtime copy or allocation is explicit. For example, HTTP `text` constructs an owned response from a borrowed string.

Adding an ordinary library operation should not require adding a language keyword. `std.http.server` and `std.actor` can be imported, while JSON and DB operations still include built-ins. Their separation into modules is incomplete.

Evidence: the [type/ownership checker](compiler/src/check.rs), [ownership tests](compiler/tests/ownership.rs), [view-origin tests](compiler/tests/view_origins.rs), [custom-error tests](compiler/tests/typed_errors.rs), [standard-import tests](compiler/tests/stdlib_imports.rs), and [HTTP response implementation](runtime/src/http_server.rs). These tests cover regression cases; they are not proofs for every program.

### Adopted direction for values, failures, and tasks

Python informs the writing style, Rust informs ownership, and Elixir informs actors and supervision. Nagi does not share all their behavior.

| Intent | Direction | Difference from today |
|---|---|---|
| Hand a value over | Move the value and its cleanup responsibility | Existing argument, return, and field consumption rules are retained. Assigning an owned non-Copy local itself requires an explicit operation |
| Read a value again later | Lend a view; make an explicit copy when an independent value is needed | Implemented; changes and moves of the owner are restricted while needed |
| Keep the same value in several places | Share ownership; distinguish handle duplication from payload copying | Implemented; shared alone does not prove thread safety or completed shutdown |
| Write ordinary `a = b` | Use ordinary assignment for Copy; use `std.ownership.move` to transfer an existing non-Copy local | Published in Nagi 0.1.11. Fresh values and existing argument, return, and field/index rules are retained |
| Represent absence or failure | Use nullable or Result; avoid panic for ordinary rejection | Implemented. Nagi try propagates Err; it is not Python try/except |
| Receive a concurrent result | Let scope own child lifetime and receive the result once through a handle | Published in Nagi 0.1.11. Task receives once; legacy spawn retains unit/Result[unit, Error] |
| Receive a child's business Err | Treat it as a result, separately from task failure | A Task's inner business Err lets siblings continue. A legacy spawn Err cancels siblings |
| Send shared values to an actor | Allow explicit shared messages subject to type, capacity, and lifetime conditions | **Unimplemented**. Shared messages/replies are rejected today |

Explicit operations should make the difference from Python reference assignment visible in code. Fresh construction should not mechanically require a move annotation; size thresholds should not decide whether a value is implicitly copied. Arguments, returns, and match are not all being changed at once.

Ordinary arguments and operators evaluating both operands should run left to right; and/or short-circuit. No complete execution order is promised between spawned tasks. A move transfers cleanup responsibility rather than closing the resource. Simple owned locals in one block are intended to be cleaned up in reverse declaration order, with separate rules for reassignment, partial moves, temporaries, fields, Lists, shared values, and Futures. Existing RHS evaluation and cleanup positions are preserved.

[ADR 011](docs/internal/adr/011-language-behavior-and-docs.md) records reasons, evidence, differences, and migration and test conditions. Exact current rules remain in the [language contracts](docs/internal/language-invariants.md). The [tutorial](docs/en/language-guide.md) starts with concrete Python comparisons and does not depend on unimplemented syntax.

Explicit move and the narrow assignment migration are published in [Nagi 0.1.11](https://github.com/disnana/Nagi/releases/tag/nagi-v0.1.11). Use the existing import mechanism, `from std.ownership import move`, then `a = move(b)` to transfer the value and cleanup responsibility. This does not clone, create shared ownership, or extend a lifetime. Fresh construction does not require a move annotation; current Copy rules are retained. Supported local async function aliases remain Copy; moving a Future or nested Future is unsupported. A view is a read borrow, shared provides safe shared ownership, and copy creates an independent duplicate.

The [implementation plan](docs/internal/value-task-implementation-plan.md) records the rules, migration scope, and tests added first. The explicit operation and rejection of ordinary non-Copy local assignment are implemented together; [progress](docs/internal/progress.md) separately records validation across High/Low, Rust generation, samples, both languages of the docs, and four OS CI. S1 Task result handles were merged into main through PR #88. The S2 Supervisor/HTTP monitor migration is implemented with existing APIs; PR #90 passed final four-OS CI and was merged into main. S1 and S2 are published in Nagi 0.1.11.

S1 Task result handles and the S2 Supervisor/HTTP monitor migration are published in Nagi 0.1.11. S2 uses existing APIs; PR #90 passed final four-OS CI before merging into main. [ADR 012](docs/internal/adr/012-task-result-handles.md) is **connected on main**. `task = spawn work()` creates a scope-local handle; `await task` receives once. Every T requires await or `std.task.discard` at normal exits. Move transfers the obligation. Tasks cannot escape via scopes, arguments/returns, fields, containers, wrappers, or another task. Task is non-Copy, non-Clone, and non-shared. Canonical metadata provides TaskFailure kind/message getters. Business Results remain nested inside outer task failures. Handling a receive Err leaves the scope fault sticky: observation, sibling cancellation requests, all actual joins, then scope Error. The original body Err is preserved. Discard abandons receipt without confirming termination.

Only the nearest scope containing a Task binding uses public TaskScope. Legacy-only Scope and Supervisor/HTTP failure coupling remain intact. Private checker ScopeIds/binding obligations and sealed plans drive emission, without implicit clones or ownership inference in the emitter. [Stage 1](docs/internal/task-bridge-stage1-results.md) is historical; [connection results](docs/internal/task-handles-s1-results.md) separately records High/saved/handwritten Low, native paths, regressions, four OS, measurements, and independent review. S2 uses existing APIs to propagate the monitor’s inner Err through the parent body’s `try`, retaining the legacy HTTP spawn. The [service example](test-nagi-code/library-examples/supervised-service/README.en.md) and [validation results](docs/internal/task-handles-s2-results.md) document migration and termination conditions. No new fault escalation API or public Pool/Tx is added. See [Task result handles](docs/en/task-handles.md) for usage.

## Why use Rust?

Nagi performs parsing, name resolution, type/move/view checking, and Rust generation. Rust/Cargo handles dependency builds, final borrow and trait checks, optimization, and machine-code generation. Nagi already has a compiler, but it has no independent machine-code backend.

This division lets us use Tokio, Hyper, Serde, and rusqlite while testing Nagi's syntax and public APIs. It also brings costs: Rust toolchain setup, Cargo build times, consistency with generated code, and Rust-specific diagnostics.

The Nagi checker exists to report Nagi rules at Nagi source locations in `nagic check` and editors. It does not replace rustc. Complex view reassignment and Rust trait boundaries can still cause `build` to fail after `check` succeeds. Nagi can also conservatively reject code that Rust would accept.

Maintaining two checkers requires tracking their differences. Changes to Nagi rules must be checked against generated Rust as well as High/Low checker results. A future independent backend would not automatically inherit the current semantics or runtime.

If supported Nagi code is accepted but compiler-generated Rust is rejected for a type, move, or lifetime problem Nagi could detect, that is a compiler bug. Checks delegated to Rust for crate APIs, traits, handwritten bodies, and the build environment are separate. The internal [language contracts](docs/internal/language-invariants.md), [pipeline comparison](docs/internal/compiler-pipeline.md), and [test guide](docs/internal/compiler-testing.md) describe this boundary; these guides are currently in Japanese.

For returned Lists, Results, and Options containing views, a private storage plan separates the current borrow origin from the source cleanup position. It uses the checker's move/borrow and control-flow facts, the final checked loop body, and ordinary Rust `Option<T>` storage. Public types stay unchanged. Async functions use the same plan; scopes keep local borrowing in the same generated coroutine rather than an extra async boundary.

Rust still handles borrowing, move-related drop flags, unwinding, and Future destruction. Nagi must preserve RHS evaluation, replacement cleanup, and source cleanup positions. The [generation tests](compiler/tests/view_flow_completion.rs), [resource tests](compiler/tests/view_container_drop.rs), and [real scope tests](compiler/tests/scope_runtime_contract.rs) cover these contracts. No payload copies are added, but storage and Future size can increase. This does not solve every borrow pattern or arbitrary resource type.

Evidence: the [compilation pipeline](compiler/src/emit.rs), [ownership-boundary tests](compiler/tests/ownership_boundaries.rs), [build-diagnostic tests](compiler/tests/build_diagnostics.rs), and [Rust dependency tests](compiler/tests/rust_dependencies.rs). See [ownership](docs/en/ownership.md#borrowing-and-the-limits-of-checking) for checking limits.

## Make Rust integration a central goal

Support both application authors using libraries and authors building Rust adapters. Common operations should have standard APIs; custom assets should connect through small, reusable adapters. Exposing every Rust type and trait in High, or wrapping every crate in a dedicated standard API, is not an adopted plan.

| Responsibility | Contents |
|---|---|
| Nagi | Application classes/enums, validation and calculation, Result, async calls, and Nagi diagnostics |
| Rust adapter | Supported type and error conversions, library-specific configuration, and connections to generated Nagi functions |
| Rust libraries | HTTP transport and middleware, DB drivers and pools, cryptography, and OS integration |

Today, `nagi.toml` specifies Cargo dependencies and a Rust file, while `extern` declarations with `@rust` call its functions. Rust can also call a particular generated Nagi function by name and await it. This is source integration within one Cargo build, without a stable external ABI.

The [Axum quote API](test-nagi-code/application-examples/axum-service/README.en.md) puts routing, JSON extraction, and response statuses in Rust; Nagi validates typed quantities and calculates prices. Rust calls a Nagi async function, which also awaits a Rust async operation. This depends on known function names and generated types. It is separate from passing arbitrary async function values through extern parameters.

A server built in Rust follows that adapter's limits, shutdown, and panic handling. Nagi's standard HTTP settings do not apply automatically. Copies, serialization, and error conversions also depend on the adapter.

Mapped Rust type errors report the corresponding Nagi file and statement line. `build/run --rust-diagnostics` also displays generated Rust details. Precise expression columns and coverage of arbitrary Rust diagnostics remain unsupported. Handwritten Rust and dependency errors retain their Rust locations.

Adapter reuse across applications and ownership, sharing, and resource cleanup still need evaluation. User-defined opaque resource types, general async callbacks, and generated declarations remain unimplemented.

### Request-bound authentication and authorization

Unreleased 0.2.0 SF01 binds `std.auth.AuthScope` and one `Grant[P]` to standard requests. Published 0.1.11 Principal/indefinite issuers are migration targets; coexistence does not preserve a downgrade. P is a nominal class/enum, the actual target is i64. Proofs are opaque, nonCopy/Clone/Serde/shared/field and SameTask. Owned same-task arguments, returns, Option/Result and async delegation remain available; Task/Actor transfer is rejected. Ordinary classes and subject IDs are not proofs.

Every standard route requires explicit Policy. The checker matches Policy[S,A] against handler(Request,shared[S],A): public supplies unit, authenticated AuthScope, authorized Grant[P]. Verifiers and authorizers remain trusted callbacks; cryptography, expiry assertions and business permission logic are not statically proven. The dispatcher privately owns a finite lease invalidated by request end/cancellation/Drop. After native capacity wait, one private execution permit is issued under the same short gate as invalidation, checking active state and current time. Later cancellation does not roll back admitted operations. Synchronous enqueue and bound-target use remain trusted adapter obligations.

[ADR 013](docs/internal/adr/013-request-bound-auth-and-http-policy.md), the [public contract](docs/en/security.md) and [migration](docs/en/migration-0.2.0.md) describe APIs and limits. This is not a sandbox proving arbitrary Rust/SQL tenant restrictions, DTO secrecy or all business policies. Retired decorators/global serve/raw HTML/unchecked issuers are removed from standard paths. Typed HTML, durable Sessions, CSRF/CORS, Query and outbound HTTP remain later SF steps.


Evidence: [Rust dependency loading](compiler/src/project.rs), [extern checking](compiler/src/check.rs), [Rust generation](compiler/src/emit.rs), [dependency regressions](compiler/tests/rust_dependencies.rs), and [application verification](scripts/verify_application_examples.py). See [Rust integration](docs/en/modules-and-rust.md) for usage and supported types.

## Is Low necessary?

Low currently provides another syntax over the same AST and type/ownership rules as High. The usual High pipeline also emits Low text, parses and checks it again, then generates Rust. Rewriting code in Low alone does not make it faster or bypass Rust borrow checking.

Its current benefits are inspecting generated code under Nagi rules, replacing a function without changing its High source, and running existing Low code. `@replace` checks argument types, return types, and async signatures. It does not prove behavioral equivalence with the original function.

However, ordinary improvements can also be made by editing the original High function or splitting code into Nagi modules. Rust integration covers low-level implementation. There is currently no evidence that Low is indispensable compared with these alternatives.

Maintaining Low means maintaining two syntaxes, text round-trips, replacement integration, module identity, and diagnostic mappings. Low has no raw pointers, layout control, unsafe, C ABI, SIMD, or independent optimizer. Saved Low preserves module information but not the original High source map or checker proof state. It is not a stable external IR or ABI.

For now, we retain compatibility and existing uses while prioritizing High and Rust integration. Investment in Low should be judged by concrete cases where it reduces inspection, modification, or reproduction effort compared with alternatives, actual use, and maintenance costs. The existence of another way to do something does not by itself negate Low's value. This is not a decision to remove Low or set a migration schedule.

Evidence: the [parser](compiler/src/parser.rs), [lowering and rechecking](compiler/src/emit.rs), [replacement integration](compiler/src/check.rs), [High/Low boundary tests](compiler/tests/ownership_boundaries.rs), and [saved-Low module identity tests](compiler/tests/stdlib_imports.rs). See [High and Low](docs/en/low-language.md) for examples and limits.

Current call maps follow Low replacement bodies and their direct internal calls, while Rust bodies stop at the extern boundary ([tests](compiler/tests/graph_relations.rs)). Definition navigation from a High call still points to its High declaration, rather than automatically following the Low replacement ([tests](compiler/tests/symbols.rs)). Ease of investigation, including Rust-side tools, still needs separate evaluation.

### Work Low could improve

We will test whether Low's current features reduce the effort of investigating or implementing code in the following uses.

- **Understand generated code.** Inspect inferred types, resolved definitions, and explicit view/copy operations in Low; test whether this makes causes easier to follow than reading High or generated Rust alone. Low does not display every ownership move or internal copy.
- **Compare implementations while preserving High.** Replace a function with the same signature and compare outputs, generated Rust, and performance. Evaluate whether separating changes makes experiments easier to reproduce. Low alone does not guarantee faster code or equivalent behavior.

Alternative High implementations, AST views, and Rust adapters are comparison points. Detailed ownership visualization and a future backend IR are separate, unimplemented candidates. Current Low is not evidence of a stable IR or backend independence.

## What libraries handle and what Nagi must decide

Standard HTTP uses Hyper for transport; legacy HTTP uses Axum. Async execution uses Tokio, JSON uses Serde, and SQLite uses rusqlite. Reimplementing HTTP or databases is not a goal in itself.

Using existing libraries leaves design responsibilities in Nagi: which types to expose, when arguments move or borrow, which failures become Result, and what cancellation or close actually completes. Hiding Rust types alone does not produce an easy-to-use Nagi API.

Standard HTTP exposes typed requests, responses, shared state, and async handlers. The existing SQLite `db_*` API converts rows into classes and uses fixed bind shapes. Nagi 0.1.10 supports explicit [SQL/schema checks](docs/en/sql-check.md) for names, required result columns, and bind counts in the existing API; they do not validate actual value types or NULL behavior. Development source also contains an unreleased `std.db.sqlite` Pool/Tx API with variable typed Parameters and explicit transactions. Its SQL checks leave Parameters bind counts and value types unchecked for runtime. See the [SQLite Pool/Tx reference](docs/en/sqlite-pool.md). This API is not included in 0.1.11. PR #99 is merged into main, and main 62bbda9 passed four-platform CI. PostgreSQL has no standard API.

Axum/Tower adoption requires comparison with the same API, connection capacity, deadlines, body limits, panic responses, and shutdown conditions. Since Axum also uses Hyper, Router/middleware evaluation and listener changes should be separate. Existing benchmarks with different conditions do not establish an adoption decision.

Evidence: [dependencies](runtime/Cargo.toml), [standard HTTP](runtime/src/http_server.rs), [SQLite](runtime/src/database.rs), [SQL checking](compiler/src/sql_check/mod.rs) and its [tests](compiler/tests/sql_check.rs), and [HTTP integration tests](tests/http_stdlib_integration.py).

## Failures and concurrency boundaries

Expected failures use Result; missing values use nullable types. Custom classes/enums can represent errors, and HTTP routes can override the application's default error handling. Ordinary input rejection should not be expressed through panic.

The HTTP implementation on main converts catchable handler panics before response start into a 500 without internal details. This does not guarantee recovery during a response, recovery from abort or OOM, or rollback of state changes or database work. Check the [CHANGELOG](CHANGELOG.md) for inclusion in published releases.

Scopes and actors/Supervisors run within one process on Tokio. An Err in an actor's business reply differs from failure of the worker itself. Supervisor restart policies apply to worker failures. Scope sibling cancellation and worker termination conditions also affect application lifetime.

Legacy Scope joins children after its body ends, canceling siblings on a child's Err or panic. A child's failure does not interrupt the running body. Normal error exits await cleanup; direct destruction of the parent Future or unwinding requests abort through synchronous Drop without awaiting child completion. Scopes containing Tasks observe faults during await or exit draining. A Supervisor result handle is connected to the parent's `try` of the inner Result, preserving terminal failure propagation to HTTP cancellation.

Actors use one-for-one supervision: restart a failed child by initializing fresh state. TEMPORARY, TRANSIENT, and PERMANENT distinguish normal completion; explicit stop and parent cancellation are separate. Restart limits do not provide automatic message redelivery or exactly-once processing. Dropping a Control handle is different from terminating the Supervisor owner.

Dropping a future does not necessarily cancel accepted DB work or external sends. Restarting does not prove that repeating a business operation is safe. Idempotency, transactions, cleanup, and resource lifetimes require attention in both APIs and applications.

An independent VM, distributed actors, and hot updates are unimplemented. Nagi does not claim Elixir/BEAM's fault isolation or operational capabilities.

Evidence: [scopes](runtime/src/concurrent.rs), [actors/Supervisors](runtime/src/actor.rs), [lifecycle tests](runtime/src/actor/lifecycle_adversarial_tests.rs), and [HTTP panic tests](runtime/src/http_server/panic_tests.rs). See [errors](docs/en/error-handling.md), [scopes](docs/en/concurrency.md), and [Supervisors](docs/en/supervisor.md) for contracts.

## How to judge value against Rust + Axum

Nagi currently provides High syntax, Nagi-location type/move/view diagnostics, typed HTTP and custom errors, scope and actor/Supervisor APIs, editor integration, and static code maps. Using these through Nagi code is implemented; none of these capabilities is inherently impossible in Rust.

Compared with Rust + Axum, Nagi lacks library choices, type/trait expressiveness, resource handling, and an established tooling and operational record. Some situations still require understanding generated code or Rust adapters. There is currently no evidence of greater maturity or better performance.

Assess Nagi on applications with equivalent API, DB operations, and failure conditions: code required, diagnostic clarity, frequency of Rust adapters, and maintenance burden. Compare throughput, latency, CPU, memory, and overload recovery under equal limits and measurement conditions. Poor results should lead us to consider reducing custom implementations or abstractions.

Evidence and examples: [symbols](compiler/tests/symbols.rs), [code maps](compiler/tests/graph_relations.rs), [application verification](scripts/verify_application_examples.py), and [Rust integration examples](test-nagi-code/library-examples/README.en.md). Current performance results and conditions are in [measurements](docs/en/measurements.md).

## When to update this document

The [compiler/Rust boundary plan](docs/internal/compiler-rust-boundary-plan.md) stages a sealed CheckedProgram, isolated build generations, shared resource contracts, and Pool/Transaction validation. It retains the High-to-Low text round-trip and the Rust backend.

Phase 1's CheckedProgram was merged into main in PR #78 and shipped in Nagi 0.1.11. Code generation uses plans fixed during sealing rather than inferring types or borrows again. See the [final factory](compiler/src/check/checked.rs), [boundary tests](compiler/src/check/checked_tests.rs), and [ADR 006](docs/internal/adr/006-sealed-codegen-input.md).

CheckedProgram carries checked Nagi and private Rust generation plans; it is not a complete backend-independent IR. Self-hosting and another backend are separate possibilities. A compiler written in Nagi could conceptually continue generating Rust. Neither is added to the current implementation plan.

Phase 2's development changes separate application identity from successful generations. An OS lock serializes writers to the same output directory. Cargo builds a generation-specific bin, then Nagi copies its executable and updates latest only after success. Builds do not overwrite, delete, or kill earlier executables. Dependency caches remain shared, and the lock is released before running the application. Each generation stores generated Low, Rust, the manifest, read source text, and existing line mappings. This does not provide an atomic snapshot of external Rust and dependency sources, process isolation, or power-loss durability. See the [publication implementation](compiler/src/generation.rs), [real Cargo regressions](compiler/tests/build_generations.rs), and [ADR 007](docs/internal/adr/007-build-generations.md).

Phase 2's PR #79 passed the four-platform and editor/package CI and was merged into main. These changes target Nagi 0.1.11; check the official release record to confirm published availability. Phase 3 first records current resource behavior in characterization tests, then consolidates capability and type-argument retention metadata. It preserves purpose-specific checks and existing APIs and does not add resource lifecycle guarantees. [ADR 008](docs/internal/adr/008-resource-contracts.md) records the structure and validation order. PR #80 passed the four-platform, editor, website, and merge-gate CI and was merged into main. The subsequent SQLite Pool/Tx API is implemented in development source and documented in the [public reference](docs/en/sqlite-pool.md). It is not included in Nagi 0.1.11. PR #99 is merged into main, and main 62bbda9 passed four-platform CI. The [decision record](docs/internal/open-questions.md) and [progress](docs/internal/progress.md) distinguish verified coverage, changes on main or in published versions, and planned work.

Q002 approved the SQLite API, SQL restrictions, termination contract, and runtime rusqlite hooks on 2026-10-06. The initial deadpool comparison was later replaced by an adapter built on the existing Tokio Semaphore and lazy worker state; deadpool/deadpool-runtime were removed, with no new crate or Tokio/rusqlite version. Development source now contains `std.db.sqlite` with eight resources and 18 operations; the existing `db_*` API is unchanged. See the [SQLite Pools and Transactions reference](docs/en/sqlite-pool.md) for its use, failure outcomes, SQL restrictions, and lifecycle limits. It is not in Nagi 0.1.11. Compiler/runtime changes are merged into main through #99, and main 62bbda9 passed four-platform CI. The new API has not yet been shipped in a formal release.

The initial private transaction/SQL regressions are now in [SQLite session](runtime/src/sqlite/session.rs) and [SQLite tests](runtime/src/sqlite/tests.rs). A SQL error alone does not prove that earlier changes were rolled back. Adapter close/capacity/join regressions are in [adapter tests](runtime/src/sqlite/adapter_tests.rs), and public API regressions are in [public tests](runtime/src/sqlite/public_tests.rs). The earlier private prototype and deadpool comparison are historical context; the current API does not use those deadpool dependencies.

PR #84/#85 checked multiple connections, native capacity, independent join, and acquisition budgets in private stages, then passed their corresponding four-platform CI and reached main. Those prototype results do not count as acceptance for the current public API. Its current contract and validation limits are summarized in the [SQLite Pools and Transactions reference](docs/en/sqlite-pool.md). Main 62bbda9, including the public API, passed four-platform CI.

Changes to semantics, public APIs, or the High/Low/Rust division should update the rationale, alternatives, compatibility, and verification results here. Detailed API descriptions and measurement logs belong in their corresponding documents.

When a proposal becomes implemented, add implementation and test references. Do not expand test coverage into a language-wide guarantee or describe unmeasured effects as measured. Agreement in a discussion and verified behavior remain separate evidence.

## 0.2.0 Security Foundation design stage

The [Security Foundation RFC](docs/internal/security-foundation/rfc.en.md) records current-main investigation, AuthScope/CSRF/XSS/SQL injection/SSRF/CORS/Cookie/Session/DoS proposals, static/runtime boundaries, migration, feature PRs, and completion gates. The latest instruction establishes [security-first D1–D3 decisions and migration](docs/internal/security-foundation/decisions-and-migration.en.md) as the implementation direction: mandatory policy across standard HTTP, one request-bound Grant, and durable Sessions, replacing legacy coexistence. SF01 is connected in development source; SF02–SF08 remain unimplemented. Formal 0.2.0 is unreleased and does not retroactively apply to 0.1.x. Preserve move/Task/spawn, High/Low and SQLite native lifecycle. Formal 0.2.0 publication and tags require separate explicit approval.
