# HTTP API reference

These are unreleased 0.2.0 SF01 APIs. See [migration](migration-0.2.0.md) and [authentication and authorization](security.md).

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

Paths, queries, bodies, and headers borrow the Request's data. You cannot move the Request while those views remain in use. Routes accept templates such as `"/users/{id}"`; typed capture extraction and peer IP access are not available. The body is collected up to the configured limit before the handler runs; it is not a receive stream.

| Function | Return type |
|---|---|
| `header(view(request), name)` | `Result[Option[view[bytes]], Error]` |
| `header_text(view(request), name)` | `Result[Option[view[str]], Error]` |
| `headers(view(request), name)` | `Result[List[view[bytes]], Error]` |
| `is_json_content_type(view(request))` | `Result[bool, Error]` |
| `method_name(view(request.method))` | `view[str]` |

Header names are case-insensitive. Single-header getters reject duplicates; `headers` returns all values. `header_text` validates UTF-8.

`is_json_content_type` matches `application/json` case-insensitively, allowing surrounding spaces or tabs and parameters such as `charset=utf-8`. Missing or different media types return `False`; duplicate headers or malformed syntax return `Err`. `application/problem+json` is a different media type. Parameters are validated, but `charset` does not select an encoding: JSON bodies are decoded as UTF-8. The check borrows the header, copies no body data, and allocates nothing on success.

## Response

| Function | Return type |
|---|---|
| `empty(status)` | `Response` |
| `text(status, text)` | `Response` |
| `bytes(status, body)` | `Response` |
| `json[T](status, value)` | `Result[Response, Error]` |
| `append_header(response, name, value: view[bytes])` | `Result[Response, Error]` |
| `append_header_text(response, name, value: view[str])` | `Result[Response, Error]` |

`text` and `bytes` copy borrowed input into a body owned by the Response. `json` borrows its value, so it does not move the original. String literals can be passed directly to view parameters; borrow variables with `view(value)`.

Header append consumes and returns the response. It preserves duplicate nonreserved headers such as X-Trace. It rejects Content-Length, Transfer-Encoding, Content-Type, Set-Cookie, WWW-Authenticate, Cache-Control, Vary, CORS, CSP, nosniff, and other security-managed headers. Cookie/Session issuance remains unimplemented until SF02. HEAD omits the body while retaining its GET-equivalent length. 204, 205, and 304 omit the body. Authority-form CONNECT returns 400 at the startup-configuration gate below. If an origin-form CONNECT handler returns success, tunnels remain unsupported: the server returns 501 and closes the connection.

## App and routes

```nagi
app = http.app[State, AuthError](state, map_error)
app = try http.route(app, http.Method.GET, "/", http.public_policy[State](), handle)
app = try http.route_mapped(app, http.Method.POST, "/login", http.public_policy[State](), login, map_login_error)
limits = try http.authority(http.default_options(), "https://localhost", ["localhost:8080", "127.0.0.1:8080"], 2, 256)
return await http.serve(app, 8080, limits)
```

- `app[S, E](state, mapper)` owns the state and uses `fn(E) -> Response` as its default error mapper.
- `app_default[S](state)` uses the default mapper for `Error`.
- Policy is created by `public_policy[S]()`, `authenticated_policy[S](verifier)` or `authorized_policy[S,P](verifier, authorizer)`. Its output A is respectively unit, AuthScope or Grant[P]. The compiler checks S and A against the handler; old arities receive migration diagnostics.
- A handler has the signature `async def handle(request: Request, state: shared[S], access: A) -> Result[Response, E]`. The state itself is not copied per request. Rust requires bounds such as `Send + Sync` on state and `Send` on the handler future; build performs the final validation.
- `route_mapped` accepts a handler with its own error type and a matching mapper.
- GET routes automatically accept HEAD unless an explicit HEAD route takes precedence. A different method on an existing path returns 405 with Allow.

Mappers handle application failures. The server handles malformed HTTP, limits, and deadlines. Avoid exposing database error details or credentials in responses.

In Nagi 0.1.10, an unwinding panic in a handler or mapper before the response starts returns a generic 500 and closes that connection. Catching a panic does not roll back shared state or database updates. Return ordinary failures through Result. Recovery is not guaranteed for `panic=abort`, process termination such as OOM, a second panic during unwinding, custom Rust cleanup, or failures after the response starts.

When an error response needs a request ID, retain the validated ID in the handler and move it into a custom error only on failure. The [quote API example](../../test-nagi-code/application-examples/quote-api/README.en.md) passes its ID to a shared mapper this way.

## Limits and shutdown

The 0.2.0 development source requires checked authority configuration for `serve` and Rust `serve_listener`. Bare `default_options()`/`options(...)` fails startup.

| Startup setting | Contract |
|---|---|
| `authority(options, external_origin, authorities: List[str], entry_limit, byte_limit)` | Checks an HTTPS origin and a finite accepted wire-authority set; returns `Result[Options, Error]`. Limits are positive finite counts/bytes. Empty sets, canonical duplicates, invalid origin/authority and exceeded limits fail |
| `trusted_proxy(options, peer_ips: List[str])` | Requires configured authority; sets a real TCP peer IP ACL under the same finite count/byte limits. Empty, invalid, unspecified/multicast and duplicate canonical peers fail. Exact IPs, not CIDR/DNS |

Origin is only `https://host[:port]`, without path/userinfo/query/fragment. Omitted and explicit wire ports such as `:443` are separate entries. DNS ASCII case is normalized; IPs use the standard IP parser. IPv6 needs brackets. A host made only of digits and dots is rejected unless the standard IPv4 parser accepts it. Ports are canonical decimal 1–65535; leading zeros, userinfo, `%`/zones, trailing dots, Unicode and comma lists are rejected. Configure pre-canonical ASCII IDNA A-labels; runtime performs no Unicode conversion or IDNA-equivalence guarantee. Proxy peers alone normalize IPv4-mapped IPv6 to IPv4 with standard IP types and reject resulting duplicates.

Reconfiguring authority or overwriting a proxy ACL fails. Setters move Options; pass only a successful Result into startup. Failure never falls back to an earlier configuration.

Before every route, 404/405, body reception, verifier and proof issuance, the gate checks HTTP/1.1 origin-form and exactly one nonempty Host. Malformed/missing/duplicate/nonallowlisted authority, absolute-form/authority-form/`*`, and any Forwarded or X-Forwarded-* presence return 400; HTTP/1.0 returns 505. These runtime rejections use no-store and close. HTTP syntax rejected by Hyper before service invocation, including HTTP/2 prefaces, is delegated to its official parser and reaches no handler/proof. The standard listener is HTTP/1 only.

Direct/proxy profiles are fixed at startup. Proxies must strip Forwarded headers before backend transmission and use a registered internal or pass-through Host. Only the real peer returned by `accept` selects ACL admission; headers never choose trust or origin. A loopback ACL does not authenticate other processes on the same host. Host matching does not waive Origin/CSRF/CORS; this Host slice does not implement those SF03 features.

`serve` provides plaintext loopback HTTP; configuring an origin does not add TLS. Public local response checks and browser authentication have separate requirements. Use a TLS frontend with a browser-trusted certificate, for example external origin `https://localhost:8443`, backend wire `localhost:8080`, peer `127.0.0.1`. HTTP/2 frontends must terminate to HTTP/1.1. Secure Cookies have no localhost exception. Actual TLS/browser and four-OS verification are separate acceptance gates.

| Setting | Default | Setter |
|---|---|---|
| Body size | 1 MiB | `options(body_bytes, body_ms, handler_ms, shutdown_ms)` |
| Body deadline | 10 seconds | `options(...)` |
| Handler deadline | 2 seconds | `options(...)` |
| Shared verifier + authorizer deadline | 2 seconds | `security_timeout(options, milliseconds)` |
| Shutdown deadline | 10 seconds | `options(...)` |
| Connections / concurrent requests | 1024 / 256 | `capacity(options, connections, requests)` |
| Headers / next request wait | 10 seconds | `header_timeout(options, milliseconds)` |
| Response send deadline | 10 seconds | `send_timeout(options, milliseconds)` |
| Header buffer / count | 32 KiB / 100 | `header_limits(options, bytes, count)` |

The security deadline is one absolute budget shared by verifier and authorizer. `security_timeout` accepts positive milliseconds; zero or negative values cannot select an unlimited deadline. Body and handler keep their separate deadlines after authentication. An identity that expires earlier is checked again before entering the handler.

Every setter returns `Result[Options, Error]`. Connection and request limits control admission, not thread counts. Ctrl+C stops admission and waits for active connections. The server aborts and joins remaining connection tasks after the shutdown deadline. Already running blocking work cannot be forcibly stopped.

In Nagi 0.1.10, a handler that completes normally after its deadline returns 504, including when it did not yield. The deadline cannot forcibly stop synchronous work: the response waits for control to return, and completed state changes are not rolled back.

An oversized body is rejected with 413 and `Connection: close`, without calling the handler. In the 0.2.0 development version, after sending the response, the server discards at most 8 KiB of remaining input for the shorter of 100 ms and the body deadline, then closes the write side. It retains the connection slot during cleanup and never dispatches discarded input. Normal responses and idle keep-alive do not enter this cleanup path. It does not drain the whole remaining body: a deadline, byte limit or cancellation ends cleanup. Closing with unread data can still cause a TCP reset; receipt of the 413 is not guaranteed for every OS, client, or upload pattern.

Example:

```nagi
limits = try http.options(1048576, 10000, 2000, 10000)
limits = try http.capacity(limits, 2048, 512)
limits = try http.header_limits(limits, 32768, 100)
limits = try http.authority(limits, "https://localhost", ["localhost:8080", "127.0.0.1:8080"], 2, 256)
return await http.serve(app, 8080, limits)
```

## Implementation

The API is implemented in the [standard module registry](../../compiler/src/stdlib.rs) and [HTTP runtime](../../runtime/src/http_server.rs). Hyper handles HTTP/1 transport, Tokio manages async work and connection tasks, matchit matches paths, and Serde converts JSON. The implementation uses Axum HTTP types, but its routes do not run through Axum Router.

[HTTP runtime tests](../../runtime/src/http_server/tests.rs) use real sockets to check responses, limits, deadlines, and connection closure. [Rust integration](modules-and-rust.md) can provide a custom server, but these limits and failure handling do not apply to it automatically.
