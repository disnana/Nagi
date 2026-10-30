# What to build with Nagi

Nagi 0.2.3 is a language in development for small backends whose inputs, outputs, and failures are explicit in types, using HTTP, JSON, SQLite, and Rust assets. Choose a use case and run a complete example before reading individual API references.

The published default target is Rust. The planned 0.3.0 Go target is under implementation and is not included in the public release. Go HTTP and Actor support are specifically unavailable. Entries under `Unreleased` in the [changelog](../../CHANGELOG.md) are not installed features.

## Run a small first program

Version 0.2.3 can display a prompt before reading input, while keeping I/O failure explicit as a `Result`.

```nagi
def main() -> Result[unit, Error]:
    name = try read_line("Name > ")
    return ok(print(name))
```

Run it as a native program with `nagic run hello.nagi --profile dev`. The [Result and try lesson](../../test-nagi-code/result-try-tutorial/README.en.md) explains success extraction, failure propagation, and local recovery; the [first CLI app](first-app.md) is a complete input example.

## Find a capability

| Capability | What it builds or does | Rust target 0.2.3 | Go target | Runnable example | Detailed guide |
|---|---|---|---|---|---|
| Types, classes, `Result`, `try`, ownership/views | Validated CLI input, aggregation, typed failure propagation | Released | Unpublished partial implementation | [Stock report CLI](../../test-nagi-code/application-examples/stock-report/README.en.md) | [Language guide](language-guide.md), [error handling](error-handling.md) |
| HTTP server, JSON, and input limits | JSON APIs with explicit status/errors and bounded requests | Released | Unsupported | [Task API](task-api-tutorial.md), [complete source](../../test-nagi-code/web-demo/tasks.nagi) | [HTTP](http.md), [JSON](json.md) |
| SQLite, typed Parameters, Pool/Tx | CRUD, nullable values, persistence across restarts | Released | Unsupported | [Device settings API](../../test-nagi-code/application-examples/device-settings/README.en.md) | [SQLite](database.md), [Pool/Tx](sqlite-pool.md) |
| Async, scopes, and Task handles | Waiting, child joins, typed results, and cancellation | Released | Unpublished experimental stage | [Task results](../../test-nagi-code/library-examples/task-results/README.en.md) | [Async](async.md), [Tasks](task-handles.md) |
| Actors and Supervisors | Stateful workers, one-for-one restart, and shutdown | Released | Unsupported | [Supervised service](../../test-nagi-code/library-examples/supervised-service/README.en.md) | [Actors](actor.md), [Supervisors](supervisor.md) |
| Rust crate adapters | Explicit boundaries to Axum and existing crates | Released | Rust assets are not a Go target input | [Axum quote API](../../test-nagi-code/application-examples/axum-service/README.en.md) | [Rust integration](modules-and-rust.md) |
| Dev builds, incremental mode, runtime/lock selection | Select edit-loop cost and dependency resolution explicitly | Released (runtime/seed are opt-in) | Unpublished | [Development builds](development-builds.md) | [Runtime selection](runtime-profiles.md), [lock seed](lock-seeds.md) |

## Choose by code and observed output

These excerpts come from complete samples that target published Nagi 0.2.3. Use the button at the top right of each code block to copy it. Each link provides the complete source and execution conditions. The examples are not yet available on the Go target.

### HTTP, JSON, and SQLite

The [complete task API](../../test-nagi-code/web-demo/tasks.nagi) decodes JSON into a type, validates it, and then writes to SQLite.

The following block is a handler excerpt. Run it as the linked project, which supplies `State`, `SaveTask`, and the storage functions; it is not a standalone file.

```nagi
async def create_task(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    input = try json_decode[SaveTask](request.body)
    try validate_task(view(input.title))
    done: i32 = 0
    if input.done:
        done = 1
    task = try await storage_create_task(view(state.db), input.title, done)
    return http.json[Task](http.response_context(view(request)), http.Status.OK, task)
```

Start the server from the repository root and run the real HTTP checks in another terminal.

```sh
nagic run --project test-nagi-code/web-demo --profile dev
python test-nagi-code/web-demo/smoke_api.py --base-url http://127.0.0.1:8091
```

The expected final line is `PASS: 21 HTTP checks`. This sample does not include authentication or a browser UI.

### Async, scopes, and Task results

The [complete Task result sample](../../test-nagi-code/library-examples/task-results/main.nagi) moves a handle, awaits it once, and distinguishes Task failure from the inner business `Result`.

The following block is an excerpt from the scope. Run it as the linked project, which supplies the imports and `answer` function; it is not a standalone file.

```nagi
async with scope:
    original = spawn answer()
    transferred = move(original)
    received = await transferred
    match received:
        case Ok(value):
            print(value)
        case Err(failure):
            print(message(failure))
```

```sh
nagic run --project test-nagi-code/library-examples/task-results
```

It prints `42`, `business sentinel`, `sibling finished`, `after scope`, and `task-results: OK`. The order of the two concurrent messages is not fixed.

### Actors and Supervisors

The [complete Supervisor sample](../../test-nagi-code/library-examples/supervised-service/main.nagi) registers an Actor with explicit state creation and message handling functions.

The following block is the registration excerpt. Run it as the linked project, which supplies the types, factory, and handler; it is not a standalone file.

```nagi
group = actor.supervisor[Context](Context(initial=0), actor.default_options())
counter = try actor.register[CounterState, Message, i64, CounterError](
    view(group), "counter", create_counter, update_counter, actor.default_actor_options()
)
```

```sh
nagic run --project test-nagi-code/library-examples/supervised-service
curl http://127.0.0.1:8090/counter
curl -H 'Content-Type: application/json' -d '5' http://127.0.0.1:8090/counter
```

Start the server with the first command and run `curl` in another terminal. The first response is `0`; the response after adding is `5`. The sample uses Tokio tasks in one process and does not promise BEAM process isolation.

### Rust crate adapter

The [local Rust library sample](../../test-nagi-code/rust-library/library-pricing.nagi) declares the typed boundary with `extern def` and keeps the implementation in `native.rs`.

The following block is a shortened explanatory excerpt. Run it as the linked project, which supplies `Quote`, the remaining assertions, and `native.rs`; it is not a standalone file.

```nagi
@rust("native::quote")
extern def quote(product: view[str], unit_price_cents: i64, quantity: i64) -> Result[Quote, Error]

def main() -> Result[unit, Error]:
    product = "Workshop notebook"
    order = try quote(view(product), 250, 12)
    return ok(print(order.total_cents))
```

```sh
nagic check --project test-nagi-code/rust-library
nagic run --project test-nagi-code/rust-library
```

The complete sample prints total `2700` and `Local Rust library verified.`. `check` does not execute the Rust implementation, so the adapter needs `run` for final verification.

### Development and release builds

```sh
nagic check examples/tutorial/prompt-input.nagi --no-project
nagic run examples/tutorial/prompt-input.nagi --no-project --profile dev
nagic build examples/tutorial/prompt-input.nagi --no-project --profile release
```

`check` avoids Rust compilation, `dev` is for the edit loop, and `release` is for distribution. Every profile keeps Nagi type, ownership, and Policy checks.

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
- Nagi 0.2.3 lacks standard APIs for PostgreSQL, WebSocket, HTTP/2, streaming, and server-side TLS.
- Use Rust and Axum directly when unrestricted Rust traits, generics, and async ecosystem access are requirements.
- Account for Nagi's current development stage when established large-scale operations, broad middleware, or long-term compatibility are adoption requirements.

The [concrete task API comparison](task-api-comparison.md) implements the same service in Nagi, FastAPI, and Quart. It compares code, dependencies, validation, operation, and current limits rather than presenting a speed ranking.
