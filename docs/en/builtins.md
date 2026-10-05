# Built-in functions

[Contents](README.md) · [Language guide](language-guide.md) · [Syntax reference](syntax.md)

These functions need no imports. `T` stands for a supported type. User-defined generic functions are not supported.

## Output, input, and environment

| Call | Argument types, in order | Return type | Usage and notes |
|---|---|---|---|
| `print(value)` | Numbers, bool, str, view[str], UUID | `unit` | Prints one number, bool, string, or similar value with a newline; no direct class/list printing API |
| `write(value)` | Same as print | `unit` | Prints without a newline; accepts the same inputs as print |
| `read_line()` | None | `Result[str, Error]` | `text = try read_line()`; synchronous console input |
| `env("NAME", "default")` | str, str | `str` | Returns the default when the environment variable is unavailable |
| `assert_true(condition)` | bool | `unit` | Requires bool; panics on False |

`read_line` displays pending output, then reads a line and removes its trailing newline. It retains other whitespace and returns an empty string at the end of input. It blocks while waiting for input, so use it in console applications.

`env` evaluates its second argument only when the variable is unavailable. When a value exists, a function constructing the default is not called. See [Unreleased](../../CHANGELOG.md) for the generation fix for defaults containing `try` or `await`.

## Strings, lists, and borrowing

| Call | Argument types, in order | Return type | Usage and notes |
|---|---|---|---|
| `len(value)` | str / bytes / List[T] and their views | `i64` | Bytes for str/bytes, elements for List; also accepts views |
| `range(end)` | i64 | Range for for loops | `for i in range(3):`; one i64 end, from zero up to but excluding it |
| `append(list, value)` | List[T] variable, T | `unit` | Adds a value of the same element type to a List variable |
| `view(text)` | str | `view[str]` | Borrows an owned string for reading |
| `view(bytes)` | bytes | `view[bytes]` | Borrows owned bytes |
| `view(list)` | List[T] | `view[T]` | Borrows `List[T]`; do not write `view[List[T]]` |
| `copy(borrowed)` | view[str] / view[bytes] / view[T] | Owned str/bytes/List | Creates an independent owned copy from a view |
| `slice(borrowed, start, end)` | view, i64, i64 | `Result[view[...], Error]` | `try slice(view(text), 0, 3)`; end is exclusive |

Slice positions are i64. For str/bytes they are byte offsets; for List they are element indices. Out-of-range positions and invalid UTF-8 string boundaries return failures. The original data cannot be moved, reassigned, or appended to while a view is alive.

## Conversion, success, and failure

| Call | Argument types, in order | Return type | Usage and notes |
|---|---|---|---|
| `parse_i64(text)` | str / view[str] | `Result[i64, Error]` | Parses str/view[str]; `try parse_i64("42")` |
| `parse_f64(text)` | str / view[str] | `Result[f64, Error]` | Parses str/view[str] as a float |
| `i64(value)` | i8 / i16 / i32 / u8 / u16 / u32 | `i64` | Lossless widening of i8/i16/i32/u8/u16/u32 |
| `i32(value)` | i64 | `Result[i32, Error]` | Range-checks and narrows i64; `try i32(value)` |
| `ok(value)` | T | Contextual `Result[T, E]` | Returns success; E defaults to Error without context |
| `error("reason")` | str | Contextual `Result[T, Error]` | Invalid input; HTTP 400 |
| `not_found("reason")` | str | Contextual `Result[T, Error]` | Missing target; HTTP 404 |
| `internal_error("reason")` | str | Contextual `Result[T, Error]` | Internal failure; HTTP 500 with details withheld |
| `fail(problem)` | Error, custom class, or enum | Contextual `Result[T, E]` | Moves and returns the failure value E |
| `error_kind(problem)` | Error | `str` | Borrows Error to obtain its kind, such as `invalid` or `database` |
| `error_message(problem)` | Error | `str` | Borrows Error and copies its message |
| `some(value)` | T | `T?` | Constructs a present nullable value; absent is `None` |
| `uuid_parse(text)` | str / view[str] | `Result[UUID, Error]` | Parses a UUID from text |
| `uuid_format(value)` | UUID | `str` | Formats a UUID as text |

`try` is syntax. Inside a function returning Result with the same error type E, it extracts T or returns failure to the caller. Handle a result locally with `match` and `case Ok(value)`/`case Err(problem)`.

The success type T comes from the surrounding Result type. Without context, `fail(problem)` returns `Result[unit, E]`. `error`, `not_found`, and `internal_error` create built-in Error values and take owned message strings. `error_kind` and `error_message` apply only to built-in Error and do not consume it. See [custom errors](error-handling.md#define-your-own-error-type).

## JSON and HTML

| Call | Argument types, in order | Return type | Usage and notes |
|---|---|---|---|
| `json_decode[User](input)` | str / bytes / view[str] / view[bytes] | `Result[User, Error]` | Reads a typed class from str/bytes or their views |
| `json_encode(value)` | JSON-encodable type ([details](json.md)) | `Result[str, Error]` | Encodes a JSON string; no type argument |
| `html(text)` | str | `Html` | Takes ownership of str for an HTML response |
| `include_text("index.html")` | String literal | `str` | Embeds a neighboring UTF-8 file at compile time |

`[User]` is a type argument: write `json_decode[User](input)`, not `json_decode(User, input)`. The include_text path must be a string literal. See [JSON](json.md) and [HTTP](http.md).

## Async, HTTP, and SQLite

These functions are asynchronous. The table lists types **after awaiting**. Extract a Result with `try await ...`, or return the Result directly with `return await ...`.

| Call | Argument types, in order | Type after await | Purpose |
|---|---|---|---|
| `sleep(milliseconds)` | i64 | `unit` | Waits for an i64 number of milliseconds |
| `db_open(path)` | str / view[str] | `Result[Db, Error]` | Opens SQLite; `:memory:` uses memory |
| `serve(db, port)` | Db, i64 | `Result[unit, Error]` | Starts a loopback HTTP server; takes ownership of Db |
| `db_exec(db, sql)` | Db, str / view[str] | `Result[i64, Error]` | Executes SQL and returns affected row count |
| `db_all[User](db, sql)` | Db, str / view[str] | `Result[List[User], Error]` | Reads multiple rows; currently no bind arguments |
| `db_query[User](db, sql, id)` | Db, str / view[str], i64 | `Result[User?, Error]` | Reads one row with one i64 bind argument |
| `db_write(db, sql, id)` | Db, str / view[str], i64 | `Result[i64, Error]` | One i64 bind argument; returns affected row count |
| `db_insert[User](db, sql, text, number)` | Db, str / view[str], str, i32 | `Result[User, Error]` | Bind arguments are str and i32 |
| `db_update[User](db, sql, id, text, number)` | Db, str / view[str], i64, str, i32 | `Result[User, Error]` | Bind arguments are i64, str, and i32 |

SQL and path accept str/view[str]. Port is i64. Insert/update text is an owned str moved to the worker. Their SQL must use `RETURNING` with columns matching the specified class. The SQL API currently has fixed argument shapes. See [SQLite](database.md).

## Sharing and type sizes

| Call | Argument types | Return type | Behavior and limits |
|---|---|---|---|
| `share(value)` | T | `shared[T]` | Takes ownership and creates a shared value |
| `clone_shared(value)` | shared[T] | `shared[T]` | Adds a reference to the same value without copying all its data |
| `size_of[Point]()` | None; one type argument | `i64` | Type size in bytes; excludes separately allocated string/list storage |

See [memory handling](memory-model.md) for ownership and [classes](classes.md) for a size example.

## Test and measurement functions

These functions test the runtime and measure performance. Functions such as `actor_demo` run fixed examples; they do not register user-defined actors or workers.

This table lists asynchronous functions and their return types after awaiting.

| Call | Argument types | Type after await | Test behavior |
|---|---|---|---|
| `actor_demo(count)` | i64 | `Result[i64, Error]` | Sends messages to a counter |
| `actor_pair_demo(count)` | i64 | `Result[i64, Error]` | Communicates through a forwarding actor and a counter |
| `supervisor_demo()` | None | `Result[i64, Error]` | Restarts a worker after a panic |
| `queue_demo(count)` | i64 | `Result[i64, Error]` | Queues jobs and retries failures |
| `task_demo(count)` | i64 | `Result[i64, Error]` | Starts child tasks and waits for completion |
| `cpu_sum(count)` | i64 | `Result[i64, Error]` | Runs CPU work on a separate executor |

See [actors](actor.md), [worker restarts](supervisor.md), [queues](queue.md), [concurrency](concurrency.md), and the matching `examples/`.

The following functions are synchronous.

| Call | Argument types, in order | Return type | Behavior |
|---|---|---|---|
| `clock_ns()` | None | `i64` | Measurement timestamp in nanoseconds |
| `make_ints(count)` | i64 | `List[i64]` | Creates an integer list for CPU tests |
| `bench_i64(name, count, kernel)` | str, i64, a function taking view[i64] and returning i64 | `unit` | Measures integer-list processing |
| `bench_f64(name, count, kernel)` | str, i64, a function taking view[f64] and returning f64 | `unit` | Measures floating-point-list processing |
| `bench_scalar(name, count, kernel)` | str, i64, a function taking i64 and returning i64 | `unit` | Measures integer processing without a list |

Pass a synchronous function as `kernel`. See [reading benchmarks](performance.md) for methods and units, and the [CPU test](../../examples/cpu.nagi) for an example.
