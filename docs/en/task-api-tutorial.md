# Build a task JSON API end to end

This tutorial uses published Nagi 0.2.1 and connects a runnable HTTP server, SQLite CRUD, rejected input, tests, restart persistence, and a release build. Install Rust/Cargo and the platform C build tools first by following [setup](getting-started.md).

## 1. Create the smallest HTTP project

Create two files in a new `my-nagi-api` directory:

```text
my-nagi-api/
  nagi.toml
  main.nagi
```

`nagi.toml`:

```toml
entry = "main.nagi"
```

`main.nagi` is a complete runnable program:

```nagi
import std.http.server as http

class State:
    greeting: str

async def home(request: http.Request, state: shared[State], policy: unit) -> Result[http.Response, Error]:
    return http.text(http.response_context(view(request)), http.Status.OK, view(state.greeting))

async def main() -> Result[unit, Error]:
    app = http.app_default[State](State(greeting="Hello, Nagi!"))
    app = try http.route(app, http.Method.GET, "/", http.public_policy[State](), home)
    rate = try http.rate_policy(1000, 1000)
    app = try http.rate_route(app, http.Method.GET, "/", rate)
    options = http.default_options()
    options = try http.authority(options, "https://localhost", ["127.0.0.1:8094"], 1, 256)
    return await http.serve(app, 8094, options)
```

Check the project and run the development profile:

```sh
nagic --version
nagic check --project .
nagic run --project . --profile dev
```

Run `curl http://127.0.0.1:8094/` in another terminal to receive `Hello, Nagi!`. `http.authority` explicitly lists the accepted Host/authority. This is a loopback HTTP/1 example, not an Internet-facing TLS endpoint.

## 2. Move to complete SQLite CRUD

The repository contains this complete application:

```text
test-nagi-code/web-demo/
  nagi.toml
  tasks.nagi
  models.nagi
  validation.nagi
  smoke_api.py
```

`tasks.nagi` contains routes, finite HTTP budgets, a SQLite Pool/Tx flow, bound parameters, and commit/rollback. `models.nagi` defines JSON types; `validation.nagi` applies the 1–240 UTF-8 byte title limit. It has no browser UI or authentication.

From the repository root:

```sh
nagic check --project test-nagi-code/web-demo
```

Run the development profile. When changing the port, change the accepted authority to match:

```sh
NAGI_PORT=8091 \
NAGI_HTTP_AUTHORITY=127.0.0.1:8091 \
NAGI_DB=/tmp/nagi-tasks.sqlite \
nagic run --project test-nagi-code/web-demo --profile dev
```

Windows PowerShell:

```powershell
$env:NAGI_PORT = "8091"
$env:NAGI_HTTP_AUTHORITY = "127.0.0.1:8091"
$env:NAGI_DB = "$env:TEMP\nagi-tasks.sqlite"
nagic run --project test-nagi-code/web-demo --profile dev
```

## 3. Use the API

Representative operations:

```sh
curl http://127.0.0.1:8091/health
curl -H 'Content-Type: application/json' \
  -d '{"title":"Read the Docs","done":false}' \
  http://127.0.0.1:8091/api/tasks
curl http://127.0.0.1:8091/api/tasks
curl http://127.0.0.1:8091/api/stats
```

Routes are `GET /api/tasks`, `GET /api/tasks/{id}`, `POST /api/tasks`, `PUT /api/tasks/{id}`, `DELETE /api/tasks/{id}`, and `GET /api/stats`. Type mismatches, malformed JSON, and empty or oversized titles produce 400; fetching a missing item produces 404. SQL values use typed parameters bound to `?`; request text is never executed as SQL structure.

## 4. Run HTTP regression checks and verify persistence

Keep the server running and use another terminal:

```sh
python3 test-nagi-code/web-demo/smoke_api.py --base-url http://127.0.0.1:8091
```

The script checks 21 success, CRUD, malformed JSON, type/length, 404, and statistics cases. To check persistence, create a task, stop the server, restart with the same `NAGI_DB`, and request `GET /api/tasks/{id}`. A different database path selects different data.

## 5. Build for distribution

Use `--profile dev` for daily iteration and explicitly select a release build for a distribution candidate:

```sh
nagic build --project test-nagi-code/web-demo --profile release
```

The `native:` line identifies the executable. Verify release behavior and performance on each target OS. The first build compiles Rust dependencies, so it is not comparable to merely starting a Python server.

In a fresh-lock test of published Nagi 0.2.1 on 2026-10-26, dependency resolution selected `uuid 1.29.0`, which requires Rust 1.89 or later. Rust 1.88 failed after resolution; that day's stable Rust 1.99.1 completed the release build and all 21 HTTP checks. Use the current stable Rust for a new environment. An existing `Cargo.lock` can have different requirements, and Nagi 0.2.1 does not automatically pin dependencies for an older toolchain.

## Boundaries before production

- The standard server is loopback HTTP/1. For external exposure, terminate TLS and enforce Host, body, connection, and timeout limits at a trusted reverse proxy, while keeping Nagi authority and budgets aligned.
- This task API has no authentication and is for local learning and verification. The [authentication sample](../../test-nagi-code/library-examples/http-auth/README.en.md) demonstrates wiring, not a production identity system.
- Production authentication needs a reviewed token verifier, issuer/audience/expiry checks, key rotation, revocation, user management, authorization rules, and auditing. Do not reuse a fixed demo credential.
- This example configures one SQLite connection. Test write contention, backup, migration, and recovery under your own workload.
- Nagi 0.2.1 has no standard WebSocket, HTTP/2, streaming, or server-side TLS API.

Continue to the [FastAPI and Quart comparison](task-api-comparison.md) for the same API in Python.

