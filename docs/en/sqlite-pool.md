# SQLite Pools and Transactions

> `std.db.sqlite` is an unreleased API in the current development source. It is not included in Nagi 0.1.11. Users of published releases should continue with the existing [`db_*` API](database.md).

[Docs index](README.md) · [Existing SQLite API](database.md) · [SQL preflight checks](sql-check.md)

Rust embedding code that exhaustively matches the compiler's public standard-module, resource, or operation metadata enums must handle the new SQLite variants. Existing Nagi syntax and `db_*` signatures are unchanged. Applications that do not adopt the new API need no migration.

## What this API provides

This module provides a `Pool` that manages multiple SQLite connections and a `Tx` that reserves one connection for a transaction session. A `Pool` is the shared entry point for acquiring a connection. A `Tx` owns that connection session until it commits, rolls back, or is cleaned up. SQL values go through typed, owned `Parameters`.

This is a separate API from `db_open`, `db_exec`, `db_query`, and related functions. The old API keeps its fixed bind arguments, multi-statement behavior, row counts, statement cache, and row-type support. Do not mechanically replace existing calls. The [existing SQLite page](database.md) explains its behavior and helps choose between the two APIs.

## Run the example

### Prerequisites

- A Rust toolchain (`rustc` and `cargo`) and a C toolchain that can build Rust crates with native dependencies.
- A Nagi source checkout (a local working folder of the source) containing this page, `examples/sqlite_pool.nagi`, and the unreleased `std.db.sqlite` API. Run the commands from the repository root.
- No SQLite command-line tool is needed. SQLite is bundled with the Nagi runtime.

See [how to obtain the source and build the compiler](getting-started.md#build-the-compiler-from-source). If `main` does not yet contain this sample and API, use a checkout of a development branch that does. The published 0.1.11 binary alone cannot run this walkthrough.

If `cargo build` fails while compiling a native dependency, check that an OS C compiler and linker are available as well as Rust. If `nagic` is not found, invoke the release binary below or add `target/release` to PATH.

### Build the compiler and run the sample

From the repository root, where `Cargo.toml` is located, build the compiler, check the sample, and then run it:

```sh
cargo build --release --locked -p nagic
./target/release/nagic check examples/sqlite_pool.nagi
./target/release/nagic run examples/sqlite_pool.nagi
```

A successful `check` exits without diagnostics. `run` writes the following to standard output:

```text
7
closed
```

On Windows, use the release executable from the same repository root. If it is on PATH, you can use `nagic.exe` directly.

```powershell
cargo build --release --locked -p nagic
./target/release/nagic.exe check examples/sqlite_pool.nagi
./target/release/nagic.exe run examples/sqlite_pool.nagi
```

The sample opens a one-connection `:memory:` database, creates a table, saves and reads back `7`, then closes the pool. `query` returns `None` for zero rows and `Some` with the first row otherwise. On success, `main` prints `7` and `closed`. How `nagic run` formats a failure is not an API guarantee.

The sample uses `std.result.map_error` to explicitly convert different error types into the application's `AppFailure`. `options` and `bind_f64` return the ordinary `Error` type; database operations return `sqlite.Failure`. `Failure` does not implicitly convert to `Error`.

```nagi
import std.db.sqlite as sqlite
import std.result as result

class Total:
    amount: i64

enum AppFailure:
    Configuration(cause: Error)
    Database(cause: sqlite.Failure)

def configuration_error(cause: Error) -> AppFailure:
    return AppFailure.Configuration(cause)

def database_error(cause: sqlite.Failure) -> AppFailure:
    return AppFailure.Database(cause)

async def write_and_read(pool: view[sqlite.Pool]) -> Result[i64, sqlite.Failure]:
    tx = try await sqlite.begin(pool, sqlite.BeginMode.DEFERRED)
    try await sqlite.exec(tx, "CREATE TABLE IF NOT EXISTS amounts(amount INTEGER NOT NULL)", sqlite.parameters())
    values = sqlite.bind_i64(sqlite.parameters(), 7)
    try await sqlite.exec(tx, "INSERT INTO amounts VALUES (?)", values)
    try await sqlite.commit(tx)
    tx = try await sqlite.begin(pool, sqlite.BeginMode.DEFERRED)
    row = try await sqlite.query[Total](tx, "SELECT amount FROM amounts", sqlite.parameters())
    try await sqlite.rollback(tx)
    match row:
        case Some(total):
            return ok(total.amount)
        case None:
            return ok(0)

async def main() -> Result[unit, AppFailure]:
    config = try result.map_error(sqlite.options(1, 2, 1000, 0), configuration_error)
    pool = try result.map_error(await sqlite.open(":memory:", config), database_error)
    work = await write_and_read(view(pool))
    ending = await sqlite.close(pool, 1000)
    # Observe close even when the transaction failed; preserve the work error first.
    amount = try result.map_error(work, database_error)
    try result.map_error(ending, database_error)
    print(amount)
    return ok(print("closed"))
```

Even if `write_and_read` fails, `main` awaits close before returning the work error. If both work and close fail, the sample gives priority to the work error and does not return the close error. A production application should record both errors in logs or its own error type.

### If a step fails

| Symptom | Check |
|---|---|
| `cargo build` fails in a C/native dependency | Confirm the repository root and that a Rust toolchain and C compiler/linker are available |
| `nagic: command not found` | Invoke `target/release/nagic` directly (Windows: `target/release/nagic.exe`) or check PATH |
| `check` reports an unsupported module or API | Confirm that the compiler was built from this worktree. The API is absent from installed Nagi 0.1.11 |
| `check` rejects a row type | Ensure `T` in `query[T]` / `all[T]` is a class supported by the generated row reader |
| `run` returns a SQLite Failure | Inspect its `kind` and `message`. Because open is lazy, a path/native connection failure may first appear in `begin` |

## Types and ownership

| Type | Role and main constraints |
|---|---|
| `Options` | Holds connection count, per-worker queue capacity, acquisition timeout, and SQLite busy timeout. Supply every constructor argument |
| `Pool` | Manages acquisition and close. `clone_pool` creates a handle that shares close and failure state |
| `Tx` | A transaction session on one connection. Non-Copy, same-task only, and cannot be stored in a field or shared value |
| `Parameters` | An owned list of SQL bind values. Each builder consumes it and returns the next `Parameters` value |
| `BeginMode` | `DEFERRED`, `IMMEDIATE`, `EXCLUSIVE` |
| `Failure` | A DB failure with `kind`, `outcome`, `retired`, and a borrowed `message` field |
| `FailureKind` | Failure classification |
| `Outcome` | The transaction result that was observable when the failure was made |

`Pool` is non-Copy. Call `sqlite.clone_pool(pool)` to create another handle; all clones share close and failure state. `Tx` is non-Copy, non-Clone, non-shared, and bound to the same task. It cannot be saved in a class field. It can be used in same-task locals, `Option` / `Result` values, and owned function delegation. Standard SQLite operations borrow it as `view[Tx]`. When your own function accepts `view[Tx]`, pass `view(tx)` explicitly at the call site.

`commit(tx)` and `rollback(tx)` consume the `Tx` as soon as their Future is created. You cannot use that handle after awaiting the operation. To continue with another transaction, call `begin` again instead of reusing the old handle.

### Options and paths

All four arguments to `options(connections, queue_capacity, acquire_ms, busy_ms)` are required. There are no numeric defaults.

| Argument | Condition and meaning |
|---|---|
| `connections` | Positive and within `usize` and Tokio Semaphore permit limits |
| `queue_capacity` | Positive and within the same permit limit. This is the **command inbox capacity per worker**; it does not bound all callers waiting for Pool slots or total heap use |
| `acquire_ms` | Non-negative and convertible to native `Duration` / deadline. One budget for waiting for a logical slot and registering a native slot |
| `busy_ms` | Non-negative, within the native time range and SQLite's `i32` millisecond range. How long SQLite waits for a database lock |

Capacity overflow, negative values, and time values outside the native range return `Error` from `options`. A timeout of 0ms means “do not wait,” not “always fail”: it can succeed if the condition already holds.

`open(path, options)` rejects an empty path and `file:` URIs. `:memory:` works only with one connection because each connection would otherwise have a separate database. Other paths must be ordinary filesystem paths; relative paths use the process's current working directory. `open` owns the path when its Future is created, but creates native connections lazily when `begin` is called. Therefore, a filesystem or native open error may first appear as a `WORKER` failure from the first `begin`. URI interpretation, shared-memory URIs, and automatic WAL setup are not provided.

### Typed parameters and SQL shape

Start with `parameters()` and pass each builder's result to the next builder. The resulting order is the order of anonymous SQLite `?` placeholders.

| Builder | Added value | Condition |
|---|---|---|
| `bind_i64(parameters, value)` | `i64` | Integer |
| `bind_f64(parameters, value)` | `f64` | Finite values only; NaN/Infinity returns `Error` |
| `bind_text(parameters, value)` | `str` | Passed as owned text |
| `bind_bytes(parameters, value)` | `bytes` | Passed as owned bytes |
| `bind_null(parameters)` | Plain SQLite NULL | Not a typed NULL |

New API SQL must contain one statement at a time and use anonymous `?` placeholders only. Named `:name` / `@name` and numbered `?1` placeholders are unsupported. Runtime checks bind counts, value types, and NULL decoding. Ordinary `check` does not execute SQL against database values. Bind values with `Parameters` rather than concatenating them into SQL text.

### query, all, and exec

| Operation | Accepted SQL | Return value |
|---|---|---|
| `query[T]` | One statement SQLite classifies as readonly and that returns columns. SQLite metadata decides this; the checker does not rely on the `SELECT` keyword alone | `Result[Option[T], Failure]`. `None` for zero rows, otherwise `Some` with the first row |
| `all[T]` | Same readonly row-producing SQL as `query` | `Result[List[T], Failure]` with every row |
| `exec` | One statement with no result columns. Ordinary table/index/view/trigger DDL is supported | `Result[i64, Failure]` with that statement's change count |

A write with `RETURNING` returns columns, so it is neither readonly for `query` / `all` nor column-free for `exec`. Do not put `BEGIN` / `COMMIT` / `ROLLBACK` (including their variants), `SAVEPOINT`, `PRAGMA`, `ATTACH`, or `DETACH` in user SQL. The module operations own transaction boundaries. Virtual tables, extensions, and raw connections are not part of the public API.

`T` must be a class with fields supported by the generated FromRow adapter: scalar `i8/i16/i32/i64/u8/u16/u32/f32/f64/bool/str/bytes` fields, their `Option` form, and supported `owned[...]` wrappers. Field names must match returned column names. Use `T?` for a field that accepts SQL NULL. Enums, resources, arbitrary generics, and unsupported field types are rejected by `check`. Runtime `Result` handles actual value types, NULL decoding, and integer-range errors. Handwritten Rust `FromRow` implementations do not bypass the new API's checker.

## Public types and operation signatures

The module exposes eight types and 18 operations. `view[T]` borrows a value, `Future[T]` is asynchronous work to `await`, and `Result[T, E]` contains either a value or an error. `try` propagates Err from the current function. See [views](view-and-zero-copy.md), [async](async.md), and [error handling](error-handling.md) for the language rules.

| Type | Copy / field storage / shared / debug | Meaning |
|---|---|---|
| `Pool` | non-Copy / storable / shared / Debug | `clone_pool` makes a handle to the same pool |
| `Tx` | non-Copy / not storable / not shared / no Debug | Affine transaction handle bound to the same task |
| `Parameters` | non-Copy / storable / not shared / no Debug | Builders consume and extend owned bind values |
| `Options` | non-Copy / storable / not shared / Debug | Validated configuration from `options` |
| `BeginMode` | Copy and equality / storable, shared, Debug | `DEFERRED` / `IMMEDIATE` / `EXCLUSIVE` |
| `Failure` | non-Copy / storable, shared, Debug | Error and transaction/retirement metadata |
| `FailureKind` | Copy and equality / storable, shared, Debug | 13 classifications below |
| `Outcome` | Copy and equality / storable, shared, Debug | Five observable states below |

| Operation | Signature | Use and return |
|---|---|---|
| `options` | `(connections: i64, queue_capacity: i64, acquire_ms: i64, busy_ms: i64) -> Result[Options, Error]` | Set all values and validate their ranges |
| `open` | `(path: view[str], options: Options) -> Future[Result[Pool, Failure]]` | Own the path and create a Pool; native connections are lazy |
| `clone_pool` | `(pool: view[Pool]) -> Pool` | Create a handle sharing close/failure state |
| `begin` | `(pool: view[Pool], mode: BeginMode) -> Future[Result[Tx, Failure]]` | Acquire a logical slot and start a transaction |
| `parameters` | `() -> Parameters` | Create an empty bind list |
| `bind_i64` | `(parameters: Parameters, value: i64) -> Parameters` | Add an integer |
| `bind_f64` | `(parameters: Parameters, value: f64) -> Result[Parameters, Error]` | Add a finite float |
| `bind_text` | `(parameters: Parameters, value: str) -> Parameters` | Add owned text |
| `bind_bytes` | `(parameters: Parameters, value: bytes) -> Parameters` | Add owned bytes |
| `bind_null` | `(parameters: Parameters) -> Parameters` | Add plain SQLite NULL |
| `query[T]` | `[T](tx: view[Tx], sql: view[str], parameters: Parameters) -> Future[Result[Option[T], Failure]]` | First row of readonly row-producing SQL |
| `all[T]` | `[T](tx: view[Tx], sql: view[str], parameters: Parameters) -> Future[Result[List[T], Failure]]` | All rows of readonly row-producing SQL |
| `exec` | `(tx: view[Tx], sql: view[str], parameters: Parameters) -> Future[Result[i64, Failure]]` | One statement with no result columns |
| `commit` | `(tx: Tx) -> Future[Result[unit, Failure]]` | End the transaction with commit |
| `rollback` | `(tx: Tx) -> Future[Result[unit, Failure]]` | End the transaction with rollback |
| `close` | `(pool: view[Pool], timeout_ms: i64) -> Future[Result[unit, Failure]]` | Start closing and wait for worker joins |
| `copy_primary_error` | `(problem: view[Failure]) -> Option[Error]` | Explicitly copy the primary cause |
| `copy_cleanup_error` | `(problem: view[Failure]) -> Option[Error]` | Explicitly copy the cleanup cause |

After awaiting an async operation, handle `Result` with `try` or `match`. `options` and `bind_f64` return `Error` on failure; other database operations return `Failure`.

#### Building Parameters

`bind_i64` and the other builders move the incoming `Parameters` and return an extended value. Pass that result to the next builder. `query`, `all`, and `exec` also consume `Parameters`, so it cannot be reused.

```nagi
def make_values() -> sqlite.Parameters:
    values = sqlite.parameters()
    values = sqlite.bind_i64(values, 4)
    values = sqlite.bind_text(values, "Nagi")
    return values
```

The values bind in order to anonymous `?` placeholders. To use values in another SQL statement after a query, build a new `Parameters` for that statement or build them again. `copy` cannot clone `Parameters`.

#### Read multiple rows

`all[T]` takes a row class and returns all rows. For the `Total` class above, reading amounts looks like this:

```nagi
async def read_all(tx: view[sqlite.Tx]) -> Result[List[Total], sqlite.Failure]:
    return await sqlite.all[Total](tx, "SELECT amount FROM amounts ORDER BY amount", sqlite.parameters())
```

Choose `query` for the first row and `all` to iterate through every row. Use `exec` for DDL or writes that return no columns. Runtime rejects a mismatch between the operation and SQL shape, multiple statements, and named or numbered placeholders.

## Common mistakes and fixes

| Mistake | Stage and reason | Fix |
|---|---|---|
| `sqlite.query[i64](tx, ...)` | Rejected by `check`. The new API needs a class handled by generated FromRow | Define a class with an `amount: i64` field and use `query[Amount]` |
| `sqlite.exec(tx, "DELETE FROM items WHERE id = ?1", params)` | Runtime SQL failure: the new API accepts anonymous `?` only | Use `?` and build Parameters in that order |
| Reuse `values` after `sqlite.all[Row](tx, sql, values)` | `all` consumes `Parameters` | Build separate Parameters for each SQL statement |
| Use `tx` after `try await sqlite.commit(tx)` | `commit` consumes `Tx` when its Future is created | Call `begin` for a new transaction |
| Save `Tx` in a class field or move it to a child task | Rejected by `check`; Tx cannot cross storage, sharing, or task boundaries | Finish it in the same task, or call `begin` inside the child task |
| Pass a local `tx` to `def read(tx: sqlite.Tx)` | Moves Tx into an owned parameter, leaving the caller without it | Take `view[sqlite.Tx]` and call `read(view(tx))` |

An application helper can borrow a Tx and use it in the same task:

```nagi
async def read_total(tx: view[sqlite.Tx]) -> Result[Total?, sqlite.Failure]:
    return await sqlite.query[Total](tx, "SELECT amount FROM amounts", sqlite.parameters())

async def call_read_total(tx: sqlite.Tx) -> Result[Total?, sqlite.Failure]:
    return await read_total(view(tx))
```

The SQLite authorizer also rejects `BEGIN` / `COMMIT` / `ROLLBACK` in SQL, `SAVEPOINT`, `PRAGMA`, `ATTACH`, and `DETACH`. Use the module operations for transaction boundaries.

## Transaction and concurrency boundaries

Pool's logical slot wait uses Tokio's FIFO semaphore. The FIFO statement applies only to this **logical permit wait**. It does not promise FIFO completion for native worker startup, native slot registration, or SQLite `BEGIN`.

`acquire_ms` uses one budget from logical permit wait through native record registration. Connection startup and ready/`BEGIN` SQL busy waits after registration are outside that budget. `busy_ms` controls SQLite busy waiting. There is no separate interrupt deadline for SQL execution or commit.

One `Tx` reserves one connection. Pool handles may be shared, but a Tx cannot be moved or captured by a child task or used by another task. If a child needs a transaction, call `begin` with a Pool inside that child. An outer `TaskFailure` (failure of the child itself) is separate from `Result[..., sqlite.Failure]` returned by the child's DB work.

An ordinary SQL, bind, or decode error can leave a Tx usable when native SQLite confirms that the transaction is still active. The failing statement may still have made earlier changes, so an error does not promise no changes. If SQLite automatically rolls the transaction back, later operations are rejected with `ABORTED` / `ROLLED_BACK`. Cancellation does not undo accepted SQL or external side effects. Do not infer that retry or rollback is safe; design idempotency and explicit rollback for the application.

## Read a Failure

Read the failure category separately from the transaction outcome. `outcome` reports what could be observed; it does not mean that retrying is safe.

| `FailureKind` | Meaning |
|---|---|
| `INVALID` | Invalid path/options or timeout setting |
| `CLOSED` | Pool is closing or closed |
| `ACQUIRE_TIMEOUT` | No logical/native slot was available within the acquisition budget |
| `BUSY` | SQLite reported a busy or locked database |
| `SQL` | SQL preparation or execution failed |
| `BIND` | Placeholder/value binding failed |
| `DECODE` | A SQLite row could not be converted into a class field |
| `ABORTED` | The native SQLite transaction had already ended |
| `CLEANUP` | Transaction or connection cleanup failed |
| `WORKER` | Native connection startup or worker operation failed |
| `REPLY_LOST` | No result reached the caller; the outcome is unknown |
| `CLOSE_TIMEOUT` | All workers were not joined before the close deadline |
| `ALLOCATION` | A fallible ledger/idle reservation failed |

`ALLOCATION` reports explicit fallible reservations. The API does not promise recovery from every OOM or allocator abort.

| `Outcome` | Meaning |
|---|---|
| `NOT_APPLICABLE` | There is no active Tx related to this failure, or one has not started |
| `ACTIVE` | Native SQLite confirmed that the Tx was active when the failure was made |
| `COMMITTED` | Commit completion was confirmed |
| `ROLLED_BACK` | Explicit or automatic rollback was confirmed |
| `UNKNOWN` | Completion could not be determined |

`retired` reports whether retirement of the connection was confirmed for this Failure. `false` only means retirement was not confirmed from this failure; it does not prove the worker or Pool is healthy. In particular, do not infer that `REPLY_LOST` / `UNKNOWN` means rollback completed, the connection can be reused, or retry is safe. Debug output is limited to `kind` / `outcome` / `retired`, but the public `message` string is not a stable protocol.

`Failure.message` is a `view[str]` borrowing from the `Failure` owner. To keep it longer than the Failure, use `copy(problem.message)` to make an owned string. Use `copy_primary_error(problem)` and `copy_cleanup_error(problem)` to explicitly copy native causes into `Option[Error]`. They return `None` when that cause is absent; otherwise they copy its kind and message. A failure can carry distinct primary and cleanup information, such as when commit completed but cleanup failed.

## Close the Pool

`close(pool, timeout_ms)` starts a closing state shared by every clone, stops new `begin` calls, and waits for active Tx cleanup, native connection close, and actual worker joins. A timeout of zero can still succeed if shutdown is already complete. Timeout or cancellation after closing starts does not clear the closing state. Active transactions are not killed; a later `close` can wait for completion.

Dropping the last `Pool` handle only requests shutdown. It does not confirm async cleanup or worker joins. If the application needs to know that shutdown completed, an owner must await `close`. Successful close confirms native close and actual worker-thread join. There is no guarantee that close returns if SQL blocks indefinitely or the process is killed or aborts on allocation failure.

## SQL preflight checks

`check --sql-schema FILE --sql-dialect sqlite` is an explicit offline check. For the new API, it can inspect literal SQL passed directly to `query`, `all`, and `exec`. It checks the supplied schema, table/column names, required row fields, and operation SQL shape. DDL passed to `exec` is prepared only; the checker does not execute or modify it.

Dynamic SQL is not checked. The number and types of values in `Parameters` are not statically known, so the checker reports **bind unchecked** and leaves those checks to runtime. The [SQL preflight page](sql-check.md) documents how the feature applies to both the old `db_*` functions and the new API.

## Choosing an API and related pages

- Use this API for typed, variable-length parameters, explicit transactions across statements, multiple connections, and controlled close. It is available only from a compiler built from source that includes this unreleased API.
- Existing applications that run on the published release should keep using the [`db_*` API](database.md). Its SQL uses numbered `?1` placeholders and fixed bind shapes/operations.
- For schema-based offline checks, see [SQL preflight checks](sql-check.md). For Nagi `Result`, `try`, nullable values, `view`, and ownership, see [error handling](error-handling.md), [Option and nullable types](types.md), [ownership](ownership.md), [views](view-and-zero-copy.md), and [async](async.md).
