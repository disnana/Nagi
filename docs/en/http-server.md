# HTTP API reference

```nagi
import std.http.server as http
from std.http.server import Status as Code
```

## Method and Status

Use `http.Method.GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `HEAD`, `OPTIONS`, `CONNECT`, or `TRACE`. Comparisons borrow `request.method`; they do not copy it. `request.is_get` and `request.is_post` are also available. `http.method("PROPFIND")` validates an extension method.

Status is a small, copyable numeric value. Comparisons and responses do not require a string conversion.

| Operation | Example |
|---|---|
| Named status | `Code.OK`, `CREATED`, `NO_CONTENT`, `UNAUTHORIZED`, `FORBIDDEN`, `CONFLICT`, `TOO_MANY_REQUESTS` |
| Construct from a number | `try http.status(418)`: validates a final response status from 200 through 599 |
| Read the number | `Code.NOT_FOUND.value` → `404` |
| Read the description | `Code.NOT_FOUND.phrase` → `"Not Found"` |
| Classify | `.is_success`, `.is_redirection`, `.is_client_error`, `.is_server_error` |

Standard 2xx–5xx constants are available. `.phrase` borrows static text and is empty for an unassigned number. Informational responses and 101 protocol switching require separate protocol APIs.

## Request

| Field | Type |
|---|---|
| `method` | `http.Method` |
| `path` | `view[str]` |
| `query` | `Option[view[str]]`: raw query without `?` |
| `body` | `view[bytes]` |
| `is_get`, `is_post`, etc. | `bool` |

Paths, queries, bodies, and headers borrow the Request's data. You cannot move the Request while those views remain in use. Routes accept templates such as `"/users/{id}"`; a typed capture extraction API is not available yet.

| Function | Return type |
|---|---|
| `header(view(request), name)` | `Result[Option[view[bytes]], Error]` |
| `header_text(view(request), name)` | `Result[Option[view[str]], Error]` |
| `headers(view(request), name)` | `Result[List[view[bytes]], Error]` |
| `method_name(view(request.method))` | `view[str]` |

Header names are case-insensitive. Single-header getters reject duplicates; `headers` returns all values. `header_text` validates UTF-8.

## Response

| Function | Return type |
|---|---|
| `empty(status)` | `Response` |
| `text(status, text)`, `html(status, text)` | `Response` |
| `bytes(status, body)` | `Response` |
| `json[T](status, value)` | `Result[Response, Error]` |
| `append_header(response, name, value: view[bytes])` | `Result[Response, Error]` |
| `append_header_text(response, name, value: view[str])` | `Result[Response, Error]` |

`text`, `html`, and `bytes` create an owned response body from borrowed input. `json` borrows its value, so it does not move the original. String literals can be passed directly to view parameters; borrow variables with `view(value)`.

Header append consumes and returns the response. It preserves duplicate Set-Cookie values and rejects invalid names, line breaks, and explicit Content-Length/Transfer-Encoding. HEAD omits the body while retaining its GET-equivalent length. 204, 205, and 304 omit the body. Successful CONNECT tunnels are unsupported: the server returns 501 and closes the connection.

## App and routes

```nagi
app = http.app[State, AuthError](state, map_error)
app = try http.route(app, http.Method.GET, "/", handle)
app = try http.route_mapped(app, http.Method.POST, "/login", login, map_login_error)
return await http.serve(app, 8080, http.default_options())
```

- `app[S, E](state, mapper)` owns the state and uses `fn(E) -> Response` as its default error mapper.
- `app_default[S](state)` uses the default mapper for `Error`.
- A handler has the signature `async def handle(request: Request, state: shared[S]) -> Result[Response, E]`. The state itself is not copied per request.
- `route_mapped` accepts a handler with its own error type and a matching mapper.
- GET routes automatically accept HEAD unless an explicit HEAD route takes precedence. A different method on an existing path returns 405 with Allow.

Mappers handle application failures. The server handles malformed HTTP, limits, and deadlines. Avoid exposing database error details or credentials in responses.

## Limits and shutdown

| Setting | Default | Setter |
|---|---|---|
| Body size | 1 MiB | `options(body_bytes, body_ms, handler_ms, shutdown_ms)` |
| Body deadline | 10 seconds | `options(...)` |
| Handler deadline | 2 seconds | `options(...)` |
| Shutdown deadline | 10 seconds | `options(...)` |
| Connections / concurrent requests | 1024 / 256 | `capacity(options, connections, requests)` |
| Headers / next request wait | 10 seconds | `header_timeout(options, milliseconds)` |
| Response send deadline | 10 seconds | `send_timeout(options, milliseconds)` |
| Header buffer / count | 32 KiB / 100 | `header_limits(options, bytes, count)` |

Every setter returns `Result[Options, Error]`. Connection and request limits control admission, not thread counts. Ctrl+C stops admission and waits for active connections. The server aborts and joins remaining connection tasks after the shutdown deadline. Already running blocking work cannot be forcibly stopped.

Example:

```nagi
limits = try http.options(1048576, 10000, 2000, 10000)
limits = try http.capacity(limits, 2048, 512)
limits = try http.header_limits(limits, 32768, 100)
return await http.serve(app, 8080, limits)
```
