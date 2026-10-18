# HTTP

This page describes unreleased 0.2.0 SF01 development source. For published 0.1.x code, see the [migration guide](migration-0.2.0.md). Every route, including public routes, requires an explicit Policy.

Use `std.http.server` to build an HTTP server without a database. Define async handlers, shared state, and error responses in Nagi.

## A minimal handler

This is a handler fragment, not a complete SF07 server startup.

```nagi
import std.http.server as http

class State:
    greeting: str

async def hello(request: http.Request, state: shared[State], access: unit) -> Result[http.Response, Error]:
    context = http.response_context(view(request))
    return http.text(context, http.Status.OK, view(state.greeting))

```

This handler returns `Hello, Nagi!` as `Result[Response, Error]`. The 0.2.0 candidate requires explicit response and rate budgets plus a rate policy on every route before startup. Do not reuse unverified values as defaults. See [SF07 budgets and startup](http-server.md#explicit-sf07-budgets-and-rate-policy-development-candidate) for the required configuration; runtime acceptance remains pending.

## Requests and responses

| Operation | Expression |
|---|---|
| Check for GET | `request.is_get` or `request.method == http.Method.GET` |
| Read the path | `request.path` |
| Borrow the body | `request.body` |
| Read a header | `http.header_text(view(request), "Authorization")` |
| Return text | `http.text(context, http.Status.OK, "hello")` |
| Return HTML notation as text | `http.text(context, http.Status.OK, "<h1>Hello</h1>")`; it is not parsed as HTML |
| Return a typed HTML document | `http.html_response(context, http.Status.OK, document)` |
| Return JSON | `http.json[User](context, http.Status.CREATED, user)` |
| Return no body | `http.empty(context, http.Status.NO_CONTENT)` |

In this table, `context` is borrowed from the current `request` with `http.response_context(view(request))`. Every response factory returns `Result[http.Response, Error]`; return it directly or propagate with `try`, and do not wrap it in a second `ok(...)`. There are no status-first or context-free compatibility factory calls.

A header may be absent, so the getter returns `Result[Option[view[str]], Error]`. Use `match` with `Ok`/`Err`, then `Some`/`None`. Register POST routes with `http.Method.POST`.

## Customize error responses

`http.app[State, AuthError](state, auth_error)` accepts a mapper of type `fn[view[http.ResponseContext], AuthError, Result[http.Response, Error]]`. `http.route_mapped` accepts the same mapper type for one route. The mapper uses the request-borrowed context to create a response; successful responses bypass it. Return a factory result directly or propagate it with `try`.

The [authentication example](../../test-nagi-code/library-examples/http-auth/README.en.md) includes Authorization headers, 401 with WWW-Authenticate, a route-specific 403, and typed shared state.

## Further reference

- [HTTP API reference](http-server.md): Status, headers, routes, and limits
- [Standard HTTP measurements](http-stdlib-performance.md): latency, memory, and continuous load
- [JSON](json.md): decoding a body into a class
- [Retired HTTP APIs](http-legacy.md): migrate legacy decorators and serve

The standard server currently serves HTTP/1.1 on loopback. Use a reverse proxy for TLS and external access. Peer IP access, streaming, WebSocket, and HTTP/2 APIs are not implemented. A standard HTTP client for calling external APIs is also not implemented.

Internally, Hyper handles HTTP transport and Tokio runs async work. Nagi provides route registration, typed state, responses, limits, and error mapping. Replacing this implementation with Axum/Tower is under evaluation; neither replacement nor a performance improvement has been established.
