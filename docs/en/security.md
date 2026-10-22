# Authentication and authorization (0.2.0)

Authentication verifies who a caller is; authorization checks whether that caller may perform an operation on its actual target. Nagi 0.2.0 includes explicit standard HTTP Policy, request-bound proofs and invalidation at request end, durable Sessions/Cookies, Session CSRF, CORS, an Ed25519 verifier, a typed HTML subset and outbound HTTP. It was [released on 2026-10-20](https://github.com/disnana/Nagi/releases/tag/nagi-v0.2.0), after [four-OS and browser acceptance](https://github.com/disnana/Nagi/actions/runs/39422234923). Arbitrary active HTML is unsupported. These APIs do not retroactively change published 0.1.x.


## Tokio timer dependency update (unreleased)

The development runtime now requires Tokio `>=1.54.1,<2`. [Tokio #8692](https://github.com/tokio-rs/tokio/pull/8692), released in [1.54.1](https://github.com/tokio-rs/tokio/releases/tag/tokio-1.54.1), restores synchronization before timer entries are dropped; the reported 1.54.0 regression can cause a data race and use-after-free during concurrent interval polling. Nagi's previously tracked locks resolved 1.53.1/1.53.2, not 1.54.0. Remote exploitability in Nagi has not been established.

Generated applications have their own `Cargo.lock`; changing the repository lock does not update them or previously compiled executables. Use the updated runtime and rerun the normal `nagic build`/`run` command with the same source, project, output and profile. Cargo rechecks runtime dependencies even when generated Rust is reused. Verify the actual resolved Tokio version with `cargo tree --manifest-path <output>/Cargo.toml -i tokio`, rebuild every deployed executable and deploy the new artifact. Keep the old successful artifact if dependency resolution or build fails; do not delete locks or caches as an update procedure. A project explicitly pinning Tokio 1.54.0 must update its requirement first. Published Nagi 0.2.0 distributions and existing binaries are not changed by this unreleased source update.

## Choose a policy for every route

| Policy | Handler third argument | Purpose |
|---|---|---|
| `http.public_policy[S]()` | `unit` | Explicit public route; no proof is issued |
| `http.authenticated_policy[S](verify)` | `auth.AuthScope` | Require a subject verified until a finite deadline |
| `http.authorized_policy[S,P](verify, authorize)` | `auth.Grant[P]` | Require nominal permission P bound to an actual i64 target |

```nagi
import std.http.server as http
class State:
    greeting: str
async def hello(request: http.Request, state: shared[State], access: unit) -> Result[http.Response, Error]:
    context = http.response_context(view(request))
    return http.text(context, http.Status.OK, view(state.greeting))
```

This shows the handler shape only; it is not a runnable server by itself. Nagi 0.2.0 requires explicit response and rate budgets plus a rate policy for every route before startup. No unverified values are recommended here. See the [HTTP API reference](http-server.md#explicit-sf07-budgets-and-rate-policy) for the requirements.

A complete loopback server still needs a TLS frontend for HTTPS; an HTTPS origin setting does not add TLS or complete browser authentication, Secure Cookies or Origin/CSRF. See [authority configuration](http-server.md#limits-and-shutdown) for peer and proxy deployment.

Policy is nonCopy/nonshared configuration moved into a route; create another policy for another registration. The checker matches Policy state/output against the three handler arguments. Public policy cannot connect a proof-requiring handler. It does not prove that choosing public is appropriate for the business or that verifier/authorizer logic is correct.

Policy, VerifiedIdentity and auth.Failure are known nonClone/nonshared resources. Wrapping them in Option/List/classes does not allow copying or sharing. Copying a function pointer does not copy its return value. Copy-value assignment and the Clone capability required by explicit copy are separate. Final Rust build still checks user-provided Clone adapters for user classes.

## Verifiers and authorizers

Use named async functions or their local aliases. A verifier has `(http.Request, shared[S]) -> Result[auth.VerifiedIdentity, auth.Failure]`; an authorizer has `(auth.AuthScope, http.Request, shared[S]) -> Result[auth.Grant[P], auth.Failure]`. They see a bounded method/path/query/header snapshot with an empty body. The handler receives the original Request after body limits.

SF01 recognizes exactly one Authorization Bearer header. Missing or invalid credentials return 401; duplicate headers or a Cookie combined with Bearer return 400, without invoking the handler. Public policy does not authenticate. Fixed-credential fixtures are not a production login/session/browser solution.

A trusted Rust verifier checks cryptography, audience, expiry and revocation before returning `VerifiedIdentity::from_verified(subject, absolute_expiry)`. This is not a proof and has no Nagi factory. Only the dispatcher issues the private request lease and binds an AuthScope. A trusted authorizer checks P's policy and actual target before consuming the scope through `Grant::from_authorized(scope, target)`. See the [complete authorization example](../../test-nagi-code/application-examples/auth-boundary/README.en.md); its fixture credentials do not implement a production verifier.

## Ownership and lifetime

AuthScope and Grant are opaque: construction, Copy/Clone, JSON decoding, shared use, and class/enum fields are rejected. Same-task owned arguments/returns, local Option/Result and async delegation are allowed. Task capture/results, Actor transfer and transferring a Future capturing a proof violate SameTask. `move` transfers ownership without extending expiry. The i64 from `auth.subject(view(scope))` is an ID, not a proof.

The lease expires at the minimum of finite security/body/handler budgets and the verified identity's absolute deadline. Normal return, business Err, panic, timeout, cancellation, shutdown and unpolled/Pending Future Drop invalidate the dispatcher owner. Proofs retained by Rust cannot admit another protected operation. Synchronous Drop does not confirm completion or rollback of arbitrary native work.

A protected adapter first obtains bounded native capacity. Reserving capacity is not admission. `grant.submit(reservation, callback)` checks current time and active state under the same short gate as invalidation and issues a single private execution permit. The callback synchronously enqueues using its bound subject/target/reservation. Do not await inside the gate or return a deferred Future instead of submitting. Correctness inside arbitrary Rust remains a trusted-adapter obligation. Later invalidation does not cancel already admitted I/O.

## Failures and HTTP boundaries

| auth.FailureKind | HTTP | Stable message |
|---|---|---|
| `INVALID_CREDENTIAL` | 401 + Bearer challenge | `invalid credential` |
| `DENIED` | 403 | `permission denied` |
| `EXPIRED` | 401 + Bearer challenge | `authority expired` |
| `INVALID_REQUEST` | 400 | `invalid security request` |
| `UNAVAILABLE` | 503 | `security service unavailable` |
| `INTERNAL` | 500 | `security service failure` |

Read failures with `auth.kind(view(failure))` and `auth.message(view(failure))`; the message view borrows its Failure. An exhausted security budget returns 504, and an unwinding panic returns a generic 500. Business Result Err is handled by the app/route mapper and is not flattened into security Failure. A mapper receives a request-borrowed `ResponseContext` and returns `Result[http.Response, Error]`; response-factory errors propagate as that Error.

`security_timeout(options, positive_ms)` sets one absolute budget shared by verification and authorization. It is not reset at each wait. HEAD fallback inherits GET policy; explicit HEAD and OPTIONS also require Policy. 404/405 issue no proof and invoke no handler. The CORS layer adds metadata-only preflight; see [HTTP CORS](http-server.md#explicit-cors-route-metadata). It does not waive authentication or CSRF.

The finalizer owns Content-Type, nosniff and reserved framing/security headers. Raw HTML, Set-Cookie, CORS/CSP, arbitrary cache and authentication-challenge headers cannot bypass this boundary. There is no guarantee of recovery from panic=abort, OOM, forced stopping of non-yielding work, arbitrary Rust double panic, or rollback of external side effects. Custom Rust HTTP hosts outside standard HTTP remain an explicit trusted boundary and do not inherit Policy guarantees automatically.

[Migration](migration-0.2.0.md) · [Ownership](ownership.md) · [Task](task-handles.md) · [SQLite](sqlite-pool.md) · [Adopted contract](../internal/security-foundation/sf01-contract.md)

## Protected SQLite operations

0.2.0 SF05 unifies the standard entry point around Query from a direct literal and actual values bound through Parameters. Query restricts the origin of SQL structure; it contains no Grant or tenant predicate.

A reviewed trusted adapter holds fixed SQL and ownership predicates. Bind the **actual subject and target** supplied by `Grant.submit` into a predicate such as `WHERE owner=? AND id=?`. Adding a Grant to an arbitrary query cannot prove target isolation. Mutable session/owner conditions need a predicate or recheck in the same Tx. Durable Session and generation management are included in 0.2.0 SF02, but it is not an SF05 guarantee and does not imply atomic business commits.

A trusted Rust host waits for bounded queue capacity using Tx's opaque `reserve_exec`, then invokes `reservation.enqueue(Query, Parameters)` inside `Grant.submit`'s synchronous callback. Capacity reservation is not admission. Admission linearizes at single-use permit issuance under the gate. Revocation before issuance means zero native enqueues; invalidation after issuance leaves the operation admitted even before the callback synchronously enqueues. Later revocation or HTTP cancellation does not promise cancellation or rollback. The callback must synchronously send to the actual queue rather than return a Future that enqueues later.

DDL/bootstrap belongs to trusted management code separate from requests. Fixed Rust SQL factories and handwritten FromRow adapters require review. Do not reexport a dynamic string factory to requests and bypass the standard literal contract. [SQLite](sqlite-pool.md) · [Migration](migration-0.2.0.md) · [SF05 contract](../internal/security-foundation/sf05-contract.en.md)

## Session policy and CSRF

The standard runtime rejects registration of unsafe methods using `session_authenticated_policy` or `session_authorized_policy` without a CSRF guard. Only GET, HEAD and OPTIONS are accepted. POST, PUT, PATCH, DELETE and every other method are rejected with an Invalid `Error`. This applies to both `route` and `route_mapped`; `serve` and `serve_listener` also recheck every route before binding or accepting connections. Ordinary Bearer policies continue to reject every Cookie. There is no exemption boolean or authentication-source fallback.

Nagi 0.2.0 provides `std.auth.csrf` (`Options`, `CsrfStore`, `Failure`; `options`, async `open`, explicit `clone_store`, `kind`, `message`, `outcome`) and `std.http.server.session_csrf_tokens[S, E]` / `session_csrf_policy[S, A]`. `open` borrows the actual Session store and checked HTTP options. Both HTTP operations move the CSRF handle; `session_csrf_policy` seals only an unguarded policy for the same actual Session store. The runtime checks that identity and rejects a mismatched store or a second guard. The compiler checks nominal types, move/borrow and opaque construction, but does not statically prove store identity. Plain unsafe Session registration remains rejected; the authenticated token endpoint and guarded unsafe route are separate closed paths.

B1 guards unsafe ambient-cookie requests with exact configured Origin, a bounded token ledger and a Session-generation checkpoint after the body. Token delivery uses authenticated GET or explicit HEAD with no-store finalization. The scope is Session-backed CSRF. CORS policy/preflight and public login are included; release CI covers real TLS/browser/proxy Session flows. Deployment still requires its own trusted TLS frontend and correct proxy configuration. The checker does not prove that GET/HEAD/OPTIONS handlers have no side effects. Do not move mutations to GET to bypass the restriction. Cancellation does not guarantee rollback of admitted DB operations or the later business commit.
