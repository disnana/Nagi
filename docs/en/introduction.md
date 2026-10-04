# About Nagi

Nagi is a programming language in development for writing backends in readable, typed High code with Python-like syntax. Its standard APIs cover HTTP, JSON, and SQLite; small handwritten Rust adapters connect more specialized work to existing Rust libraries.

The name comes from the Japanese word *nagi* (凪), meaning calm seas: the surface can remain calm while the internals are busy.

## Write an application

Applications normally use High, in `.nagi` files. Indentation defines blocks; types and failures appear in the code. High aims for a consistent, readable style across authors.

Available features include HTTP routes, headers and responses, typed JSON, SQLite, async, actors, and Supervisors. Moves, views, and copies describe data passing; `Result` represents failures. Python library imports are unsupported.

Start with [setup](getting-started.md), then try [HTTP](http.md) or the [sample projects](library-examples.md). The [Axum quote API](../../test-nagi-code/application-examples/axum-service/README.en.md) combines a Rust HTTP layer with Nagi types, async business logic, and `Result`.

## Adjust an implementation

| Layer | Current role |
|---|---|
| High | Application code, types, validation, and business logic |
| Low | Brace syntax using the same type and ownership rules; inspection of generated code and function replacements |
| Rust integration | Calls to Rust libraries and custom implementations through handwritten adapters |

Low has no raw pointers, unsafe syntax, memory layout declarations, or C ABI. Development will prioritize High and Rust integration while preserving existing Low code. See [High and Low](low-language.md) and [Rust integration](modules-and-rust.md) for the current scope.

## Compiler and existing libraries

Nagi has its own lexer, parser, type checker, and Rust generator. It currently emits Rust; rustc performs final checks, optimization, and machine-code generation. Generated executables do not require Rust/Cargo to be installed.

HTTP transport, asynchronous execution, JSON, and SQLite use libraries including Hyper, Tokio, Serde, and rusqlite. Nagi provides typed APIs over these foundations and checks and diagnostics for Nagi source. The standard HTTP server currently uses Hyper; whether to replace it with Axum remains undecided. See [compiler internals](compiler-internals.md) for the division of responsibilities.

Rust code generation remains the backend for now. An independent backend, VM, and self-hosting are neither implemented features nor promises for the next release. See [design decisions](../../DESIGN.en.md) for the rationale, Low's maintenance costs, and open questions.

## Development status

Nagi is in the 0.1 series. The goal of writing common backends entirely in Nagi has not yet been met. SQLite bind argument shapes are fixed. There is no standard PostgreSQL API, outgoing HTTP client, or user-defined opaque resource registration.

`check` checks Nagi types, moves, views, and related rules. rustc performs the final borrow and Rust trait checks, so a successful `check` can still be followed by a failed build. See [ownership](ownership.md) for the boundaries and [syntax](syntax.md) for supported notation.

No speed or maturity advantage over Rust plus Axum has been established. The current APIs, required Rust adapters, diagnostics, measured performance, and limitations are published for evaluation. See the [roadmap](roadmap.md) for remaining work and priorities.
