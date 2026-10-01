# HTTP and HTML

[Contents](README.md) · Previous: [Language guide](language-guide.md) · Next: [SQLite](database.md)

Async functions with attributes such as `@get` become HTTP handlers. Returning a class produces JSON; returning `Html` produces HTML. Start with a small server that does not store data.

## 1. Write a server

This complete program follows [examples/tutorial/http.nagi](../../examples/tutorial/http.nagi), with the embedded HTML translated into English.

```nagi
class Greeting:
    id: i64
    name: str

@get("/")
async def home() -> Result[Html, Error]:
    return ok(html("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Nagi</title><h1>Hello, Nagi!</h1><a href=\"/greet/1\">View JSON</a></html>"))

@get("/greet/{id}")
async def greet(id: i64) -> Result[Greeting, Error]:
    if id < 1:
        return error("id must be positive")
    return ok(Greeting(id=id, name="Nagi"))

@post("/echo")
async def echo(req: Greeting) -> Result[Greeting, Error]:
    return ok(req)

async def main() -> Result[unit, Error]:
    db = try await db_open(":memory:")
    return await serve(db, 8094)
```

The current `serve` API needs Db. This example opens an in-memory SQLite database without creating tables or writing data. `serve` starts the server and keeps waiting for requests.

## 2. Start and call it

Make sure port 8094 is free and run from the repository root:

```powershell
.\target\release\nagic.exe run examples/tutorial/http.nagi
```

On Linux/WSL2, use `./target/release/nagic run examples/tutorial/http.nagi`. Open [http://127.0.0.1:8094/](http://127.0.0.1:8094/) in a browser to see “Hello, Nagi!”.

Call the API from another PowerShell terminal:

```powershell
Invoke-RestMethod http://127.0.0.1:8094/greet/7
Invoke-RestMethod http://127.0.0.1:8094/echo -Method Post -ContentType 'application/json' -Body '{"id":2,"name":"sample"}'
```

On Linux/WSL2:

```bash
curl http://127.0.0.1:8094/greet/7
curl -H 'Content-Type: application/json' -d '{"id":2,"name":"sample"}' http://127.0.0.1:8094/echo
```

GET returns `{"id":7,"name":"Nagi"}`; POST echoes `{"id":2,"name":"sample"}`. `/greet/0` returns a 400 JSON error. Press Ctrl+C in the server terminal to stop it.

## 3. Define arguments and responses

| Form | HTTP meaning |
|---|---|
| `@get("/greet/{id}")` with `id: i64` | Reads `{id}` as an integer |
| `req: Greeting` | Reads request JSON into a class |
| `db: Db` | Supplies the database passed to serve |
| Primitive argument not present in the path | Reads a query parameter; see `/query` in [crud.nagi](../../examples/crud.nagi) |
| `body: view[bytes]` | Borrows request body bytes |
| `Result[Greeting, Error]` and `ok(...)` | JSON on success |
| `Result[Greeting?, Error]` and `ok(None)` | 404 for an absent value |
| `Result[Html, Error]` and `ok(html(...))` | `text/html` on success |
| `error("reason")` | 400 JSON input error |
| `not_found("reason")` | 404 JSON missing-target error |
| `internal_error("reason")` | 500 JSON error with details withheld |
| `fail(problem)` | Response preserving Error kind; database errors become 500 |

Handlers use `async def` and return `Result[..., Error]`. Attributes include `@get`, `@post`, `@put`, and `@delete`. Missing JSON fields, wrong types, and unknown fields are input errors.

Use `match await operation(...)` to handle failure or return a default. The [Result API example](result-api.md) demonstrates 400 for invalid input, 404 for absent data, 500 for database failure, and recovery with a 200 response over real HTTP.

## 4. Move HTML into a separate file

For longer HTML, put `index.html` beside the `.nagi` file and use:

```nagi
@get("/")
async def home() -> Result[Html, Error]:
    return ok(html(include_text("index.html")))
```

`include_text` embeds HTML in the executable at compile time. Rebuild after editing it. The distributed application does not need a separate HTML file.

Browser JavaScript can call the same server with `fetch("/api/tasks")`. The [task management demo](web-demo.md) combines add/edit/delete operations with SQLite.

## Current server scope

High attributes generate Axum routing. Implemented features include HTTP/1.1, keep-alive, path parameters, typed query parameters, request bodies, and JSON responses.

Standard test endpoints include `/health`, a five-chunk `/stream`, and the `/ws` echo WebSocket. Middleware limits request processing to two seconds and body/WebSocket messages to 1 MiB. Database and internal errors become statuses such as 500 without exposing details in responses.

HTTP waits expire after ten seconds by default. This covers silence after accept, incomplete headers, and the interval from a completed response until the next complete request headers. Sending a few bytes does not extend the deadline. Expired connections close; clients should reconnect when needed. This wait does not apply to active responses, streams, or upgraded WebSocket sessions.

To change it, set `NAGI_HTTP_REQUEST_WAIT_SECONDS` to a positive integer before starting the server. For example, use `$env:NAGI_HTTP_REQUEST_WAIT_SECONDS = "30"` in PowerShell or `export NAGI_HTTP_REQUEST_WAIT_SECONDS=30` in bash. Headers and unused keep-alive share this deadline. There is no fixed limit of 128 concurrent connections.

The server currently binds to loopback only. HTTP/2, TLS, authentication, arbitrary middleware declarations in High, and deployment mechanisms are not implemented. Large-class JSON streaming and general High streaming syntax are also absent. JSON responses encode classes into `Vec<u8>` and pass it to Body.
