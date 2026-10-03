# HTTP

Use `std.http.server` to build an HTTP server. No database is required. Register an async handler and the state it shares with other requests.

## A minimal server

Save this as `server.nagi`:

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

Open [http://127.0.0.1:8080/](http://127.0.0.1:8080/) to see `Hello, Nagi!`. Press Ctrl+C to stop.

## Requests and responses

| Operation | Expression |
|---|---|
| Check for GET | `request.is_get` or `request.method == http.Method.GET` |
| Read the path | `request.path` |
| Borrow the body | `request.body` |
| Read a header | `http.header_text(view(request), "Authorization")` |
| Return text | `http.text(http.Status.OK, "hello")` |
| Return HTML | `http.html(http.Status.OK, "<h1>Hello</h1>")` |
| Return JSON | `http.json[User](http.Status.CREATED, user)` |
| Return no body | `http.empty(http.Status.NO_CONTENT)` |

A header may be absent, so the getter returns `Result[Option[view[str]], Error]`. Use `match` with `Ok`/`Err`, then `Some`/`None`. Register POST routes with `http.Method.POST`.

## Customize error responses

`http.app[State, AuthError](state, auth_error)` sets a function that converts your error type into a response. `http.route_mapped` overrides that function for one route. Successful responses bypass the error mapper.

The [authentication example](../../test-nagi-code/library-examples/http-auth/README.en.md) includes Authorization headers, 401 with WWW-Authenticate, a route-specific 403, and typed shared state.

## Further reference

- [HTTP API reference](http-server.md): Status, headers, routes, and limits
- [Standard HTTP measurements](http-stdlib-performance.md): latency, memory, and continuous load
- [JSON](json.md): decoding a body into a class
- [Existing HTTP attributes](http-legacy.md): code using `@get`/`@post` and `serve(Db, port)`

The standard server currently serves HTTP/1.1 on loopback. Use a reverse proxy for TLS and external access. This module does not expose streaming, WebSocket, or HTTP/2 APIs.
