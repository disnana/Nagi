# Handling failures with Result

[Contents](README.md) · [Syntax](syntax.md) · [Functions](builtins.md)

`Result[T, E]` returns either a success value `T` or a failure value `E`. Use the built-in `Error` or your own class or enum for `E`. Use `try` to return failures to the caller, or `match` to handle them locally.

## Propagate failure to the caller

`try` extracts the success value or returns the failure. The called function and your function must use the same error type `E`.

```nagi
def read_id(text: view[str]) -> Result[i64, Error]:
    id = try parse_i64(text)
    if id < 1:
        return error("id must be positive")
    return ok(id)
```

For async operations, write `value = try await operation(...)`. The value of `try` is `T`; to return a Result, write `return ok(try operation(...))`. Convert a different error type explicitly in a `match`'s `Err` case.

## Separate success and failure

Recovering to a default lets the function return an ordinary `i64`. This complete program is in [result.nagi](../../examples/tutorial/result.nagi).

```nagi
def number_or(text: str, fallback: i64) -> i64:
    match parse_i64(view(text)):
        case Ok(number):
            return number
        case Err(problem):
            print(error_kind(problem))
            message = error_message(problem)
            assert_true(len(view(message)) > 0)
            return fallback

def main():
    print(number_or("21", 0))
    print(number_or("oops", -1))
```

Save it as `result.nagi` and run it from that folder:

```powershell
nagic run result.nagi
```

Output: `21`, `invalid`, `-1`. Patterns begin with uppercase `Ok` and `Err`; constructors use lowercase `ok(...)` and `error(...)`.

- Write exactly one `case Ok(...)` and one `case Err(...)`, in either order.
- Bound names receive the success or failure type. Use `case Err(_):` for an unused value.
- Names are available only within their case. Reusing an outer variable's name is rejected.
- Matching consumes the Result, moving owned payloads such as strings. The origin of a borrowed payload remains checked as borrowed within the case.
- When both cases return, the function is checked as returning a value on every path.

Matching is a statement over Result, Option, or enums. For Option, write both `case Some(value):` and `case None:`. Match expressions, guards, nested patterns, and `case _` are unsupported. [Low](low-language.md) supports the same branches.

## Define your own error type

An enum groups failure kinds and the data each kind needs.

```nagi
enum QuantityError:
    InvalidNumber
    OutOfRange(minimum: i64)

def positive(value: i64) -> Result[i64, QuantityError]:
    if value < 1:
        return fail(QuantityError.OutOfRange(minimum=1))
    return ok(value)

def fallback(problem: QuantityError) -> i64:
    match problem:
        case QuantityError.InvalidNumber:
            return 0
        case QuantityError.OutOfRange(minimum):
            return minimum
```

Construct a unit variant with `QuantityError.InvalidNumber`. For payload variants, use positional arguments such as `QuantityError.OutOfRange(1)` or named arguments. Match every variant exactly once. Bind payloads in field order, using `_` for unused values. Unknown variants, invalid arguments, and missing or duplicate cases fail `check`.

A class can also be the failure value in `Result[T, MyError]`. For example, a `StorageError` class can have a `cause: Error` field; `fail(StorageError(cause=problem))` preserves the original cause. Error classes and enums do not need JSON or database conversions. JSON conversion of enums or classes containing built-in Error is unsupported.

The [typed-error CLI example](../../test-nagi-code/library-examples/typed-errors/README.en.md) covers explicit conversion from built-in Error, propagation with the same error type, and enum matching.

## Inspect, construct, and return Errors

| Form | Meaning |
|---|---|
| `error("reason")` | Creates an input failure with kind `invalid` |
| `not_found("reason")` | Creates a missing-target failure with kind `not_found` |
| `internal_error("reason")` | Creates an internal failure with kind `internal` |
| `error_kind(problem)` | Returns the kind name as an owned string |
| `error_message(problem)` | Returns an owned copy of the message |
| `return fail(problem)` | Moves Error, preserving its original kind and message |

The success type comes from the return or variable context. `fail(problem)` moves a built-in Error, class, or enum failure value. Without context its type is `Result[unit, E]`. `error_kind` and `error_message` apply only to built-in Error and do not consume it. Messages can contain internal information such as database details.

Traditional HTTP handlers such as `@get` return `Result[..., Error]`. The unreleased [`std.http.server`](http.md) accepts a custom error type `E`, with an App or route mapper converting it to Response. Built-in Error kinds in traditional handlers map as follows:

| Kind | HTTP status |
|---|---|
| `invalid` | 400 |
| `not_found` | 404 |
| `busy` | 503 |
| `database`/`internal` | 500 |

Database/internal 500 responses contain `{"error":"internal error"}`; details go to server logs. A successful `None` in `Result[T?, Error]` also becomes 404.

Try the [Result API example](result-api.md) for invalid input, missing data, database failure, and recovery. Its Python smoke script sends HTTP requests to an already running server and checks responses.

## Checks and panics

The checker rejects directly discarded Results and futures that are not awaited. Checking that an assigned Result is handled on every path remains incomplete.

Result failures differ from panics. Scopes detect child task panics. In the unreleased [`std.actor`](actor.md), a business error `E` inside `Turn` keeps the next state and becomes the reply. A handler's own Error or panic invokes the Supervisor's restart policy. `call` returns `Result[Result[R, E], CallError]`, separating business errors from not-ready, stopped, timeout, and other call failures. The [sample](../../test-nagi-code/library-examples/supervised-service/README.en.md) also maps them to HTTP responses.

The older `supervisor_demo` remains a fixed-worker restart test API. Neither mechanism recovers from memory corruption or process aborts.

Diagnostics show the filename, line, relevant source text, and reason. Build errors with an identifiable origin first show the Nagi or Low statement or definition line, followed by the full generated Rust diagnostic. Rust edit suggestions apply to Rust; do not apply them directly to Nagi. Handwritten Rust and unmapped diagnostics retain Rust's output. Precise columns and mappings for every Rust diagnostic are not implemented. Definition navigation also uses original columns.
