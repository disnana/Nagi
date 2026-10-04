# Supply your own HTTP backend in Rust

This project uses Axum 0.8 and Tokio through Nagi's existing Rust bridge. Rust owns the listener, routes and request policy; an ordinary synchronous Nagi function chooses the greeting. The application does not call the built-in `serve`, `Db` or SQLite functions. For standard async HTTP handlers written entirely in Nagi, see [http-auth](../http-auth/README.en.md). This example instead passes a synchronous Nagi function to a custom Rust server.

Run these commands from this directory. You need the `nagic` command and Rust/Cargo. Cargo may download dependencies on the first build.

```sh
nagic check
nagic run
```

From the repository root, the equivalent commands are:

```sh
nagic check --project test-nagi-code/library-examples/custom-http
nagic run --project test-nagi-code/library-examples/custom-http
```

`check` validates the Nagi declaration and callback types. `run` also compiles the Rust adapter, which checks its implementation against those types. Stop with Ctrl+C: Axum stops accepting connections and drains active requests.

The server binds only to `127.0.0.1:8088`. Change the port before starting it:

```sh
NAGI_SAMPLE_PORT=8089 nagic run
```

In PowerShell:

```powershell
$env:NAGI_SAMPLE_PORT = "8089"
nagic run
```

The port must be an integer from 1 to 65535. Zero, negative values, values above 65535 and non-integers fail during startup. An occupied port also produces a startup error.

## Try the routes

Use a second terminal while the server is running. These commands use the default port; on Windows use `curl.exe` if `curl` is a PowerShell alias.

```sh
curl http://127.0.0.1:8088/health
curl http://127.0.0.1:8088/hello/7
curl http://127.0.0.1:8088/hello/42
curl -i http://127.0.0.1:8088/missing
curl -i http://127.0.0.1:8088/hello/not-a-number
```

| Request | Result |
|---|---|
| `GET /health` | 200, `ok` |
| `GET /hello/7` | 200, `Hello, Nagi! 7` |
| `GET /hello/42` | 200, `Hello from the Nagi callback! 42` |
| `GET /missing` | 404, `Route not found` |
| `GET /hello/not-a-number` | 400 from Axum's typed path extractor |
| An integer outside the i64 range | 400 from the same extractor |
| `POST /hello/7` | 405; this route accepts GET |

Successful text responses end with a newline. The different greetings show that Rust calls the supplied Nagi function with the path's i64 value.

To exercise the body cap, this optional test uses Python to upload 65537 bytes; the response is 413:

```sh
python -c "import sys; sys.stdout.write('x' * 65537)" | curl -i -X GET --data-binary @- http://127.0.0.1:8088/hello/7
```

## What this backend controls

`native.rs` implements these policies:

- At most eight requests may read a body or run a handler at once. An additional request receives 503 with `Retry-After: 1`; it does not wait in an application queue.
- Every request body, including an otherwise unused GET body, is read through Axum's `Bytes` extractor with a 65536-byte limit. Oversized bodies receive 413; other body read failures receive 400.
- A three-second deadline covers body receipt and handler work after the request reaches the middleware. An expired request receives 408.
- The synchronous callback runs on Tokio's blocking pool. Its request permit remains held until it finishes, even if its HTTP response has timed out. Tokio cannot forcibly interrupt a callback that has already started; keep callbacks finite.

Nagi's built-in HTTP protection and `NAGI_HTTP_REQUEST_WAIT_SECONDS` do not automatically apply to this server. This sample does not configure request-header or idle-connection deadlines, a connection count cap, response-write deadlines, TLS or authentication. Those are policies for the Rust backend author to choose. A callback that never finishes can also delay runtime shutdown; graceful shutdown has no separate deadline here.

## Change the implementation

| File | Responsibility |
|---|---|
| [custom_http.nagi](custom_http.nagi) | Declares the adapter, implements `render_greeting`, reads `NAGI_SAMPLE_PORT` |
| [native.rs](native.rs) | Builds the Axum router and implements limits and shutdown |
| [nagi.toml](nagi.toml) | Selects the entry, Rust source and version dependencies |

The bridge declaration `fn[i64, str]` becomes Rust's `fn(i64) -> String`. `extern async def run_server(...) -> Result[unit, Error]` becomes an async Rust function returning `Result<(), nagi_runtime::Error>`. The generated application still uses `nagi-runtime` for execution and the bridge's Error type.

You can replace the Rust router or add another Rust-backed service while keeping Nagi's greeting policy. Use the manifest's `[rust.dependencies]` table for crates. This example passes a plain synchronous function pointer. The bridge used here cannot accept an async callback or a capturing closure.

[日本語](README.md) · [Rust integration](../../../docs/en/modules-and-rust.md)
