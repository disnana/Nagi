# Security Foundation: implementation and verification plan

[RFC](rfc.en.md) · [日本語](implementation-plan.md) · [Baseline audit](baseline-audit.md)

2026-10-08 JST. **Planned work, not completed implementation.** D1–D3 and public signatures must be recorded in adopted ADRs before adding APIs/guarantees. Independent internal investigations, dependency comparisons, and harness design can proceed. IDs below are candidate units, not assigned GitHub PR numbers.

## PR boundaries and dependencies

| PR | Scope | Dependencies/decisions | Completion evidence |
|---|---|---|---|
| SF00 | Baseline/RFC/migration/acceptance/independent design review | Main; D1–D3 proposed | Source/CI/PR readback, links/site, review tracking; no new-feature GREEN claims |
| SF01 | Resource/failure metadata, AuthScope/ScopedGrant, policy-required HTTP, lifecycle, common budget/managed-header boundary | SF00, D1/D2/API ADR | Forgery/sharing/escape/permission/request-ID rejection, linearized single-use execution-permit issuance/invalidation/admission/Drop, methods/fallbacks/dynamic routes/legacy compatibility |
| SF02 | Credential verifier adapters, Cookie/Session store/rotation/revocation, crypto/cookie comparison, secret handling | SF01, D3/store/dependencies | Ambiguous credentials/claims, rotation/logout/expiry/capacity, fail-closed/unknown outcome, issuance/token no-store and conflicts, real TLS browser flow |
| SF03 | Separate CSRF/CORS, origin/proxy trust, preflight, early-response finalization | SF01/02, credential-source/origin contracts | Unsafe cookie requests including public login/logout, token/origin, wildcard/credentials, Vary, denial without handler calls and resource leaks |
| SF04 | Typed HTML subset, URL attributes, CSP/nosniff, raw compatibility docs | SF01/subset/header contract | Context/unsupported syntax/re-encoding/output bounds; browser structure and High/Low native wire |
| SF05 | Literal Query/Parameters and protected target binding, SQL migration | SF01/Query API; #99 already merged | Checker-rejected dynamic construction; native literals/binds/schema/shape/NULL/outcomes/authorizer/predicates |
| SF06 | Policy-constrained outbound HTTP; URL/DNS/socket/proxy/TLS/pooling library comparison | SF01/dependency selection | Actual socket bound to checked address, retries/pooling/no redirects/budgets/cancellation; mocks alone insufficient |
| SF07 | Cross-cutting budgets, bounded rate limiting/caches/observability/cleanup, necessary peer/proxy boundary | Budgets mandatory in each feature PR, then SF01–06 integration | Small deterministic capacity/deadline/release/recovery oracles, unchanged existing defenses |
| SF08 | Migration/public API diff/docs/examples/IDE/extracted package/integration/final independent review | SF01–07 CI/review | Every acceptance item linked to latest-head evidence; zero unresolved blockers. Release requires separate approval |

Recommended order: SF00→SF01→SF02→SF03, then independent SF04/05/06, then SF07→SF08. Separate investigations may overlap, never concurrent edits of common checker/registry/plan files. Retain Hyper transport; a wholesale Axum migration is outside scope.

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

Preserve ownership/move/views/resources/sealed facts/auth/Task contracts and 16 native bridge oracles/legacy spawn/Supervisor/HTTP/SQL/Tx/frontend/output/cache/editor/examples/distribution coverage. Use existing commands as applicable:

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
python website/build.py
```

Separate infrastructure failures from contract failures and previous-main CI from new-head CI. Skipped jobs are not executions. A docs-only SF00 normally needs docs/site/change-detection gates, not a fresh Rust full suite.

Compare generated Nagi and manual Rust under equal guarantees/inputs/dependencies/targets. Measure allocation/retention/cancel retention/Future size, check/Low/check/compile/binary cost and finite ordinary-case timing. State worker-heap exclusions/cache effects/distribution/limits; never disable security to improve results or retain secrets in artifacts.

Before declaring 0.2.0 Foundation complete, link each item to feature PR/latest source head/test/artifact:

- Adopted D1–D3, APIs/failures/ownership/expiry/routes/sources/budgets, matching public API diff and bilingual current/proposed/unsupported tables.
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
