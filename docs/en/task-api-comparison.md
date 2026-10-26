# Compare the same task API in Nagi, FastAPI, and Quart

This comparison implements the same SQLite-backed JSON CRUD and highlights decisions that differ. Nagi is a language and runtime; FastAPI and Quart are Python web frameworks. The comparison includes the application server, dependencies, and distribution implications. It is not a speed ranking.

Comparison code:

- Nagi 0.2.1: [`test-nagi-code/web-demo/`](../../test-nagi-code/web-demo/)
- FastAPI 0.162.0: [`fastapi_app.py`](../comparisons/task-api/fastapi_app.py), Uvicorn 0.54.0
- Quart 0.23.1: [`quart_app.py`](../comparisons/task-api/quart_app.py), Hypercorn 0.18.0
- Shared Python verification: [`test_apps.py`](../comparisons/task-api/test_apps.py)

The Python examples were verified with Python 3.13.16 and HTTPX 0.28.1 on 2026-10-26. Quart 0.23.1 requires Python 3.13 or later. Python 3.13.18 was released with security fixes on the same date, so use 3.13.18 or a later patched release for a new environment and rerun the checks. Version 3.13.18 was not yet present in this work environment's `uv` download catalogue and is not part of the measured result. See the [comparison code README](../comparisons/task-api/README.md) to reproduce it.

## Matched behavior

All three implementations create, list, fetch, update, and delete tasks, compute completion statistics, and persist to SQLite. Input has `title: str` and `done: bool`; titles contain 1–240 UTF-8 bytes and IDs range from 1 through `i64::MAX`. Values are bound to SQL placeholders. The shared Python check covers types, extra fields, empty titles, an unpaired surrogate, the ID upper bound, missing IDs, and restart persistence. Nagi's 21 checks cover types, extra fields, empty/length boundaries, malformed JSON, ID zero, SQL binding, and CRUD; they do not directly send an unpaired surrogate or `i64::MAX + 1`.

| Decision | Nagi 0.2.1 | FastAPI 0.162.0 | Quart 0.23.1 |
|---|---|---|---|
| Input model | Pass a Nagi `class` to `json_decode[SaveTask]` | Declare a Pydantic model as a handler parameter | Validate request JSON in application code |
| Input failure | A route mapper converts handler Error to 400 | Pydantic validation uses 422; this example emits a generic 422 body without reflecting invalid Unicode | This example calls `abort(400)` |
| SQLite | Standard typed Query/Parameters and async Pool/Tx | Python `sqlite3` in synchronous handlers | `sqlite3` through `asyncio.to_thread` to keep it off the event loop |
| HTTP server | Nagi runtime loopback HTTP/1 | Explicit Uvicorn process | Explicit Hypercorn process |
| Development feedback | `nagic check`, then Cargo builds when needed | Python startup, Pydantic, and tests; choose a separate type checker if needed | Python startup and tests; choose schema and type-checking tools separately |
| Distribution | Native target-OS executable and any native dependencies | Python environment, application dependencies, Uvicorn | Python environment, application dependencies, Hypercorn |
| First setup | Compile Rust dependencies | Install Python wheels/packages | Install Python wheels/packages |

## Differences in the code

### Nagi

Application setup explicitly configures accepted HTTP authority, rate policy, and response, queue, and memory budgets. SQLite uses Pool/Tx and typed binds, and `Result` propagates failures. The application therefore spells out route policy and transaction completion. The standard APIs cover a narrower set of integrations and cannot directly import Python packages.

### FastAPI

A Pydantic model declared on a handler combines JSON decoding, schema, type/field validation, and OpenAPI integration. This example uses strict strings and booleans and rejects extra fields. It accesses SQLite from synchronous handlers; production connection management and migrations require a separate library/design choice. An ASGI server such as Uvicorn is required.

### Quart

Quart provides a Flask-like route API with async handlers. This example validates the JSON schema explicitly. Python's `sqlite3` is synchronous, so the code follows Quart guidance and sends the work to a thread rather than blocking the event loop. Adding a schema library or async database library would change the code. Deployment needs an ASGI server such as Hypercorn.

## Choosing among them

Evaluate Nagi for more than Python-like appearance: it is relevant when you want Nagi's type, ownership, Result, and capability checks together with native Rust distribution or crate adapters. FastAPI is a strong candidate when Python ecosystem access, broad middleware, OpenAPI-oriented development, and short setup matter. Quart is useful to compare when you want an async application with an API close to Flask.

Nagi 0.2.1 has no standard PostgreSQL, WebSocket, HTTP/2, streaming, or server-side TLS API. Its standard HTTP server is for loopback use and needs a reverse proxy for external exposure. FastAPI and Quart deployments likewise need explicit choices for the production server, TLS/proxy, timeouts, body/connection limits, authentication, database pooling, migrations, and monitoring.

## Verified scope and non-claims

- Published Nagi 0.2.1 passed `check`, a development server run, 21 HTTP smoke checks, and restart retrieval from the same SQLite file.
- Pinned FastAPI/Uvicorn and Quart/Hypercorn processes ran on real sockets under Python 3.13.16 and passed matching CRUD, type/UTF-8/ID-boundary, 404, and restart checks. Startup verification compares a per-process nonce so that an unrelated service on the candidate port cannot count as success.
- Source line counts depend on framework defaults, explicit safety boundaries, comments, and formatting, so they are not treated as a productivity score.
- No matched-load benchmark was run. Native code generation alone does not establish that Nagi is faster, smaller, or safer than FastAPI or Quart.

Official references: [FastAPI request bodies and Pydantic](https://fastapi.tiangolo.com/tutorial/body/), [FastAPI SQL database tutorial](https://fastapi.tiangolo.com/tutorial/sql-databases/), [running a FastAPI server manually](https://fastapi.tiangolo.com/deployment/manually/), [Quart quickstart](https://quart.palletsprojects.com/en/latest/tutorials/quickstart/), [handling synchronous code in Quart](https://quart.palletsprojects.com/en/latest/how_to_guides/sync_code/), [Quart deployment](https://quart.palletsprojects.com/en/latest/tutorials/deployment/), and the [Python 3.13.18 release notice](https://blog.python.org/2026/10/python-3152-31410-31318-31216-31118/).
