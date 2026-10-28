# What to build with Nagi

Nagi 0.2.2 is a language in development for small backends whose inputs, outputs, and failures are explicit in types, using HTTP, JSON, SQLite, and Rust assets. Choose a use case and run a complete example before reading individual API references.

The published default target is Rust. The planned 0.3.0 Go target is under implementation and is not included in the public release. Go HTTP and Actor support are specifically unavailable. Entries under `Unreleased` in the [changelog](../../CHANGELOG.md) are not installed features.

## Run a small first program

Version 0.2.2 can display a prompt before reading input, while keeping I/O failure explicit as a `Result`.

```nagi
def main() -> Result[unit, Error]:
    name = try read_line("Name > ")
    return ok(print(name))
```

Run it as a native program with `nagic run hello.nagi --profile dev`. The [Result and try lesson](../../test-nagi-code/result-try-tutorial/README.en.md) explains success extraction, failure propagation, and local recovery; the [first CLI app](first-app.md) is a complete input example.

## Find a capability

| Capability | What it builds or does | Rust target 0.2.2 | Go target | Runnable example | Detailed guide |
|---|---|---|---|---|---|
| Types, classes, `Result`, `try`, ownership/views | Validated CLI input, aggregation, typed failure propagation | Released | Unpublished partial implementation | [Stock report CLI](../../test-nagi-code/application-examples/stock-report/README.en.md) | [Language guide](language-guide.md), [error handling](error-handling.md) |
| HTTP server, JSON, and input limits | JSON APIs with explicit status/errors and bounded requests | Released | Unsupported | [Task API](task-api-tutorial.md), [complete source](../../test-nagi-code/web-demo/tasks.nagi) | [HTTP](http.md), [JSON](json.md) |
| SQLite, typed Parameters, Pool/Tx | CRUD, nullable values, persistence across restarts | Released | Unsupported | [Device settings API](../../test-nagi-code/application-examples/device-settings/README.en.md) | [SQLite](database.md), [Pool/Tx](sqlite-pool.md) |
| Async, scopes, and Task handles | Waiting, child joins, typed results, and cancellation | Released | Unpublished experimental stage | [Task results](../../test-nagi-code/library-examples/task-results/README.en.md) | [Async](async.md), [Tasks](task-handles.md) |
| Actors and Supervisors | Stateful workers, one-for-one restart, and shutdown | Released | Unsupported | [Supervised service](../../test-nagi-code/library-examples/supervised-service/README.en.md) | [Actors](actor.md), [Supervisors](supervisor.md) |
| Rust crate adapters | Explicit boundaries to Axum and existing crates | Released | Rust assets are not a Go target input | [Axum quote API](../../test-nagi-code/application-examples/axum-service/README.en.md) | [Rust integration](modules-and-rust.md) |
| Dev builds, incremental mode, runtime/lock selection | Select edit-loop cost and dependency resolution explicitly | Released (runtime/seed are opt-in) | Unpublished | [Development builds](development-builds.md) | [Runtime selection](runtime-profiles.md), [lock seed](lock-seeds.md) |

## Use cases available now

| What you want to build | Start here | What it demonstrates |
|---|---|---|
| A CLI that reads JSON | [Stock report CLI](../../test-nagi-code/application-examples/stock-report/README.en.md) | Typed JSON, validation, custom Errors, aggregation |
| A SQLite-backed JSON API | [End-to-end task API tutorial](task-api-tutorial.md) | HTTP routes, JSON, validation, CRUD, restart persistence, tests |
| A settings API | [Device settings API](../../test-nagi-code/application-examples/device-settings/README.en.md) | NULL, bool, float, BLOB, SQLite Pool/Tx |
| An API using Rust web infrastructure | [Axum quote API](../../test-nagi-code/application-examples/axum-service/README.en.md) | Calling typed Nagi async logic from an Axum HTTP layer |
| A stateful worker | [Reservation workers](../../test-nagi-code/application-examples/seat-reservations/README.en.md) | Business errors, actor state, supervised restart |
| An application using a Rust crate | [Local Rust library](../../test-nagi-code/rust-library/README.en.md) | Path dependencies, features, and a handwritten adapter |

## Conditions that make Nagi worth evaluating

- You want Python-style indentation with explicit function types, `Result`, and move/view rules.
- You want `nagic check` for everyday feedback and a native executable for distribution.
- The application fits HTTP, JSON, and SQLite, or missing integration can live behind a small Rust adapter.
- Reviewable type, ownership, and capability boundaries matter to the project.

## Conditions that favor another choice today

- FastAPI or Quart is a more direct choice when the application depends on Python packages or data tooling.
- Nagi 0.2.2 lacks standard APIs for PostgreSQL, WebSocket, HTTP/2, streaming, and server-side TLS.
- Use Rust and Axum directly when unrestricted Rust traits, generics, and async ecosystem access are requirements.
- Account for Nagi's current development stage when established large-scale operations, broad middleware, or long-term compatibility are adoption requirements.

The [concrete task API comparison](task-api-comparison.md) implements the same service in Nagi, FastAPI, and Quart. It compares code, dependencies, validation, operation, and current limits rather than presenting a speed ranking.
