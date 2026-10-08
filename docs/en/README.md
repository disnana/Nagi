# Nagi Docs

Follow [installation](getting-started.md) with [your first CLI app](first-app.md). Use the [language feature index](#language-feature-index) for a path through the language, or open the [syntax reference](syntax.md) and [built-in functions](builtins.md) for lookup.

These Docs target Nagi 0.1.11, officially released on 2026-10-07; the VS Code extension 0.1.13 and JetBrains plugin 0.1.1 are separate components. The official site is normally built from main and may include changes not yet published. See the [JetBrains installation guide](../../editors/jetbrains-nagi/README.en.md) to select an IDEA or PyCharm ZIP. See the [Changelog](../../CHANGELOG.md) and the [0.1.11 migration guide](migration-0.1.11.md).

See [About Nagi](introduction.md) for its purpose and current scope. The references below describe implemented APIs and their limits. Unimplemented proposals are kept in [design](library-design.md) and the [roadmap](roadmap.md).

## First steps

1. [Setup and first run](getting-started.md): install the compiler and run Hello World.
2. [Learn by writing code](language-guide.md): try values, functions, lists, borrowing, and Results.
3. [Your first small CLI app](first-app.md): connect input, typed failures, and a manual boundary check.
4. [HTTP and HTML](http.md): build an API, then store data with [SQLite](database.md).

See the [VS Code guide](editor.md) for editor support. See [JetBrains installation and Release ZIP selection](../../editors/jetbrains-nagi/README.en.md) for IntelliJ IDEA and PyCharm.

## Language feature index

Run an example and check its expected result, then read the detail page for conditions and limits. The columns below point to each feature's purpose, runnable example and result, common mistakes and fixes, and alternatives. A code fragment is not always a complete program; use a section with `nagic run` instructions or the linked sample.

| Feature | Use and runnable example/result | Limits, common mistakes, and fixes | Choices and detail |
|---|---|---|---|
| Values, annotations, assignment, and reassignment | [Guide §1](language-guide.md#1-values-and-types) updates a number and prints `11` | A variable keeps its inferred type; reassignment cannot change it | [Types](types.md), [values and variables](syntax.md#values-and-variables) |
| Functions, parameters, return, and function values | [Guide §2](language-guide.md#2-define-functions) returns `42` | Annotate parameter/return types; call executable code from `main` | [function syntax](syntax.md#functions-and-return), [function types](types.md#pass-a-function-as-a-value) |
| if, match, for, while, and operators | [Guide §3](language-guide.md#3-lists-classes-branches-and-loops) prints `OK` and loop results | Conditions require bool. `break`, `continue`, and `elif` are unsupported | [branches, loops, operators](syntax.md#branches-and-loops), [match](error-handling.md#separate-success-and-failure) |
| Lists, classes, fields, and enums | [Guide §3](language-guide.md#3-lists-classes-branches-and-loops) and the [class example](classes.md) print a sum and field values | Lists have one element type. Classes use named fields; methods, inheritance, and field reassignment are unsupported | [types and enums](types.md#distinguish-variants-with-an-enum), [classes](classes.md) |
| Nullable `T?` / `Option[T]` | [Absent-value example](error-handling.md#handle-an-absent-value) handles `Some` and `None` | Match both cases. There is no `unwrap` or Python-style `if value is not None` narrowing | [types](types.md), [match syntax](syntax.md#result-async-and-scopes) |
| `Result[T, E]`, `try`, `match`, and custom Errors | [First CLI app](first-app.md) propagates bad input; [Guide §5](language-guide.md#5-return-failures-with-result) shows recovery | `try` returns Err to the caller. Use `match` with both Ok and Err to recover locally | [failure types and examples](error-handling.md), [built-ins](builtins.md#conversion-success-and-failure) |
| Ownership, explicit move for assignment, and copy | [Guide §4](language-guide.md#4-borrow-with-view-when-you-only-need-to-read) shows values after a call | Use `move(...)` for ordinary assignment of an owned non-Copy local; the source is unavailable afterward | [assignment, move, and mistakes](ownership.md#assignment-and-explicit-move), [migration](migration-0.1.11.md) |
| Borrowing and `view` | The [view example](view-and-zero-copy.md) reads a string range and compares an owned copy | A view cannot outlive its owner. Do not save a view of a temporary value | [borrow checks](ownership.md#borrowing-and-the-limits-of-checking), [views](view-and-zero-copy.md) |
| `shared`, `owned`, and Copy | `share`/`clone_shared` examples are in [built-ins](builtins.md#sharing-and-type-sizes) | `shared` does not make every type thread-safe. `owned[T]` is unfinished | [types](types.md#unsupported-type-operations), [memory](memory-model.md) |
| Files, imports, and module aliases | [Guide §6](language-guide.md#6-split-code-into-files) calls a function from a second file | Relative files and registered standard modules are supported; cycles and general package discovery are not | [modules and Rust](modules-and-rust.md) |
| `async` functions and `await` | The [async example](async.md#use-a-result-after-waiting) prints after waiting | `await` does not create a task or thread. Storing a Future in a variable is unsupported | [async](async.md), [CPU concurrency](concurrency.md) |
| `scope`, `spawn`, and Task result handles | The [scope example](async.md#let-other-work-proceed-while-waiting) waits for its children | Normal exits follow child completion rules. A Task cannot escape its scope and is received once | [Task results, errors, faults](task-handles.md), [scopes](async.md) |
| Actors and Supervisors | The [supervised service](../../test-nagi-code/library-examples/supervised-service/README.en.md) registers, calls, and stops workers | APIs run within one process. Check restart, business error, call failure, and shutdown conditions | [actors](actor.md), [Supervisors](supervisor.md), [API types](actor-reference.md) |
| High and Low | [Run High and Low](low-language.md#write-and-run-a-standalone-low-program) | Low follows the same type and ownership rules; it is not a pointer/unsafe/C ABI layer | [High and Low](low-language.md), [Rust integration](modules-and-rust.md) |
| Built-in functions | Find signatures in the [built-in reference](builtins.md); common examples are linked by section and in [runnable samples](library-examples.md) | Accepted types, borrowing/consumption, sync/async behavior, and runtime conditions vary by function | [all built-ins](builtins.md), [HTTP](http.md), [SQLite](database.md) |

Each detail page describes current behavior and release scope, then provides or links to usage, code and output, conditions/limits, common mistakes and fixes, and the choice between related features. For tasks involving several features, also browse the [sample projects](library-examples.md).

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
| Use the existing SQLite API and preflight SQL | [SQLite](database.md), [SQL checks](sql-check.md) |
| Use typed parameters, a Pool, and transactions | [SQLite Pools and Transactions (unreleased development source)](sqlite-pool.md) |
| Split code into files or call Rust | [Imports and Rust](modules-and-rust.md) |
| Share your own libraries or use Rust crates | [Libraries and Rust assets](libraries.md) |
| Save an entry file and build settings | [nagi.toml](projects.md) |
| Use your own Rust crate from an application | [Local library example](../../test-nagi-code/rust-library/README.en.md) |
| Wait for async work or start child operations | [Async and scopes](async.md), [concurrency](concurrency.md) |
| Send messages to stateful tasks and manage restart and shutdown | [Actors](actor.md), [Supervisors](supervisor.md), [API reference](actor-reference.md) |
| Read working applications | [Sample projects](library-examples.md) |
| Diagram types, modules, and calls | [Code maps](code-map.md) |

Runnable examples are in [examples/tutorial/](../../examples/tutorial/). References also include code fragments.

Tutorial code uses the current syntax. Explicit move and Task result handles are published in Nagi 0.1.11; confirm that the installed compiler is version 0.1.11 or later with `nagic --version`. See the [migration guide](migration-0.1.11.md) and [Task result handles](task-handles.md). Shared actor messages remain future design work.

The new [SQLite Pool/Tx API](sqlite-pool.md) is available in the development source only; it is not included in 0.1.11.

## Implementation and development

[About Nagi](introduction.md) · [Design decisions](../../DESIGN.en.md) · [Low](low-language.md) · [Memory](memory-model.md) · [Compiler](compiler-internals.md) · [Language interfaces](ffi.md) · [Roadmap](roadmap.md)

Follow the [first contribution walkthrough](contributing.md) to locate a source file, make a small change, run the relevant checks, and open a PR.

`std.actor` is a standard library available from Nagi 0.1.8. The [sample](../../test-nagi-code/library-examples/supervised-service/README.en.md) covers registration, calls, and shutdown. The older actor/Supervisor built-ins and [queues](queue.md) remain test APIs. For performance, read the [measurement methods](performance.md), [results](measurements.md), and [HTTP load tests](http-capacity.md).

These pages cover the current source and Nagi 0.1. See the [change log](../../CHANGELOG.md) for changes by version. Unsupported features are listed on each page.

- [Authentication/authorization (unreleased SF01)](security.md) and [0.2.0 migration](migration-0.2.0.md)
