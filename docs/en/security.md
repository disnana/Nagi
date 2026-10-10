# Authentication and authorization (unreleased 0.2.0 SF01)

Authentication verifies who a caller is; authorization checks whether that caller may perform a particular operation on its actual target. SF01 provides explicit standard HTTP Policy, request-bound proofs and invalidation at request end. Durable Sessions, Cookie issuance, CSRF/CORS, cryptographic verifiers, typed HTML and outbound HTTP remain later steps. This does not establish completion of Security Foundation or retroactively change published 0.1.x.

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
    return ok(http.text(http.Status.OK, view(state.greeting)))
async def main() -> Result[unit, Error]:
    app = http.app_default[State](State(greeting="Hello, Nagi!"))
    app = try http.route(app, http.Method.GET, "/", http.public_policy[State](), hello)
    limits = try http.authority(http.default_options(), "https://localhost", ["localhost:8080", "127.0.0.1:8080"], 2, 256)
    return await http.serve(app, 8080, limits)
```

Save this as `server.nagi` in your working directory and run `nagic run server.nagi` there with the development compiler. In another terminal, `curl http://127.0.0.1:8080/` returns `Hello, Nagi!`. Stop with Ctrl+C. Choose another port if occupied. See [setup](getting-started.md) and the [HTTP reference](http-server.md) for installation, limits and diagnostics.

This checks a public response over plaintext loopback HTTP. An HTTPS origin setting does not add TLS or complete browser authentication, Secure Cookies or Origin/CSRF. Migrate TLS frontend and actual peer/proxy deployment through [authority configuration](http-server.md#limits-and-shutdown).

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

Read failures with `auth.kind(view(failure))` and `auth.message(view(failure))`; the message view borrows its Failure. An exhausted security budget returns 504, and an unwinding panic returns a generic 500. Business Result Err is handled by the app/route mapper and is not flattened into security Failure.

`security_timeout(options, positive_ms)` sets one absolute budget shared by verification and authorization. It is not reset at each wait. HEAD fallback inherits GET policy; explicit HEAD and OPTIONS also require Policy. 404/405 issue no proof and invoke no handler. CORS preflight remains unimplemented until SF03.

The finalizer owns Content-Type, nosniff and reserved framing/security headers. Raw HTML, Set-Cookie, CORS/CSP, arbitrary cache and authentication-challenge headers cannot bypass this boundary. There is no guarantee of recovery from panic=abort, OOM, forced stopping of non-yielding work, arbitrary Rust double panic, or rollback of external side effects. Custom Rust HTTP hosts outside standard HTTP remain an explicit trusted boundary and do not inherit Policy guarantees automatically.

[Migration](migration-0.2.0.md) · [Ownership](ownership.md) · [Task](task-handles.md) · [SQLite](sqlite-pool.md) · [Adopted contract](../internal/security-foundation/sf01-contract.md)

## Protected SQLite operations

Unreleased SF05 unifies the standard entry point around Query from a direct literal and actual values bound through Parameters. Query restricts the origin of SQL structure; it contains no Grant or tenant predicate.

A reviewed trusted adapter holds fixed SQL and ownership predicates. Bind the **actual subject and target** supplied by `Grant.submit` into a predicate such as `WHERE owner=? AND id=?`. Adding a Grant to an arbitrary query cannot prove target isolation. Mutable session/owner conditions need a predicate or recheck in the same Tx. Durable Session and generation management remain SF02 work and are not counted as SF05 guarantees.

A trusted Rust host waits for bounded queue capacity using Tx's opaque `reserve_exec`, then invokes `reservation.enqueue(Query, Parameters)` inside `Grant.submit`'s synchronous callback. Capacity reservation is not admission. Admission linearizes at single-use permit issuance under the gate. Revocation before issuance means zero native enqueues; invalidation after issuance leaves the operation admitted even before the callback synchronously enqueues. Later revocation or HTTP cancellation does not promise cancellation or rollback. The callback must synchronously send to the actual queue rather than return a Future that enqueues later.

DDL/bootstrap belongs to trusted management code separate from requests. Fixed Rust SQL factories and handwritten FromRow adapters require review. Do not reexport a dynamic string factory to requests and bypass the standard literal contract. [SQLite](sqlite-pool.md) · [Migration](migration-0.2.0.md) · [SF05 contract](../internal/security-foundation/sf05-contract.en.md)
