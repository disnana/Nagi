# Handling failures with Result

[Contents](README.md) · [Syntax](syntax.md) · [Functions](builtins.md)

`Result[T, Error]` returns either a success value `T` or a failure `Error`. Use `try` to propagate failure to the caller; use `match` to recover locally or return a different response.

## Propagate failure to the caller

This function extracts a success value with `try` and returns the original Error on failure. A function using `try` must also return Result.

```nagi
def read_id(text: view[str]) -> Result[i64, Error]:
    id = try parse_i64(text)
    if id < 1:
        return error("id must be positive")
    return ok(id)
```

For async operations, write `value = try await operation(...)`.

## Separate success and failure

This complete program is in [result.nagi](../../examples/tutorial/result.nagi). Because it recovers to a default, the function returns an ordinary `i64`.

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

Run it from the repository root:

```powershell
.\target\release\nagic.exe run examples/tutorial/result.nagi
```

Output: `21`, `invalid`, `-1`. Patterns begin with uppercase `Ok` and `Err`; constructors use lowercase `ok(...)` and `error(...)`.

- Write exactly one `case Ok(...)` and one `case Err(...)`, in either order.
- Bound names receive the success or failure type. Use `case Err(_):` for an unused value.
- Names are available only within their case. Reusing an outer variable's name is rejected.
- Matching consumes the Result, moving owned payloads such as strings. The origin of a borrowed payload remains checked as borrowed within the case.
- When both cases return, the function is checked as returning a value on every path.

Current matching is a statement over Result. Match expressions, nullable `Some`/`None`, guards, and nested patterns are not supported. [Low](low-language.md) supports the same branches.

## Inspect, construct, and return Errors

| Form | Meaning |
|---|---|
| `error("reason")` | Creates an input failure with kind `invalid` |
| `not_found("reason")` | Creates a missing-target failure with kind `not_found` |
| `internal_error("reason")` | Creates an internal failure with kind `internal` |
| `error_kind(problem)` | Returns the kind name as an owned string |
| `error_message(problem)` | Returns an owned copy of the message |
| `return fail(problem)` | Moves Error, preserving its original kind and message |

The success type of constructors and `fail` comes from the return or variable context. Without context it is `Result[unit, Error]`. Since inspection does not consume Error, you can call `fail(problem)` afterward. Messages can contain internal information such as database details.

HTTP translates kinds as follows:

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

JSON, database, and input failures differ from panics. Scopes detect child task panics; supervisors can restart panicking workers. These mechanisms do not recover from memory corruption or process aborts.

Diagnostics show the filename, line, relevant source text, and reason. Build errors with an identifiable origin first show the Nagi or Low statement or definition line, followed by the full generated Rust diagnostic. Rust edit suggestions apply to Rust; do not apply them directly to Nagi. Handwritten Rust and unmapped diagnostics retain Rust's output. Precise columns and mappings for every Rust diagnostic are not implemented. Definition navigation also uses original columns.
