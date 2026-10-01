# Nagi 0.1.3 — a working prototype of a two-level backend language

[日本語](README.md)

Nagi compiles readable High code into editable Low code, then into a native executable. The path from an HTTP request body to a typed class, SQLite, and a JSON response works today. See the [measurements](docs/en/measurements.md) and their raw logs for performance evidence.

This is a prototype, not a finished language specification. The compiler and runtime let us test ownership, views, replacement of generated code, and execution costs.

The name comes from the Japanese word *nagi* (凪), meaning calm seas. It reflects the idea that the surface can stay calm even while the internals are busy.

## Learn the language

Start with **[Docs](docs/en/README.md) → [Setup and first run](docs/en/getting-started.md) → [Learn by writing code](docs/en/language-guide.md)**. Use the [syntax reference](docs/en/syntax.md) to look up notation, the [built-in functions](docs/en/builtins.md) to check arguments, and [HTTP and HTML](docs/en/http.md) to build a site or API. Complete introductory programs are in [examples/tutorial/](examples/tutorial/).

The introduction and all Docs are available on the [English website](https://disnana.github.io/Nagi/en/) and the [Japanese website](https://disnana.github.io/Nagi/). See the [site source and Pages deployment guide](website/README.md) (Japanese).

Downloads are published on [GitHub Releases](https://github.com/disnana/Nagi/releases). Increasing the Nagi or VS Code extension version on `main` publishes that component after CI succeeds. The [release guide](scripts/releases/README.md) (Japanese) describes the conditions and artifacts.

## Run an example

You need Rust/Cargo and a C compiler to build the bundled SQLite code. [GitHub Releases](https://github.com/disnana/Nagi/releases) provides compiler archives for Windows x64, Linux x86_64, macOS Apple Silicon, and macOS Intel. CI builds and runs them on each platform. See [setup](docs/en/getting-started.md) for MSVC, PowerShell, and macOS instructions. WSL2 follows the Linux commands below.

If you use a compiler archive, skip the first command. Rust/Cargo and the C build environment are still needed to compile Nagi applications. Extract the whole archive so that the compiler can find `runtime/`.

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
| `tests/` | HTTP integration tests |
| `fuzz/` | Parser/JSON mutation checks with fixed seeds |
| `benchmarks/` | Comparison implementations, wrk scripts, and raw logs |
| `docs/` | Language documentation, design rationale, implementation scope, and development plans |
| `scripts/` | Scripts to reproduce builds, HTTP checks, load tests, and reports |

High and Low take separate paths within the same compiler crate. Splitting them into independent crates and organizing the standard library into modules are future work.

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

Implemented features include primitive types, value classes, contiguous arrays, nullable values, Result, functions, branches, loops, async/await, scopes, basic HTTP, HTML responses, JSON, SQLite, handwritten Low calls and replacements, relative file imports, and typed Rust function calls. Actors, supervisors, and queues have runtime implementations and experimental High built-ins. See [imports and Rust](docs/en/modules-and-rust.md) and the [web demo](docs/en/web-demo.md).

Result matching lets you handle success and failure, recover with defaults, and return errors while preserving their kind. See [error handling](docs/en/error-handling.md) and the [HTTP Result API example](docs/en/result-api.md).

The [VS Code extension 0.1.8](docs/en/vscode-extension.md) provides F12 navigation to functions, classes, imported files, and local bindings; type hovers; class field completion; and signature help. It also handles unsaved edits to files saved at least once. The [editor walkthrough](docs/en/editor.md) shows how to use these features.

Dedicated actor declarations, generic functions, traits, named modules and aliases, nullable and general pattern matching, PostgreSQL, compile-time SQL validation, High request arenas, Low raw pointers/unsafe/C ABI, a custom scheduler, and self-hosting are not implemented. Rust integration uses calls within the same build, without a stable external ABI. `Map` and `owned` are design proposals, not complete standard APIs.

CPU performance comes from typed native operations, avoiding boxing, and LLVM loop optimizations. The runtime uses Tokio, Axum, Serde, and rusqlite; those libraries handle the runtime work covered by the benchmarks.

Read the [introduction](docs/en/introduction.md), [roadmap](docs/en/roadmap.md), and [performance results](docs/en/measurements.md) for more detail.

## Contributing and license

Bug reports, code fixes, documentation, and translations are welcome. See the [contribution guide](CONTRIBUTING.en.md) for the workflow and AI use policy. Nagi is available under the [MIT license](LICENSE).
