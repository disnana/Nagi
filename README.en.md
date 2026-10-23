# Nagi 0.2.1 — a backend language in development

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

The [VS Code extension](https://marketplace.visualstudio.com/items?itemName=Disnana.nagi-lang) is available from the Marketplace or [GitHub Releases](https://github.com/disnana/Nagi/releases). [Nagi](editors/jetbrains-nagi/README.en.md) for IntelliJ IDEA and PyCharm is available from [JetBrains Marketplace](https://plugins.jetbrains.com/plugin/34891-nagi) or [GitHub Releases](https://github.com/disnana/Nagi/releases). For Marketplace installs, check the listing's Versions view for available versions and compatible IDE builds. The published GitHub 0.1.2 release includes one common `nagi-jetbrains-0.1.2.zip` and its `.sha256` for both IDEs. The minimum IDE builds for that ZIP are IDEA 2025.1.1 build 251.25410.109 and PyCharm 2025.1.1 build 251.25410.122. Marketplace installs directly to the compatible IDE; Release ZIPs are installed manually. Compiler-backed completion, diagnostics, and navigation in plugin 0.1.3 require published compiler 0.2.0 or later. Check GitHub Releases for available plugin ZIPs. See the [editor guide](docs/en/editor.md).

Nagi 0.2.1 was [released on 2026-10-23](https://github.com/disnana/Nagi/releases/tag/nagi-v0.2.1). Use `nagic run --project . --profile dev` during development. Find task-specific information in the [development quick reference](docs/en/developer-workflow.md) and [language comparisons](docs/en/comparison.md). The install scripts select the latest public release. The HTTP and SQLite examples below require 0.2.0; published 0.1.11 does not include these APIs. Check the installed compiler with `nagic --version` and read the [0.2.0 migration guide](docs/en/migration-0.2.0.md) before upgrading.

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

## Run the 0.2.0 HTTP example

Save this as `server.nagi`. This HTTP sample performs no database writes. Check and run it with Nagi 0.2.0.

```nagi
import std.http.server as http

class State:
    greeting: str

async def hello(request: http.Request, state: shared[State], policy: unit) -> Result[http.Response, Error]:
    return http.text(http.response_context(view(request)), http.Status.OK, view(state.greeting))

async def main() -> Result[unit, Error]:
    app = http.app_default[State](State(greeting="Hello, Nagi!"))
    app = try http.route(app, http.Method.GET, "/", http.public_policy[State](), hello)
    route_policy = try http.rate_policy(1000, 1000)
    app = try http.rate_route(app, http.Method.GET, "/", route_policy)
    limits = http.default_options()
    limits = try http.authority(limits, "https://localhost", ["localhost:8080", "127.0.0.1:8080"], 2, 256)
    # 明示的な有限予算の例。用途に応じて調整してください。
    response_budget = try http.response_limits(
        1048576, 8192, 32, 8192, 32, 65536, 64, 83886080, 1024, 285212672, 8192, 32
    )
    limits = try http.with_response_limits(limits, response_budget)
    rate_budget = try http.rate_limits(64, 64, 4096, 4194304, 65536)
    limits = try http.with_rate_limits(limits, rate_budget)
    return await http.serve(app, 8080, limits)

```

```sh
nagic run server.nagi
```

Open [http://127.0.0.1:8080/](http://127.0.0.1:8080/) to receive `Hello, Nagi!`. Stop it with Ctrl+C. The budgets are illustrative; they are not capacity, SLA, or production-security recommendations. To build the compiler from the same repository revision, run `cargo build --release --locked` from its root, then use `./target/release/nagic run server.nagi`, or `.\target\release\nagic.exe run server.nagi` in Windows PowerShell. See the [0.2.0 migration guide](docs/en/migration-0.2.0.md) for API changes.

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

## Current scope (0.2.1)

| Available in this source | Main limits |
|---|---|
| Typed values, classes, enums, Lists, nullable values, Results, moves, views, and shared values | User-defined generic functions/traits and standard Map operations are unsupported. `owned[T]` has no general constructor or transparent operations |
| HTTP, headers, response statuses, custom errors, and shared state | The standard server uses loopback HTTP/1. It has no public TLS, WebSocket, or streaming API |
| JSON, SQLite Pool/Tx, typed Parameters, and opt-in SQL/schema checks | SQL must be one direct literal statement and uses anonymous `?` placeholders. No standard PostgreSQL API |
| Async/scopes, typed actors, and Supervisors | Tokio tasks within one process; no custom VM, hot code replacement, or distributed actors |
| File imports, standard modules, Rust integration, and Low replacements | No direct use of arbitrary Rust types or stable external ABI |

SQLite uses fixed SQL literals passed to `sqlite.literal(...)` to create `Query` values, and typed `Parameters` for bind values. There is no request-derived SQL string factory. Explicit [schema preflight](docs/en/sql-check.md) checks the shape and schema names of direct Query constructors, required columns, and bind counts from direct Parameters builder chains. Actual value types, NULL, and numeric ranges remain runtime checks; preflight does not prove that the deployed database matches. See [SQLite Pools and Transactions](docs/en/sqlite-pool.md) for Pool/Tx details. This API is not included in published 0.1.11.


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
