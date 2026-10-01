# Writing Nagi

If this is your first time using Nagi, read **[Setup and first run](getting-started.md) → [Learn by writing code](language-guide.md)**. Use High (`.nagi`) to write applications. You do not need to learn Low first.

## Start here

| What you want to do | Page | What you will learn |
|---|---|---|
| Run Hello World | [Setup and first run](getting-started.md) | Windows/Linux commands, creating files, building, and VS Code |
| Use types and completion in VS Code | [Editor walkthrough](editor.md) | Variable type hovers, field completion, argument hints, and F12 |
| Learn the language step by step | [Learn by writing code](language-guide.md) | Variables, functions, lists, classes, views, Result, and imports |
| Look up syntax | [Syntax reference](syntax.md) | Notation, operators, and differences from Python |
| Find a built-in function | [Built-in functions](builtins.md) | Arguments, return values, and when await/try are needed |
| Build a small site and API | [HTTP and HTML](http.md) | A working server, JSON requests and responses, and HTML |
| Read a working application | [Task management demo](web-demo.md) | Browser UI, CRUD API, SQLite, and executable distribution |
| Handle API failures | [Result API example](result-api.md) | Matching, 400/404/500 responses, and fallback data |

## Read when you need it

| Topic | Page |
|---|---|
| Type annotations, numbers, and nullable values | [Types](types.md) |
| Define structured data | [Classes](classes.md) |
| Why some values cannot be reused after passing them | [Ownership](ownership.md), [views and copying](view-and-zero-copy.md) |
| Return failures and use try/match | [Error handling](error-handling.md) |
| Multiple files and Rust libraries | [Imports and Rust](modules-and-rust.md) |
| Configure an entry file, Rust dependencies, and Low | [Projects and nagi.toml](projects.md) |
| Read and write JSON | [JSON](json.md) |
| Store data | [SQLite](database.md) |
| Async operations and waiting for child tasks | [Async and scopes](async.md), [concurrency](concurrency.md) |
| Read generated Low and replace functions | [Low](low-language.md) |

## Design and implementation

These pages are intended for reading after learning the basic language.

- [Goals and scope](introduction.md), [development roadmap](roadmap.md)
- [Memory model](memory-model.md), [compiler internals](compiler-internals.md), [FFI](ffi.md)
- [Actors](actor.md), [supervisors](supervisor.md), and [queues](queue.md): currently experimental built-in functions
- [Reading benchmarks](performance.md), [measurements](measurements.md)

## Reading the examples

Code blocks contain Nagi code. Its indentation resembles Python, but the `python` command cannot run it. Use `nagic run file.nagi`.

Complete introductory examples are in [examples/tutorial/](../../examples/tutorial/). Some short examples in the syntax and function references are fragments to place inside functions. Examples marked as complete programs can be saved in a file and run directly.

These Docs describe the current 0.1 implementation. Each page identifies features that are not implemented. File imports such as `import "file.nagi"` and Result matching work. `import package as alias`, class methods, and nullable matching are not supported.
