# Nagi Docs

Start with [installation](getting-started.md). To look up code, open the [syntax reference](syntax.md) or [built-in functions](builtins.md).

## First steps

1. [Setup and first run](getting-started.md): install Nagi and run Hello World.
2. [Learn by writing code](language-guide.md): variables, functions, lists, and error handling.
3. [HTTP and HTML](http.md): build an API, then store data with [SQLite](database.md).

See the [VS Code guide](editor.md) for editor support.

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
| Split code into files or call Rust | [Imports and Rust](modules-and-rust.md) |
| Share your own libraries or use Rust crates | [Libraries and Rust assets](libraries.md) |
| Save an entry file and build settings | [nagi.toml](projects.md) |
| Use your own Rust crate from an application | [Local library example](../../test-nagi-code/rust-library/README.en.md) |
| Wait for async work or start child operations | [Async and scopes](async.md), [concurrency](concurrency.md) |
| Read working applications | [Sample projects](library-examples.md) |

Runnable examples are in [examples/tutorial/](../../examples/tutorial/). References also include code fragments.

## Implementation and development

[About Nagi](introduction.md) · [Low](low-language.md) · [Memory](memory-model.md) · [Compiler](compiler-internals.md) · [Language interfaces](ffi.md) · [Roadmap](roadmap.md)

[Actors](actor.md), [worker restarts](supervisor.md), and [queues](queue.md) are experimental implementations. For performance, read the [measurement methods](performance.md), [results](measurements.md), and [HTTP load tests](http-capacity.md).

These pages cover Nagi 0.1. Unsupported features are listed on each page.
