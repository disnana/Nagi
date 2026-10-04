# Nagi Docs

Start with [installation](getting-started.md). To look up code, open the [syntax reference](syntax.md) or [built-in functions](builtins.md).

These Docs describe the repository source; code examples use Nagi 0.1.10 and VS Code extension 0.1.13. The official site is normally built from main and can include unreleased changes. See the [Changelog](../../CHANGELOG.md) for versioned changes and work under `Unreleased`.

See [About Nagi](introduction.md) for its purpose and current scope. The references below describe implemented APIs and their limits. Unimplemented proposals are kept in [design](library-design.md) and the [roadmap](roadmap.md).

## First steps

1. [Setup and first run](getting-started.md): install Nagi and run Hello World.
2. [Learn by writing code](language-guide.md): variables, functions, lists, and error handling.
3. [HTTP and HTML](http.md): build an API, then store data with [SQLite](database.md).

See the [VS Code guide](editor.md) for editor support. See [JetBrains installation](../../editors/jetbrains-nagi/README.en.md) for IntelliJ IDEA and PyCharm.

## Language reference

| Topic | Page |
|---|---|
| Syntax, operators, and function definitions | [Syntax reference](syntax.md) |
| Built-in function arguments, return values, and limits | [Built-in functions](builtins.md) |
| Types and annotations | [Types and inference](types.md) |
| Named data fields | [Classes](classes.md) |
| Passing, borrowing, and copying values | [Ownership](ownership.md), [views](view-and-zero-copy.md) |
| Returning and handling failures | [Error handling](error-handling.md) |

## Build applications

| Task | Page |
|---|---|
| HTTP without a database, headers, and response statuses | [HTTP](http.md), [API reference](http-server.md) |
| Read and write JSON | [JSON](json.md) |
| Check SQLite SQL against a schema | [SQL preflight checks](sql-check.md) |
| Split code into files or call Rust | [Imports and Rust](modules-and-rust.md) |
| Share your own libraries or use Rust crates | [Libraries and Rust assets](libraries.md) |
| Save an entry file and build settings | [nagi.toml](projects.md) |
| Use your own Rust crate from an application | [Local library example](../../test-nagi-code/rust-library/README.en.md) |
| Wait for async work or start child operations | [Async and scopes](async.md), [concurrency](concurrency.md) |
| Send messages to stateful tasks and manage restart and shutdown | [Actors](actor.md), [Supervisors](supervisor.md), [API reference](actor-reference.md) |
| Read working applications | [Sample projects](library-examples.md) |
| Diagram types, modules, and calls | [Code maps](code-map.md) |

Runnable examples are in [examples/tutorial/](../../examples/tutorial/). References also include code fragments.

## Implementation and development

[About Nagi](introduction.md) · [Design decisions](../../DESIGN.en.md) · [Low](low-language.md) · [Memory](memory-model.md) · [Compiler](compiler-internals.md) · [Language interfaces](ffi.md) · [Roadmap](roadmap.md)

`std.actor` is a standard library available from Nagi 0.1.8. The [sample](../../test-nagi-code/library-examples/supervised-service/README.en.md) covers registration, calls, and shutdown. The older actor/Supervisor built-ins and [queues](queue.md) remain test APIs. For performance, read the [measurement methods](performance.md), [results](measurements.md), and [HTTP load tests](http-capacity.md).

These pages cover the current source and Nagi 0.1. See the [change log](../../CHANGELOG.md) for changes by version. Unsupported features are listed on each page.
