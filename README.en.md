# Nagi 0.1.9 — a working prototype of a two-level backend language

[日本語](README.md)

Nagi compiles readable High code into editable Low code, then into a native executable. The path from an HTTP request body to a typed class, SQLite, and a JSON response works today. See the [measurements](docs/en/measurements.md) and their raw logs for performance evidence.

This is a prototype, not a finished language specification. The compiler and runtime let us test ownership, views, replacement of generated code, and execution costs.

The name comes from the Japanese word *nagi* (凪), meaning calm seas. It reflects the idea that the surface can stay calm even while the internals are busy.

## Learn the language

Start with **[Docs](docs/en/README.md) → [Setup and first run](docs/en/getting-started.md) → [Learn by writing code](docs/en/language-guide.md)**. Use the [syntax reference](docs/en/syntax.md) to look up notation, the [built-in functions](docs/en/builtins.md) to check arguments, and [HTTP and HTML](docs/en/http.md) to build a site or API. Complete introductory programs are in [examples/tutorial/](examples/tutorial/). [Seven application examples](test-nagi-code/application-examples/README.en.md) cover CLI tools, HTTP, SQLite, and Supervisors.

The introduction and all Docs are available on the [English website](https://nagi.disnana.com/en/) and the [Japanese website](https://nagi.disnana.com/). See the [site source and Pages deployment guide](website/README.md) (Japanese).

Downloads are published on [GitHub Releases](https://github.com/disnana/Nagi/releases). Increasing the Nagi or VS Code extension version on `main` publishes that component after CI succeeds. The [release guide](scripts/releases/README.md) (Japanese) describes the conditions and artifacts.

## Install

The main branch also includes changes planned for the next release. See [change log](CHANGELOG.md) for differences from the published version.

Building Nagi applications needs Rust/Cargo and a C build environment. See [setup](docs/en/getting-started.md) for prerequisites.

Windows (PowerShell):

```powershell
& ([scriptblock]::Create((Invoke-RestMethod 'https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.ps1')))
```

Linux / macOS (bash):

```bash
(set -o pipefail; curl -fsSL https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.sh | bash) && export PATH="$HOME/.local/bin:$PATH"
```

These commands download the latest published Nagi release, verify SHA-256, install it for your user, and add it to PATH. **Run the same command to update.** The installer comes from main; the compiler comes from a published GitHub Release. Install Rust and the VS Code extension separately.

Install the VS Code extension from the [Marketplace](https://marketplace.visualstudio.com/items?itemName=Disnana.nagi-lang). See the [extension guide](docs/en/vscode-extension.md) for settings and usage.

An initial plugin for IntelliJ IDEA and PyCharm provides High/Low highlighting, indentation assistance, checking and execution. See [building and installing it](editors/jetbrains-nagi/README.en.md).

Stop any Nagi builds before updating and restart VS Code afterward. After a successful update, unchanged older distributions are removed so only the selected version remains. Modified or added files are preserved. The installer reports older copies it cannot verify or remove, including files locked by Windows. A failed update preserves the previous command. See [updating and selecting a version](docs/en/getting-started.md#update).

```bash
nagic --version
nagic --help
```

For manual installation, extract the complete OS archive from [GitHub Releases](https://github.com/disnana/Nagi/releases) and add the extracted folder to PATH. Keep `runtime/` beside the compiler; `NAGI_ROOT` is unnecessary.

## Run an example

The commands below build the compiler and samples from source. With an installed compiler, use `nagic run <file>`.

```bash
cargo build --release --locked
./target/release/nagic run examples/hello.nagi
./target/release/nagic run examples/values.nagi
./target/release/nagic run examples/crud.nagi --cost-report
```

Call the API from another terminal:

```bash
curl http://127.0.0.1:8080/health
curl -H 'Content-Type: application/json' -d '{"name":"alice","age":18}' http://127.0.0.1:8080/users
curl http://127.0.0.1:8080/users/1
```

```nagi
class User:
    id: i64
    name: str
    age: i32

@get("/users/{id}")
async def get_user(db: Db, id: i64) -> Result[User?, Error]:
    return await db_query[User](db, "SELECT id, name, age FROM users WHERE id = ?1", id)
```

This resembles Python, but it is Nagi code. Build it with `nagic`.

For applications with multiple files, [nagi.toml](docs/en/projects.md) stores the entry file, Rust dependencies, and handwritten Low files. For example, `./target/release/nagic run --project test-nagi-code/rust-bridge` runs the Rust integration sample. The [VS Code extension](docs/en/vscode-extension.md) uses the same settings to check, build, and run the entry file while you edit other files in the project.

## Map code structure

`nagic map` inspects types, modules, and function calls, and exports Mermaid, D2, JSON, or standalone HTML. SVG and PNG export uses D2 when it is available on PATH.

```bash
nagic map types --project examples/code-map --format d2
nagic map calls --project examples/code-map --format html --output calls.html
```

See [Code maps](docs/en/code-map.md) for filtering and rendering.

## High and Low

```bash
./target/release/nagic lower examples/override.nagi
./target/release/nagic run examples/override.nagi --native examples/native/override.low
./target/release/nagic run examples/low_call.nagi --native examples/native/math.low
./target/release/nagic run examples/hello.low
```

`build/<name>/generated.low` is readable Low source. Low is parsed and type-checked separately. To keep changes to generated code, move the function into a handwritten Low file and mark it with `@replace generated::function_name`. Regeneration leaves handwritten files intact.

## Repository layout

| Path | Contents |
|---|---|
| `compiler/` | Rust lexer, High/Low parsers, type/move/view checks, Low merging, Rust code generation, and CLI |
| `runtime/` | HTTP, JSON, a dedicated SQLite worker, scopes, actors, supervisors, queues, and metrics |
| `examples/` | High, Low, and handwritten replacement examples |
| `test-nagi-code/` | Task management, inventory API, Result recovery, and Rust integration samples |
| `editors/vscode-nagi/` | VS Code highlighting, diagnostics, definitions, type hovers, completion, signature help, and check/build/run commands |
| `editors/jetbrains-nagi/` | IntelliJ IDEA/PyCharm highlighting, indentation, folding, and check/run commands |
| `tests/` | HTTP integration tests |
| `fuzz/` | Parser/JSON mutation checks with fixed seeds |
| `benchmarks/` | Comparison implementations, wrk scripts, and raw logs |
| `docs/` | Language documentation, design rationale, implementation scope, and development plans |
| `scripts/` | Scripts to reproduce builds, HTTP checks, load tests, and reports |

High and Low take separate paths within the same compiler crate. `std.http.server` and `std.actor` are registered standard libraries. Splitting the compiler into independent crates remains future work.

[Library and Rust integration samples](docs/en/library-examples.md) demonstrate shared CLI/report logic, serde_json, Tokio, HTTP, Supervisors, and Low replacements. See [library structure](docs/en/libraries.md) and the [design proposal](docs/en/library-design.md).

## Reproduce the checks

```bash
cargo test --locked
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --release --examples --bins --locked
python3 scripts/build_examples.py
python3 -m pip install -r tests/requirements.txt
python3 tests/http_integration.py
./target/release/examples/fuzz-smoke
./native-target/release/nagi-cpu
./target/release/examples/microbench
./target/release/examples/concurrency_bench
python3 benchmarks/python_cpu.py
node benchmarks/node_cpu.js
python3 tests/connections.py
python3 scripts/http_bench.py --wrk /absolute/path/to/wrk --soak 120
python3 scripts/summarize_results.py
```

Servers bind to loopback. Keep ports 8080–8083 available for the tests. Set `NAGI_DB=users.sqlite` for persistent storage. `NAGI_THREADS` controls HTTP executor workers and defaults to 4. Comparison benchmarks pin each server to one logical CPU. See [HTTP load tests](docs/en/http-capacity.md) for connection limits, long runs, and recovery measurements.

## Current scope

Implemented features include primitive types, value classes, enums, contiguous arrays, nullable values, Results with custom errors, functions, branches, loops, async/await, scopes, basic HTTP, HTML responses, JSON, SQLite, handwritten Low calls and replacements, relative file imports, and typed Rust function calls. The older actor, Supervisor, and queue built-ins remain test APIs. See [imports and Rust](docs/en/modules-and-rust.md) and the [web demo](docs/en/web-demo.md).

Result matching lets you handle success and failure, recover with defaults, and return errors while preserving their kind. See [error handling](docs/en/error-handling.md) and the [HTTP Result API example](docs/en/result-api.md).

The [VS Code extension 0.1.12](docs/en/vscode-extension.md) provides F12 navigation to functions, classes, imported files, and local bindings; type hovers; class field completion; and signature help. It also handles unsaved edits to files saved at least once. The [editor walkthrough](docs/en/editor.md) shows how to use these features.

Available from Nagi 0.1.8, the [standard HTTP module](docs/en/http.md) supports database-free apps, headers, Method/Status, custom state, and error mapping. Standard-module imports and Option Some/None matching are also supported. See [change log](CHANGELOG.md) for differences from the published version.

Available from Nagi 0.1.8, [`std.actor`](docs/en/actor.md) uses ordinary async functions for arbitrary owned state, typed messages and replies, restart policies, observation, and shutdown. See its [API](docs/en/actor-reference.md) and [sample](test-nagi-code/library-examples/supervised-service/README.en.md). It runs natively within one process; a BEAM-style VM, hot code replacement, and distributed actors are unsupported.

Dedicated actor declarations, user-defined generic functions and traits, package imports, general pattern matching, PostgreSQL, compile-time SQL validation, High request arenas, Low raw pointers/unsafe/C ABI, a custom scheduler, and self-hosting are not implemented. Rust integration uses calls within the same build, without a stable external ABI. Operations for `Map` and `owned` are incomplete.

CPU performance comes from typed native operations, avoiding boxing, and LLVM loop optimizations. The runtime uses Tokio, Axum, Serde, and rusqlite; those libraries handle the runtime work covered by the benchmarks.

Read the [introduction](docs/en/introduction.md), [roadmap](docs/en/roadmap.md), and [performance results](docs/en/measurements.md) for more detail.

## Contributing and license

Bug reports, code fixes, documentation, and translations are welcome. See the [contribution guide](CONTRIBUTING.en.md) for the workflow and AI use policy, and the [security policy](SECURITY.en.md) to report vulnerabilities privately. Nagi is available under the [MIT license](LICENSE).
