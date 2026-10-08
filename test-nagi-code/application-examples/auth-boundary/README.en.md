# Authentication and authorization through standard HTTP policies

[日本語](README.md)

The standard HTTP dispatcher verifies credentials and creates an `AuthScope`; a Nagi policy authorizes access to a document; and a native adapter consumes a target-bound `Grant[Read]` to read SQLite. The dispatcher does not call a handler before authentication, and Nagi cannot construct an `AuthScope` or `Grant`.

```sh
nagic run --project test-nagi-code/application-examples/auth-boundary
curl -i http://127.0.0.1:8098/health
curl -i -H 'Authorization: Bearer demo-alice' http://127.0.0.1:8098/documents/1
curl -i -H 'Authorization: Bearer demo-alice' http://127.0.0.1:8098/documents/2
```

`/health` is an explicitly public route. `/me`, `/documents/{id}`, and `POST /documents/read` use `authenticated_policy`. Alice can read document 1 and Bob can read document 2. Another subject's document and blocked document 3 return 403; a missing document returns 404. Invalid JSON syntax returns 400 and a type error returns 422.

`verify` receives the `Request` and application state from the standard dispatcher and returns a `VerifiedIdentity` or a finite-lived `Failure`. The dispatcher supplies `AuthScope` to the policy. Either the named Nagi `read_policy` (the default) or a handwritten Rust policy checks the target; only then does the adapter create `Grant::from_authorized(scope, document_id)`. The adapter awaits bounded semaphore capacity before issuing a synchronous SQLite command bound to the grant's subject and target. The query rechecks mutable owner/blocked state. There is no read path authorized by a bare subject or ID.

`NAGI_AUTH_POLICY_MODE=nagi` (default) and `rust` use the same standard HTTP router, database, payload, native verifier, and Grant-consuming operation. They select only the policy implementation. Set `NAGI_AUTH_PROBES=1` to enable authenticated panic and timeout routes. The body limit is 4096 bytes; security, body, and handler deadlines are each 1000 ms; HTTP admission is 32; and up to 8 database reads can be admitted at once. Body timeout returns 408, handler timeout 504, and a post-response panic is mapped to 500.

Authentication failures from standard policies use secret-free fixed messages, and Bearer failures include `WWW-Authenticate: Bearer`. Missing and incorrect credentials share the same 401 body, `invalid credential`. The expired fixture returns `authority expired`; a duplicate or invalid Authorization header returns 400 with `invalid security request`. Business responses from Nagi handlers are separate.

The native verifier's `demo-alice`, `demo-bob`, and `demo-expired` values are credential fixtures. These strings are not built-in usernames; the verifier decides their subject mapping. The example does not verify JWT signatures, issuer/audience, revocation, or token issuance. Connect a reviewed existing verifier for production use. The compiler does not prove the correctness of the Nagi/Rust policy or the design of every route. SQLite serves small demonstration queries; this is not a generic pool or transaction API, and cancellation does not promise rollback. The server uses loopback HTTP/1 and does not provide TLS, HTTP/2, or connection admission limits.

This sample replaces the former Axum glue and `Principal` factory with the standard HTTP request-bound auth API. The separate [custom Axum service](../axum-service/README.en.md) remains an independent trusted host boundary that does not bypass standard HTTP policies.
