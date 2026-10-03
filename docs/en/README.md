# Nagi Docs

Nagi's guides and reference documentation. To look up syntax or function behavior, use the [language reference](#language-reference) below.

## First steps

If you are new to Nagi, start here:

1. [Setup and first run](getting-started.md): install Nagi and run Hello World.
2. [Learn by writing code](language-guide.md): variables, functions, lists, and error handling.
3. [HTTP and HTML](http.md): build an API, then store data with [SQLite](database.md).

Write applications in `.nagi` files and run them with `nagic run file.nagi`. See the [VS Code guide](editor.md) for editor support.

## Language reference

| Topic | Page |
|---|---|
| Syntax, operators, and function definitions | [Syntax reference](syntax.md) |
| Built-in function arguments, return values, and limits | [Built-in functions](builtins.md) |
| Types and annotations | [Types and inference](types.md) |
| Grouping related data | [Classes](classes.md) |
| Passing, borrowing, and copying values | [Ownership](ownership.md), [views](view-and-zero-copy.md) |
| Returning failures and handling success or failure | [Error handling](error-handling.md) |

## Build applications

| Task | Page |
|---|---|
| Read and write JSON | [JSON](json.md) |
| Split code into files or call Rust | [Imports and Rust](modules-and-rust.md) |
| Share your own libraries or use Rust crates | [Libraries and Rust assets](libraries.md) |
| Save an entry file and build settings | [nagi.toml](projects.md) |
| Use your own Rust crate from an application | [Local library example](../../test-nagi-code/rust-library/README.en.md) |
| Wait for async work or start child operations | [Async and scopes](async.md), [concurrency](concurrency.md) |
| Read working applications | [Sample projects](library-examples.md) |

Runnable introductory examples are in [examples/tutorial/](../../examples/tutorial/). Function and syntax references also contain code fragments rather than complete programs.

## Implementation and development

See [About Nagi](introduction.md), [Low](low-language.md), [memory handling](memory-model.md), [compiler internals](compiler-internals.md), [language interfaces](ffi.md), and the [roadmap](roadmap.md).

[Actors](actor.md), [worker restarts](supervisor.md), and [queues](queue.md) are experimental implementations. For performance, read the [measurement methods](performance.md), [results](measurements.md), and [HTTP load tests](http-capacity.md).

These Docs describe the current 0.1 series. Each page identifies unsupported features.
