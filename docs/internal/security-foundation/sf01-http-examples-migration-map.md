# SF01 HTTP sample migration map

SF01 follows the settled D1–D3 contract: standard HTTP routes require an explicit policy, request grants come from the standard dispatcher, and the retired decorator/global-server/raw-HTML/Principal paths do not coexist as compatibility modes. Typed HTML and SQL hardening remain later work; HTTP policy downgrade closes in SF01.

## Standard HTTP examples

| Source | Migration and retained intent | New boundary or limitation |
|---|---|---|
| [`runtime/examples/http_stdlib_baseline.rs`](../../../runtime/examples/http_stdlib_baseline.rs) | The small JSON, echo, and health routes now register `public_policy()` and handlers accept the unit authority argument. | Benchmark setup uses the SF01 dispatcher. Timing from a different runtime build or older route contract must not be described as same-condition data. |
| [`examples/crud.nagi`](../../../examples/crud.nagi) | Explicit `App` routes retain CRUD, JSON, path/query handling, and database behavior. Health is registered as a public route. | `/stream` is a finite bytes response; this is not general streaming support. There is no WebSocket route. HTML remains unavailable until typed HTML is implemented. |
| [`examples/tutorial/http.nagi`](../../../examples/tutorial/http.nagi) | Explicit public routes retain greeting, dynamic path, and JSON echo behavior. | The root response is ordinary text. It does not pass HTML markup as text while typed HTML is pending. |
| [`test-nagi-code/inventory.nagi`](../../../test-nagi-code/inventory.nagi), [`result-api/server.nagi`](../../../test-nagi-code/result-api/server.nagi), [`application-examples/device-settings/main.nagi`](../../../test-nagi-code/application-examples/device-settings/main.nagi), [`byte-inspector/main.nagi`](../../../test-nagi-code/application-examples/byte-inspector/main.nagi), and [`quote-api/main.nagi`](../../../test-nagi-code/application-examples/quote-api/main.nagi) | Legacy decorators/global serving are replaced by explicit routes and public policies while the existing API operations, typed payloads, DB behavior, and business error mapping remain. | A route's public access is visible in registration; no implicit endpoint or name-based authorization is inferred. |
| [`web-demo/tasks.nagi`](../../../test-nagi-code/web-demo/tasks.nagi) and [`web-demo/smoke_api.py`](../../../test-nagi-code/web-demo/smoke_api.py) | CRUD, aggregation, validation, and DB behavior remain covered by HTTP smoke checks. The root check now expects the documented plain-text API landing response. | The former raw-HTML browser UI is unserved while SF04 typed HTML is pending. `index.html` remains an unconnected reference, not an endpoint. |
| [`library-examples/supervised-service/main.nagi`](../../../test-nagi-code/library-examples/supervised-service/main.nagi) and saved [`main.low`](../../../test-nagi-code/library-examples/supervised-service/main.low) | Counter read, update, and shutdown behavior remains; each route now carries an explicit public policy. | No hidden `/health` route is added. The sample remains unauthenticated and loopback-only. |

## Request-bound authentication sample

[`application-examples/auth-boundary/main.nagi`](../../../test-nagi-code/application-examples/auth-boundary/main.nagi), [`native.rs`](../../../test-nagi-code/application-examples/auth-boundary/native.rs), and [`smoke.py`](../../../test-nagi-code/application-examples/auth-boundary/smoke.py) replace the previous Axum authorization glue and Principal factory with the standard dispatcher, `authenticated_policy`, `AuthScope`, and a target-bound grant. The verifier receives the request and app state and produces a finite-lived `VerifiedIdentity`. The named Nagi or Rust policy checks the database row before `Grant::from_authorized(scope, document_id)`. The adapter waits for bounded semaphore capacity and consumes the grant synchronously with its target bound into the SQLite command; the query rechecks current owner and blocked state.

The smoke test keeps the Alice/Bob ownership cases, blocked document denial, malformed and typed JSON distinctions, missing-document behavior, bounded body behavior, panic recovery, timeout, and listener shutdown. Policy failures use the stable secret-free response contract: missing and incorrect credentials both return 401 `invalid credential` with `WWW-Authenticate: Bearer`; expiry returns `authority expired`; malformed/duplicate Authorization headers return 400 `invalid security request`. These are dispatcher failures; handler-owned business errors retain their own mapping. Demo credentials are not production token validation.

The strings `demo-alice` and `demo-bob` are test credentials, not built-in usernames or implicit identity rules. Subject mapping stays inside the verifier. No handler or dispatcher special-cases an account because of its name.

[`library-examples/http-auth/main.nagi`](../../../test-nagi-code/library-examples/http-auth/main.nagi) uses the same standard policy boundary. Its `native.rs` verifier is only a configured demo-credential comparison. Its public `/restricted` route deliberately returns a business 403, so it does not exercise policy rejection or receive the policy Bearer challenge.

The separate [`application-examples/axum-service`](../../../test-nagi-code/application-examples/axum-service) and [`library-examples/custom-http`](../../../test-nagi-code/library-examples/custom-http) remain custom host integrations. Their trusted Rust hosts are independent boundaries, not a fallback or a way to bypass the standard dispatcher for policy-protected standard routes.

## HTTP integration expectations

[`tests/http_integration.py`](../../../tests/http_integration.py) still checks CRUD, JSON, path/query behavior, HEAD, status, body framing, capacity, timeout, panic and shutdown. Its bounded `/stream` response checks a finite bytes body only; `/ws` is expected to be absent (404), so no general stream or WebSocket success contract is implied. [`tests/route_integration.py`](../../../tests/route_integration.py) keeps dynamic paths and their original business outcomes. [`tests/http_stdlib_integration.py`](../../../tests/http_stdlib_integration.py) now selects an explicit public or authenticated policy, checks policy-owned 401 responses, verifies that a managed response header such as `Set-Cookie` is rejected through the handler mapper, and preserves duplicate nonreserved `X-Trace` headers.

The HTTP route API has no source-name heuristic for reserved spellings. A user declaration named `serve` retains ordinary name-shadowing behavior; accepting or rejecting an endpoint is based on route registration and policy values, never on recognizing a same-named user function as the standard server.

## Custom Axum benchmark reference

[`runtime/examples/axum_baseline.rs`](../../../runtime/examples/axum_baseline.rs) is a benchmark-only trusted custom host and uses `axum::serve(listener, router)` directly after `rt::serve` was retired. This example binds loopback and contains only `/small` and `/echo`; it does not add a health endpoint. Direct Axum serving does not reproduce the old wrapper's bind behavior, hidden endpoints, request bounds, panic framing, or lifecycle policy. Measurements from the old wrapper and this direct host are not evidence of an SF01 same-condition comparison.

## Accepted removals and unimplemented capabilities

The prior hidden health, chunked-stream, and WebSocket behavior is retired as recorded in [`sf01-legacy-transport-map.md`](sf01-legacy-transport-map.md). Health checks are explicit application routes. Finite byte responses are supported; arbitrary streaming is not promised. WebSocket support is absent. The former raw HTML success sample is removed until SF04 typed HTML has a safe response type; do not mark markup as plain text to retain that success path. SF05 SQL typed work is later, while HTTP downgrade and middleware bypass are rejected in SF01.
