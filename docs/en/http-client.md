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

Operation handles are typed, opaque, and immutable after registration. `C` is retained shared host state, `P` is a nominal permission type, and `I` is an owned typed business input. Nagi has no constructor for a client or operation. A trusted Rust host explicitly registers fixed operations and finite budgets at startup, retains an independent `Owner`, and awaits actual shutdown and joins. Budgets are configured once at startup rather than on every call; there are no implicit defaults.

### Startup profile and explicit budgets

The following is the initial 0.2.0 contract adopted on October 16, 2026. Adoption does not mean that implementation acceptance or production connection tests on all four OSes are complete.

Destinations use a canonical lowercase ASCII DNS name, explicit port, explicit resolver endpoint, and explicit TLS roots. DNS labels contain 1–63 bytes, with 1–253 bytes overall, and use alphanumeric characters with internal hyphens only. Unicode, uppercase, a trailing dot, empty labels, IP literals, and alternate numeric hosts are rejected rather than normalized. Startup checks that the DNS query, URL, Host, and TLS SNI/certificate name identify the same destination. Only HTTPS/HTTP1 is supported, with normal TLS verification.

The standard profile has no internal-address exceptions. It checks private, loopback, link-local and metadata destinations, mapped/translation forms, deprecated site-local/IPv4-compatible forms, and the pinned IANA deny set. `localhost`, `invalid`, `local`, `onion`, `arpa`, and their subdomains are also refused at startup. Being outside the pinned set does not establish Internet reachability or safety in general.

| Startup setting | Initial finite support envelope |
|---|---|
| jobs, queue, job_roots, dns_roots, dns_children, http_drivers, waiters, senders, sockets | Each 1–64 |
| waiter_bytes / input_bytes | 1–1 MiB / 1–waiter_bytes |
| max_request_bytes / max_response_bytes | Each 1–1 MiB |
| max_headers / max_header_bytes | 1–128 / 1–64 KiB, including required/fixed headers |
| max_http_buffer_bytes / max_target_bytes | 8192–65536 bytes / 1–4096 encoded bytes |
| whole / connect | Each a positive Duration with representable native Instant addition |
| attempts / catalog capacity | 1–service candidate cap / 1–64 operations |
| Service candidate cap | 1–1024; DNS refuses more than 128 A/AAAA addresses in total |

The startup adapter supplies all 20 `OperationLimits` values and the catalog/candidate caps. In addition to individual ranges, checked validation covers input≤waiter_bytes, attempts≤candidate cap, retained-capacity products/sums, deadline addition, and fixed-header count/bytes. Arbitrary combinations of the table minima can fail startup when required headers or capacity do not fit. DNS also has finite internal ceilings, including 16 cumulative IO starts, 128 KiB sent and received each, 128 records, and eight CNAME hops. These are support limits, not performance recommendations or an SLA.

### Catalog-wide retained configuration budgets

Under adopted C3, trusted Rust startup supplies all three values to `StartupCatalog::new(operation_capacity, RetentionLimits)`. Each is a positive finite `usize`, with no implicit default or additional fixed numerical ceiling. Required array lengths and arithmetic are checked. Large finite values do not guarantee allocation success or performance. The 20 `OperationLimits` values remain separate.

| Required value | Retained charge in the same catalog |
|---|---|
| `max_root_anchors` | Sum of accepted anchor entries, including unregistered services, distinct services, and duplicate entries |
| `max_root_bytes` | Actual Vec capacities of normalized anchor subject, SPKI, and optional name constraints; not the complete original X.509 certificates |
| `max_config_bytes` | Total under the accounting model below, including root bytes; all three constraints must hold |

Configuration charges include actual root-vector slot capacity, normalized DER, the retained DNS String and its TLS-name copy, ALPN vector/payload, authority/fixed-header/Bearer payload capacities, and the catalog identity vector. Fixed `size_of` charges cover budget state, catalog, service/trust, policy/resolver, header owner, readiness, and Spec/Core/claim metadata. Opaque Rustls/provider representations have fixed structure charges; the resolver's canonical input, Method token, and origin use logical charges. The origin charge is `DNS name length + 15`, derived from `https://`, separators, at most five decimal u16 port digits, and the trailing slash. Shared payloads are charged once. Opaque internal capacity is not reported as measured bytes.

`ServiceSpec::checked`, `FixedRequestHeaders::checked`, `ServiceCredential::bearer`, and the test-network seam require the same catalog reference. Protected/Public specs and registration also check catalog/service/operation bindings. The old unbudgeted constructor signatures are removed. Accepted unregistered specs, headers, and credentials consume the same remaining budget. Reservation precedes acceptance and payload copying. A short String or Vec with large spare capacity can be refused. Hidden input DER capacity is detached by one owned normalization copy, whose actual capacity is checked again.

Operations and TLS for the same service share one immutable trust owner. Equal numeric identities or equal root contents do not qualify distinct services for sharing or root merging. Header claims survive the final native HeaderValue/Bytes clone; trust claims survive the actual native TLS stream/driver Drop. Failed partial data is dropped before its reservation is refunded. Freeze releases the catalog's own identity vector and closes creation authority. Close/shutdown does not clear charges for surviving services, operations, or native headers/TLS.

Insufficient budget returns `Capacity`; invalid settings, overflow, and binding mismatches return `InvalidInput`; explicit allocation failures return `Unavailable`; closure returns `Closing`. Startup failures use `RequestNotStarted` and no auth failure. Accounting inconsistencies close the domain with `Internal`. The model does not cover host input allocated before acceptance, hidden foreign-crate heap capacity, Arc/Bytes allocator bookkeeping, allocator rounding/fragmentation, runtime DNS/TLS/HTTP buffers or Future/stack storage, the existing operation ledger, or application state. It does not guarantee total malloc/heap/RSS bounds or universal OOM recovery.

C3 candidate acceptance still requires Rust type checking, native ownership fixtures, exact-head four-OS CI, and independent review. Measured small-configuration examples and a production-positive connection profile remain separate acceptance work. Source/parse checks and unexecuted fixture settings are not production recommendations.

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

The client checks all A and AAAA candidates. It refuses a lookup if any candidate is unsafe or either address-family lookup fails. It tries permitted candidates sequentially under the original deadline, with a finite attempt count, and advances only for eligible TCP connection failures. TTL 0 is usable for that single resolution operation and is never cached. TLS certificate verification remains enabled and response bodies are bounded. Requests use `Accept-Encoding: identity`. In 0.2.0, responses without `Content-Encoding` or with only `identity` are accepted; other encodings fail as `UNSUPPORTED_ENCODING`, without implicit decompression. Compression remains a 0.2.1 task in #124, with bounds before/after decompression, memory/time limits, corrupt-input and cancellation/cleanup tests, and independent review.

| Validation | Result and scope |
|---|---|
| High, saved Low, and handwritten Low forms across two fixture groups | PASS. These validate compiler calls and API projections, not arbitrary production host setup |
| Positive TLS native fixture | PASS, using a `cfg(test)`-only destination-admission seam. This does not establish a positive production connection profile |
| Production loopback denial | PASS. A lookup returns loopback and AAAA completes successfully with NoData; denial before TCP is confirmed after actual task shutdown |
| Public API connection CI on four OSes | Pending. Earlier private-head CI results are not evidence for this candidate |

This candidate is not a released feature and has not passed four-OS production-connection acceptance.
