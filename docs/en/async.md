# Async functions and scopes

Use `async def` for functions that wait for timers, database operations, or other async work. Use `await` to wait for the result. Other async work can run during that wait.

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

If a child returns a Result error or panics, the scope cancels the remaining children and waits for them. Spawned work must return `unit` or `Result[unit, Error]`. Returning from inside a scope and passing a view to a child are not supported.

If the parent operation itself is dropped, or the scope body panics, cancellation is requested without a guarantee that every child has already stopped. See [Concurrency](concurrency.md) for CPU work and cancellation.
