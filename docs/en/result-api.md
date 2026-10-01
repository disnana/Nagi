# An API that handles Result success and failure

This example doubles an input number and reads an item from SQLite. Match separates success and failure, letting you try error responses and recovery with fallback data.

## Start

Run from the repository root. Builds require Rust/Cargo and a C compiler.

```powershell
cargo build --release --locked -p nagic
.\target\release\nagic.exe check --project test-nagi-code/result-api
.\target\release\nagic.exe run --project test-nagi-code/result-api
```

On Linux/WSL, replace the executable in the last two lines with ./target/release/nagic.

The default port is 8097 and the database is in memory. Startup inserts id=1, name="notebook". To change the port in PowerShell, set `$env:NAGI_PORT = "8098"` before starting. Stop with Ctrl+C.

## Call it

From another PowerShell terminal:

```powershell
Invoke-RestMethod 'http://127.0.0.1:8097/api/double?value=21'
Invoke-RestMethod 'http://127.0.0.1:8097/api/items/1'
Invoke-RestMethod 'http://127.0.0.1:8097/api/fallback'
```

Responses are `{"value":42}`, `{"id":1,"name":"notebook"}`, and `{"id":0,"name":"cached item"}`. Use curl with the same URLs on Linux/WSL.

| GET path | Result |
|---|---|
| /api/double?value=21 | 200: converts text to an integer and doubles it |
| /api/double?value=oops | 400: input cannot be parsed as an integer |
| /api/double?value=-1 | 400: outside the 0–1,000,000 range |
| /api/items/1 | 200: existing data |
| /api/items/2 | 404: no matching row |
| /api/items/0 | 400: ID must be positive |
| /api/items/1000001 | 404: returns not_found for an out-of-range ID |
| /api/fallback | 200: catches a database failure and returns fallback data |
| /api/db-error | 500: returns the original database error kind |
| /api/internal-error | 500: creates an internal error |

read_optional in storage.nagi deliberately queries **the nonexistent optional_items table**. This causes a real SQLite error, handled either by recovering with alternative data or by returning failure. The 500 body is `{"error":"internal error"}`; SQL and internal reasons are withheld from responses. Server logs include error kinds and details.

## Send smoke requests

Start the server above, then run this in another terminal. It uses only the Python 3 standard library.

```powershell
python test-nagi-code/result-api/smoke_api.py --base-url http://127.0.0.1:8097
```

It checks status, Content-Type, and JSON for 15 requests. It does not start/stop the server, create databases, or manipulate files. All requests are reads; you can repeat them against the same server.

## Read the code

- [server.nagi](../../test-nagi-code/result-api/server.nagi): routes, match, not_found, fail, and recovery with fallback data.
- [storage.nagi](../../test-nagi-code/result-api/storage.nagi): async database operations returning Result[Item?, Error].
- [models.nagi](../../test-nagi-code/result-api/models.nagi): JSON and database types.
- [nagi.toml](../../test-nagi-code/result-api/nagi.toml): entry configuration shared by CLI and VS Code.

The [VS Code extension](vscode-extension.md) has supported F12 for read_item and Item since 0.1.2, with the matching compiler. F12 on an import string opens its file. Save edited project files before running commands; newer extensions also support unsaved definition queries.

See [Result error handling](error-handling.md) for syntax and limits. Nullable matching is not implemented. This example returns Item? to HTTP and lets routing turn absence into 404.
