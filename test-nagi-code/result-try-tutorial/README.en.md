# Result and try: from input to function boundaries

[日本語](README.md)

This small console project runs on Nagi 0.2.1. `main.nagi` places many Python comparisons and Nagi behavior notes beside the relevant lines. `clean.nagi` is a shorter version, and `companion.py` implements the same input, calculation, and recovery with Python exceptions.

Run from the repository root:

```sh
nagic check --project test-nagi-code/result-try-tutorial
nagic run --project test-nagi-code/result-try-tutorial
```

Run the clean version directly with `nagic run test-nagi-code/result-try-tutorial/clean.nagi`.

Input `21` to see `21`, `42`, and `完了`. Input `abc` to see the parse error handled by `match`; input `-1` to handle the error created by this example.

`Result[i64, Error]` represents either `Ok(i64)` or `Err(Error)`. Result did not originate in Nagi. Nagi makes success and failure explicit return values and provides `try` and `match` to work with them.

`make_quote` calls `parse_amount`. On success, `try parse_i64(text)` puts the integer inside `amount`. On failure, Err returns from `parse_amount` to its caller, `make_quote`. The next `try` returns the same Err from `make_quote` to its caller, `main`. Here `main` matches the result and recovers by displaying the message. In the clean version, `main` uses `try` too, so the Err propagates to the program entry point and is reported as a runtime error.

Python's `int(text)` raises `ValueError`; if uncaught, it propagates through callers. The companion catches it in `main` and prints the same guidance. Nagi's `try` is not Python's `try/except`: it returns an Err in the function's `Result` value.

`Quote` is a class with two integer fields, demonstrating a data type that groups related values. This example does not use methods or field reassignment.

## Continue

- [Result, try, and match reference](../../docs/en/error-handling.md)
- [Syntax reference](../../docs/en/syntax.md)
- [HTTP/JSON/SQLite Result API](../../docs/en/result-api.md)
- [HTTP and SQLite tutorial](../../docs/en/task-api-tutorial.md)
