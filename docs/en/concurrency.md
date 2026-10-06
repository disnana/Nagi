# Concurrency

Nagi's async work runs on Tokio. While an HTTP request or timer is waiting, other work can proceed. Start with [Async and scopes](async.md) for the syntax.

## Keep CPU work from blocking other work

In Python too, a long synchronous calculation inside `async def` delays other work on that event loop. APIs such as `asyncio.to_thread(...)` can offload it. Nagi does not yet have an equivalent API accepting arbitrary user functions.

SQLite runs on a dedicated thread. Arbitrary CPU work is not automatically moved elsewhere. The `cpu_sum` test API below offloads work to Tokio's blocking workers.

```nagi
async def main() -> Result[unit, Error]:
    result = try await cpu_sum(100000)
    print(result)
    return ok(print("Done"))
```

This complete example prints the sum of a generated integer sequence, then `Done`. The result of `cpu_sum(100000)` is `-3184`.

A common mistake is to assume that making a function `async def` moves any synchronous calculation to another thread. Split long work to provide opportunities to await, or consider Rust integration or a separate process whose shutdown you can manage.

**In one sentence: treat waiting and continuous CPU work separately.** See [Async and scopes](async.md) for starting and joining tasks, and [Supervisors](supervisor.md) for stopping long-lived workers.

## Current CPU execution and data reference

`cpu_sum` demonstrates offloading CPU work. At most four such operations run at once. An API for submitting arbitrary user functions is not implemented.

## Passing data

Mutable values are not automatically shared between tasks. Move ownership when passing a value, or share it with `shared`. The shared reference is read-only; it does not remove any interior mutability provided by the underlying type. Views cannot be passed directly to spawn. Wrapping a value in shared does not make every payload thread-safe. Cloning a shared handle also differs from copying its payload. Build performs the final check of Rust's `Send`/`Sync` requirements. Actor messages and replies do not yet support shared values either (see [actor limits](actor.md#capacity-and-deadlines)).

Cancelling the caller does not stop CPU work that has already started. Long-running work needs to check for a stop request itself.

Task counts and HTTP connection counts measure different things. See [Measurements](measurements.md) and [HTTP load tests](http-capacity.md) for results, including waiting time and memory use.

Nagi does not have its own VM or time-slice scheduler. Non-yielding work can delay other tasks and shutdown. Concurrency and shared-data safety rely on Nagi checks together with Rust type checking and Tokio's execution rules. See the [concurrency runtime](../../runtime/src/concurrent.rs).
