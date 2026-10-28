# What to build with Nagi

Nagi 0.2.1 is a language in development for small backends whose inputs, outputs, and failures are explicit in types, using HTTP, JSON, SQLite, and Rust assets. Choose a use case and run a complete example before reading individual API references.

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
- Nagi 0.2.1 lacks standard APIs for PostgreSQL, WebSocket, HTTP/2, streaming, and server-side TLS.
- Use Rust and Axum directly when unrestricted Rust traits, generics, and async ecosystem access are requirements.
- Account for Nagi's current development stage when established large-scale operations, broad middleware, or long-term compatibility are adoption requirements.

The [concrete task API comparison](task-api-comparison.md) implements the same service in Nagi, FastAPI, and Quart. It compares code, dependencies, validation, operation, and current limits rather than presenting a speed ranking.
