# 0.2.0: reassessing D1–D3 and migration contracts

[日本語](decisions-and-migration.md) · [RFC](rfc.en.md) · [Implementation plan](implementation-plan.en.md)

2026-10-08 JST (decision snapshot; status updated 2026-10-10). **D1–D3 are adopted implementation decisions, not whole-Foundation completion.** This document's initial snapshot had PR #100 as a design PR and main at `62bbda9`. Since then PRs #100/#101 have merged, and current main `e609aba158921226a632f16d41eb8b0f4ad5aebd` contains SF01 and SF05 implementation. SF02/SF03/SF04/SF06, SF07 cross-cutting budget acceptance, and SF08 remain incomplete. Published 0.1.11 is unchanged; formal 0.2.0 is unreleased. Do not repeat approval requests solely because previous documents deferred these necessary breaking changes. Feature PRs must specify signatures, dependencies and native oracles; record and reassess newly discovered significant risks or contradictions.

## Historical design baseline and selection criteria

At historical design-baseline main `62bbda9`, [route/route_mapped](../../../runtime/src/http_server.rs):596 lacked policy; [legacy serve](../../../runtime/src/lib.rs):240 had a separate dispatcher and injected endpoints. [Principal/Grant](../../../runtime/src/auth.rs):29 factories/consumers required no lease; [ADR 001](../adr/001-backend-boundaries.md) allowed owned delegation as a minimum experiment. [Raw HTML](../../../runtime/src/http_server.rs):430 and [Db.exec](../../../runtime/src/database.rs):120 were standard entry points then. Their existence alone is not a vulnerability finding, but preserving them permitted execution without using the stronger standard contracts.

Select complete standard-entry coverage, rejection on failure, one contract implementation, bounded resources and observable runtime behavior. Size of removal is not an achievement. Reuse Hyper/Tokio/rusqlite, resource registries, sealed plans, Pool/Tx and Task. No wholesale transport/scheduler replacement, general taint/regions or arbitrary Rust security inference.

## D1: one policy-required HTTP path

**Recommend breaking old signatures and unifying standard App/route/serve.** Withdraw parallel legacy/new apps because they retain policy-free registration and two dispatch specifications. Decorator-only protection misses dynamic/Low routes.

Require an actual public/authenticated/authorized policy per method/path, without implicit public. Anonymous access is legitimate but never disables limits/CSRF/CORS. Canonical metadata and sealed plans connect handlers that can obtain AuthScope, protected operations requiring Grant and mappers. Dynamic registration validates policies fallibly. The checker does not prove an application policy is not unconditionally permissive.

Remove policy-free arities, legacy decorators/serve, unchecked registration, raw-app conversions and feature-flag restoration from standard APIs. HEAD/OPTIONS/404/405/early errors use the same dispatcher/finalizer. Remove injected `/health`/`/stream`/`/ws`. Applications explicitly register health; migrate the stream demonstration to a finite bytes/response example. Standard WebSocket and arbitrary streaming APIs are absent in 0.2.0; reject old entries with migration diagnostics. A trusted Rust host alternative requires its author to review authentication/termination. Record lost functionality in migration/blocker tables rather than hide it behind a legacy path.

Require typed HTML fragments and remove raw Html factories. Finalizers own body classification and Content-Type/CORS/Session/CSP headers across bytes/mappers/append_header, preventing active HTML/JS/SVG or managed-header bypasses. Retain non-active byte output; do not promise classification of secrets or safety of every download.

Unify standard SQL execution around fixed Query structure and Parameters. Remove legacy Db and dynamic Sql/string factories. Migrate Db users to Pool and explicit Options without importing old queue/timeout defaults. Trusted-host management owns schema migration/bootstrap; never re-export an arbitrary request-facing SQL constructor. Literal Query still does not prove SQL meaning or tenant authorization.

## D2: AuthScope and a single request-bound Grant

**Replace Principal with AuthScope and change Grant[P] to a request-bound contract.** No separate ScopedGrant plus indefinite Grant. Generic Grant[P, S] was compared: actual i64 target and private request ID/lease express current needs without general target-payload/region machinery. Retain nominal P, excluding it from actual payload checks. This concretizes the [planned actual Scope direction](../compiler-rust-boundary-plan.md#auth-scopeの将来方針) and supersedes its compatibility deferral for 0.2.0.

AuthScope carries verified subject, credential source, request ID, expiry and private lease. Grant binds that lease, subject, target and permission, consumed once. Remove subject-only public Rust factories and unchecked parts extraction. A trusted verifier's identity result is not itself a proof: the dispatcher binds it to a live request context, and an authorizer uses that AuthScope after reviewed policy success. No public factory can freely create, extend or reactivate leases. False verifier/policy assertions remain the trusted issuer boundary, not a static guarantee.

Proofs are non-Copy/Clone/Serde/shared/owned-field and same-task. Same-task arguments/returns and owned local Option/Result are allowed. Reject Task/Actor/background, nested shared state and transfer of Futures capturing proofs. Break old owned delegation: transfer non-proof job input and independently reauthorize through a trusted service adapter. Do not add general service-proof APIs or request-to-indefinite-proof conversion in 0.2.0.

Invalidate on request completion/Err/panic/timeout/cancel/unpolled Drop. Expiry is the minimum of request, credential and session absolute deadlines; awaits and idle touches do not extend it. After bounded capacity waits, check time/validity and issue a private single-use target/operation-bound execution permit under the same short gate used for local invalidation. Issuance is the local linearization point, not queue reservation/Future construction. Native protected commands require permits, including trusted consumer admission APIs; bare parts do not grant access to a standard protected operation.

Already admitted DB/external I/O is not rolled back by cancellation/expiry. DB authority changes or session revocation commits do not share this local gate. Updates requiring immediate consistency need authority/session-generation predicates in the same DB transaction. Session tables and protected data must share one SQLite file and be accessible in that Tx. The current authorizer rejects ATTACH; a separate-file/store lookup cannot provide the same guarantee. No static guarantee covers every external effect or arbitrary Rust adapter implementation.

## D3: durable server sessions

**Use existing SQLite Pool/Tx for durable server sessions and opaque host-only cookies.** Do not make memory-only production storage standard because restart/multiple processes split state. Reject stateless signed cookies because revocation/rotation adds server state and a second specification. No external-store public interface until distributed requirements and native oracles justify it. Memory fakes remain private test-only.

- IDs are 256-bit OS-CSPRNG values. Store an ID digest produced by a reviewed hash primitive, not raw bearer IDs. Reject entropy/generation failure; no secret Debug/Serde/log. Digests do not prevent DB modification, backup compromise or disclosure of CSRF secrets.
- Lookup checks ID/generation/idle/absolute expiry and atomically performs bounded idle touch. Never extend absolute expiry. Rotation compares the old generation and atomically invalidates the old ID and inserts the new one in one transaction. Only one concurrent rotation succeeds; losers send no new cookie.
- Independently bound live sessions, total retained rows including expired/revoked/rotation records, and per-row bytes. Insertion/rotation and cleanup transactionally check the total; reject insertion when bounded cleanup cannot keep it within the limit. Process-local counts are insufficient. Retain revocation information only for a finite period no longer than the original absolute expiry; missing IDs still reject after deletion. Never deliberately reuse IDs. No unbounded standard audit/history table. Cleanup batches/queues and anonymous pre-login sessions are finite. Fail issuance instead of silently evicting another valid session.
- Logical record/byte limits are not physical DB-page/journal/WAL limits. Distinguish SQLite max_page_count from journal_size_limit reclamation targets; the latter is not a hard cap on active WAL. Filesystem quotas and equivalent deployment controls provide physical disk bounds; disk-full/I/O failures fail closed. Never silently modify shared-DB settings. SF02 fixes supported settings, long-reader/checkpoint assumptions and quota diagnostics. Verify configuration/failure classification with small ordinary inputs, without disk-exhaustion experiments.
- Revoke/logout commits invalidate old IDs/generations. **New lookups beginning after commit reject them.** Previously looked-up request snapshots remain bounded, without an immediate cross-process broadcast promise. Local cancellation invalidates through the local gate. Only protected updates with session tables and protected data in one SQLite file, using session-generation predicates in that same Tx, carry that consistency guarantee. Separate-file/store prechecks are insufficient; do not bypass the authorizer through ATTACH.
- Reject store timeout/busy/corruption/crypto failure; distinguish invalid credentials (401) from operational errors (such as 503). Never fall back to memory storage or revive old IDs on unknown outcomes. Lost/cancelled rotation-commit replies produce OutcomeUnknown and reauthentication, with no new cookie before observed commit. Existing accepted-Tx cancellation is not upgraded to rollback-complete.
- Process-local deadlines use monotonic time. Persistent expiry uses trusted wall time and a durable nondecreasing observed-time watermark; reject detected clock rollback. Restart does not renew full lifetimes. Document clock-error recovery, clock synchronization for shared-file processes, file permissions and SQLite durability. Backup restoration requires administrative session invalidation; stale backups cannot automatically reconstruct revocations. Test limited shared-file multi-process behavior, without promising multi-node/network-filesystem/distributed availability.
- Session cookies require Secure/HttpOnly/Path=/, no Domain, __Host- form and explicit SameSite. The finalizer owns no-store on standard issuance/rotation/authentication-state/CSRF responses and rejects overrides. Real TLS and supported-browser tests are acceptance requirements.

0.1.x has no standard Session data to migrate automatically. Custom cookies/tokens migrate through reauthentication/new issuance, never unchecked conversion. SF02 must define schema/version migrations with tests and distribution procedures.

## Retained compatibility

| Retained | Reason and scope |
|---|---|
| High/Low grammar/import/canonical IDs/sealed facts | Both inputs receive the same checks; no old security-metadata reinterpretation/exemption |
| move/view/Result/Task/spawn/Actor/Supervisor | No semantic change needed except auth-proof transfer rejection. Business denial stays Result; faults/HTTP termination/join remain separate |
| Pool/Tx/Parameters/acquisition/busy/close/OutcomeUnknown | Existing native lifecycle remains useful; change SQL argument signatures only |
| JSON/text/non-active bytes/non-reserved headers | Useful output without raw active-content/managed-policy bypasses; DTO confidentiality remains outside guarantees |
| Typed Rust extern/trusted adapters | Needed crypto/native boundary. Custom servers/raw networking/management SQL remain outside Foundation and are not re-exported standard bypasses |
| Existing 0.1.x tags/distributions | Keep historical artifacts intact; do not repackage them as an unchecked 0.2.0 mode |

Do **not** retain deprecated parallel HTTP/proofs/raw HTML/Db/dynamic SQL, warning-only execution or old factory aliases.

## Migration and acceptance matrix

| Old use | 0.2.0 replacement | Required evidence |
|---|---|---|
| route/route_mapped/decorators/legacy serve | One App with explicit policy and matching handler/mapper | Old source gets checker migration diagnostic/source line; migrated public/protected/HEAD/OPTIONS/dynamic/mapper wire/native business behavior |
| Injected health/stream/ws | Explicit health, finite response example, documented standard WS absence | No injection, rejected old entries, only selected endpoints; trusted-host examples state their boundary |
| Principal factory/indefinite Grant issuer/parts/Task delegation | AuthScope-bound issuer/Grant consumer; separate jobs and reauthorize | Old Rust consumer compile-fail; forgery, permission/target/request mismatch, expiry/gate ordering and captured-Future rejection |
| Raw Html/string/bytes plus HTML headers | Typed renderer and managed response | Alias/mapper/Low/byte-header bypass rejection, browser context/output bounds, non-active bytes success |
| Db/dynamic Sql/concatenated values | Pool/literal Query/Parameters; trusted bootstrap for management | Ownership/shape/values/NULL/schema/authorizer/Tx-close regression, dynamic-structure checker rejection, equivalent migrated DB outcomes |
| Custom cookies/sessions | Reauthenticate and issue durable Sessions with explicit origin/source/CSRF | Duplicate/attributes/TLS/no-store, concurrent rotation/revocation/total-record capacity, small repeated expiry/rotation/revoke with delayed cleanup, restart/crash/clocks/OutcomeUnknown and handler/cleanup evidence |

Map every row to High, High→Low→Rust, saved Low, handwritten Low, native Low/replace, Rust consumers and extracted distribution. Old-entry rejection cannot rely only on parser/import/undefined-symbol failures: implement checker migration diagnostics. Non-executable tombstone identity/signature recognition is allowed; keeping callable legacy implementations is not. Runner oracles validate primary locations, unrelated limits, panics and zero-test filters.

Every feature PR couples RED contracts, correct-layer implementation, native paths/regressions/four OS, bilingual Docs/DESIGN/executable examples and independent review. SF08 inventories removed entries and standard public exports. Warnings alone are not security guarantees; arbitrary trusted Rust and permissive policies are not statically proven. No merge/tag/formal release before approval.
