# Nagi 0.1.11 — a backend language in development

[日本語](README.md)

Nagi is a programming language for writing typed logic in Python-like High code and combining it with Rust libraries or custom code. Backends are its main focus. Common HTTP, JSON, and database operations should have Nagi APIs; advanced integrations connect through Rust adapters.

Nagi currently generates Rust code, which Rust/Cargo compiles into native executables. The language specification and standard APIs are still in development. Types, ownership, failures, and concurrency are being tested through working applications; passing `check` does not guarantee that the Rust build will succeed.

The name comes from the Japanese word *nagi* (凪), meaning calm seas: the surface can remain calm while the internals are busy.

## Learn the language

Start with [setup](docs/en/getting-started.md), the [language guide](docs/en/language-guide.md), [your first CLI app](docs/en/first-app.md), and then [HTTP](docs/en/http.md). Use the [language feature index](docs/en/README.md#language-feature-index) and [reference index](docs/en/README.md) to look up features and APIs, or read the [sample projects](docs/en/library-examples.md).

The [official website](https://nagi.disnana.com/en/) includes the introduction and English Docs. See [purpose and current scope](docs/en/introduction.md), [design decisions and rationale](DESIGN.en.md), and the [development priorities](docs/en/roadmap.md).

AI coding docs and the development skill live in [ai/](ai/README.md), separately from the human guides in `docs/`.

## Install

Building Nagi applications requires Rust/Cargo and a C build environment. Follow the [platform setup instructions](docs/en/getting-started.md). Installing the compiler downloads a prebuilt distribution.

Windows (PowerShell):

```powershell
& ([scriptblock]::Create((Invoke-RestMethod 'https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.ps1')))
```

Linux / macOS (bash):

```bash
(set -o pipefail; curl -fsSL https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.sh | bash) && export PATH="$HOME/.local/bin:$PATH"
```

The installer downloads the latest [GitHub Release](https://github.com/disnana/Nagi/releases), checks SHA-256, and adds it to PATH. Run the same command to update. The [setup guide](docs/en/getting-started.md#update) explains version selection and cleanup. For manual installation, extract the whole archive and add the folder containing `runtime/` to PATH.

```sh
nagic --version
nagic --help
```

The [VS Code extension](https://marketplace.visualstudio.com/items?itemName=Disnana.nagi-lang) is available from the Marketplace or [GitHub Releases](https://github.com/disnana/Nagi/releases). [Nagi](editors/jetbrains-nagi/README.en.md) for IntelliJ IDEA and PyCharm is available from [JetBrains Marketplace](https://plugins.jetbrains.com/plugin/34891-nagi) or [GitHub Releases](https://github.com/disnana/Nagi/releases). For Marketplace installs, check the listing's Versions view for available versions and compatible IDE builds. The published GitHub 0.1.2 release includes one common `nagi-jetbrains-0.1.2.zip` and its `.sha256` for both IDEs. The minimum IDE builds for that ZIP are IDEA 2025.1.1 build 251.25410.109 and PyCharm 2025.1.1 build 251.25410.122. Marketplace installs directly to the compatible IDE; Release ZIPs are installed manually. Plugin 0.1.3 is an unreleased candidate. Published compiler 0.1.11 supports ordinary Check/Run, while candidate completion, diagnostics, and navigation require the matching compiler CI artifact built from the same PR source. See the [editor guide](docs/en/editor.md).

The installer comes from main; the compiler comes from a published Release. These examples target Nagi 0.1.11, officially released on 2026-10-07. Verify your installed compiler with `nagic --version`. See [CHANGELOG](CHANGELOG.md) for changes and the [0.1.11 migration guide](docs/en/migration-0.1.11.md) for compatibility notes.

### Uninstall

Windows (PowerShell):

```powershell
& ([scriptblock]::Create((Invoke-RestMethod 'https://raw.githubusercontent.com/disnana/Nagi/main/scripts/uninstall.ps1')))
```

Linux / macOS (bash):

```bash
(set -o pipefail; curl -fsSL https://raw.githubusercontent.com/disnana/Nagi/main/scripts/uninstall.sh | bash)
```

To preview removals, append `-WhatIf` to the PowerShell command or replace `bash` with `bash -s -- --dry-run`. For a custom installation, pass the same `-InstallDir` or `--prefix`/`--bin-dir` values used to install, and the same `--profile` if you chose a custom shell profile.

Modified or unverifiable distributions are retained. Projects, Rust/Cargo, and the VS Code extension remain unchanged. See the [setup guide](docs/en/getting-started.md#uninstall) for details.

## Run an example

Save this as `server.nagi`. It serves HTTP without a database.

```nagi
import std.http.server as http

class State:
    greeting: str

async def hello(request: http.Request, state: shared[State]) -> Result[http.Response, Error]:
    return ok(http.text(http.Status.OK, view(state.greeting)))

async def main() -> Result[unit, Error]:
    app = http.app_default[State](State(greeting="Hello, Nagi!"))
    app = try http.route(app, http.Method.GET, "/", hello)
    return await http.serve(app, 8080, http.default_options())
```

```sh
nagic run server.nagi
```

Open [http://127.0.0.1:8080/](http://127.0.0.1:8080/) to receive `Hello, Nagi!`. Stop it with Ctrl+C. To build the compiler from source, run `cargo build --release --locked`, then use `./target/release/nagic`.

See [HTTP](docs/en/http.md) and the [authentication example](test-nagi-code/library-examples/http-auth/README.en.md) for headers, JSON, custom errors, and shared state.

HTTP infrastructure can also live in Rust. The [Axum quote API](test-nagi-code/application-examples/axum-service/README.en.md) uses Axum for routing and JSON extraction, then calls a Nagi async function for typed validation and pricing. See the [design direction](DESIGN.en.md#make-rust-integration-a-central-goal) for responsibilities and limits.

## Map code structure

`nagic map` inspects types, modules, and function calls, and exports Mermaid, D2, JSON, or standalone HTML.

```sh
nagic map calls --project examples/code-map --format html --output calls.html
```

SVG and PNG export requires D2. See [code maps](docs/en/code-map.md) for filters, relationships that cannot be inferred, and rendering.

## High and Low

Applications normally use indentation-based High (`.nagi`). Low (`.low`) uses braces and the same type and ownership rules, allowing inspection of generated code and function replacements. Raw pointers, unsafe syntax, layout declarations, and C ABI are unsupported.

Development will prioritize High and Rust integration while preserving Low compatibility. See [High and Low](docs/en/low-language.md) and [Rust integration](docs/en/modules-and-rust.md).

## Current scope

| Available in this source | Main limits |
|---|---|
| Typed values, classes, enums, Lists, nullable values, Results, moves, views, and shared values | User-defined generics/traits and standard Map operations are unsupported; owned handling is incomplete |
| HTTP, headers, response statuses, custom errors, and shared state | The standard server uses loopback HTTP/1. It has no public TLS, WebSocket, or streaming API |
| JSON, SQLite, and explicit SQL/schema checks | Preflight checks are opt-in. Value types, NULL, and dynamic SQL are checked at runtime. Bind shapes are fixed. No standard PostgreSQL, pool, or transaction API |
| Async/scopes, typed actors, and Supervisors | Tokio tasks within one process; no custom VM, hot code replacement, or distributed actors |
| File imports, standard modules, Rust integration, and Low replacements | No direct use of arbitrary Rust types or stable external ABI |

SQLite SQL literals support explicit [schema checks](docs/en/sql-check.md) for names, required result columns, and bind counts. Value types, NULL behavior, and consistency with the deployed schema are not guaranteed. Check the [CHANGELOG](CHANGELOG.md) for inclusion in published releases.

Check the [reference](docs/en/README.md) for individual API conditions. [Ownership](docs/en/ownership.md) explains Nagi's and rustc's checks; the [introduction](docs/en/introduction.md#compiler-and-existing-libraries) describes the implementation foundations.

Performance results apply to the [recorded conditions and raw logs](docs/en/measurements.md). They do not establish that Nagi is faster or more mature than Rust plus Axum.

## Repository layout

| Path | Contents |
|---|---|
| `compiler/` | Lexer, High/Low parsers, type/move/view checks, Rust generation, and CLI |
| `runtime/` | HTTP, JSON, SQLite worker, scopes, actors, and Supervisors |
| `examples/`, `test-nagi-code/` | Tutorials, applications, Rust integration, and Low replacements |
| `editors/` | VS Code and JetBrains plugins |
| `docs/`, `website/` | Japanese/English references, proposals, and the official site |
| `tests/`, `scripts/`, `benchmarks/` | Verification, distribution, measurements, and raw logs |

## Reproduce the checks

Basic compiler and runtime checks:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
cargo build --release --examples --bins --locked
python3 scripts/build_examples.py
```

See [CONTRIBUTING](CONTRIBUTING.en.md#check-your-work) for component-specific checks, [verification methods](docs/en/performance.md) for HTTP and benchmarks, and the [site build instructions](website/README.md#手元で確認する) (Japanese) for the website.

The [compiler test guide](docs/internal/compiler-testing.md) covers contracts, regression cases, and generated tests through High, Low, and Rust. Repository development agents should read [AGENTS.md](AGENTS.md). These internal guides are currently in Japanese.

## Contributing and license

Bug reports, fixes, documentation, and translations are welcome. The [contribution guide](CONTRIBUTING.en.md) covers proposals, verification, and AI use; the [first contribution walkthrough](docs/en/contributing.md) follows a small change from repository map to tests and PR. Report vulnerabilities privately using the [security policy](SECURITY.en.md). Nagi uses the [MIT license](LICENSE).
