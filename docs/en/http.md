# HTTP

This page describes unreleased 0.2.0 SF01 development source. For published 0.1.x code, see the [migration guide](migration-0.2.0.md). Every route, including public routes, requires an explicit Policy.

Use `std.http.server` to build an HTTP server without a database. Define async handlers, shared state, and error responses in Nagi.

## A minimal server

Save this as `server.nagi`:

```nagi
import std.http.server as http

class State:
    greeting: str

async def hello(request: http.Request, state: shared[State], access: unit) -> Result[http.Response, Error]:
    return ok(http.text(http.Status.OK, view(state.greeting)))

async def main() -> Result[unit, Error]:
    app = http.app_default[State](State(greeting="Hello, Nagi!"))
    app = try http.route(app, http.Method.GET, "/", http.public_policy[State](), hello)
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
| Display HTML notation as text | `http.text(http.Status.OK, "<h1>Hello</h1>")`; active HTML awaits SF04 |
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
- [Retired HTTP APIs](http-legacy.md): migrate legacy decorators and serve

The standard server currently serves HTTP/1.1 on loopback. Use a reverse proxy for TLS and external access. Peer IP access, streaming, WebSocket, and HTTP/2 APIs are not implemented. A standard HTTP client for calling external APIs is also not implemented.

Internally, Hyper handles HTTP transport and Tokio runs async work. Nagi provides route registration, typed state, responses, limits, and error mapping. Replacing this implementation with Axum/Tower is under evaluation; neither replacement nor a performance improvement has been established.
