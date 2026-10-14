# HTTP client API (SF06)

This page describes an unreleased Nagi 0.2.0 development candidate. The API is not included in public Nagi 0.1.11. For server-side HTTP, see the [HTTP server API](http-server.md); for scope and sequencing, see the [SF06 implementation plan](../internal/security-foundation/implementation-plan.en.md).

## API

```nagi
import std.http.client as client
import std.auth as auth
```

`std.http.client` defines six resources and ten functions.

| Resource | Purpose |
|---|---|
| `ProtectedOperation[C, P, I]` | A fixed-purpose operation that requires a Grant |
| `PublicOperation[C, I]` | An explicitly registered public operation |
| `Response` | An owned final response |
| `Failure` | A classified failure |
| `FailureKind` | Failure classification |
| `Outcome` | Whether HTTP submission began |

| Function | Signature |
|---|---|
| `execute` | `execute[C, P, I](view[ProtectedOperation[C, P, I]], auth.Grant[P], I) -> Future[Result[Response, Failure]]` |
| `execute_public` | `execute_public[C, I](view[PublicOperation[C, I]], I) -> Future[Result[Response, Failure]]` |
| `status` | `(view[Response]) -> i64` |
| `headers` | `(view[Response], view[str]) -> Result[List[view[bytes]], Error]` |
| `body` | `(view[Response]) -> view[bytes]` |
| `take_body` | `(Response) -> bytes` |
| `kind` | `(view[Failure]) -> FailureKind` |
| `auth_kind` | `(view[Failure]) -> Option[auth.FailureKind]` |
| `outcome` | `(view[Failure]) -> Outcome` |
| `message` | `(view[Failure]) -> view[str]` |

These call forms are exercised by the High, saved Low, and handwritten Low compiler fixtures. They show the caller-side types; they are not a standalone application or host-startup example.

```nagi
class State:
    n: i64
enum Permission:
    Allowed
async def protected_call(operation: view[client.ProtectedOperation[State, Permission, bytes]], grant: auth.Grant[Permission], input: bytes) -> Result[client.Response, client.Failure]:
    return await client.execute(operation, grant, input)
async def public_call(operation: view[client.PublicOperation[State, bytes]], input: bytes) -> Result[client.Response, client.Failure]:
    return await client.execute_public(operation, input)
```

## Registration and authority boundary

Operation handles are typed, opaque, and immutable after registration. `C` is retained shared host state, `P` is a nominal permission type, and `I` is an owned typed business input. Nagi has no constructor for a client or operation, and the standard API supplies no default destinations or numeric limits. A trusted Rust host must register fixed operations at startup, retain an independent `Owner`, and await actual shutdown and joins. A supported production profile, accepted canonical/address forms, defaults, and numeric limits have not been selected. Do not use test-only values as production settings.

`execute` consumes the actual `Grant[P]` for protected work. The reviewed host adapter binds the subject and target from that Grant into the actual request. Callers cannot supply an arbitrary URL, an arbitrary method/path combination, a raw header map, or a Cookie or cookie jar. Headers are limited to fixed registration-time `Accept` and `Content-Type` declarations and an optional Bearer credential bound to a protected operation. A public operation is registered under a separate type and accepts no Grant or credential. The checker rejects passing a `ProtectedOperation` to `execute_public` to bypass authorization.

These types do not verify the trusted Rust adapter or the remote service's authentication. The adapter must use the actual subject and target, and the remote service must authenticate and authorize requests independently.

Operation handles, `Response`, and `Failure` are owned values without Copy or Clone. Read them with `view`; use the existing explicit `move` when transferring an owned local into another local. Adding an owner of a `shared` value is different from independently copying its payload, and `take_body` cannot extract the body from a shared `Response`. `FailureKind` and `Outcome` are Copy values. A `Grant` is a short-lived authorization proof, distinct from the operation handle, and cannot escape into another Task or an input wrapper.

Dropping a handle does not confirm shutdown of the separately retained host Owner. The host closes new admission and awaits actual termination and joins of cancelled work. Dropping an execution Future is different from confirming its termination. Cancellation does not roll back a submitted request or remote business effects. Unexpected panic or task faults are not universally converted to ordinary business `Failure`; existing fault and cleanup contracts remain. Arbitrary Rust code cannot be forcibly stopped, and universal OOM recovery is not guaranteed.

## Responses and failures

A completed response is returned as an owned `Response`, including its body, only after all actual child work has completed and joined and the final publication check has passed. The response does not retain a socket or running job. `status` returns the numeric HTTP status. Ordinary received statuses such as 403 and 302 are `Ok(Response)`, not `Err(Failure)`. The client does not follow redirects or automatically retransmit an HTTP request. Only initial response headers are exposed; trailers are validated within their limits but are not part of the public API.

`headers` looks up names case-insensitively and preserves duplicate values. It borrows the bytes from `Response`, not from the lookup name, without copying the bytes. The returned List itself allocates. An invalid header name returns the accessor's `Error`. You cannot move `Response` while a returned view remains live. `body` borrows its bytes; `take_body` consumes the `Response` and transfers the original Vec buffer without another payload copy. This does not promise an allocation-free or cost-free operation overall. Remote headers and bodies remain untrusted data; successful receipt is not successful business validation.

The 14 `FailureKind` values are `INVALID_INPUT`, `SECURITY`, `DESTINATION_DENIED`, `CAPACITY`, `CLOSING`, `DEADLINE`, `DNS`, `TCP`, `TLS`, `PROTOCOL`, `RESPONSE_LIMIT`, `UNSUPPORTED_ENCODING`, `UNAVAILABLE`, and `INTERNAL`. A protected-admission authentication or authorization failure is classified as `SECURITY`, and `auth_kind` preserves its original `auth.FailureKind`; other failures return `None`. `message` is fixed, redacted text. Accessor `Error` values are separate from request `Failure` values.

`Outcome` has two values: `REQUEST_NOT_STARTED` and `UNKNOWN`. A failure before synchronous HTTP submission begins has `REQUEST_NOT_STARTED`. A failure after submission begins has `UNKNOWN`, because the client cannot determine whether the remote service received the request. DNS or TCP activity may have occurred before submission begins. `UNKNOWN` does not permit a retry.

## Connection behavior and validation status

The client checks all A and AAAA candidates. It refuses a lookup if any candidate is unsafe or either address-family lookup fails. It tries permitted candidates sequentially under the original deadline, with a finite attempt count, and advances only for eligible TCP connection failures. TTL 0 is usable for that single resolution operation and is never cached. TLS certificate verification remains enabled and response bodies are bounded. Compressed responses are unsupported in 0.2.0; support is planned for 0.2.1 (#124).

| Validation | Result and scope |
|---|---|
| High, saved Low, and handwritten Low forms across two fixture groups | PASS. These validate compiler calls and API projections, not arbitrary production host setup |
| Positive TLS native fixture | PASS, using a `cfg(test)`-only destination-admission seam. This does not establish a positive production connection profile |
| Production loopback denial | PASS. A lookup returns loopback and AAAA completes successfully with NoData; denial before TCP is confirmed after actual task shutdown |
| Public API connection CI on four OSes | Pending. Earlier private-head CI results are not evidence for this candidate |

This candidate is not a released feature and has not passed four-OS production-connection acceptance.
