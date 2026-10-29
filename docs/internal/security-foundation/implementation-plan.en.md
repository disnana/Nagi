# Security Foundation: implementation and verification plan

[RFC](rfc.en.md) · [日本語](implementation-plan.md) · [Baseline audit](baseline-audit.md)

## 2026-10-19: integration PR #133 and cause-specific CI repairs

[Integration PR #133](https://github.com/disnana/Nagi/pull/133) remains Draft. Superseded #121, #122, #123, #126, #131 and #132 were closed with their history retained. The October 18 observations below apply to their own earlier HEADs, not current acceptance.

The initial PR HEAD `688ea61a62630d50597213e6ba006e5dac4bef5e` failed required CI, including four-OS and real-browser checks. The Session 20ms fixture now separates a deferred DB touch spending that budget (504) from mapper/finalizer behavior after the touch completes during preparation; deadlines and original refusals remain. Cause-specific, independently reviewed repairs restore fixed HTTP 500/408/413 bodies and finite fixture capacity, and make a test-only path reject conflicting Cache-Control before canonicalization. Local default-parallel runtime execution passed 860 tests with no failures; the explicitly invoked Task cost measurement also passed. This does not establish final-HEAD four-OS acceptance.

[Dedicated validation HEAD `0b9e4f24`](https://github.com/disnana/Nagi/actions/runs/39228101806) passed focused HTTP/Session checks on all four OS. An intermediate WebKit wrong-host check failed because certificate-specific refusal evidence was insufficient. [Real-browser CI for `b7e38467`](https://github.com/disnana/Nagi/actions/runs/39234029025) retained the strict acceptance conditions and passed 12 TLS/business executions and 12 resets across three engines, with actual child-process termination confirmed. The additional change provides closed failure diagnostics; the earlier cancellation's cause remains unknown. This success is not evidence of a causal fix. Generic cancellation, timeout and teardown events remain insufficient proof of certificate refusal. Real-browser success is now a required publication gate, but these results apply to their own validation HEADs and do not replace final integrated-HEAD acceptance. Temporary validation workflows are excluded. Nonreproduction of the earlier Windows login failure is also not a causal resolution.

Remaining acceptance requires the final integrated HEAD's mandatory CI, four OS, High/independently saved Low/handwritten Low/native, HTTP/TLS/real browsers, distributions, performance/resources, Japanese/English Docs, independent final review and required GUI acceptance. IDE automated tests and Plugin Verifier do not establish GUI success. Published Nagi remains 0.1.11; the approved candidate 0.2.0 version change does not authorize merge, tags, release or Marketplace publication.

## 2026-10-18: integration candidate and remaining acceptance

**Separate adopted contracts, candidate implementation, execution evidence and formal publication.** The confirmed main HEAD is `7da0068254c1895ed4e6f2c824aab45eb2fbbf2c`. The changes developed in PRs #121, #122, #123, #126, #131 and #132 are consolidated into a single main-targeted 0.2.0 integration PR. Retain the original PRs as implementation and review history; bind acceptance to the integration PR's latest HEAD and required verification. The dedicated validation branch below is an intermediate evidence snapshot, neither merged nor a formal distribution.

Earlier scoped snapshot `4337422f5f632fe73e65d512a9ea170001301952` connects Session/CSRF/CORS, the Ed25519 verification-only profile, typed HTML/SQL, registered outbound HTTP operations, SF07's borrowed ResponseContext, Result factories/mappers and explicit route rates/budgets, and the SF08 browser fixture. [Scoped CI 39126273963](https://github.com/disnana/Nagi/actions/runs/39126273963) passed production runtime typechecking, compilation of every runtime test target, one actual protected HTTP/TLS operation, 14 Ed25519 core tests and one public test, and standalone fixture typechecking (`cargo check --locked`) using Cargo's refreshed dependency graph. These results do not substitute for running all tests, four-OS validation or real-browser acceptance.

The integration PR combines the complete changes from candidate `9dd70dc73b2722fb537edc34452916030670d027` with reviewed source-gate, SF06 generated-fixture, SF08 browser-fixture and Japanese/English documentation fixes. Temporary validation workflows are excluded. The Nagi 0.1.11→0.2.0 version change is explicitly approved for this PR, without changing external dependency versions. A version change is not formal publication.

[The 9dd CI](https://github.com/disnana/Nagi/actions/runs/39128739265) passed IDEA/PyCharm stable and EAP, Plugin Verifier, the common ZIP, MSRV Ed25519, Docs and VSIX; its four-OS result failed, including source-gate and generated-fixture failures. After fixes, scoped runs passed [real TLS/Grant execution from High, independently saved Low without High, and handwritten Low](https://github.com/disnana/Nagi/actions/runs/39130975356), and [12 real TLS/business executions across three browser engines](https://github.com/disnana/Nagi/actions/runs/39129492943). [Scoped Windows CI](https://github.com/disnana/Nagi/actions/runs/39131370586) passed three send-deadline tests, two stop/join tests and the SF07 wire groups, but failed the private Session head_finalizer diagnostic within its 20ms handler budget. Deadline expiry during DB commit was observed; this neither proves the same cause as an earlier failure nor resolves it. Diagnosis and cause-specific regression validation remain. These are scoped results for their own HEADs, not final integrated-HEAD acceptance.

Release still requires the final candidate's four OS/MSRV, High/saved-Low/handwritten-Low native paths, Session/login/cancellation/release regressions, real browsers/TLS, distributions, performance/resources, Japanese/English Docs/migration and independent final review. A scoped nonreproduction does not resolve the known Session 504 or B1 short-TTL failures. Do not rename unverified work a limitation and remove it from acceptance. Candidate validation and version-change approval do not authorize merging, tags, releases or Marketplace publication.

The 2026-10-10 status and CI observations below are historical; they do not describe this candidate's current implementation or CI results.

2026-10-10 UTC. **Plan, not whole-Foundation completion.** GitHub main `e609aba158921226a632f16d41eb8b0f4ad5aebd` includes merged SF00 #100 and SF01 #101, plus SF05's canonical Query/Parameters implementation and the bounded SF07 reader change. SF02/SF03/SF04/SF06 remain incomplete; SF07 cross-cutting budget acceptance and SF08 remain incomplete. Formal 0.2.0 is unreleased and published 0.1.11 is unchanged. Merged code alone does not mean the corresponding Foundation acceptance is complete. Keep the [SF01 contract](sf01-contract.md) and results, and SF05 [contract](sf05-contract.en.md) and [evidence/remaining checks](../security-foundation-sf05-results.md), as separate records of scope and limits. [Revised D1–D3 decisions](decisions-and-migration.en.md) govern implementation; previous compatibility deferrals do not require renewed approval. Fix feature signatures/contracts before RED tests. Independent internal investigations, dependency comparisons, and harness design can proceed. IDs below are candidate units, not assigned GitHub PR numbers.

The 2026-10-10 14:04 UTC GitHub readback confirms that Linux, all four OS packages, four IDE Stable/EAP checks, the common ZIP in main checks `38055073417`, and the website passed. `publish-release` failed because CHANGELOG lacks the required `## JetBrains 0.1.3` heading; no 0.1.3 tag or Release was created. Do not retry publication before approval. CodeQL run `38055084000` reported `Extraction failed: No source files found` during C/C++ extraction. This is not an all-checks-passed result.

## 2026-10-16: adopted SF07 finite-budget policy

The user's explicit replies adopt Q1's public API and explicit configuration for every route, Q2's mandatory budget settings, and Q3's finite fallback policy. This records a design decision, not completed public runtime/compiler integration or acceptance. The CI/publication observations dated 2026-10-10 above are historical observations.

- In 0.2.0, finite budgets require explicit settings. Missing required settings are diagnosed and refused before listen; `default_options` does not silently supply the new budgets.
- 4096 is not a fixed public ceiling. Check counts, retained bytes, exploration work, response retention and fallback capacity with checked arithmetic; verify finite startup allocation and the implementation's supported acceptance range. Configurable does not mean unlimited.
- Japanese and English Docs must provide measured small configurations, measurement conditions and tuning steps. Illustrative numbers are neither implicit defaults, recommendations for every App, nor an SLA.
- Keep acceptance for bound formulas, boundary refusal, budget retention/release, final close when finite fallback is exhausted, concurrency, and four-OS performance. Preserve existing deadlines, cancellation, actual join, CORS/managed headers and the release gate.
- SF07 remains incomplete until public Context/factory/mapper/rate integration, High/saved-Low/handwritten-Low native evidence and integrated independent review are complete. Always producing an HTTP response and universal OOM recovery are not guaranteed.

## PR boundaries and dependencies

| PR | Scope | Dependencies/decisions | Completion evidence |
|---|---|---|---|
| SF00 | Baseline/RFC/migration/acceptance/independent design review | Main; revised security-first D1–D3 decisions/migration | Source/CI/PR readback, links/site, review tracking; no new-feature GREEN claims |
| SF01 | Resource/failure metadata, AuthScope/single Grant[P], policy-required HTTP, lifecycle, common budget/managed-header boundary | SF00, D1/D2/API ADR | Forgery/sharing/escape/permission/request-ID rejection, linearized single-use execution-permit issuance/invalidation/admission/Drop, methods/fallbacks/dynamic routes/old-entry migration diagnostics and removal/migrated native behavior |
| SF02 | Credential verifier adapters, Cookie/Session store/rotation/revocation, crypto/cookie comparison, secret handling | SF01/SF05, D3/durable SQLite store/dependencies | Ambiguous credentials/claims, rotation/logout/expiry/capacity/restart/crash/clocks/multi-process generation conflicts, live/total-row limits with delayed cleanup, same-file predicate placement, physical versus logical bounds, fail-closed/unknown outcome, issuance/token no-store and conflicts, real TLS browser flow |
| SF03 | Separate CSRF/CORS, origin/proxy trust, preflight, early-response finalization | SF01/02, credential-source/origin contracts | Unsafe cookie requests including public login/logout, token/origin, wildcard/credentials, Vary, denial without handler calls and resource leaks |
| SF04 | Typed HTML subset, URL attributes, CSP/nosniff, raw-standard removal and response-bypass rejection | SF01/subset/header contract | Context/unsupported syntax/re-encoding/output bounds; browser structure and High/Low native wire |
| SF05 | Unify standard SQLite with literal Query/Parameters, remove Db/dynamic entries, protected target binding and management migration | SF01/Query API; #99 already merged | Checker-rejected dynamic construction; native literals/binds/schema/shape/NULL/outcomes/authorizer/predicates |
| SF06 | Policy-constrained outbound HTTP; URL/DNS/socket/proxy/TLS/pooling library comparison | SF01/dependency selection | Actual socket bound to checked address, retries/pooling/no redirects/budgets/cancellation; mocks alone insufficient |
| SF07 | Cross-cutting budgets, bounded rate limiting/caches/observability/cleanup, necessary peer/proxy boundary | Budgets mandatory in each feature PR, then SF01–06 integration | Small deterministic capacity/deadline/release/recovery oracles, unchanged existing defenses |
| SF08 | Migration/public API diff/docs/examples/IDE/extracted package/integration/final independent review | SF01–07 CI/review | Every acceptance item linked to latest-head evidence; zero unresolved blockers. Release requires separate approval |

Recommended order: SF00→SF01→SF05→SF02→SF03, then independent SF04/06, then SF07→SF08. Separate investigations may overlap, never concurrent edits of common checker/registry/plan files. Retain Hyper transport; a wholesale Axum migration is outside scope.

Use latest main for PRs; identify stacked dependencies if necessary. Do not count a feature-branch merge as main integration. Dependent work requires preceding contracts/required CI/review acceptance. Do not automatically merge, tag, formally release, or destructively alter existing branches.

## Per-PR workflow

1. Adopt signatures, ownership, lifecycle, failures, budgets, and compatibility in contract tables.
2. Save positive/minimized negative/runtime RED tests. Compiler negatives require successful parse/name resolution, intended checker rejection, diagnostic fragment, and primary source line. Undefined APIs/panics/parser errors/unrelated limits do not satisfy semantic negatives.
3. Implement registry/checker facts/sealed plans/runtime boundaries, without security inference by display names or emitter.
4. Check/build/run High→Low→Rust, independently loaded saved Low, and handwritten Low. Include metadata/aliases/shadowing/wrappers/callbacks/phantoms/native replacement.
5. Run targeted regressions, necessary full workspace/fmt/clippy, bounded generation/mutation, native E2E, and four-platform CI. Zero-test filters fail.
6. Keep Japanese/English API docs, limitations, diagnostics/fixes, DESIGN/ADR/examples synchronized. Distinguish executable examples from proposals.
7. Independent Sol High review after implementation, fixes, relevant revalidation, latest-head CI readback. Implementer success reports do not replace review.

Compare actual locked dependency sources, licensing, MSRV/features/platform/TLS/backend, maintenance/advisories, and constraints before crypto/cookie/URL/client adoption. A crate name is not evidence of safety.

## Execution matrix

| Route | Required observations |
|---|---|
| High | All positive/negative contracts and original High/module primary lines; supported accepted programs build and run |
| High→generated Low→Rust | Final native Low integration check, same-compilation High mapping, ordinary CLI build/run and matching cleanup |
| Independently saved Low | Recheck without High facts; Low locations; every feature built/run independently |
| Handwritten Low | Independently authored positive/negative inputs, native same runtime, no policy omission bypass |
| Native Low/`@replace` | Final signatures/facts checked; native/replace provenance; no implicit legacy downgrade |
| Trusted Rust adapter | Public consumer signatures/ownership and locked traits/Send/Sync; separate verifier/store/driver evidence |
| Extracted distribution | High/Low diagnostics/build/run outside checkout with bundled runtime/lock, complete feature sample |

Plan a manifest-backed security runner with its own stage/location/bad-oracle tests. Do not weaken expectations to parse failures or count compile-only/stub paths as runtime evidence.

## Bounded defensive tests

Use owned local fixtures, small inputs, fake clocks, deterministic barriers, and finite queues/workers. No third-party attacks, exploit/PoC construction, unbounded generation, exhaustion/stress testing, or paid scans.

| Feature | Oracles |
|---|---|
| Auth | Fake/missing/wrong nominal permission/reuse/wrappers/sharing/Task/Actor/module identity; request matching/expiry/Drop; zero protected calls on denial, actual target on success. Barrier invalidation during reservation or after earlier checks but before permit issuance: zero native enqueue; inverse order permits admitted work without rollback claims. Repeat at SQL/client queue/socket adapters |
| Route | Missing/mismatched/dynamic/alias/mapper policy; HEAD inheritance/explicit policy; preflight versus ordinary OPTIONS; no injected legacy builtins |
| Credentials | Ambiguous sources; bounded token/key/algorithm/issuer/audience/exp/nbf; trusted keys; secrets absent from public diagnostics; actual verification separate from typing |
| Session | Cookie ambiguity/attributes; RNG failure; expiry/clock/rotation/logout/concurrent atomicity/store outage/reply loss; small full store; issuance/rotation/authentication/token wire no-store and conflicting cache/304 reuse rejection; restart/multi-instance limits |
| CSRF | Unsafe cookie/public login/logout; missing/wrong/other-session/rotated token; malformed/missing/null/duplicate/wrong origins; unique form/header token; denial cleanup |
| CORS | Allowed/denied origins, wildcard/credentials, methods/headers, preflight metadata, early errors, Vary; browser readability distinct from non-browser transport |
| XSS | Benign DOM context/attributes/URLs, unsupported contexts, encoding/Unicode/NUL/bounds/CSP; structural assertions rather than executable attack demos |
| SQL | Literal structure/binding, rejected dynamic/format constructors, shape/schema/NULL/authorizer, bound grant predicates; assert native rows/changes and preserve legacy outcomes |
| SSRF | Deterministic resolver/connector plus owned small TLS endpoint: address families, pool/retry/redirect/proxy; zero connections on denial; checked address/hostname/TLS observed on success. Never contact real metadata/private third-party services |
| DoS/lifecycle | One/few entries/permits/workers, boundary±1 and fake time, cancel/Err/panic/shutdown, live counters/actual join/next healthy request; notification is not completion |

Browser integration uses real TLS and controlled origins; record Chromium/Firefox/WebKit versions and executed targets. Unavailable browsers remain unverified, not replaced with mock “success”. Add four-platform runtime/native coverage to Linux/Windows/macOS ARM/Intel and a Linux browser job with explicit supported matrix.

## Regression, cost, and release acceptance

Deliberately replace only accepted legacy HTTP/auth/SQL/raw-output cases listed in the [migration matrix](decisions-and-migration.en.md#migration-and-acceptance-matrix) with migration diagnostics plus equivalent migrated business behavior. Preserve unrelated ownership/move/views/resources/sealed facts/auth/Task contracts and 16 native bridge oracles/legacy spawn/Supervisor/HTTP/SQL/Tx/frontend/output/cache/editor/examples/distribution coverage. Use existing commands as applicable:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked -p nagic --example fuzz-smoke
python scripts/verify_compiler_contracts.py
python scripts/verify_task_handle_contract_inputs.py
python scripts/verify_application_examples.py
python scripts/verify_onboarding_examples.py
python scripts/verify_sqlite_example.py
cd website
npm ci --ignore-scripts
npm run build
```

Separate infrastructure failures from contract failures and previous-main CI from new-head CI. Skipped jobs are not executions. A docs-only SF00 normally needs docs/site/change-detection gates, not a fresh Rust full suite.

Compare generated Nagi and manual Rust under equal guarantees/inputs/dependencies/targets. Measure allocation/retention/cancel retention/Future size, check/Low/check/compile/binary cost and finite ordinary-case timing. State worker-heap exclusions/cache effects/distribution/limits; never disable security to improve results or retain secrets in artifacts.

Before declaring 0.2.0 Foundation complete, link each item to feature PR/latest source head/test/artifact:

- Revised D1–D3, complete old-entry export/migration inventory, APIs/failures/ownership/expiry/routes/sources/budgets, matching public API diff and bilingual current/direction/unsupported tables; no deprecated parallel, policy-free standard, raw HTML/header/SQL bypasses.
- AuthScope/routes/CSRF/typed HTML/SQL/actual-connection SSRF/CORS/Session/DoS positive/negative/lifecycle oracles GREEN, without new check/build mismatch.
- Complete High/generated/saved/hand Low/native integration/extracted-package matrix, diagnostics, wire/DB outcomes, denial counters, resource/actual join evidence.
- Full existing regressions/fuzz/latest-head four-platform CI/site/package, explicit skipped/ignored/infrastructure limitations.
- Browser/TLS/proxy/session/SSRF integration under documented deployment assumptions; no mocks or weaker bare HTTP comparison substituted for guarantees.
- Matching bilingual docs/DESIGN/ADR/executable app/migration/diagnostics/cost/dependencies/licenses/changelog/release-note drafts.
- Independent final contract/lifecycle/concurrency/error/security/compatibility/coverage review; zero unresolved major/high-priority findings. Unverified claims stay outside guarantees.
- Recorded release blockers/approval. **No version bump, tag, or formal release before explicit user approval.** After approval use existing immutable release/package/extraction/installer procedures.

Do not hide mandatory unimplemented features as mere “limitations”. Explicitly adopted scope changes must update RFC and acceptance before deferring features.

## Review and handoff

Prefer Luna Max for routine implementation/fixtures/docs; use Sol High for difficult security/compiler/runtime boundaries and independent reviews after implementation. Escalate to xHigh only with unresolved architectural reasons. No routine Max/Astra, competing same-problem agents, or duplicate reviews. Usually at most three including parent, one active model/tier.

[Review log](review-log.md) records IDs, priority, source/contract, disposition, evidence, recheck, and blockers. Feature PRs keep separate artifacts. Handoffs preserve source head/adopted decisions/completed PRs/unfinished work/order/limitations without rewriting historical snapshots.

### SF06-COMP-021: HTTP response content decoding (planned for 0.2.1)

**Status: registered, not started; completed implementation/acceptance items: 0.** This depends on the SF06 outbound HTTP URL, destination, TLS, and budget contracts and the 0.2.0 identity-only policy. Preserve adopted SF06 Q1 transport behavior: finite TCP failover, with TTL=0 allowing one operation. That transport rule does not authorize retry after content-decoding failure.

For 0.2.0, accept identity encoding only and fail on non-identity `Content-Encoding` without implicit body decoding. Do not substitute decoded-byte checks for encoded-byte checks, bypass limits, or automatically retry failures. This is unreleased design policy; it does not describe published 0.1.x behavior or claim that an HTTP client is implemented. Preserve existing public API signatures and adopted safety contracts in the follow-up.

For 0.2.1, select supported codecs only after independent dependency, threat, and performance review. Define separate encoded-size, decoded-size, memory, and processing-time limits for each selected implementation, including values and enforcement boundaries. Codec list, concrete values, and a universal strict CPU-time guarantee remain unselected and unapproved. Malformed input and cancellation must stop safely; join the worker to actual completion before freeing buffers and decoder resources. A timeout notification or cancellation request alone is not completion. Keep existing API signatures and never return partial decoded data as success after rejection or failure.

**Acceptance:** test encoded/decoded limits at boundary±1 and overflow, malformed streams, memory/time budgets, and a small owned cancellation barrier. Verify actual join, resource release, and the next healthy request. Unsupported encodings fail without decoding or retry while identity-path safety checks remain intact. Link four-OS/dependency-specific evidence, independent review, bilingual docs, and generated website to completion. Implementation and independent review have not started. Website/CI generation has not run here.

