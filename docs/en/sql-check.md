# SQL checks

Supply a schema to `check` to validate names, result columns, and bind counts in literal SQL passed to the existing SQLite `db_*` API. Ordinary `check`, `build`, and `run` do not enable this automatically. In the development source, the unreleased `std.db.sqlite` `query` / `all` / `exec` operations are also checked. The new API's Parameters bind counts and value types are not statically known, so they remain runtime checks and are reported as `bind unchecked`. The new API is not included in Nagi 0.1.11. See the [Changelog](../../CHANGELOG.md) for versioned changes.

## Usage

Save the application's table definitions in `schema.sql`:

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);
```

Put the row type and SQL in `app.nagi`:

```nagi
class User:
    id: i64
    name: str

async def main() -> Result[unit, Error]:
    db = try await db_open(":memory:")
    try await db_exec(db, "CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
    users = try await db_all[User](db, "SELECT id, name FROM users")
    for user in users:
        print(user.name)
    return ok(print("Done"))
```

```sh
nagic check app.nagi --sql-schema schema.sql --sql-dialect sqlite
```

`SELECT id, naem FROM users` fails because the column does not exist. `SELECT id FROM users` fails because User requires a name column. Diagnostics identify the Nagi or Low file and line containing the query.

For a project, use `nagic check --project ./app --sql-schema ./app/schema.sql --sql-dialect sqlite`. Relative schema paths use the command's working directory. Supply both SQL options together; they apply only to `check`. Configuration in nagi.toml and automatic editor checks are unsupported.

## What is checked

| Target | Checks |
|---|---|
| `db_all`, `db_query` | A single statement, table/column names, fixed API argument count, required row-class columns |
| `db_insert`, `db_update` | A single write statement, fixed API argument count, required `RETURNING` columns |
| `db_write` | A single statement without returned rows, fixed API argument count |
| `sqlite.query`, `sqlite.all` in the development source | One readonly row-producing statement, table/column names, and required class columns. Parameters bind count is unchecked |
| `sqlite.exec` in the development source | One rowless statement shape and schema references. DDL is prepare-only and never executed. Parameters bind count is unchecked |

Checks apply to string literals passed directly to calls resolved as standard database operations. A user function with the same name is not a target. Changed column order, correct aliases, and extra result columns are allowed. Placeholder counts for the existing `db_*` API follow SQLite's rules; reusing the same `?1` does not add a bind. The new `sqlite.*` API uses anonymous `?` placeholders and owned `Parameters`, so the checker cannot verify its bind count or value types.

Handwritten Rust `FromRow` bodies are not analyzed. For the new API, class field names are treated as required result columns, so use suitable SQL aliases. Ordinary `check`, `build`, and Rust row decoding remain unchanged.

SQL stored in a variable or built in Rust remains subject to runtime checks. The existing `db_exec`, including batches and schema changes, is excluded. New `sqlite.exec` calls are prepared only to check a single statement's shape; DDL is never executed and the application schema is not inferred from it. The checker does not execute application queries or schema changes. New API Parameters bind count and value types remain unchecked. Results show the count of inspected literals and the count, locations, and reasons for calls left to runtime checks.

## Schema and runtime boundaries

The checker builds the supplied schema in a new in-memory database and prepares SQL to inspect names, columns, and bind metadata. It does not execute application queries or `sqlite.exec` DDL, connect to the application's database, or invoke Cargo. Ordinary `check` creates no SQL connection or worker. Generated Rust and the application's database execution path are unchanged.

Use ordinary CREATE TABLE, INDEX, and VIEW statements in the schema. This is not a migration runner. ATTACH/DETACH, PRAGMA, external database or file writes, extension loading, TEMP/virtual tables, triggers, transactions, and CREATE TABLE AS SELECT are rejected. Functions use an allowlist; random and date/time functions, including `DEFAULT CURRENT_TIMESTAMP`, are unsupported.

The schema is limited to 2 MiB and 1024 statements, and each SQL string to 256 KiB. Column counts, expression depth, and other SQLite limits are bounded. A parent monitors a separate worker process with a five-second deadline; exceeding a limit or deadline fails the check.

Each check validates all target SQL against one supplied schema. It does not infer connections for multiple Db values. Keep the deployed database schema consistent with the supplied schema.

The check does not guarantee value types, integer ranges, NULL behavior, query results, permissions, or the state of a deployed database. New API Parameters bind counts are also unchecked statically. For example, `SELECT 'oops' AS id, 'Nagi' AS name FROM users` provides the required columns, so this check alone cannot detect the id type mismatch. Row decoding, bind values, and dynamic SQL still use runtime Result errors. See [existing SQLite limits](database.md) and [new API limits](sqlite-pool.md).

## Build a compiler without SQLite

The validation engine is included in the default Cargo feature `sql-check`. To omit it when building from source:

```sh
cargo build --release --locked -p nagic --no-default-features
```

This compiler still supports ordinary checks and builds. SQL-check options fail with an error explaining that the feature is required. This setting is separate from the application's SQLite runtime.
