# SF05 implementation and validation

2026-10-09 UTC. Base main `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`, branch `feat/security-foundation-sf05`. SF00 #100 and SF01 #101 are merged. [日本語](security-foundation-sf05-results.md), [contract](security-foundation/sf05-contract.en.md), [migration inventory](security-foundation/sf05-migration-inventory.en.md), [ADR 014](adr/014-literal-query-and-sqlite-admission.md). Independent review and four-OS CI at the latest head remain pending. No merge/version/tag/release/publishing action was performed.

Canonical sqlite.literal accepts direct literals and emits opaque Copy Query through a sealed payload plan. query/all/exec require Query. Old Db/db_* execution and public runtime Db/Sql are removed, retaining concrete checker migration diagnostics. Unrelated user definitions retain their own identities. Regular check stays independent of the SQL engine; opt-in uses the existing SQLite prepare/authorizer boundary, collecting direct Query and Parameters builders without claiming static knowledge of variables.

The reviewed native adapter binds actual Grant subject/target into owner/id predicates in the same SQLite transaction. Real bounded queue capacity is reserved before Grant.submit; admission linearizes at the private one-time permit issued under the SF01 gate. The synchronous callback enqueues once, then only the reply is awaited. Invalidation before issuance produces zero enqueues. Invalidation after issuance, even before the callback sends, leaves the operation admitted. Tests also cover enqueue-before-SQL invalidation, finite capacity waiting and unused reservation release.

Query alone does not prove tenant policy, arbitrary Rust adapter correctness or rollback after cancellation/invalidation. Mutable session/permissions require same-Tx predicates or revalidation; SF02 generation guarantees remain unfinished. Fixed reviewed DDL/seed runs as trusted application startup before HTTP registration, separate from request handlers. No request-facing dynamic factory or new authorizer DDL ban was introduced. Pool/Tx/Options/Failure/Outcome, single-statement/bind/row/NULL, zero-ms acquire, cancellation/cleanup/actual-close contracts remain.

Raw output, commands/exits, source SHA-256, compiler identity and warm-cache provenance are in [artifacts](../../benchmarks/results/security-sf05-validation-2026-10-09/). Cache reuse is explicitly not a clean-build claim.

| Evidence | Result |
|---|---|
| Initial RED | Three already-defined old boundaries accepted across all nine High/independent saved-Low/handwritten-Low paths; not an undefined-constructor failure |
| Compiler / registry / SQL preflight | 12 / 4 / 14 groups; purpose and original-source positions preserved |
| Added rejection cases | Literal concatenation and a function-returned literal rejected on all three paths; default and no-engine compiler each execute all 12 groups |
| Native final | Nine groups, 216 source/generated-Rust/build/run artifacts; real SQLite owner predicate, unrelated target preservation and HTTP denial |
| Runtime | SQLite79 plus public API1, no ignored cases; actual permit/enqueue/capacity, authorizer, decoder, cleanup and close observations |
| Workspace | Saved full run exit0, 1011 passing tests, failed0, one pre-existing ignored measurement. Compared 236 compiler/runtime files; later deltas are only two test files, checked above; production hashes match |
| Quality | fmt and clippy exit0; fuzz seed305419896, 1000 text mutations, 77 checked Low/emit mutations, 16 bounded native cases, zero panics |
| Editor | 22 files / 196 actual tests, zero failures/skips; manual VS Code host GUI remains unexecuted |
| Isolated SQL | Copied compiler in unrelated cwd, empty PATH/missing runtime; good SQL and bad column/bind cases use intended original-line diagnostics without Cargo |
| Applications | Ten projects / nineteen native High/Low runs; nullable device settings and restart included |
| Tutorial | Bilingual source matches; High and saved Low after deleting High check/build/run, reporting 7 and closed |
| HTTP business | Eight High/independent saved-Low native runs / 84 small checks; matching compiler and all application-input hashes established in a combined ledger |
| Query cost | Release/x86_64 Linux; generated/manual both literal392B/selected408B unpolled Future. Four warmups and 32 samples per mode; all caller allocation samples are two allocations/280B. No SQL-string duplication. Rollback/actual close succeed; dependency versions/checksums match the workspace lock. Timing is one shared-host observation without a threshold |
| Docs | Website102 pages with links/anchors/assets verified; relative-link inventory has no missing targets |

Network dependency timeout, missing compiler PATH, sandbox EPERM and new HTTP harness mistakes remain raw failed records. A partial HTTP command passed four CRUD/inventory runs before failing to copy a later fixture import; its successful markers and raw artifacts are distinguished from the later successful task/Result command, with identical compiler/application source hashes. No parse/import/unrelated failure is treated as intended negative success. Ignored measurement is excluded from success counts. Independent review and latest-head four-OS evidence remain to be completed.


## Independent review update

The separate Sol High review of local source head `c33e8c9` found no source/runtime blocker. Two documentation findings were recorded in the [review ledger](security-foundation/review-log.md): P2 for permit/enqueue ordering and P3 for the missing English DESIGN migration. Four bilingual documents were corrected. The gate covers one permit issuance; after releasing it, a synchronous callback enqueues and the reply is awaited. Revocation after issuance is already admitted even before enqueue. The original snapshot hashes remain historical; review-docs-correction.json records this documentation delta separately. Production/test files are unchanged, so the same full regression is not repeated. Documentation recheck and latest-head CI remain required.

The targeted independent recheck of 845d938 closed SF05-R01/R02 and approved source/docs. Additional independent test outputs were not retained as raw files; they are not represented by the implementer artifacts. Latest-head four-OS CI remains unverified.
