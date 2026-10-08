# Nagi examples

[日本語](README.md)

Examples of CLIs, HTTP APIs, SQLite, actors, and Rust integration. Each README identifies the work written in Nagi and the work handled by Rust. These demonstrate inputs, outputs, and limits; they are not production templates.

Obtain the full repository and run from its root. You need Nagi 0.1.9, Rust/Cargo, and your OS's build tools; see [setup](../docs/en/getting-started.md). A compiler built from `main` may include unreleased fixes even when its version number matches the public release.

## Libraries and Rust integration

| Example | Contents |
| --- | --- |
| [Library examples](library-examples/README.en.md) | Ten projects covering shared pricing, modules, custom errors, standard HTTP, and a custom Axum backend |
| [Application examples](application-examples/README.en.md) | JSON reports, SQLite APIs, a database-free quote API, file storage, and supervised workers |
| [Order quote in handwritten Low](low-examples/order-quote/README.en.md) | Brace syntax, Low-to-Low imports, and validation |
| [Local Rust crate](rust-library/README.en.md) | `path` dependencies, Cargo features, and conversion to Nagi types |
| [Rust bridge](rust-bridge/) | CRC-32, serde_json, and Rust async functions |
| [Code maps](../examples/code-map/README.en.md) | Visualize types, modules, and function calls |

Shared verification commands are in the [application index](application-examples/README.en.md) and [library guide](../docs/en/library-examples.md#verify-the-examples-during-development).

## Result API

The [Result API](../docs/en/result-api.md) uses integer parsing and a single-row SQLite query to distinguish invalid input, missing data, database failures, and fallback data with `match`.

## Task management JSON API

[Nagi Tasks](web-demo/README.md) adds, edits, completes, deletes, and aggregates tasks through a JSON API. Nagi defines standard HTTP routes and input validation; the runtime handles HTTP and SQLite. `GET /` returns a plain-text API landing message. The browser UI remains unconnected while typed HTML responses are pending.

## Use Rust libraries

See the [Rust crate example](rust-library/README.en.md) and [imports and Rust integration](../docs/en/modules-and-rust.md). Calling Rust does not require handwritten Low.

## Inventory API

[inventory.nagi](inventory.nagi) stores names and quantities in SQLite. Nagi defines JSON types, validation, routes, and error handling; the runtime executes SQL. Ordinary `check` does not validate SQL column names or database schemas.

### Run

```sh
nagic run test-nagi-code/inventory.nagi
```

Defaults are port 8090 and an in-memory database. To retain data, select a SQLite file in an existing directory.

```powershell
$env:NAGI_DB = "inventory-demo.sqlite"
$env:NAGI_PORT = "8090"
nagic run test-nagi-code/inventory.nagi
```

```sh
NAGI_DB=inventory-demo.sqlite NAGI_PORT=8090 nagic run test-nagi-code/inventory.nagi
```

`--cost-report` lists static cost sites. It does not measure runtime allocations or speed.

### Routes

| Method and path | Result |
| --- | --- |
| `GET /items` | Latest 100 items in descending ID order |
| `GET /items/{id}` | One item, or 404 when absent |
| `POST /items` | Create a name and quantity; return 200 and the item |
| `PUT /items/{id}` | Replace both fields, or 404 when absent |
| `DELETE /items/{id}` | Return ID and `deleted`; repeated deletion returns 200 and `false` |
| `GET /inventory/summary` | Total items, total quantity, and items with zero stock |

From bash, create an item as follows. Use `curl.exe` on Windows if `curl` is a PowerShell alias.

```sh
curl -H 'Content-Type: application/json' \
  -d '{"name":"ノート","quantity":12}' http://127.0.0.1:8090/items
curl http://127.0.0.1:8090/inventory/summary
```

### HTTP checks

Stop the persistent server and restart with an empty in-memory database. In PowerShell:

```powershell
$env:NAGI_DB = ":memory:"
nagic run test-nagi-code/inventory.nagi
```

In bash, use `NAGI_DB=:memory: nagic run test-nagi-code/inventory.nagi`. Run the verification from another terminal.

```sh
python test-nagi-code/smoke_inventory.py --base-url http://127.0.0.1:8090
```

Using only the Python 3 standard library, the script checks CRUD, UTF-8 boundaries, invalid input, binds, aggregation, and listing. It does not start or stop the server or access database files. Test data remains; restart the in-memory server before repeating the checks.

### Limits

Names contain 1–120 UTF-8 bytes, quantities are 0–1,000,000, and IDs are positive integers. Names are not trimmed and may repeat. Extra JSON fields, incorrect types, and out-of-range values return 400. Database and internal errors return 500 without private details. SQL values are bound, and the table has value constraints.

Insert/update currently use fixed `str` and `i32` parameters. Pagination, authentication, search, partial updates, and migrations are not included. The last write wins. The server listens on loopback with a two-second handler deadline and a 1 MiB body cap. An accepted database operation may complete after its caller times out.

## Console fractals

[fractal.nagi](fractal.nagi) draws Mandelbrot and Julia sets in the console. It demonstrates numeric calculations, loops, and standard input/output without a server or database.

```sh
nagic run test-nagi-code/fractal.nagi
```

Enter `1` for Mandelbrot, `2` for Julia, or `q` or an empty line to exit. Use a console at least 80 characters wide.

Build a distributable Windows x64 executable from the repository root:

```powershell
.\test-nagi-code\build_fractal_exe.ps1
.\build\distribution\nagi-fractal.exe
```

Building requires Rust/Cargo and the MSVC C toolchain. The script statically links the VC++ runtime, so recipients do not need Rust/Cargo, Nagi, or Python. Add `-Offline` when dependencies are cached.
