# Nagi's purpose and design decisions

[日本語](DESIGN.md)

Nagi aims to let developers write ordinary backends in readable High code and use Rust assets where needed. For applications using HTTP, JSON, and databases, the goal is to reduce the need for handwritten Rust and make failures understandable from Nagi code.

This document records the choices behind that goal and the questions still open. Usage belongs in the [reference](docs/en/README.md); priorities belong in the [roadmap](docs/en/roadmap.md). An adopted direction and an implemented feature are different things. This document describes the repository source, not the feature set of a published Release.

## The development we target

We target both developers building APIs without Rust knowledge and developers combining Rust libraries or their own infrastructure. Ordinary operations should not require Rust adapters; advanced integrations should remain possible.

The syntax resembles Python, but Nagi does not have Python's behavior. Users still need to understand Nagi types, move, view, and Result. Standard PostgreSQL, outgoing HTTP, general DB arguments, and user-defined opaque resource types are unimplemented. Even ordinary applications can still require Rust code today.

The name comes from the Japanese word 凪, meaning calm. The idea is that the surface can remain calm while the internals are busy. This guides the design; it is not a promise to hide ownership or failures from users.

## Current decisions

| Topic | Decision | Relationship to implementation |
|---|---|---|
| Primary application syntax | Put High first | Indentation syntax and type/ownership checks are implemented |
| Common operations | Provide Nagi APIs using existing libraries | Implemented for HTTP, JSON, SQLite, and other operations; API gaps remain |
| Advanced operations | Connect through Rust adapters and extern | Sync/async integration exists; arbitrary Rust types are not directly usable |
| Low | Limit its scope to compatibility, brace syntax, generated-code inspection, and function replacement | Implemented; expansion into an independent systems language is paused |
| Executable generation | Keep the current Rust backend | Native generation through Rust/Cargo is implemented |
| DB expansion | Separate SQLite and PostgreSQL types; align operation, row, and error conventions | New APIs are unimplemented; module names and resource contracts are undecided |
| Standard HTTP foundation | Evaluate Axum/Tower as the first candidate | Adoption is undecided; a comparison under equal conditions has not been run |
| Independent backend, VM, self-hosting | Leave future adoption open | Unimplemented; these are not current features or next-release promises |

## What High should make consistent

High prioritizes consistent ways to read types, data transfer, and failures over adding more ways to write everyday code. Classes/enums, nullable values, Result, and move/view/shared express these choices and distinguish business failures from execution failures.

Owned arguments generally move; view supports read-only borrowing, and copy can preserve the original value. This does not mean every runtime copy or allocation is explicit. For example, HTTP `text` constructs an owned response from a borrowed string.

Adding an ordinary library operation should not require adding a language keyword. `std.http.server` and `std.actor` can be imported, while JSON and DB operations still include built-ins. Their separation into modules is incomplete.

Evidence: the [type/ownership checker](compiler/src/check.rs), [ownership tests](compiler/tests/ownership.rs), [view-origin tests](compiler/tests/view_origins.rs), [custom-error tests](compiler/tests/typed_errors.rs), [standard-import tests](compiler/tests/stdlib_imports.rs), and [HTTP response implementation](runtime/src/http_server.rs). These tests cover regression cases; they are not proofs for every program.

## Why use Rust?

Nagi performs parsing, name resolution, type/move/view checking, and Rust generation. Rust/Cargo handles dependency builds, final borrow and trait checks, optimization, and machine-code generation. Nagi already has a compiler, but it has no independent machine-code backend.

This division lets us use Tokio, Hyper, Serde, and rusqlite while testing Nagi's syntax and public APIs. It also brings costs: Rust toolchain setup, Cargo build times, consistency with generated code, and Rust-specific diagnostics.

The Nagi checker exists to report Nagi rules at Nagi source locations in `nagic check` and editors. It does not replace rustc. Complex view reassignment and Rust trait boundaries can still cause `build` to fail after `check` succeeds. Nagi can also conservatively reject code that Rust would accept.

Maintaining two checkers requires tracking their differences. Changes to Nagi rules must be checked against generated Rust as well as High/Low checker results. A future independent backend would not automatically inherit the current semantics or runtime.

Evidence: the [compilation pipeline](compiler/src/emit.rs), [ownership-boundary tests](compiler/tests/ownership_boundaries.rs), [build-diagnostic tests](compiler/tests/build_diagnostics.rs), and [Rust dependency tests](compiler/tests/rust_dependencies.rs). See [ownership](docs/en/ownership.md#borrowing-and-the-limits-of-checking) for checking limits.

## Is Low necessary?

Low currently provides another syntax over the same AST and type/ownership rules as High. The usual High pipeline also emits Low text, parses and checks it again, then generates Rust. Rewriting code in Low alone does not make it faster or bypass Rust borrow checking.

Its current benefits are inspecting generated code under Nagi rules, replacing a function without changing its High source, and running existing Low code. `@replace` checks argument types, return types, and async signatures. It does not prove behavioral equivalence with the original function.

However, ordinary improvements can also be made by editing the original High function or splitting code into Nagi modules. Rust integration covers low-level implementation. There is currently no evidence that Low is indispensable compared with these alternatives.

Maintaining Low means maintaining two syntaxes, text round-trips, replacement integration, module identity, and diagnostic mappings. Low has no raw pointers, layout control, unsafe, C ABI, SIMD, or independent optimizer. Saved Low preserves module information but not the original High source map or checker proof state. It is not a stable external IR or ABI.

For now, we retain compatibility and existing uses while prioritizing High and Rust integration. Investment in Low should be judged by concrete cases where it reduces inspection, modification, or reproduction effort compared with alternatives, actual use, and maintenance costs. The existence of another way to do something does not by itself negate Low's value. This is not a decision to remove Low or set a migration schedule.

Evidence: the [parser](compiler/src/parser.rs), [lowering and rechecking](compiler/src/emit.rs), [replacement integration](compiler/src/check.rs), [High/Low boundary tests](compiler/tests/ownership_boundaries.rs), and [saved-Low module identity tests](compiler/tests/stdlib_imports.rs). See [High and Low](docs/en/low-language.md) for examples and limits.

Current call maps follow Low replacement bodies and their direct internal calls, while Rust bodies stop at the extern boundary ([tests](compiler/tests/graph_relations.rs)). Definition navigation from a High call still points to its High declaration, rather than automatically following the Low replacement ([tests](compiler/tests/symbols.rs)). Ease of investigation, including Rust-side tools, still needs separate evaluation.

### Work Low could improve

Pausing systems-language extensions does not mean abandoning Low's value. The following uses should be evaluated with current features. These are not measured benefits or promises to adopt new features on a schedule.

- **Understand generated code.** Inspect inferred types, resolved definitions, and explicit view/copy operations in Low; test whether this makes causes easier to follow than reading High or generated Rust alone. Low does not display every ownership move or internal copy.
- **Compare implementations while preserving High.** Replace a function with the same signature and compare outputs, generated Rust, and performance. Evaluate whether separating changes makes experiments easier to reproduce. Low alone does not guarantee faster code or equivalent behavior.

Alternative High implementations, AST views, and Rust adapters are comparison points. Detailed ownership visualization and a future backend IR are separate, unimplemented candidates. Current Low is not evidence of a stable IR or backend independence.

## What libraries handle and what Nagi must decide

Standard HTTP uses Hyper for transport; legacy HTTP uses Axum. Async execution uses Tokio, JSON uses Serde, and SQLite uses rusqlite. Reimplementing HTTP or databases is not a goal in itself.

Using existing libraries leaves design responsibilities in Nagi: which types to expose, when arguments move or borrow, which failures become Result, and what cancellation or close actually completes. Hiding Rust types alone does not produce an easy-to-use Nagi API.

Standard HTTP exposes typed requests, responses, shared state, and async handlers. SQLite converts rows into classes, but bind arguments have fixed shapes, and ordinary `check` does not validate SQL strings or column names. Main includes explicit [SQL/schema checks](docs/en/sql-check.md) for names, required result columns, and bind counts. These checks do not validate value types or NULL behavior and are absent from published 0.1.9. Standard pool, transaction, and PostgreSQL APIs are absent. New DB resources and adapter contracts are discussed in the [library design proposal](docs/en/library-design.md).

Axum/Tower adoption requires comparison with the same API, connection capacity, deadlines, body limits, panic responses, and shutdown conditions. Since Axum also uses Hyper, Router/middleware evaluation and listener changes should be separate. Existing benchmarks with different conditions do not establish an adoption decision.

Evidence: [dependencies](runtime/Cargo.toml), [standard HTTP](runtime/src/http_server.rs), [legacy HTTP](runtime/src/http.rs), [SQLite](runtime/src/database.rs), [SQL checking](compiler/src/sql_check/mod.rs) and its [tests](compiler/tests/sql_check.rs), and [HTTP integration tests](tests/http_stdlib_integration.py).

## Failures and concurrency boundaries

Expected failures use Result; missing values use nullable types. Custom classes/enums can represent errors, and HTTP routes can override the application's default error handling. Ordinary input rejection should not be expressed through panic.

The HTTP implementation on main converts catchable handler panics before response start into a 500 without internal details. This does not guarantee recovery during a response, recovery from abort or OOM, or rollback of state changes or database work. Check the [CHANGELOG](CHANGELOG.md) for inclusion in published releases.

Scopes and actors/Supervisors run within one process on Tokio. An Err in an actor's business reply differs from failure of the worker itself. Supervisor restart policies apply to worker failures. Scope sibling cancellation and worker termination conditions also affect application lifetime.

Dropping a future does not necessarily cancel accepted DB work or external sends. Restarting does not prove that repeating a business operation is safe. Idempotency, transactions, cleanup, and resource lifetimes require attention in both APIs and applications.

An independent VM, distributed actors, and hot updates are unimplemented. Nagi does not claim Elixir/BEAM's fault isolation or operational capabilities.

Evidence: [scopes](runtime/src/concurrent.rs), [actors/Supervisors](runtime/src/actor.rs), [lifecycle tests](runtime/src/actor/lifecycle_adversarial_tests.rs), and [HTTP panic tests](runtime/src/http_server/panic_tests.rs). See [errors](docs/en/error-handling.md), [scopes](docs/en/concurrency.md), and [Supervisors](docs/en/supervisor.md) for contracts.

## How to judge value against Rust + Axum

Nagi currently provides High syntax, Nagi-location type/move/view diagnostics, typed HTTP and custom errors, scope and actor/Supervisor APIs, editor integration, and static code maps. Using these through Nagi code is implemented; none of these capabilities is inherently impossible in Rust.

Compared with Rust + Axum, Nagi lacks library choices, type/trait expressiveness, resource handling, and an established tooling and operational record. Some situations still require understanding generated code or Rust adapters. There is currently no evidence of greater maturity or better performance.

Assess Nagi on applications with equivalent API, DB operations, and failure conditions: code required, diagnostic clarity, frequency of Rust adapters, and maintenance burden. Compare throughput, latency, CPU, memory, and overload recovery under equal limits and measurement conditions. Poor results should lead us to consider reducing custom implementations or abstractions.

Evidence and examples: [symbols](compiler/tests/symbols.rs), [code maps](compiler/tests/graph_relations.rs), [application verification](scripts/verify_application_examples.py), and [Rust integration examples](test-nagi-code/library-examples/README.en.md). Current performance results and conditions are in [measurements](docs/en/measurements.md).

## When to update this document

Changes to semantics, public APIs, or the High/Low/Rust division should update the rationale, alternatives, compatibility, and verification results here. Detailed API descriptions and measurement logs belong in their corresponding documents.

When a proposal becomes implemented, add implementation and test references. Do not expand test coverage into a language-wide guarantee or describe unmeasured effects as measured. Agreement in a discussion and verified behavior remain separate evidence.
