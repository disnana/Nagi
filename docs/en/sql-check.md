# SQL checks

Supply a schema to `check` to validate names, result columns, and bind counts in SQLite SQL strings before execution. Ordinary `check`, `build`, and `run` do not enable this automatically. This feature is supported in Nagi 0.1.10; see the [Changelog](../../CHANGELOG.md) for versioned changes.

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
| `db_all`, `db_query` | A single SQL statement, table/column names, bind count, required row-class columns |
| `db_insert`, `db_update` | A single write statement, bind count, required `RETURNING` columns |
| `db_write` | A single write statement without returned rows, bind count |

Checks apply to string literals passed directly to resolved built-in database calls. A user function with the same name is not a target. Changed column order, correct aliases, and extra result columns are allowed. Placeholder counts follow SQLite's rules; reusing the same `?1` does not add a bind.

Handwritten Rust `FromRow` bodies are not analyzed. This check treats class field names as required result columns, so use suitable SQL aliases. Ordinary `check`, `build`, and Rust row decoding remain unchanged.

SQL stored in a variable or built in Rust remains subject to runtime checks. `db_exec`, including batches and schema changes, is excluded. The checker does not execute CREATE TABLE statements in the application to infer a schema. Results show the count of inspected literals and the count, locations, and reasons for calls left to runtime checks.

## Schema and runtime boundaries

The checker builds the supplied schema in a new in-memory database and prepares SQL to inspect names, columns, and binds. It does not execute application queries, connect to the application's database, or invoke Cargo. Ordinary `check` creates no SQL connection or worker. Generated Rust and the application's database execution path are unchanged.

Use ordinary CREATE TABLE, INDEX, and VIEW statements in the schema. This is not a migration runner. ATTACH/DETACH, PRAGMA, external database or file writes, extension loading, TEMP/virtual tables, triggers, transactions, and CREATE TABLE AS SELECT are rejected. Functions use an allowlist; random and date/time functions, including `DEFAULT CURRENT_TIMESTAMP`, are unsupported.

The schema is limited to 2 MiB and 1024 statements, and each SQL string to 256 KiB. Column counts, expression depth, and other SQLite limits are bounded. A parent monitors a separate worker process with a five-second deadline; exceeding a limit or deadline fails the check.

Each check validates all target SQL against one supplied schema. It does not infer connections for multiple Db values. Keep the deployed database schema consistent with the supplied schema.

The check does not guarantee value types, integer ranges, NULL behavior, query results, permissions, or the state of a deployed database. For example, `SELECT 'oops' AS id, 'Nagi' AS name FROM users` provides the required columns, so this check alone cannot detect the id type mismatch. Row decoding and dynamic SQL still use runtime Result errors. See [SQLite limits](database.md#implementation-and-limits).

## Build a compiler without SQLite

The validation engine is included in the default Cargo feature `sql-check`. To omit it when building from source:

```sh
cargo build --release --locked -p nagic --no-default-features
```

This compiler still supports ordinary checks and builds. SQL-check options fail with an error explaining that the feature is required. This setting is separate from the application's SQLite runtime.
