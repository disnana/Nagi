# Async functions and scopes

## Use a result after waiting

Like Python's `await asyncio.sleep(0.01)`, Nagi uses `await` to wait. Python's sleep takes seconds; Nagi's `sleep` takes milliseconds. Awaiting a call does not by itself create another task or thread.

Use `async def` for functions that wait for timers, database operations, or other async work. Use `await` to wait for the result. Nagi generates Rust futures and runs them on Tokio, allowing other async work to proceed while waiting. Synchronous CPU work is not automatically moved to another thread.

```nagi
async def main() -> Result[unit, Error]:
    await sleep(10)
    return ok(print("Finished waiting"))
```

`sleep` takes milliseconds. This program prints after the wait. For an operation that can fail, use `try await db_open(...)` to handle its Result as well.

A common mistake is to store the return value as `pending = sleep(10)` and await it later. Future storage is not currently supported. Write `await sleep(10)` to await the call directly.

**In one sentence: await waits for a result.** See [Error handling](error-handling.md) for calls that also return Result.

## Let other work proceed while waiting

In Python, related tasks can be registered with `asyncio.TaskGroup`.

```python
import asyncio

async def main():
    async with asyncio.TaskGroup() as group:
        group.create_task(asyncio.sleep(0.01))
        group.create_task(asyncio.sleep(0.015))
    print("Done")

asyncio.run(main())
```

In Nagi, use `spawn` inside `async with scope`. This is a complete runnable example.

```nagi
async def main() -> Result[unit, Error]:
    async with scope:
        spawn sleep(10)
        spawn sleep(15)
    return ok(print("Done"))
```

Both waits finish before the program prints `Done` once. The execution order between children is not guaranteed. Spawn schedules child work; it does not guarantee that the child's body runs immediately at that statement.

Two successive `await sleep(...)` calls start the second wait after the first finishes. To overlap them, spawn as above; normal scope exit waits for child completion.

**In one sentence: spawn lets work proceed concurrently, and scope manages its lifetime.** Cancellation and failure behavior are not identical to Python's TaskGroup. The next section and [Concurrency](concurrency.md) give the exact limits.

## Legacy statement spawn and scope reference

The scope checks child results after its body finishes. If a child returns a Result error or panics, it cancels the remaining children and waits for them. A child failure does not interrupt the body while it runs. Spawned work must return `unit` or `Result[unit, Error]`. Returning from inside a scope and passing a view to a child are not supported.

Arguments are evaluated at the `spawn` statement, and the resulting values are passed to the child. With `spawn work(copy(part))`, the child receives an owned copy, so the parent can keep using the original data. Copying a list does not make it safe to pass if its elements still contain views.

A function using a scope returns Result. A custom error class or enum requires an explicit Rust adapter implementing `From<nagi_runtime::Error>`. The build checks that scope exit failures can be converted to that type. A legacy statement spawn child's Result uses Error as its error type. A Task binding's inner business Result may use any supported error type.

If a body `try` propagates Err out of the scope, children are canceled and awaited before the Err reaches the outer code. If the parent Future itself is dropped, or the scope body panics, cancellation is requested. Synchronous Drop cannot await async completion, so there is no guarantee that every child has already stopped at that point. Cancellation also does not roll back accepted database work or other external side effects. See [Concurrency](concurrency.md) for CPU work and cancellation.

### Receive a child's result (working branch, unreleased)

S1 implements `task = spawn work()`, `await task`, and `std.task.discard(task)` on the working branch. [Task result handles](task-handles.md) describes single consumption for every T, await/discard obligations at normal exits, scope escape rejection, and TaskFailure APIs. An inner business Err does not stop siblings; handling a receive fault leaves the scope failed. The legacy statement spawn above still cancels siblings on a child `Result[unit, Error]` Err. Supervisor/HTTP migration and public SQLite Pool/Tx remain separate work.

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

See the [scope runtime](../../runtime/src/concurrent.rs) and [code generation](../../compiler/src/emit.rs) for the implementation. [Scope tests](../../compiler/tests/scoped_tasks.rs) and [async function value tests](../../compiler/tests/async_value_types.rs) cover accepted and rejected inputs. The [real-runtime tests](../../compiler/tests/scope_runtime_contract.rs) cover child errors, child panics, and a body `try` failure through High, saved Low, and handwritten Low, checking body continuation and completed child destruction.
