# HTTP API reference

These APIs are included in Nagi 0.2.0. See [migration](migration-0.2.0.md) and [authentication and authorization](security.md).

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
| `response_context(view(request))` | `view[ResponseContext]` |
| `empty(context, status)` | `Result[Response, Error]` |
| `text(context, status, text)` | `Result[Response, Error]` |
| `bytes(context, status, body)` | `Result[Response, Error]` |
| `json[T](context, status, value)` | `Result[Response, Error]` |
| `html_response(context, status, document)` | `Result[Response, Error]` |
| `append_header(response, name, value: view[bytes])` | `Result[Response, Error]` |
| `append_header_text(response, name, value: view[str])` | `Result[Response, Error]` |

`context` is borrowed from the current Request with `response_context`. It is an opaque response-budget context, not an authentication proof. It cannot outlive its Request or be constructed with a default. The factories return an owned response with a retained budget claim; allocating or serializing beyond the explicit limits returns `Err`. Use `try` or return the factory Result directly. Handle these allocation/budget failures separately from the handler's business error.

The [0.2.0 release CI](https://github.com/disnana/Nagi/actions/runs/39422234923) passed all four OSes and browser acceptance. The budgets below bound the stated resources; they do not guarantee total process memory, an SLA, or universal recovery from allocation failure.

`text` and `bytes` copy borrowed input into a body owned by the Response. `json` borrows its value, so it does not move the original. String literals can be passed directly to view parameters; borrow variables with `view(value)`.

Header append consumes and returns the response. It preserves duplicate nonreserved headers such as X-Trace. It rejects Content-Length, Transfer-Encoding, Content-Type, Set-Cookie, WWW-Authenticate, Cache-Control, Vary, CORS, CSP, nosniff, and other security-managed headers. Cookie/Session issuance uses the dedicated Session API; header append cannot issue a cookie. HEAD omits the body while retaining its GET-equivalent length. 204, 205, and 304 omit the body. Authority-form CONNECT returns 400 at the startup-configuration gate below. If an origin-form CONNECT handler returns success, tunnels remain unsupported: the server returns 501 and closes the connection.

## App and routes

The following is the route-registration fragment, not a complete SF07 startup program. Add explicit response/rate budgets and a rate policy for every route as described below.

```nagi
app = http.app[State, AuthError](state, map_error)
app = try http.route(app, http.Method.GET, "/", http.public_policy[State](), handle)
app = try http.route_mapped(app, http.Method.POST, "/submit", http.public_policy[State](), submit, map_submit_error)
```

- `app[S, E](state, mapper)` owns the state and uses `fn(view[ResponseContext], E) -> Result[Response, Error]` as its default error mapper.
- `app_default[S](state)` uses the default mapper for `Error`.
- Policy is created by `public_policy[S]()`, `authenticated_policy[S](verifier)` or `authorized_policy[S,P](verifier, authorizer)`. Its output A is respectively unit, AuthScope or Grant[P]. The compiler checks S and A against the handler; old arities receive migration diagnostics.
- A handler has the signature `async def handle(request: Request, state: shared[S], access: A) -> Result[Response, E]`. The state itself is not copied per request. Rust requires bounds such as `Send + Sync` on state and `Send` on the handler future; build performs the final validation.
- `route_mapped` accepts a handler with its own error type and a matching mapper.
- GET routes automatically accept HEAD unless an explicit HEAD route takes precedence. A different method on an existing path returns 405 with Allow.

Mappers receive the borrowed response context explicitly and handle application failures. A mapper that itself returns `Err` uses the finite built-in fallback path; exhausting that independent fallback closes the connection without repeated allocation. CORS and managed headers remain on responses that can be produced. This does not guarantee a response after every failure or universal OOM recovery.

Mappers handle application failures. The server handles malformed HTTP, limits, and deadlines. Avoid exposing database error details or credentials in responses.

In Nagi 0.1.10, an unwinding panic in a handler or mapper before the response starts returns a generic 500 and closes that connection. Catching a panic does not roll back shared state or database updates. Return ordinary failures through Result. Recovery is not guaranteed for `panic=abort`, process termination such as OOM, a second panic during unwinding, custom Rust cleanup, or failures after the response starts.

When an error response needs a request ID, retain the validated ID in the handler and move it into a custom error only on failure. The [quote API example](../../test-nagi-code/application-examples/quote-api/README.en.md) passes its ID to a shared mapper this way.

## Explicit SF07 budgets and rate policy

Response and rate budgets have no implicit defaults. `default_options()` retains the existing transport settings but does not supply the new budgets. Configure both budgets and every route's pre-authentication rate policy before starting the listener. Missing or inconsistent configuration is rejected before listening.

| Configuration operation | Contract |
|---|---|
| `response_limits(body_bytes, user_header_bytes, user_header_count, managed_header_bytes, managed_header_count, serialization_writes, live_responses, retained_bytes, fallback_responses, fallback_retained_bytes, fallback_managed_header_bytes, fallback_managed_header_count)` | Twelve explicit `i64` inputs; returns `Result[ResponseLimits, Error]`. Ordinary and fallback budgets are separate |
| `with_response_limits(options, limits)` | Moves Options and ResponseLimits; returns `Result[Options, Error]` |
| `rate_limits(rules, domains, subject_slots, retained_bytes, request_work)` | Five explicit `i64` inputs; returns `Result[RateLimits, Error]`. Finite count, retained-capacity and work checks; no fixed public limit of 4096 |
| `with_rate_limits(options, limits)` | Moves Options and RateLimits; returns `Result[Options, Error]` |
| `rate_policy(burst, refill_ms)` | Returns `Result[RatePolicy, Error]` for a pre-authentication route bucket |
| `rate_subject(policy, burst, refill_ms, subject_slots, idle_ms)` | Consumes the policy and adds a finite verified-subject bucket; returns `Result[RatePolicy, Error]` |
| `rate_route(app, method, path, policy)` | Moves the App and policy, preserving the App's state/error types; returns `Result[App[S, E], Error]` |

`burst` is both the initial token count and its maximum. `refill_ms` is the interval for replenishing **one** token. For example, `rate_policy(10, 1000)` initially admits ten requests, then replenishes one token per second; it does not replenish ten per second. Subject buckets follow the same rule. A subject slot is reclaimed only after full refill and its explicit idle condition are both satisfied. Sample values are finite illustrative settings, not production sizing recommendations or latency SLAs.

The adopted consumption order is entry checks → route bucket → authentication/CSRF/body checks → verified-subject bucket → handler/Session issuance or update. A route 429 can precede 401/403. Errors and cancellation do not refund tokens. Normal preflight does not consume a route token, but remains subject to entry capacity, concurrency and deadlines. Subject buckets require a profile that supplies a verified subject. This is process-local control with sharing limited to one route; it is not an aggregate limit across routes, processes or machines.

Token shortage maps to 429; table/quota capacity exhaustion or closing/clock faults map to 503. When ordinary response capacity is exhausted and the mapper also returns `Busy`, the independent fallback returns 503. Other internal or mapper failures remain an opaque 500. Existing security/deadline rejections keep their priority. A finite fallback is attempted only within its independent allowance; if that allowance is exhausted, the transport closes.

Arithmetically accepted settings do not guarantee total process memory or a latency SLA. See the measured small configuration below and choose budgets for the deployment.

## Measured small configuration

The public-API fixture [sf07_small_profile.rs](../../runtime/tests/sf07_small_profile.rs) uses eight connection/request slots, four routes, and these explicit budgets. This is a tested example, not an implicit default or a production sizing recommendation.

| Budget | Explicit values |
|---|---|
| Response | body 4096 bytes; user headers 4096 bytes / 16 entries; managed headers 4096 bytes / 32 entries; serialization work 4096; 8 live responses; ordinary retained allowance 2 MiB |
| Fallback | 8 responses; independent retained allowance 2 MiB; managed headers 4096 bytes / 32 entries |
| Rate | 8 rules, 8 domains, 1 subject slot, retained allowance 1 MiB, request work 65536 |
| Routes | `/ordinary`, `/headers`, `/healthy`: burst 1000 / refill 1000 ms; `/limited`: burst 1 / refill 86400000 ms |

On GitHub-hosted runners with Rust 1.99.1 and locked dependencies, the unoptimized test profile completed four warmups followed by 32 sequential loopback HTTP/1 requests. These timings include the client, transport, parsing and current-thread Tokio tasks. CPU models were not recorded, so this is not a controlled cross-platform comparison.

| Runner / architecture | Median (ms) | p95 (ms) | Calling-thread allocated bytes over 32 requests |
|---|---:|---:|---:|
| Linux x86_64 | 0.450 | 0.489 | 1627520 |
| Windows x86_64 | 0.681 | 0.737 | 1641088 |
| macOS ARM64 | 0.183 | 0.212 | 1657152 |
| macOS x86_64 | 0.487 | 0.553 | 1657216 |

Evidence: [Linux/macOS run](https://github.com/disnana/Nagi/actions/runs/39137680565) at `2b1563d`, and [Windows followup](https://github.com/disnana/Nagi/actions/runs/39138107960) at `967ad48e`. The Rust fixture and production code are identical between these snapshots; the followup corrects two CLI-test newline expectations. The initial Windows run failed those tests before native execution. All four scoped measurements also checked header-heavy output, rate 429 followed by a healthy route, and joined shutdown. They do not replace the final integrated-HEAD checks.

Allocation counters are cumulative calling-thread events, including the loopback client; deallocations may include earlier allocations. They do not measure physical retained high-water or process RSS. Existing deterministic quota/native-owner tests check charged boundaries and final-owner release separately. Neither these observations nor accepted logical budgets establish a latency SLA or universal OOM recovery.

## Limits and shutdown

Nagi 0.2.0 requires checked authority configuration for `serve` and Rust `serve_listener`. Bare `default_options()`/`options(...)` fails startup.

| Startup setting | Contract |
|---|---|
| `authority(options, external_origin, authorities: List[str], entry_limit, byte_limit)` | Checks an HTTPS origin and a finite accepted wire-authority set; returns `Result[Options, Error]`. Limits are positive finite counts/bytes. Empty sets, canonical duplicates, invalid origin/authority and exceeded limits fail |
| `trusted_proxy(options, peer_ips: List[str])` | Requires configured authority; sets a real TCP peer IP ACL under the same finite count/byte limits. Empty, invalid, unspecified/multicast and duplicate canonical peers fail. Exact IPs, not CIDR/DNS |

Origin is only `https://host[:port]`, without path/userinfo/query/fragment. Omitted and explicit wire ports such as `:443` are separate entries. DNS ASCII case is normalized; IPs use the standard IP parser. IPv6 needs brackets. A host made only of digits and dots is rejected unless the standard IPv4 parser accepts it. Ports are canonical decimal 1–65535; leading zeros, userinfo, `%`/zones, trailing dots, Unicode and comma lists are rejected. Configure pre-canonical ASCII IDNA A-labels; runtime performs no Unicode conversion or IDNA-equivalence guarantee. Proxy peers alone normalize IPv4-mapped IPv6 to IPv4 with standard IP types and reject resulting duplicates.

Reconfiguring authority or overwriting a proxy ACL fails. Setters move Options; pass only a successful Result into startup. Failure never falls back to an earlier configuration.

Before every route, 404/405, body reception, verifier and proof issuance, the gate checks HTTP/1.1 origin-form and exactly one nonempty Host. Malformed/missing/duplicate/nonallowlisted authority, absolute-form/authority-form/`*`, and any Forwarded or X-Forwarded-* presence return 400; HTTP/1.0 returns 505. These runtime rejections use no-store and close. HTTP syntax rejected by Hyper before service invocation, including HTTP/2 prefaces, is delegated to its official parser and reaches no handler/proof. The standard listener is HTTP/1 only.

Direct/proxy profiles are fixed at startup. Proxies must strip Forwarded headers before backend transmission and use a registered internal or pass-through Host. Only the real peer returned by `accept` selects ACL admission; headers never choose trust or origin. A loopback ACL does not authenticate other processes on the same host. Host matching does not waive Origin/CSRF/CORS; these have separate contracts below.

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

The following startup fragment continues the two registered routes above (`/` and `/submit`). It includes both required SF07 budgets and both rate policies. The finite values illustrate configuration; they are not a measured sizing recommendation. Handler, state and mapper definitions are still required; for a complete buildable application, see the [HTTP authentication example](../../test-nagi-code/library-examples/http-auth/README.en.md).

```nagi
route_budget = try http.rate_policy(10, 1000)
app = try http.rate_route(app, http.Method.GET, "/", route_budget)
submit_budget = try http.rate_policy(10, 1000)
app = try http.rate_route(app, http.Method.POST, "/submit", submit_budget)
limits = try http.options(1048576, 10000, 2000, 10000)
limits = try http.capacity(limits, 8, 8)
limits = try http.header_limits(limits, 32768, 100)
limits = try http.authority(limits, "https://localhost", ["localhost:8080", "127.0.0.1:8080"], 2, 256)
response_budget = try http.response_limits(4096, 4096, 16, 4096, 32, 4096, 8, 2097152, 8, 2097152, 4096, 32)
limits = try http.with_response_limits(limits, response_budget)
rate_budget = try http.rate_limits(8, 8, 1, 1048576, 65536)
limits = try http.with_rate_limits(limits, rate_budget)
return await http.serve(app, 8080, limits)
```

## Implementation

The API is implemented in the [standard module registry](../../compiler/src/stdlib.rs) and [HTTP runtime](../../runtime/src/http_server.rs). Hyper handles HTTP/1 transport, Tokio manages async work and connection tasks, matchit matches paths, and Serde converts JSON. The implementation uses Axum HTTP types, but its routes do not run through Axum Router.

[HTTP runtime tests](../../runtime/src/http_server/tests.rs) use real sockets to check responses, limits, deadlines, and connection closure. [Rust integration](modules-and-rust.md) can provide a custom server, but these limits and failure handling do not apply to it automatically.

## Explicit CORS route metadata

`CorsPolicy` is opaque, owned configuration; it is not an authentication proof. `CorsCredentials.OMIT` omits Allow-Credentials; `ALLOW` emits `true`. Neither mode removes credentials or waives Policy/CSRF. Construct it with `cors_policy(origins: List[str], request_headers: List[str], credentials: CorsCredentials, max_age_seconds: i64, max_origins: i64, max_origin_bytes: i64, max_headers: i64, max_header_bytes: i64) -> Result[CorsPolicy, Error]`. All count/byte ceilings must be positive native sizes; lists may be empty. Max-age is explicitly 0..4294967295 seconds, and zero has no positive preflight lifetime. There is no server preflight cache.

`cors_route[S,E](app: App[S,E], method: Method, registered_path: view[str], cors: CorsPolicy) -> Result[App[S,E], Error]` consumes app/config and attaches once to that exact existing method/pattern record. Unknown record or repeated attachment fails. GET fallback HEAD inherits GET; explicit HEAD and OPTIONS are separate records. Omitted CORS denies cross-origin reads. Wildcard origins/headers and CORS methods `*`, CONNECT, TRACE and TRACK are rejected. CORS applies to ordinary and framework-managed routes alike.

```nagi
cors = try http.cors_policy(["https://app.example.invalid"], ["content-type"], http.CorsCredentials.ALLOW, 0, 2, 128, 2, 64)
app = try http.cors_route[State, Error](app, http.Method.GET, "/items/{id}", cors)
```

Origins use exact canonical http/https scheme, ASCII host and effective port. DNS case, default ports and equivalent IP spelling normalize; null, wildcard, userinfo, Unicode, trailing DNS dot, whitespace, percent, path/query/fragment and multiple Origins do not. Startup external security origins remain HTTPS-only. CORS grants reflect the configured canonical value, not raw request text.

OPTIONS with Access-Control-Request-Method or Access-Control-Request-Headers uses only target route metadata: success is empty 204, requested method/header allow metadata and explicit Max-Age, no-store, and no verifier/handler/proof/Session/DB work. Ordinary OPTIONS requires its own normal Policy. Unknown paths/methods retain 404/405 and no grant. Duplicate/malformed metadata fails 400; a valid unlisted origin/header or default-deny record fails 403. Admission and declared-body errors keep 503/413 precedence; preflight bodies fail 400 and close.

On actual requests an unlisted/malformed/missing Origin receives no CORS grant while normal authentication, CSRF, status and handler behavior remain. Allowlisted cross-origin preflight does not permit unsafe Session POST: the existing exact external HTTPS Origin and token binding still apply. The outer finalizer preserves Cookie delivery, no-store, CSP, nosniff, challenge and framing, and unions finite typed Vary dependencies even on selected early errors. Authenticated/Session 304 still becomes secret-free 500; public DTO 304 retains its status. CORS is included in 0.2.0 with Session CSRF. Release CI covers TLS/browser/proxy acceptance; deployment-specific TLS and proxy configuration still need verification.
