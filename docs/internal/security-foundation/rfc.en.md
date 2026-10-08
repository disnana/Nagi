# RFC: Nagi 0.2.0 Security Foundation

[日本語](rfc.md) · [Baseline audit (Japanese)](baseline-audit.md) · [Implementation plan](implementation-plan.en.md)

Status: **independently reviewed proposal, not yet adopted**, 2026-10-08 JST. These features are unimplemented and unreleased. Saving this RFC does not adopt new public semantics. Preserve G-AUTH, Task/spawn, Tx, and High/Low contracts; record decisions D1–D3 before implementing public APIs. API names below are candidates, not executable examples.

## Purpose and scope

Preserve 0.1.x types, ownership, structured concurrency, SQLite, and HTTP stability while providing explicit authentication, authorization, and web boundary APIs. A guarantee applies to supported standard operations under stated trust and deployment assumptions. It does not establish that an entire application, arbitrary Rust code, or every Internet input is safe.

Scope: AuthScope, HTTP route policy, CSRF, HTML output/XSS, SQL structure/binding, policy-constrained outbound HTTP/SSRF, CORS, Cookie/Session, and bounded resources/DoS. Exclude OAuth/OIDC providers, password databases, MFA, general taint/effect/region systems, PostgreSQL, custom cryptography, a wholesale HTTP replacement, VM/GC, and sandboxing. Reuse reviewed Rust credential-verification libraries/adapters and application authorization policies.

## Baseline

Main `62bbda9` includes merged SQLite PR #99; released 0.1.11 is `003a594`; open PR count was zero. See [evidence and limitations](baseline-audit.md) and [source hashes](baseline.json). Principal/Grant[P] cannot be constructed, copied, shared, or decoded from JSON in Nagi, but are not request-bound and do not enforce expiry/revocation. Owned async delegation is allowed. Standard route registration has no authentication policy argument; HTML is raw text; standard Cookie/Session/CSRF/CORS and an outbound client are absent.

SQLite Parameters bind values and native authorizer checks exist. Opt-in literal schema preflight does not establish dynamic SQL provenance or tenant isolation. Existing HTTP and frontend limits remain in force.

## Trust boundaries

Assets include credentials/session secrets, subject/resource/permission bindings, private data, database integrity, service resources, and distribution identity. HTTP fields, browser origins, session IDs/tokens, SQL values, destination inputs, DNS results, and external responses are untrusted. Application source/manifests, Rust issuers/drivers, and deployment policies remain trusted and separately reviewed.

| Boundary | Enforcement | Outside the guarantee |
|---|---|---|
| Credentials → identity | Reject ambiguity; trusted verifier checks cryptography and claims | Dishonest verifier/policy/dependency or compromised identity provider |
| Identity → operation | Nominal permission, actual target, current validity, data-side predicate | Correctness of every policy/SQL tenant condition or subsequent state changes |
| Browser → route | Explicit route policy, Session/CSRF/Origin, separate CORS | Side effects in GET, compromised trusted origins, extensions |
| Data → response | Context-aware renderer, URL validation, supplementary CSP/nosniff | Raw HTML/JS/CSS, DTO confidentiality, all DOM information flow |
| Values → SQL | Fixed structure and binding, native parsing/authorizer | Arbitrary SQL/Rust/legacy Db.exec and operational DB permissions |
| URL/DNS → socket | Enforce policy at resolution, connection, retry, pooling, redirect, proxy, TLS | Raw Rust networking and all network infrastructure |
| Work → resource | Bounded admission, queues, caches, sizes, deadlines, cleanup/join oracles | Forced termination of non-yielding work, universal OOM recovery, network DDoS, OS isolation |

Security types restrict forgery and misuse; naming an incoming string “verified” does not validate it. Rust interoperability remains a trust boundary, not an automatically protected escape route.

## Static and runtime contracts

All following guarantees are **proposed**. Do not promote them to current guarantees until positive/negative/native oracles pass.

| Feature | Nagi checker | Runtime/deployment | Not proven |
|---|---|---|---|
| Route | Required canonical policy, handler type, missing/mismatched policy rejection | Dynamic registration and all methods/fallbacks retain policy | Every route is private, or arbitrary Rust Router coverage |
| AuthScope/ScopedGrant | Opaque, nonCopy/nonClone/nonSerde/nonshared, no owned fields, same-task, nominal permission, consume | Request lease, ID/target matching, expiry/revocation, successful issuer/policy | Indefinite validity, policy logic, rollback |
| CSRF | Credential-source classification and required policy for unsafe ambient-credential requests | Token/session binding, exact origin, malformed/ambiguous/missing data, budget | XSS prevention or static proof of application side effects |
| HTML | Renderer/context types distinguish raw strings and fragments | Encoding, URL scheme, output bounds | Raw HTML/JS/CSS or universal browser safety |
| SQL | Direct literal structure, Parameters, supported row types | Native prepare/bind/shape/schema/NULL/type/authorizer | Arbitrary dynamic SQL and automatic tenant/permission inference |
| SSRF | DestinationPolicy/Target required; no implicit bare-URL conversion | DNS/all addresses/actual socket/redirect/proxy/TLS/deadline/size | Destination safety from URL parsing alone |
| CORS | Typed configuration; reject statically obvious wildcard-plus-credentials | Origin/method/header/preflight/Vary/reserved response headers | Authentication, CSRF, non-browser access control |
| Cookie/Session | Separate proof from DTO/string; reject secret Debug/Serde | Parsing/attributes/entropy/rotation/revocation/expiry/atomic store | Browser enforcement or universal store availability |
| DoS | Invalid types/constants and required finite configuration | Admission/counters/deadlines/actual payload sizes/cancel cleanup | Total CPU/heap bounds from compilation or universal OOM/DDoS recovery |

Dynamic policy values use fallible runtime constructors. Do not reimplement Rust traits, cryptography, URL parsing, or SQL parsing in the checker.

## High, Low, and Rust responsibilities

Prefer existing imports, canonical resources/operations, functions, enums, Result, view, and owned moves. Do not start with new keywords, attributes, or a general effect system.

1. High resolves module identity; the checker verifies resource metadata, Passing/borrow_owner, payload roles, and lifecycle facts.
2. High→Low preserves types and canonical identities, not serialized security proofs. Saved and handwritten Low use the same checker and their own diagnostic locations.
3. Final native Low/`@replace` integration is checked and sealed. Missing policies, forged standard IDs, invalid scope/handler facts, or absent facts never default to success. No public security-facts setter.
4. The emitter implements sealed plans without auth inference, cloning, or lifetime extension. Rust checks traits, Send/Sync, borrow correctness, and native implementations.
5. Runtime validates bounded input, current authority/leases, native I/O, and completion. Compiler success does not prove credential verification, browser behavior, or database outcomes.

User-defined lookalikes are not builtins. Register actual payload, shared state, indirect protocols, callback signatures, and phantoms separately. Unsupported arbitrary Rust security semantics are not inferred. Accepted supported Nagi rejected by generated Rust for Nagi-detectable type/move/lifetime reasons remains a P1 compiler defect.

## AuthScope and authorization

Distinguish AuthScope from Task `scope`, SQLite Tx, and query/domain scope. Preserve the existing direction that grants contain actual authorized targets; request lifetime is an additional value/lease, not a new region type system.

Recommend retaining Principal/Grant[P] and adding request AuthScope plus ScopedGrant[P]. Initial resource identity remains an actual i64, as with Grant, while P remains a nominal phantom. Changing old Grant to two parameters or introducing arbitrary target payload S requires a separate compatibility/payload design.

- AuthScope contains verified subject, credential source, request ID, and private lease. Runtime supplies it to handlers; Nagi bool/string/JSON cannot mint it. Authentication and authorization are separate.
- ScopedGrant binds subject/resource/request lease after a reviewed policy succeeds. Protected operations use its actual resource, not a separate supplied resource ID. Runtime verifies request ID matching; nominal types alone do not prove same-request identity.
- Both are opaque, same-task, and cannot enter shared/owned fields/Actor state/messages/Task capture or results/background work. Same-task calls/returns and owned local Option/Result are permitted. Inspect actual payloads and Future inputs, while retaining existing Tx semantics.
- No request-proof Task delegation in the first version. Same-task async calls are permitted; legacy Principal/Grant owned delegation stays unchanged. Background delegation would require separately designed credentials/audience/expiry.
- Dispatcher owns the lease and invalidates on normal/Err/panic/timeout/cancel/unpolled Drop, including security mappers. Check validity on use and immediately before native admission; expiration during an await must be checked before new work that has not been admitted.
- Define admission as the linearization point issuing a single-use private execution permit bound to the protected target and operation. Queue-slot reservation/Future construction is not admission. Wait and reserve bounded native capacity, then check time/validity and issue the permit inside the same short private gate used for dispatcher invalidation; never await under that gate. Consume the permit at native command submission. Invalidation while waiting or between earlier checks and permit issuance rejects and releases reservations. The gate check supplies the expiry timestamp. Invalidation after issuance does not undo admitted effects. Native adapters never start a new command for this protected operation without its permit; send failure/unknown outcome follows driver evidence. A boolean read followed later by enqueue cannot meet this contract.
- Invalidity before admission prevents starting effects. Already admitted DB/external work is not rolled back by expiry/cancellation. Authorization snapshots require predicates or transactional rechecks for TOCTOU; this is not universally inferred.
- Invalid credentials, denial, and expiry are ordinary SecurityFailure results, not TaskFailure/sticky faults. Unexpected panic/cancellation follows current runtime fault semantics. Distinguish 400/401/403 from 500/503/504 and redact secrets/internal causes.

Real JWS verification delegates signatures/algorithms/keys to an existing reviewed crate. Configure algorithm allowlist, issuer/audience, exp/nbf, finite skew, trusted key selection and bounded key caches. Token-provided URLs never determine key fetch destinations. Fixed-credential samples are not production-verifier evidence.

## Explicit HTTP policy

Options: A changes old HTTP signatures to require policy everywhere; B adds a distinct policy-required app/route and deprecates old paths; C only protects decorators. **Recommend B**. New Security Foundation apps cannot register a policy-free route; old applications are not automatically secured or covered by a universal route guarantee.

Every method/path in the new app explicitly chooses public, authenticated, or authorized. Public means deliberate anonymous access, not disabling CSRF/CORS/limits. Authenticated requires verified AuthScope; authorized additionally requires named policy/permission/target resolution. Protected operations still require grants.

Dynamic registration stores a real policy value. No implicit legacy App→security app conversion or downgrade. Seal handler/mapper/policy relations. Public handlers receive no automatically authenticated proof.

- GET→HEAD fallback inherits GET policy; explicit HEAD requires its own policy. Keep duplicate/capture constraints.
- CORS preflight OPTIONS only emits constrained metadata; it invokes no protected handler and issues no proof. Ordinary OPTIONS requires explicit policy.
- 404/405 are transport responses. Allow may disclose route existence; private route-existence concealment is not promised.
- Legacy decorators, builtin `/health`/`/stream`/`/ws`, `serve(Db, port)`, and Rust/Axum routers are separate. Never inject them into the new app. Migration registers/removes them explicitly. New-app WebSocket authentication/session renewal is out of scope and documented as a feature difference.

Baseline dispatch order: connection/header bounds → path/method → request/security admission → ambiguous header/external-origin/credential checks → authentication → CSRF applicability → bounded body/token verification → handler/authorized operation. Security verification has an absolute shared budget across verifier/key/session waits, separate from the handler budget. Public/login bodies remain bounded. Changes to existing transport-status precedence require explicit contracts. All early returns release permits/body/leases.

## Cookie, Session, and CSRF

Authentication identifies a subject; authorization permits an operation; CSRF controls cross-site use of browser-attached credentials. Keep separate policies.

Recommend opaque host-only session IDs with a server-side bounded store for subject/expiry/revocation generation. Validate Secure/HttpOnly/Path=/, explicit SameSite, no Domain, and `__Host-` shape. SameSite=None requires Secure and an explicit cross-origin use case. Compare maintained cookie parsers/serializers rather than inventing grammar. Parse all Cookie headers and reject duplicate authentication-cookie names instead of choosing one.

For standard session issuance/rotation/authentication-state responses and CSRF-token delivery, the new-app finalizer owns `Cache-Control: no-store`. Set-Cookie alone does not prohibit caching. Reject conflicting public/max-age headers and do not use 304/shared-cache reuse for token/session responses. Add explicit cookie/credential-source/authentication-dependent Vary where required, without substituting Vary for no-store. Do not claim automatic confidentiality/cache classification for ordinary DTOs; applications specify that separately.

IDs use OS CSPRNG with at least 128 bits of unpredictability, without secret Debug/Serde/response echo. Rotate on login/privilege changes, atomically invalidating old IDs. Specify idle/absolute expiry, logout/revocation, concurrent rotation, and clock rollback. Cookie removal is not server revocation. Store failure/reply loss never implies authentication success.

Candidate reference store: explicitly bounded single-process memory store, restart logout, no multi-instance sharing. Production adapters need atomic lookup/rotate/revoke semantics; Map put/get alone proves nothing about linearizability. Compare SQLite storage; D3 decides whether persistence/multi-instance support is mandatory in 0.2.0.

Treat GET/HEAD/OPTIONS as safe methods without claiming the checker proves handlers side-effect-free. Unsafe ambient-cookie requests require CSRF policy **including public login/logout**. Recommend a synchronizer token bound to a session or bounded anonymous pre-login session. Specify CSPRNG, length, constant-time comparison, and rotation using reviewed primitives; never place tokens in URL/query/log. Bounded forms require one unambiguous token; JSON APIs use a dedicated header.

Compare Origin exactly with configured external scheme/host/port. Recommend rejecting opaque/null/duplicate/malformed/missing origins for unsafe browser-session requests; no Referer fallback or reflection in the initial implementation. SameSite/Fetch Metadata supplement tokens and authorization. Verify browser compatibility in integration tests.

Header-only service credentials that never authenticate cookies are a distinct credential-source type, not a disable-CSRF boolean. Reject ambiguous sources/fallback. Signed webhooks require dedicated verifier contracts; do not add general disable-security operations.

TLS may terminate at the existing front proxy. Startup external-origin configuration is authoritative; Host/Forwarded/X-Forwarded-* are not blindly trusted. Peer-based proxy trust needs explicit listener/Rust-host configuration. Verify public Session examples with real TLS/proxy; do not add a Secure-off development bypass.

## CORS

CORS governs browser response reading, not authentication, authorization, or CSRF. Default cross-origin reading is denied. Explicit exact scheme/host/port origins, methods/headers, credentials, and finite preflight-cache configuration are typed. Reuse canonical parsers; no suffix/substring/ambiguous regex matching or unconditional origin reflection.

Reject wildcard with credentials, null and duplicate origins. Reflect only allowed origins and add Vary for Origin and preflight method/header dependencies. Apply policy to early errors including 401/403/413/429/500. Preflight success does not authorize actual requests; repeat authentication/CSRF checks.

The new app finalizer owns security-managed CORS/session/CSP headers and rejects conflicting handler additions. Preserve old raw header API semantics. Fix reserved-header sets and rejection stages in tests.

## XSS and HTML output

Use current JSON/plain-text APIs, appropriate Content-Type, and new-app nosniff. JSON encoding is not HTML-attribute or script-embedding encoding.

Recommend an opaque typed HtmlFragment builder for a supported subset, rather than inference over string interpolation. Separate text nodes, quoted attributes, and allowed URL attributes; fragment composition preserves context. Static element/attribute allowlists exclude event handlers, script/style/rawtext, arbitrary CSS, srcdoc, and unsupported namespaces. Specify Unicode/NUL/re-encoding and output budgets.

URL attributes need both HTML encoding and scheme/use validation. Navigation URLs and SSRF Target are separate types. Do not initially ship arbitrary-HTML sanitization or a Nagi raw-string→trusted-fragment cast.

Legacy Html/html/http.html remain raw compatibility paths outside XSS guarantees. Recommend typed rendering in the new app and document raw warnings/migration under D1. Restrictive CSP supplements encoding; nonce extensions require separate request/template design, without silently allowing unsafe inline content.

## SQL injection

Keep Pool/Tx/Parameters/authorizer/shape/cleanup contracts. Recommend a security Query constructor accepting **direct literal SQL structure**, with values in Parameters. Reject variables/concatenation/formatted SQL at this canonical constructor, without treating unrelated user functions as builtins.

Reuse native SQLite parsing and single-statement/bind/shape validation. Literal SQL is not necessarily authorized or semantically correct. Choose among reviewed literal queries for identifiers/order; no escaping substitute for bound values.

Legacy dynamic SQL/Db.exec remain trusted-structure compatibility paths. Query and Sql have no implicit conversion; new operations require Query. Do not make schema validation mandatory for ordinary check. Validate values/NULL/count/schema through native execution and distinguish this guarantee from opt-in G-SQL.

Protected operations use grant targets in predicates/binds or recheck within a Tx. A grant passed to a generic query does not establish all tenant restrictions. Cancellation, lease invalidation, or Err does not imply rollback completion.

## SSRF and outbound HTTP

Start with an independent Rust client/URL/TLS comparison. A URL validator alone is not SSRF protection. Require finite startup service origins and purpose-specific DestinationPolicy; do not publish unrestricted URL fetch first.

- Canonical-parse once; validate scheme/host/port/userinfo. Default HTTPS; reject userinfo/fragments. Separate relative path/query data without changing origin; preserve hostname/certificate checks.
- Require both hostname allowlist and address policy. Check every DNS candidate and bind approved addresses to actual connections. Cover re-resolution, IPv4/IPv6/mapped addresses, retries/Happy Eyeballs, and pooled connection reuse.
- Initially do not follow redirects. Future redirect support must validate every hop and avoid cross-origin credential forwarding. Do not inherit environment proxies implicitly. Proxies owning DNS/connection need separately validated trusted adapters.
- Deny private/link-local/loopback/metadata destinations by default. Internal services need fixed startup identities and bounded address ranges, never request-controlled expansion. No allow-all or TLS-verification-off bypass.
- Specify absolute resolution/connect/request budgets, decoded response limits, capacities/queues/caches, and distinguish policy denial, transport failure, and ordinary HTTP status. Verify cancellation and connection reuse/release.

If the selected library cannot bind validation to the actual socket, this PR remains blocked. DNS mocks are insufficient. General Internet browsing is outside the initial client scope.

## DoS and lifecycle

Preserve existing HTTP/Actor/Task/SQLite and frontend limits. Do not extend SQLite acquisition budgets into BEGIN/busy/SQL execution.

Bound credential/cookie/token/URL lengths, verifier concurrency/key caches, session entries/expiry cleanup, CSRF state, CORS sets/caches, renderer/response output, resolver/client queues/caches. Untrusted keys cannot create unlimited maps. Validate overflow/native capacities/deadlines fallibly.

Rate limiting uses bounded identities and explicit policies. Do not equate supplied headers/IPs with trusted subjects; implement peer/proxy trust first when needed. Rate limits do not replace bytes/capacity/deadlines or provide distributed quotas. Preserve per-API 0ms semantics: positive HTTP deadlines and SQLite immediate acquisition differ.

Normal/denied/Err/panic/unpolled/Pending cancellation/shutdown must release permits/leases/workers and distinguish notification from actual join. Redact secrets and separate internal causes from public codes. Synchronous Drop cannot certify completion of blocking crypto/DNS/native work.

## Compatibility and public decisions

Preserve move/view origins, Task normal-exit obligations/business Err/sticky faults, legacy spawn, Supervisor/HTTP, Tx affine/close/outcome, Low checks and metadata rejection. Do not delete Rust factories or alter old Grant arity.

Under recommended B, add policy-required HTTP and new resources while retaining deprecated/explicitly uncovered old app/decorator/raw HTML/dynamic SQL paths. Provide migration guides and native migrated samples. Check/editor/saved/hand Low warnings share semantics; no extension-based exemption or security suppression mechanism. A 0.2 version number alone does not authorize breaking APIs. No implicit downgrade from the new app.

| ID | Alternatives and recommendation | Impact / adoption blocker |
|---|---|---|
| D1 | A=break old HTTP to require policy; B=distinct required-policy app plus deprecated legacy; C=decorators only. **B** | Covers all new-app routes, not every Nagi HTTP program. Preserves old signatures. Universal obligation needs A and legacy/builtin/Low/Rust migrations |
| D2 | Change Grant arity; change old Grant validity; add AuthScope/ScopedGrant. **Add new same-task request proofs** | Preserve old delegation; reject Task/Actor transfer for new proofs. Fix names/target representation/factories/handler signatures in an ADR before code |
| D3 | Server store (bounded memory/SQLite/trusted external) vs stateless signed cookie. **Server store, host-only cookie, bounded memory reference and atomic adapter contract** | Restart logout; multi-instance requires appropriate store. If persistent production store is required, add SQLite adapter to acceptance. Select crypto/cookie/client dependencies using actual source/maintenance/licenses/4 OS evidence |

These are proposals, not amendments silently adopted over existing ADRs. Record decisions, then RED contracts, then implementation. See [PR plan and completion gates](implementation-plan.en.md).

## References and limits

Retrieved OWASP guidance: [Authentication](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html), [Authorization](https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html), [CSRF](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html), [XSS](https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html), [SQLi](https://cheatsheetseries.owasp.org/cheatsheets/SQL_Injection_Prevention_Cheat_Sheet.html), [SSRF](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html), [Session](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html), [DoS](https://cheatsheetseries.owasp.org/cheatsheets/Denial_of_Service_Cheat_Sheet.html); [Fetch/CORS](https://fetch.spec.whatwg.org/#http-cors-protocol), [RFC6265](https://www.rfc-editor.org/rfc/rfc6265), [RFC9110](https://www.rfc-editor.org/rfc/rfc9110), [JWT BCP RFC8725](https://www.rfc-editor.org/rfc/rfc8725). SameSite/prefixes are not guaranteed by RFC6265 alone; modern-browser verification is required. [URLs/hashes](reference-retrieval.json).

This is not certification. Runtime, dependency comparisons, browser/proxy/DNS/TLS evidence, and migration execution remain future acceptance. No third-party attacks, exploit automation, stress/resource-exhaustion experiments, or paid scanners are part of this work.
