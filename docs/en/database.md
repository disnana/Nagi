# SQLite

[Index](README.md) · [SQLite Pools and Transactions](sqlite-pool.md) · [SQL preflight checks](sql-check.md)

The development source uses `std.db.sqlite` as its standard SQLite entry point. SF05 removes `Db` and `db_open/db_exec/db_all/db_query/db_insert/db_update/db_write`, with concrete checker migration diagnostics. Formal 0.2.0 remains unreleased; these changes do not apply retroactively to published 0.1.x binaries.

Open a `Pool` with explicit `Options`; a `Tx` reserves one connection. SQL structure is an opaque `Query` made by `sqlite.literal(...)` from a direct string literal. Actual values go through `Parameters`. There is no constructor from a string variable, concatenation, formatting, or dynamic string. Query values may be stored, selected, and returned from functions.

```nagi
import std.db.sqlite as sqlite

class User:
    id: i64
    name: str

async def find(tx: view[sqlite.Tx], id: i64) -> Result[User?, sqlite.Failure]:
    sql = sqlite.literal("SELECT id, name FROM users WHERE id = ?")
    values = sqlite.bind_i64(sqlite.parameters(), id)
    return await sqlite.query[User](tx, sql, values)
```

For a runnable memory database with creation, write, read, and close, see [sqlite_pool.nagi](../../examples/sqlite_pool.nagi) and the [instructions](sqlite-pool.md#run-the-example). Relative database paths use the runtime working directory. Other databases such as PostgreSQL require trusted Rust integration.

| Old operation | Migration |
|---|---|
| `db_open` | `sqlite.options`, `sqlite.open`, and explicit `begin` |
| `db_all` / `db_query` | `sqlite.all[T]` / `sqlite.query[T]` with Query and Parameters |
| `db_exec` / `db_write` | Single-statement `sqlite.exec`; split batches inside an explicit Tx |
| RETURNING in `db_insert` / `db_update` | exec followed by a readonly query in the same Tx; use `last_insert_rowid()` for INSERT, and bind the actual id for UPDATE |

The new API accepts anonymous `?` only. `exec` accepts statements without returned columns; `query/all` require readonly statements with columns. Keep DDL/bootstrap in trusted management code separate from requests. Do not reexport a factory that accepts request-derived SQL structure. [CRUD](../../examples/crud.nagi), [inventory](../../test-nagi-code/inventory.nagi), and the [task API](../../test-nagi-code/web-demo/tasks.nagi) demonstrate explicit public policies.

Row classes decode supported scalar fields, their Option forms, and owned wrappers. Actual value types, NULL, and numeric ranges are checked by runtime Results. Unsupported handwritten FromRow implementations remain available through trusted native adapters; they do not bypass the standard Nagi API's scalar row constraint. Non-Copy rows in `all` results can be iterated readonly without cloning.

Ordinary check validates the literal boundary, types, and ownership without starting an SQL engine. Opt-in checks with an explicit schema prepare SQL structure and required columns without executing it, and compare bind counts for direct Parameters builder chains. Query or Parameters variables whose structure is unavailable are reported as runtime checks.

Database operations return `sqlite.Failure`, without implicit conversion to `Error`. HTTP examples explicitly project the primary Error; they do not automatically add Failure's Outcome, cleanup, or retired metadata to the HTTP error type. Preserve that information in an application error type when needed.

Protected data requires a reviewed adapter that binds the Grant's actual subject/target into a tenant/owner predicate. Passing a Grant to a generic query cannot promise tenant isolation. Reserve capacity, then validate the lease and issue one execution permit inside `Grant.submit`'s synchronous gate. Release the gate before the synchronous callback enqueues into the real SQLite queue, then await the reply. Revocation before permit issuance means zero enqueues. Revocation after issuance does not undo the admitted operation, even before enqueue. HTTP cancellation does not guarantee rollback of an admitted command. See the [protected database boundary](security.md#protected-sqlite-operations).

[SQLite reference](sqlite-pool.md) documents acquisition budgets, the authorizer, cancellation cleanup, Outcome, zero-millisecond acquisition, and actual close/join. An SQL Err does not imply transaction rollback or no changes.
