# RFC: Nagi 0.2.0 Security Foundation

[日本語](rfc.md) · [Baseline audit (Japanese)](baseline-audit.md) · [Implementation plan](implementation-plan.en.md)

Status: 2026-10-08 JST. **Revised implementation direction: security before compatibility.** The latest user instruction supersedes the coexistence recommendations D1–D3. [Decisions and migration](decisions-and-migration.en.md) govern implementation. SF01 is connected in a separate development draft; see its [contract](sf01-contract.md) and result log. SF02–SF08 and the formal 0.2.0 release remain incomplete; existing 0.1.x conditions are not retroactively changed. Preserve Task/spawn, Tx lifecycle, and High/Low fundamentals.

## Purpose and scope

Preserve 0.1.x types, ownership, structured concurrency, SQLite, and HTTP stability while providing explicit authentication, authorization, and web boundary APIs. A guarantee applies to supported standard operations under stated trust and deployment assumptions. It does not establish that an entire application, arbitrary Rust code, or every Internet input is safe.

Scope: AuthScope, HTTP route policy, CSRF, HTML output/XSS, SQL structure/binding, policy-constrained outbound HTTP/SSRF, CORS, Cookie/Session, and bounded resources/DoS. Exclude OAuth/OIDC providers, password databases, MFA, general taint/effect/region systems, PostgreSQL, custom cryptography, a wholesale HTTP replacement, VM/GC, and sandboxing. Reuse reviewed Rust credential-verification libraries/adapters and application authorization policies.

## Baseline

Main `62bbda9` includes merged SQLite PR #99; released 0.1.11 is `003a594`; open PR count was zero at the initial baseline; design PR #100 is now open. See [evidence and limitations](baseline-audit.md) and [source hashes](baseline.json). Principal/Grant[P] cannot be constructed, copied, shared, or decoded from JSON in Nagi, but are not request-bound and do not enforce expiry/revocation. Owned async delegation is allowed. Standard route registration has no authentication policy argument; HTML is raw text; standard Cookie/Session/CSRF/CORS and an outbound client are absent.

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
| AuthScope/Grant[P] | Opaque, nonCopy/nonClone/nonSerde/nonshared, no owned fields, same-task, nominal permission, consume | Request lease, ID/target matching, expiry/revocation, successful issuer/policy | Indefinite validity, policy logic, rollback |
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

Recommend **one AuthScope and request-bound Grant[P] family**, replacing Principal and old unbounded Grant internals/factories/consumers. Do not coexist with ScopedGrant or indefinite proofs. Retain actual i64 targets and nominal phantom P because they express the required target without arbitrary payload/region machinery, not to preserve compatibility. The previously planned actual Scope value is represented by the private request ID/lease and bound target.

- AuthScope contains verified subject, credential source, request ID, and private lease. Runtime supplies it to handlers; Nagi bool/string/JSON cannot mint it. Authentication and authorization are separate.
- Grant binds subject/resource/request lease after a reviewed policy succeeds. Protected operations use its actual resource, not a separate supplied resource ID. Runtime verifies request ID matching; nominal types alone do not prove same-request identity.
- Both are opaque, same-task, and cannot enter shared/owned fields/Actor state/messages/Task capture or results/background work. Same-task calls/returns and owned local Option/Result are permitted. Inspect actual payloads and Future inputs, while retaining existing Tx semantics.
- Reject request-proof Task delegation, including old Principal/Grant delegation. Same-task async calls remain permitted. Background jobs transfer non-proof input and independently reauthorize through a trusted service adapter with purpose/audience/expiry; no conversion into indefinite service authority.
- Dispatcher owns the lease and invalidates on normal/Err/panic/timeout/cancel/unpolled Drop, including security mappers. Check validity on use and immediately before native admission; expiration during an await must be checked before new work that has not been admitted.
- Define admission as the linearization point issuing a single-use private execution permit bound to the protected target and operation. Queue-slot reservation/Future construction is not admission. Wait and reserve bounded native capacity, then check time/validity and issue the permit inside the same short private gate used for dispatcher invalidation; never await under that gate. Consume the permit at native command submission. Invalidation while waiting or between earlier checks and permit issuance rejects and releases reservations. The gate check supplies the expiry timestamp. Invalidation after issuance does not undo admitted effects. Native adapters never start a new command for this protected operation without its permit; send failure/unknown outcome follows driver evidence. A boolean read followed later by enqueue cannot meet this contract.
- Invalidity before admission prevents starting effects. Already admitted DB/external work is not rolled back by expiry/cancellation. Authorization snapshots require predicates or transactional rechecks for TOCTOU; this is not universally inferred.
- Invalid credentials, denial, and expiry are ordinary SecurityFailure results, not TaskFailure/sticky faults. Unexpected panic/cancellation follows current runtime fault semantics. Distinguish 400/401/403 from 500/503/504 and redact secrets/internal causes.

Real JWS verification delegates signatures/algorithms/keys to an existing reviewed crate. Configure algorithm allowlist, issuer/audience, exp/nbf, finite skew, trusted key selection and bounded key caches. Token-provided URLs never determine key fetch destinations. Fixed-credential samples are not production-verifier evidence.

## Explicit HTTP policy

Choose **D1=A: unify standard HTTP under one policy-required App/route/serve dispatcher**. Reject coexistence B because it retains policy-free registration and two dispatch specifications; decorator-only C misses dynamic and handwritten Low paths.

Each method/path requires an actual public/authenticated/authorized policy. Public deliberately permits anonymous access, while retaining CSRF/CORS/limits. Authenticated requires AuthScope; authorized adds named policy/permission/target resolution. Protected operations still require grants; anonymous handlers receive no implicit proof.

Change route/route_mapped and handler/mapper signatures. Reject old arities, decorators and legacy serve with migration diagnostics in High and Low. Remove unchecked registration, implicit public defaults, legacy App/serve and feature-flag restoration from standard runtime/distribution. A trusted Rust host can provide its own server outside the guarantee; do not re-export it as an alternative standard entry point.

- Dynamic registration uses the same policy record and sealed handler/mapper relation. GET→HEAD inherits policy; explicit HEAD needs its own.
- CORS preflight emits bounded metadata only, with no handler/proof. Ordinary OPTIONS needs explicit policy.
- 404/405 are transport responses; Allow may reveal route existence.
- Remove automatic `/health`/`/stream`/`/ws`. Applications explicitly register health and supported response streaming. Standard WebSocket support is absent in 0.2.0 and old entry points are rejected. Document this lost functionality and the trusted-host alternative. A future upgrade design must cover authentication/revocation/message budgets before becoming standard; do not restore an unprotected legacy route.

Baseline dispatch order: connection/header bounds → path/method → request/security admission → ambiguous header/external-origin/credential checks → authentication → CSRF applicability → bounded body/token verification → handler/authorized operation. Security verification has an absolute shared budget across verifier/key/session waits, separate from the handler budget. Public/login bodies remain bounded. Changes to existing transport-status precedence require explicit contracts. All early returns release permits/body/leases.

## Cookie, Session, and CSRF

Authentication identifies a subject; authorization permits an operation; CSRF controls cross-site use of browser-attached credentials. Keep separate policies.

Recommend opaque host-only session IDs with a server-side bounded store for subject/expiry/revocation generation. Validate Secure/HttpOnly/Path=/, explicit SameSite, no Domain, and `__Host-` shape. SameSite=None requires Secure and an explicit cross-origin use case. Compare maintained cookie parsers/serializers rather than inventing grammar. Parse all Cookie headers and reject duplicate authentication-cookie names instead of choosing one.

For standard session issuance/rotation/authentication-state responses and CSRF-token delivery, the standard-app finalizer owns `Cache-Control: no-store`. Set-Cookie alone does not prohibit caching. Reject conflicting public/max-age headers and do not use 304/shared-cache reuse for token/session responses. Add explicit cookie/credential-source/authentication-dependent Vary where required, without substituting Vary for no-store. Do not claim automatic confidentiality/cache classification for ordinary DTOs; applications specify that separately.

IDs use OS CSPRNG with at least 128 bits of unpredictability, without secret Debug/Serde/response echo. Rotate on login/privilege changes, atomically invalidating old IDs. Specify idle/absolute expiry, logout/revocation, concurrent rotation, and clock rollback. Cookie removal is not server revocation. Store failure/reply loss never implies authentication success.

D3 standardizes a **durable SQLite server session store** using existing Pool/Transaction. Native transactions make lookup/rotate/revoke, generation compare-and-swap, capacity and cleanup atomic. No production memory fallback or stateless signed-cookie mode. Private test doubles exercise the same contract. File permissions, shared-file locking/clocks and backup/restore are deployment requirements; distributed availability is not promised.

A new lookup after revocation commits rejects the old ID/generation. Already issued request leases are bounded authentication snapshots, not a promise of immediate revocation across processes. Local dispatcher invalidation and permit issuance share a gate; a database logout commit does not share that gate. Updates requiring immediate consistency need session tables and protected data in one SQLite file, with a session-generation/authority predicate in the same Tx. Separate-file/store prechecks are insufficient; preserve the ATTACH prohibition and do not extend this guarantee to arbitrary external I/O. Lost rotation responses mean unknown outcome and reauthentication: never revive the old ID or send the new ID before commit. See [crash/time/capacity details](decisions-and-migration.en.md#d3-durable-server-sessions).

Treat GET/HEAD/OPTIONS as safe methods without claiming the checker proves handlers side-effect-free. Unsafe ambient-cookie requests require CSRF policy **including public login/logout**. Recommend a synchronizer token bound to a session or bounded anonymous pre-login session. Specify CSPRNG, length, constant-time comparison, and rotation using reviewed primitives; never place tokens in URL/query/log. Bounded forms require one unambiguous token; JSON APIs use a dedicated header.

Compare Origin exactly with configured external scheme/host/port. Recommend rejecting opaque/null/duplicate/malformed/missing origins for unsafe browser-session requests; no Referer fallback or reflection in the initial implementation. SameSite/Fetch Metadata supplement tokens and authorization. Verify browser compatibility in integration tests.

Header-only service credentials that never authenticate cookies are a distinct credential-source type, not a disable-CSRF boolean. Reject ambiguous sources/fallback. Signed webhooks require dedicated verifier contracts; do not add general disable-security operations.

TLS may terminate at the existing front proxy. Startup external-origin configuration is authoritative; Host/Forwarded/X-Forwarded-* are not blindly trusted. Peer-based proxy trust needs explicit listener/Rust-host configuration. Verify public Session examples with real TLS/proxy; do not add a Secure-off development bypass.

## CORS

CORS governs browser response reading, not authentication, authorization, or CSRF. Default cross-origin reading is denied. Explicit exact scheme/host/port origins, methods/headers, credentials, and finite preflight-cache configuration are typed. Reuse canonical parsers; no suffix/substring/ambiguous regex matching or unconditional origin reflection.

Reject wildcard with credentials, null and duplicate origins. Reflect only allowed origins and add Vary for Origin and preflight method/header dependencies. Apply policy to early errors including 401/403/413/429/500. Preflight success does not authorize actual requests; repeat authentication/CSRF checks.

The standard app finalizer owns security-managed CORS/session/CSP headers and rejects conflicting handler additions. Restrict append_header to non-reserved headers; reject security/Content-Type overrides. Fix reserved sets and rejection stages in tests.

## XSS and HTML output

Use current JSON/plain-text APIs, appropriate Content-Type, and standard-app nosniff. JSON encoding is not HTML-attribute or script-embedding encoding.

Recommend an opaque typed HtmlFragment builder for a supported subset, rather than inference over string interpolation. Separate text nodes, quoted attributes, and allowed URL attributes; fragment composition preserves context. Static element/attribute allowlists exclude event handlers, script/style/rawtext, arbitrary CSS, srcdoc, and unsupported namespaces. Specify Unicode/NUL/re-encoding and output budgets.

URL attributes need both HTML encoding and scheme/use validation. Navigation URLs and SSRF Target are separate types. Do not initially ship arbitrary-HTML sanitization or a Nagi raw-string→trusted-fragment cast.

Remove standard raw Html/html/http.html entry points. Require typed fragments for HTML responses. Finalizers own body classification and headers, including mapper results, so bytes plus manual Content-Type cannot produce active HTML/JS/SVG. Arbitrary active-content hosting belongs to the explicit trusted-host boundary. Restrictive CSP supplements encoding; nonce extensions require separate request/template design, without silently allowing unsafe inline content.

## SQL injection

Keep Pool/Tx/Parameters/authorizer/shape/cleanup contracts. Recommend a security Query constructor accepting **direct literal SQL structure**, with values in Parameters. Reject variables/concatenation/formatted SQL at this canonical constructor, without treating unrelated user functions as builtins.

Reuse native SQLite parsing and single-statement/bind/shape validation. Literal SQL is not necessarily authorized or semantically correct. Choose among reviewed literal queries for identifiers/order; no escaping substitute for bound values.

All standard SQLite query/exec operations require opaque Query. Remove dynamic string/Sql conversion and legacy Db entry points, including Db.exec. Retain Pool/Tx acquisition/busy/completion/unknown-outcome semantics. Schema migration/bootstrap is reviewed trusted-host management, never re-exported as a request-facing arbitrary SQL factory. Do not make schema validation mandatory for ordinary check. Validate values/NULL/count/schema through native execution and distinguish this guarantee from opt-in G-SQL.

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

Remove coexistence of legacy HTTP/proofs/raw HTML/dynamic SQL in 0.2.0. The latest user instruction authorizes necessary breaking changes within this security foundation. Preserve move/views, Task obligations/business Err/sticky faults, legacy spawn, Supervisor termination coupling, and Tx affine/close/unknown outcomes/acquisition budgets. Update affected HTTP/auth test expectations deliberately: pair old-entry rejection with equivalent migrated business behavior instead of preserving old acceptance everywhere.

[Decisions and migration](decisions-and-migration.en.md) lists replacements, retained compatibility reasons, removed functionality, source diagnostics and High/saved/handwritten Low/native/Rust-consumer acceptance. Warnings cannot permit policy-free execution. Do not reinterpret old metadata or inject public/lease defaults. Remove or change legacy public Rust constructors/serve too; closing only Nagi while retaining unchecked standard Rust entry points is insufficient. Existing 0.1.x tags/distributions remain intact.

| ID | Recommended final form and rejected alternatives | Compatibility and limits |
|---|---|---|
| D1 | One policy-required standard HTTP dispatcher; withdraw coexistence B and decorator-only C | Breaking old arities/decorators/serve/builtins/raw responses. Explicit public is anonymous, not universal private access. Trusted Rust host remains outside guarantees |
| D2 | Replace old proofs with AuthScope and one lease-bound Grant[P]; no indefinite Principal/Grant or ScopedGrant coexistence | Issuers/consumers/delegation break. Retain P/i64 because sufficient, with same-task checking and runtime expiry/gates. No proof of policy correctness or rollback |
| D3 | Durable SQLite server sessions and host-only cookies; no production memory fallback/stateless cookie | Define restart/rotation/store failures. No immediate cross-process invalidation of issued snapshots or distributed availability. Native atomic/crash/time evidence required |

These are implementation directions under the latest instruction, not current-language guarantees. [Decision record](decisions-and-migration.en.md) explains superseding ADR 001's minimum experiment and earlier compatibility deferral for 0.2.0. [The plan](implementation-plan.en.md) requires coherent implementation/tests/bilingual docs/executable examples, feature PRs and release gates.

## References and limits

Retrieved OWASP guidance: [Authentication](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html), [Authorization](https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html), [CSRF](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html), [XSS](https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html), [SQLi](https://cheatsheetseries.owasp.org/cheatsheets/SQL_Injection_Prevention_Cheat_Sheet.html), [SSRF](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html), [Session](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html), [DoS](https://cheatsheetseries.owasp.org/cheatsheets/Denial_of_Service_Cheat_Sheet.html); [Fetch/CORS](https://fetch.spec.whatwg.org/#http-cors-protocol), [RFC6265](https://www.rfc-editor.org/rfc/rfc6265), [RFC9110](https://www.rfc-editor.org/rfc/rfc9110), [JWT BCP RFC8725](https://www.rfc-editor.org/rfc/rfc8725). SameSite/prefixes are not guaranteed by RFC6265 alone; modern-browser verification is required. [URLs/hashes](reference-retrieval.json).

This is not certification. Runtime, dependency comparisons, browser/proxy/DNS/TLS evidence, and migration execution remain future acceptance. No third-party attacks, exploit automation, stress/resource-exhaustion experiments, or paid scanners are part of this work.
