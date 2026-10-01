# Built-in functions

[Contents](README.md) · [Language guide](language-guide.md) · [Syntax reference](syntax.md)

These functions need no imports. `T` in the tables is a descriptive placeholder for a supported element type, not a promise that user-defined generic functions are available.

## Output, input, and environment

| Call | Return type | Usage and notes |
|---|---|---|
| `print(value)` | `unit` | Prints one number, bool, string, or similar value with a newline; no direct class/list printing API |
| `write(value)` | `unit` | Prints without a newline; accepts the same inputs as print |
| `read_line()` | `Result[str, Error]` | `text = try read_line()`; synchronous console input |
| `env("NAME", "default")` | `str` | Returns the default when the environment variable is unavailable |
| `assert_true(condition)` | `unit` | Requires bool; panics on False |

`read_line` flushes stdout, reads one line, and strips trailing CR/LF. It retains whitespace and returns an empty string at EOF. It blocks the thread, so it is intended for console applications rather than HTTP handlers.

## Strings, lists, and borrowing

| Call | Return type | Usage and notes |
|---|---|---|
| `len(value)` | `i64` | Bytes for str/bytes, elements for List; also accepts views |
| `range(end)` | Range for for loops | `for i in range(3):`; one i64 end, from zero up to but excluding it |
| `append(list, value)` | `unit` | Adds a value of the same element type to a List variable |
| `view(text)` | `view[str]` | Borrows an owned string for reading |
| `view(bytes)` | `view[bytes]` | Borrows owned bytes |
| `view(list)` | `view[T]` | Borrows `List[T]`; do not write `view[List[T]]` |
| `copy(borrowed)` | Owned str/bytes/List | Creates an independent owned copy from a view |
| `slice(borrowed, start, end)` | `Result[view[...], Error]` | `try slice(view(text), 0, 3)`; end is exclusive |

Slice positions are i64. For str/bytes they are byte offsets; for List they are element indices. Out-of-range positions and invalid UTF-8 string boundaries return failures. The original data cannot be moved, reassigned, or appended to while a view is alive.

## Conversion, success, and failure

| Call | Return type | Usage and notes |
|---|---|---|
| `parse_i64(text)` | `Result[i64, Error]` | Parses str/view[str]; `try parse_i64("42")` |
| `parse_f64(text)` | `Result[f64, Error]` | Parses str/view[str] as a float |
| `i64(value)` | `i64` | Lossless widening of i8/i16/i32/u8/u16/u32 |
| `i32(value)` | `Result[i32, Error]` | Range-checks and narrows i64; `try i32(value)` |
| `ok(value)` | `Result[T, Error]` | Returns success; `return ok(value)` |
| `error("reason")` | Contextual `Result[T, Error]` | Invalid input; HTTP 400 |
| `not_found("reason")` | Contextual `Result[T, Error]` | Missing target; HTTP 404 |
| `internal_error("reason")` | Contextual `Result[T, Error]` | Internal failure; HTTP 500 with details withheld |
| `fail(problem)` | Contextual `Result[T, Error]` | Moves an Error while preserving its kind and message |
| `error_kind(problem)` | `str` | Borrows Error to obtain its kind, such as `invalid` or `database` |
| `error_message(problem)` | `str` | Borrows Error and copies its message |
| `some(value)` | `T?` | Constructs a present nullable value; absent is `None` |
| `uuid_parse(text)` | `Result[UUID, Error]` | Parses a UUID from text |
| `uuid_format(value)` | `str` | Formats a UUID as text |

`try` is syntax, not a function. Inside a Result-returning function, it extracts success or returns failure to the caller. Handle a result locally with `match` and `case Ok(value)`/`case Err(problem)`.

Error constructors take an owned message string; `fail` consumes Error. `error_kind` and `error_message` do not consume it. The success type comes from the surrounding Result type, such as a return annotation; without type context it is `Result[unit, Error]`. See [error handling](error-handling.md).

## JSON and HTML

| Call | Return type | Usage and notes |
|---|---|---|
| `json_decode[User](input)` | `Result[User, Error]` | Reads a typed class from str/bytes or their views |
| `json_encode(value)` | `Result[str, Error]` | Encodes a JSON string; no type argument |
| `html(text)` | `Html` | Takes ownership of str for an HTML response |
| `include_text("index.html")` | `str` | Embeds a neighboring UTF-8 file at compile time |

`[User]` is a type argument: write `json_decode[User](input)`, not `json_decode(User, input)`. The include_text path must be a string literal. See [JSON](json.md) and [HTTP](http.md).

## Async, HTTP, and SQLite

These functions are asynchronous. The table lists types **after awaiting**. Extract a Result with `try await ...`, or return the Result directly with `return await ...`.

| Call | Type after await | Purpose |
|---|---|---|
| `sleep(milliseconds)` | `unit` | Waits for an i64 number of milliseconds |
| `db_open(path)` | `Result[Db, Error]` | Opens SQLite; `:memory:` uses memory |
| `serve(db, port)` | `Result[unit, Error]` | Starts a loopback HTTP server; takes ownership of Db |
| `db_exec(db, sql)` | `Result[i64, Error]` | Executes SQL and returns affected row count |
| `db_all[User](db, sql)` | `Result[List[User], Error]` | Reads multiple rows; currently no bind arguments |
| `db_query[User](db, sql, id)` | `Result[User?, Error]` | Reads one row with one i64 bind argument |
| `db_write(db, sql, id)` | `Result[i64, Error]` | One i64 bind argument; returns affected row count |
| `db_insert[User](db, sql, text, number)` | `Result[User, Error]` | Bind arguments are str and i32 |
| `db_update[User](db, sql, id, text, number)` | `Result[User, Error]` | Bind arguments are i64, str, and i32 |

SQL and path accept str/view[str]. Port is i64. Insert/update text is an owned str moved to the worker. Their SQL must use `RETURNING` with columns matching the specified class. The SQL API currently has fixed argument shapes. See [SQLite](database.md).

## Other functions

`share(value)` creates an explicit shared owner, `shared[T]`. `clone_shared(value)` duplicates the shared reference. `size_of[Point]()` returns the type's size as i64.

For actor, supervisor, queue, and measurement built-ins, see [actors](actor.md), [supervisors](supervisor.md), [queues](queue.md), [performance](performance.md), and the matching `examples/`. Some APIs exist for tests and are not yet a complete general-purpose application library.
