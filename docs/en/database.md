# SQLite

[Contents](README.md) · Previous: [HTTP and HTML](http.md) · Arguments: [Built-in functions](builtins.md)

## First reads and writes

This page describes the existing standard `db_*` API. Define the stored type with a class, open a database with `db_open`, and create a table with `db_exec`. Handle these fallible async operations with `try await`. Other databases, including PostgreSQL, currently require Rust integration.

> The development source has a separate `std.db.sqlite` Pool/Transaction API, but it is not included in Nagi 0.1.11. See [SQLite Pools and Transactions](sqlite-pool.md) for the new API and the limits below for the existing one.

Save the following code as `database.nagi` and run `nagic run database.nagi`. It inserts one row in memory and prints `Nagi`, then `Saved`.

```nagi
class User:
    id: i64
    name: str
    age: i32

async def main() -> Result[unit, Error]:
    db = try await db_open(":memory:")
    try await db_exec(db, "CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER NOT NULL)")
    user = try await db_insert[User](db, "INSERT INTO users(name, age) VALUES (?1, ?2) RETURNING id, name, age", "Nagi", 18)
    print(user.name)
    return ok(print("Saved"))
```

Use `db_open("app.sqlite")` to persist data. Relative database paths use the program's working directory, unlike imports and include_text, which use the source file's location.

`[User]` specifies the return type. Match `RETURNING` column names and types to the class. Bind values to placeholders such as `?1`; do not concatenate them into SQL.

Use a class as the type argument of a function that returns rows. `db_all[i64]` and `db_query[str]` are rejected by `check`. Define a class with that field even when you read only one column.

Generated row readers support `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `f32`, `f64`, `bool`, `str` and `bytes` fields. Use `T?` for a column that accepts SQL NULL. Nullable boolean, float and bytes fields are supported from Nagi 0.1.9. See the [settings API example](../../test-nagi-code/application-examples/device-settings/README.en.md).

Nagi 0.1.10 also generates row readers for these types wrapped in `owned[...]`. Fields such as `owned[str]` and `owned[i64?]` need no handwritten `FromRow`; remove any earlier workaround implementation that would now duplicate the generated one. For unsupported fields, use a Rust row reader or type conversion.

| Operation | Form | Return type after await |
|---|---|---|
| Create a table, etc. | `db_exec(db, sql)` | `Result[i64, Error]` |
| Read all rows | `db_all[User](db, sql)` | `Result[List[User], Error]` |
| Read one row | `db_query[User](db, sql, id)` | `Result[User?, Error]` |
| Insert | `db_insert[User](db, sql, name, age)` | `Result[User, Error]` |
| Update | `db_update[User](db, sql, id, name, age)` | `Result[User, Error]` |
| Delete, etc. | `db_write(db, sql, id)` | `Result[i64, Error]` |

`db_exec` counts changes from the entire SQL batch in that call. Table creation or SELECT alone returns 0. Changes from multiple statements are added together, including changes from triggers and foreign-key actions counted by SQLite.

Bind argument shapes are currently fixed: query/write take one i64, all takes none, insert takes str/i32, and update takes i64/str/i32. Columns do not have to be named name/age; tables with matching argument types also work.

## Read from an API

Pass SQL values as arguments rather than concatenating them into the SQL string. Each returned row fills the fields of the specified class.

This handler fragment uses the User type above:

```nagi
async def get_user(db: Db, id: i64) -> Result[User?, Error]:
    return await db_query[User](db, "SELECT id, name, age FROM users WHERE id = ?1", id)
```

Iterate over the result of `db_all[User]` with `for user in users` to borrow each row for reading, without cloning its strings. See [ownership](ownership.md) for the borrow restrictions. An HTTP handler can still return `await db_all[User](...)` as a JSON array. See the [CRUD API](../../examples/crud.nagi) and [task management source](../../test-nagi-code/web-demo/tasks.nagi).

## Existing API implementation and limits

This section describes only the existing `Db` / `db_*` API. Nagi provides typed calls and class row conversion. rusqlite and SQLite execute SQL, store data, and enforce database constraints. SQLite is bundled with the runtime.

With this API, each opened Db runs operations in order on a dedicated thread. Its queue holds up to 64 waiting jobs; senders wait when it is full. Nagi waits asynchronously for results, but SQLite reads and writes are synchronous. Releasing the last Db owner waits for the worker to exit, so destruction is not guaranteed to return immediately. The new Pool API has separate worker and queue rules described on [its own page](sqlite-pool.md).

This API caches prepared statements. Column names are resolved on each call; `db_all` reuses those column indices for every row in that result. Returned strings and byte sequences are owned so they remain valid after processing the SQLite row.

Ordinary `check` validates Nagi argument types and requires a class for returned rows. It does not check SQL syntax, schema, column names, bind counts, or the correspondence between SQL NULL and class fields.

Nagi 0.1.10 supports explicit [SQL checks](sql-check.md) for SQLite string literals, including syntax, names, required result columns, and bind counts in the existing API. Dynamic SQL, the old `db_exec`, and actual value types, NULLs, and ranges remain outside this check; supported field types handle these at runtime through Result. Unsupported row fields or other unmet Rust conversion requirements may instead fail at build time. The development source also checks literal SQL passed to the new `query` / `all` / `exec` operations; Parameters bind counts and value types remain unchecked statically and are checked at runtime. See [SQL checks](sql-check.md).

The existing `db_*` API does not provide variable-length typed parameters, a transaction that reserves a connection across calls, or a connection pool. Writing BEGIN/COMMIT in SQL does not reserve the connection across multiple calls: other operations using the shared Db can run between them. The unreleased API in the development source provides explicit Tx and Pool; it is not part of 0.1.11.

A database job already accepted may complete and commit even after its HTTP caller times out. Cancelling the caller does not guarantee that a write is rolled back.

See the existing API's [DB runtime](../../runtime/src/database.rs), [generated class row conversion](../../compiler/src/emit.rs), [CRUD example](../../examples/crud.nagi), and [DB tests](../../runtime/src/database.rs). For the new Pool API, see [SQLite Pools and Transactions](sqlite-pool.md). Separate SQLite/PostgreSQL types with common operation rules remain a [design proposal](library-design.md).
