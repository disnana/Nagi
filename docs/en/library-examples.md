# Sample projects

Try command-line applications, reusable libraries, Rust integration, HTTP, Supervisors, and Low replacements. Each project includes `nagi.toml` and instructions. Standard HTTP and `std.actor` examples require Nagi 0.1.8 or later.

## Examples for libraries and foundations

| Project | What it demonstrates |
| --- | --- |
| [Pricing CLI](../../test-nagi-code/library-examples/foundation-cli/README.en.md) | Validate input and call a shared calculation interface; switch between Nagi and Rust implementations |
| [JSON report](../../test-nagi-code/library-examples/foundation-report/README.en.md) | Reuse the CLI's library and aggregate valid rows while handling invalid ones |
| [Read JSON with Rust](../../test-nagi-code/library-examples/rust-json/README.en.md) | Use serde_json and convert results to a Nagi class and Result |
| [Rust async operation](../../test-nagi-code/library-examples/rust-async/README.en.md) | Await a Tokio timer from Nagi |
| [Custom HTTP foundation](../../test-nagi-code/library-examples/custom-http/README.en.md) | Pass a Nagi function to Axum/Tokio and serve HTTP without opening a database |
| [Standard HTTP and authentication](../../test-nagi-code/library-examples/http-auth/README.en.md) | Handle headers, 401, route-specific errors, and typed shared state in Nagi |
| [Supervisor and HTTP](../../test-nagi-code/library-examples/supervised-service/README.en.md) | Update actor state in order; map business errors and shutdown to HTTP responses |
| [Low calculation kernel](../../test-nagi-code/library-examples/low-kernel/README.en.md) | Replace a High implementation with Low while keeping the application's calls |
| [Modules and aliases](../../test-nagi-code/library-examples/module-imports/README.en.md) | Distinguish same-named classes and use one type through a module name and a from alias |
| [CLI with custom errors](../../test-nagi-code/library-examples/typed-errors/README.en.md) | Distinguish failures with an enum, preserve their causes, and choose display messages |

See [libraries and Rust assets](libraries.md) for the structure. Both pricing applications need `shared/`; obtain the [whole sample directory](../../test-nagi-code/library-examples/).

## Run a project

You need Rust/Cargo, Nagi, and a toolchain that can build applications on your OS. See [setup and first run](getting-started.md).

From the repository root:

```sh
nagic check --project test-nagi-code/library-examples/rust-json
nagic run --project test-nagi-code/library-examples/rust-json
```

Inside a project's directory, `nagic check` and `nagic run` use the same configuration. The pricing CLI waits for input; the HTTP foundation runs until Ctrl+C.

Selecting just a source file skips its neighboring `nagi.toml`. Use `--project` or a command without a source filename inside the directory to include its Rust, Low, and dependency settings.

## Read complete applications

| Project | Contents |
| --- | --- |
| [Task management](web-demo.md) | Browser UI, JSON API, and SQLite persistence |
| [Result API](result-api.md) | Invalid input, missing data, DB failures, and recovery |
| [Inventory](../../test-nagi-code/README.md#在庫管理api) | Typed JSON input, CRUD, and aggregation |
| [Stock JSON report](../../test-nagi-code/application-examples/stock-report/README.en.md) | Read typed JSON from stdin and validate it with custom errors |
| [Device settings API](../../test-nagi-code/application-examples/device-settings/README.en.md) | Read SQLite NULL, boolean, float and BLOB columns; retain settings across restarts |
| [Reservation worker](../../test-nagi-code/application-examples/seat-reservations/README.en.md) | Handle duplicate bookings and capacity errors; restart one actor independently |
| [Rust bridge](../../test-nagi-code/rust-bridge/) | CRC-32, serde_json, and async Rust functions |
| [Fractal](../../test-nagi-code/README.md#exe単体で見られるフラクタル) | Console output and distribution as an executable |

Each README explains its inputs, outputs, and limits. Rust integration examples cover the APIs and types listed there.

## Verify the examples during development

With Python 3.12 or later, verify checks, native builds, output, and HTTP responses. Use a `nagic` built from the same source revision as the repository examples:

```sh
python scripts/verify_library_examples.py --compiler /path/to/nagic
python scripts/verify_application_examples.py --compiler /path/to/nagic
```

The script compares both pricing implementations, checks module and type aliases, custom errors, malformed JSON, and async errors, compares High/Low results, and verifies HTTP 400/404 responses, the body limit, and shutdown. The Supervisor sample also checks updates, state retained after business errors, a 204 shutdown response, and 503 after shutdown. It builds projects sequentially and shares the dependency cache.

On Unix it checks a clean exit after SIGINT. On Windows it terminates the process and checks that the listener closes; check console Ctrl+C manually using the HTTP sample's instructions.

`verify_application_examples.py` checks [seven apps](../../test-nagi-code/application-examples/README.en.md). It independently checks and builds six High apps from the original source and saved generated Low, and checks the handwritten Low order quote CLI directly. Its 13 runs cover input boundaries, existing-file protection, SQLite persistence, HTTP headers and errors, and Supervisor restarts, readiness and shutdown. Build warnings fail verification. Results and logs are written to `build/application-example-verification/`. CI runs it through `build_examples.py`.
