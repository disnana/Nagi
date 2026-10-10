# SQL Preflight Checks

Development-source `std.db.sqlite` requires a Query created from a direct literal during ordinary check. SQL syntax and schema validation are a separate stage enabled by explicit options. SF05 is unreleased; it is neither formal 0.2.0 nor a retroactive feature of published 0.1.x. See [SQLite](sqlite-pool.md) and [migration](migration-0.2.0.md).

## Usage

Save a DDL snapshot of the deployed schema in `schema.sql`.

```sql
CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
```

Direct Query constructors in `app.nagi` allow checks of schema names, returned columns, and bind counts.

```nagi
import std.db.sqlite as sqlite
class User:
    id: i64
    name: str
async def find(tx: view[sqlite.Tx], id: i64) -> Result[User?, sqlite.Failure]:
    return await sqlite.query[User](tx, sqlite.literal("SELECT id, name FROM users WHERE id = ?"), sqlite.bind_i64(sqlite.parameters(), id))
```

```sh
nagic check app.nagi --sql-schema schema.sql --sql-dialect sqlite
```

`naem` is rejected as an unknown column; `SELECT id` lacks User's name field. Diagnostics retain the original Nagi/Low module and line. For projects use `nagic check --project ./app --sql-schema ./app/schema.sql --sql-dialect sqlite`. Relative schema paths use the working directory. Both options are required together and apply only to check. Automatic nagi.toml/editor configuration is unsupported.

## What is checked

| Input | Checks |
|---|---|
| Direct literal Query in `sqlite.query/all` | One readonly statement, table/column names, required row fields |
| Direct literal Query in `sqlite.exec` | One statement without returned columns and schema references; DDL is prepared without execution |
| Direct `parameters()` / `bind_*` builder chain | Count matches anonymous `?` placeholders |
| Query/Parameters structure hidden behind variables/functions | Report that site for runtime checks; do not count it as statically validated |

Only calls resolved to canonical `stdlib:std.db.sqlite` are collected. Same-named user functions are not inferred as SQL. Changed column order, aliases, and additional columns are allowed. Numbered `?1` and named placeholders are rejected by the standard API. The checker does not duplicate an SQL parser; the opt-in engine and runtime use SQLite's authorizer and prepare metadata to validate shape.

Query variables created from literals pass ordinary check, but this collector does not infer their SQL through dataflow. Even a direct constructor with a Parameters variable reports `bind unchecked`. Actual value types, NULL, and numeric ranges always remain runtime checks. For example, `'oops' AS id` supplies the column without guaranteeing successful i64 decoding. Native handwritten FromRow bodies are not analyzed.

DDL/bootstrap belongs to trusted management code. The checker does not execute queries to infer a schema or update the check schema from exec's CREATE TABLE. There is no request-facing dynamic string factory. Legacy Db/db_* entry points receive concrete migration diagnostics before SQL preflight.

## Safety and limits

The engine uses a separate worker with an in-memory SQLite database. It loads a DDL snapshot of ordinary CREATE TABLE, INDEX, and VIEW statements. It rejects arbitrary migrations, ATTACH/DETACH, PRAGMA, external file operations, extensions, TEMP/virtual tables, triggers, transactions, and CREATE TABLE AS SELECT. A function allowlist excludes random/time functions and DEFAULT CURRENT_TIMESTAMP.

The schema is limited to 2 MiB and 1024 statements, each SQL to 256 KiB, with limits for columns and expression depth. A separate worker process has a five-second deadline; violations return check errors. All sites use one explicit schema. The check does not guarantee deployment targets of multiple Pools, authorization, tenant predicates, or actual database state.

## Build without SQLite in the compiler

The opt-in engine belongs to the default Cargo feature `sql-check`.

```sh
cargo build --release --locked -p nagic --no-default-features
```

Ordinary literal Query checks and builds remain available without the engine. SQL preflight options report that the feature is required. Application-side SQLite runtime is configured separately.
