# Async functions and scopes

Use `async def` for functions that wait for timers, database operations, or other async work. Use `await` to wait for the result. Nagi generates Rust futures and runs them on Tokio, allowing other async work to proceed while waiting. Synchronous CPU work is not automatically moved to another thread.

```nagi
async def main() -> Result[unit, Error]:
    await sleep(10)
    return ok(print("Finished waiting"))
```

`sleep` takes milliseconds. This program prints after the wait. For an operation that can fail, use `try await db_open(...)` to handle its Result as well.

## Starting multiple operations

Inside `async with scope`, `spawn` starts child work. Leaving the scope waits for every child to finish.

```nagi
async def main() -> Result[unit, Error]:
    async with scope:
        spawn sleep(10)
        spawn sleep(15)
    return ok(print("Done"))
```

The scope checks child results after its body finishes. If a child returns a Result error or panics, it cancels the remaining children and waits for them. A child failure does not interrupt the body while it runs. Spawned work must return `unit` or `Result[unit, Error]`. Returning from inside a scope and passing a view to a child are not supported.

Arguments are evaluated at the `spawn` statement, and the resulting values are passed to the child. With `spawn work(copy(part))`, the child receives an owned copy, so the parent can keep using the original data. Copying a list does not make it safe to pass if its elements still contain views.

A function using a scope returns Result. A custom error class or enum requires an explicit Rust adapter implementing `From<nagi_runtime::Error>`. The build checks that child failures can be converted to that type. Spawned children still use Error as their error type.

If the parent operation itself is dropped, or the scope body panics, cancellation is requested without a guarantee that every child has already stopped. See [Concurrency](concurrency.md) for CPU work and cancellation.

## Call a function stored in a variable

An async function name can be assigned to a variable and called through it. This example prints `42`.

```nagi
async def answer(value: i64) -> i64:
    return value + 1

async def main():
    selected = answer
    print(await selected(41))
```

This assignment stores the function itself. Storing a call result with `pending = answer(41)` is unsupported; await the call directly. See [types and inference](types.md#pass-a-function-as-a-value) for supported function signatures.

You cannot reassign a different async function to that variable. Use a separate variable or call each function in a branch. Reassigning the same function, and replacing a synchronous function, are supported.

See the [scope runtime](../../runtime/src/concurrent.rs) and [code generation](../../compiler/src/emit.rs) for the implementation. [Scope tests](../../compiler/tests/scoped_tasks.rs) and [async function value tests](../../compiler/tests/async_value_types.rs) cover accepted and rejected inputs.
