# Concurrency

Nagi's async work runs on Tokio. While an HTTP request or timer is waiting, other work can proceed. Start with [Async and scopes](async.md) for the syntax.

SQLite runs on a dedicated thread. Arbitrary CPU work is not automatically moved elsewhere. The `cpu_sum` test API below offloads work to Tokio's blocking workers.

```nagi
async def main() -> Result[unit, Error]:
    result = try await cpu_sum(100000)
    print(result)
    return ok(print("Done"))
```

`cpu_sum` demonstrates offloading CPU work. At most four such operations run at once. An API for submitting arbitrary user functions is not implemented.

## Passing data

Mutable values are not automatically shared between tasks. Move ownership when passing a value, or share it with `shared`. The shared reference is read-only; it does not remove any interior mutability provided by the underlying type. Views cannot be passed directly to spawn. Build performs the final check of Rust's `Send`/`Sync` requirements.

Cancelling the caller does not stop CPU work that has already started. Long-running work needs to check for a stop request itself.

Task counts and HTTP connection counts measure different things. See [Measurements](measurements.md) and [HTTP load tests](http-capacity.md) for results, including waiting time and memory use.

Nagi does not have its own VM or time-slice scheduler. Non-yielding work can delay other tasks and shutdown. Concurrency and shared-data safety rely on Nagi checks together with Rust type checking and Tokio's execution rules. See the [concurrency runtime](../../runtime/src/concurrent.rs).
