# Sample projects

Try command-line applications, reusable libraries, Rust integration, HTTP, Supervisors, and Low replacements. Each project includes `nagi.toml` and instructions. Standard HTTP and `std.actor` examples need the latest unreleased source.

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
| [Rust bridge](../../test-nagi-code/rust-bridge/) | CRC-32, serde_json, and async Rust functions |
| [Fractal](../../test-nagi-code/README.md#exe単体で見られるフラクタル) | Console output and distribution as an executable |

Each README explains its inputs, outputs, and limits. Passing these examples does not establish support for every Rust crate or a complete production HTTP/database foundation.

## Verify the examples during development

With Python 3 and a `nagic` built from the latest source, verify the projects' checks, native builds, output, and HTTP responses:

```sh
python scripts/verify_library_examples.py --compiler /path/to/nagic
```

The script compares both pricing implementations, checks module and type aliases, custom errors, malformed JSON, and async errors, compares High/Low results, and verifies HTTP 400/404 responses, the body limit, and shutdown. The Supervisor sample also checks updates, state retained after business errors, a 204 shutdown response, and 503 after shutdown. It builds projects sequentially and shares the dependency cache.

On Unix it checks a clean exit after SIGINT. On Windows it terminates the process and checks that the listener closes; check console Ctrl+C manually using the HTTP sample's instructions.
