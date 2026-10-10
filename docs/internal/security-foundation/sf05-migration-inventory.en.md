# SF05 removal and migration inventory

Baseline main is `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`. [Source inventory and SHA-256](../../../benchmarks/results/security-sf05-validation-2026-10-09/legacy-inventory.json). [日本語](sf05-migration-inventory.md). Historical artifacts retain their original results.

| Boundary | Migration |
|---|---|
| Compiler builtins and types | Remove Db/db_* execution and emission; retain source-position migration tombstones. Preserve unrelated user definitions by canonical identity. |
| Canonical standard module | Add opaque Query and literal operation; seal the literal payload once; require Query for query/all/exec. |
| Runtime exports | Remove public Db/Sql/dynamic factories and the old worker. Retain FromRow/indices for the current decoder. Private dynamic SQL is test-only. |
| SQLite lifecycle | Preserve Options/Pool/Tx/Parameters/Failure/Outcome, native authorizer, shape/bind/NULL, zero-ms acquisition, cancellation/cleanup/actual close. |
| Optional SQL engine | Collect canonical direct Query and direct Parameters builders; leave unknown structures to runtime. Regular checking remains engine-free. |
| Protected adapter | Reserve bounded real queue capacity, issue the one-time permit through Grant.submit, synchronously enqueue with actual subject/target bound into a reviewed predicate, then await only the reply. |
| Examples | Explicit Options and transactions, anonymous binds, same-Tx exec plus readonly query replacing RETURNING. Fixed schema/seed management runs from main before HTTP registration. |
| Docs/editor/tools | Remove old recommended APIs and completion entries; keep release history distinct; update bilingual API/migration/tutorial and isolated distribution verification. |

The independent handwritten resource inventory compares all 39 resources and 83 operations, including SQLite's nine resources and nineteen operations. Query allows Copy, storage, shared payloads and Debug; it excludes Serde and equality. The literal operation consumes its argument. Execution uses Reference/Move/Move. Independent accessor/constant/enum inventories and corrupted sealed-plan tests remain in place.

Native consumers cover High, independently saved Low after removing High, handwritten Low, evaluation order, business Err/panic, nullable/numeric/blob decoding, implicit Tx loans, Query selection/return/copy/shared storage, real HTTP denial and actual SQLite predicates. Manual FromRow stays in the trusted host; generic standard row restrictions remain unchanged. Negative row/field/Task tests migrate to valid Pool/Query or non-DB fixtures so a retired Db error cannot masquerade as their intended rejection. Canonical graph operations remain database nodes; user Db/db_all/literal names remain usable.

Initial RED used three already-defined legacy boundaries: old Db, db_open, and sqlite.exec accepting a string. Parse and resolution succeeded. The [all-path RED](../../../benchmarks/results/security-sf05-validation-2026-10-09/initial-red-all-paths.json) recorded acceptance in all nine High/saved-Low/handwritten-Low paths. Undefined new constructors were not counted as the intended RED. The implemented tests assert specific migration purpose and original source lines on every path.

Db spelling in modules/check is diagnostic-only. FailureKind::Sql, rusqlite set_db_config, user db_all fixtures, historical Drop comments and private test Sql::Owned are not old standard execution paths. Earlier architecture documents now link to the superseding SF05 contract without rewriting their historical measurements. [Validation and remaining limits](../security-foundation-sf05-results.md).
