# Migrating to 0.2.0 SF01/SF05 (unreleased)

This describes SF01/SF05 development-source changes. Neither all of Security Foundation nor formal 0.2.0 is released. Published 0.1.x binaries do not provide the new Policy APIs. The change avoids preserving policy-free standard HTTP and indefinite issuers through legacy coexistence.

| Retired API/behavior | Diagnostic and migration | Equivalent business verification |
|---|---|---|
| `@get/@post/@put/@delete` | `SF01 migration`; register method/path/explicit Policy/handler in App | http_entrypoint, http_route_conflicts, error_routes, Python route/CRUD integration |
| `serve(Db, port)` | `SF01 migration`; use `http.serve(app,port,options)` without requiring a DB | High/saved-Low native startup, invalid port and error handling |
| Old 4-argument route / 5-argument route_mapped | `SF01 migration`; add Policy before handler, and unit/AuthScope/Grant as its third argument | Existing HTTP checker/native and all three security_sf01_native paths |
| Principal, bare-subject issuers, unchecked parts | `SF01 migration` / removed Rust API; verifier→dispatcher AuthScope→authorizer Grant→native submit | Authorization example, lease invalidation, permit ordering, nominal permission/actual target/single consume |
| Implicit `/health` | Register explicitly with public policy; unregistered path is 404 | High/Low native and socket tests |
| Implicit `/stream` and `/ws` | Removed; complete bounded bytes require explicit registration. General streaming/WS remain unimplemented | No implicit route; equivalent WS business remains outstanding |
| Raw `html`/`Html`/`http.html` | Standard entry rejected; text displays notation, active HTML awaits SF04 typed migration | Raw-entry rejection, octet-stream/text and nosniff. Equivalent HTML screen migration remains outstanding |
| Arbitrary Set-Cookie/CORS/CSP/cache/challenge/framing headers | append returns Invalid; dedicated later security layers own them | Managed-header and wire regressions; Cookie/CSRF/CORS features remain outstanding |
| Arbitrary Host / startup with unconfigured Options | Before startup call `try http.authority(options, https_origin, wire_list, count_limit, byte_limit)`; missing, duplicate canonical or invalid settings fail. Authority/proxy overwrites also fail | Parser/config, gate before all routes, actual socket/peer Host slice. Full Cargo/four-OS and TLS/browser acceptance remain separate |
| HTTP/1.0, absolute/authority-form, `*`, interpreted Forwarded | Migrate to HTTP/1.1 origin-form. Register omitted/explicit ports separately; strip Forwarded headers and configure real peer IPs with `trusted_proxy`. Runtime rejection is 400, HTTP/1.0 is 505, before 404/405 | Small bounded Host/port/IPv6/duplicate/peer negative requests and capacity recovery on the next valid request |

The Host change is an unreleased SF03 slice. Migrate arbitrary Hosts, trailing dots, special port spellings and internal proxy Hosts to explicit registration/rewriting. Setters use move and Result; failure cannot revive earlier Options. APIs use the existing resource registry, checked facts and sealed emission, with the same contract for High, saved Low and handwritten Low.

Local examples set the listener port and wire Host to the same value. The default supervised-service uses port `8090` and `localhost:8090`. If you change the port, update `NAGI_HTTP_AUTHORITY` too. This variable is one HTTP Host authority, separate from the HTTPS external origin. Unix:

```sh
NAGI_SAMPLE_PORT=8090 NAGI_HTTP_AUTHORITY=localhost:8090 nagic run --project test-nagi-code/library-examples/supervised-service
```

PowerShell:

```powershell
$env:NAGI_SAMPLE_PORT = '8090'
$env:NAGI_HTTP_AUTHORITY = 'localhost:8090'
nagic run --project test-nagi-code/library-examples/supervised-service
```

HTTPS origin and backend Host are separate startup facts. Plaintext loopback `serve` alone does not complete browser authentication. A local TLS frontend can use origin `https://localhost:8443`, wire `localhost:8080`, real peer `127.0.0.1`; verify certificate trust, Forwarded stripping and HTTP/1.1 termination in deployment. No Secure Cookie exception or Host-based CSRF exemption is provided. Origin/CSRF/pre-login implementation is outside this change.

Handlers have `async (http.Request,shared[S],A)->Result[http.Response,E]`. Replace automatic body/query/path extraction with explicit parsing/decoding and `http.json` responses. Existing CRUD and Result behavior is mapped to migrated native tests. Registration remains fallible: duplicate method/path and ambiguous capture patterns return runtime registration Err before startup. Dynamic paths remain part of the existing standard API; decorator-specific static path checking is not replaced with a new string-inference language specification.

Owned local Option/Result and same-task async calls remain available for AuthScope/Grant. Owned delegation into another Task or Actor is a breaking change rejected by SameTask. Long-running work needs independently authorized business commands rather than a retained request proof queued for future use. Protected operations issue a permit under the invalidation gate after capacity wait. Cancellation/rollback of already admitted commands is not guaranteed.

[Minimal HTTP](http.md) · [Authentication and failure table](security.md) · [API reference](http-server.md) · [Migration test map](../internal/security-foundation/sf01-compiler-migration-map.md)

Move, Task/spawn semantics and SQLite Pool/Tx acquisition budgets, close, actual join and Outcome are preserved. Unifying old Db/dynamic SQL belongs to SF05. Custom Rust/Axum hosts remain explicit trusted boundaries outside the standard dispatcher guarantee; no compatibility layer automatically falls back to them.

## SQLite SF05

| Old entry | Checker/Rust migration | Equivalent business verification |
|---|---|---|
| Db, db_open/exec/all/query/insert/update/write | SF05 migration; remove public runtime Db/Sql. Explicit Options, Pool/Tx, literal Query, Parameters | Original High/saved-Low/handwritten-Low positions and native NULL/owned/manual rows |
| string/view in query/all/exec | Require Query returned by direct `sqlite.literal("...")` | Query storage/selection/return and native binds |
| Variables/concatenation/formatting/dynamic str in constructor | Reject at argument; bind values and select reviewed finite Queries | Aliases, direct imports, unrelated user names, positive/negative paths |
| INSERT/UPDATE RETURNING, multi-statement exec | exec+readonly query in the same Tx; anonymous ? in SQL occurrence order; trusted fixed bootstrap | Native CRUD/inventory/task/device-settings/result API |
| Protected SQL | Bind actual Grant subject/target into owner/tenant predicate; reserve then synchronously enqueue inside Grant.submit | Other tenant/target affects zero; earlier revoke enqueues zero; admitted work executes; revoke while waiting capacity |

Ordinary check needs no SQL engine. Opt-in checks prepare direct Query and Parameters builder chains, leaving unknown structure and actual types/NULL to runtime. FromRow and fixed Rust SQL remain trusted adapter boundaries, without reexporting request-facing dynamic factories. Session generation checks in the same Tx remain SF02 work. Adding a Grant to a generic query cannot promise tenant isolation.

[SQLite](sqlite-pool.md) · [Protected operations](security.md#protected-sqlite-operations) · [SF05 contract](../internal/security-foundation/sf05-contract.en.md)
