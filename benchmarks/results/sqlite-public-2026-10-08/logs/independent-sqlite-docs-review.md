# Independent SQLite Docs QA

Reviewed the working tree in `/workspace/Nagi-sqlite-public` read-only. Compared the Japanese and English public SQLite pages, SQL preflight pages, Docs indexes, DESIGN summaries, existing `db_*` pages, `examples/sqlite_pool.nagi`, compiler API registry, runtime types/options, and the SQLite runtime decision/ADR. No source or Docs files were changed. I did not run Cargo/native builds; the parent reports a successful current-debug sample/schema check and a 94-page site build, but no raw site log was available here. Navigation and language indexes were checked from source.

## Finding

- **P2 — Japanese public guide omitted the English SQL-shape and row-type contract (resolved in follow-up).** [docs/sqlite-pool.md:140](/workspace/Nagi-sqlite-public/docs/sqlite-pool.md:140) proceeds from Options/path directly to the eight types and operation signatures. The matching English section, [docs/en/sqlite-pool.md:140](/workspace/Nagi-sqlite-public/docs/en/sqlite-pool.md:140), additionally explains allowed bind values and anonymous `?` placeholders, runtime bind-count/type checks, `query`/`all`/`exec` SQL shapes (including `RETURNING`), and supported row-class field types/nullability. Japanese readers who move beyond the happy-path sample lack those rules and may diagnose valid SQL or row types incorrectly. Add a Japanese counterpart of the English section, keeping its runtime-versus-checker boundaries aligned with `docs/sql-check.md`.

## Checks that matched

The example's `7` / `closed` output and source are consistent on inspection. The documented 8 resource types, 18 operation signatures, BeginMode/FailureKind/Outcome constants, Failure fields and borrowed `message`, explicit cause-copy functions, same-task Tx ownership, consuming commit/rollback, acquisition budget boundary, close/Drop distinction, allocation limitation, `retired=false` caveat, and logical-FIFO scope matched the compiler/runtime contract and decision records. The API is consistently marked unreleased/not in 0.1.11; both language indexes and website navigation include the new page, and the existing `Db` / `db_*` docs retain their old behavior while linking to the new API.

## Follow-up re-review (2026-10-08)

The P2 language-parity finding is **resolved** in the current public Docs. Japanese [docs/sqlite-pool.md:142](/workspace/Nagi-sqlite-public/docs/sqlite-pool.md:142) now covers the same bind builders, anonymous `?` and one-statement rule, runtime bind/type/NULL checks, `query` / `all` / `exec` SQL shapes (including `RETURNING` and transaction-control restrictions), and generated FromRow field constraints as English [docs/en/sqlite-pool.md:142](/workspace/Nagi-sqlite-public/docs/en/sqlite-pool.md:142). The key checker/runtime boundary remains consistent with the SQL preflight pages.

The revised type-table heading now says “field storage” in English and “field保存” in Japanese, clarifying that the capability column concerns storing values in fields. Both public pages also carry a matching Rust-embedding compatibility note at line 7: exhaustive matches over public compiler standard-module/resource/operation metadata enums must handle the new SQLite variants, while existing Nagi syntax and `db_*` signatures stay unchanged. This matches the SQLite variants in the compiler registry.

No additional issue was found in the requested public-Docs changes. This was a read-only review; no Cargo/native or site checks were run.
