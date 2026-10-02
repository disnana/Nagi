# Concurrency

Nagi's async work runs on Tokio. While an HTTP request or timer is waiting, other work can proceed. Start with [Async and scopes](async.md) for the syntax.

SQLite runs on a dedicated thread. CPU-heavy work also runs separately so it does not occupy an async worker for a long time.

```nagi
async def main() -> Result[unit, Error]:
    result = try await cpu_sum(100000)
    print(result)
    return ok(print("Done"))
```

`cpu_sum` demonstrates offloading CPU work. At most four such operations run at once. An API for submitting arbitrary user functions is not implemented.

## Passing data

Mutable values are not automatically shared between tasks. Move ownership when passing a value, or use `shared` for read-only sharing. Borrowed views cannot be passed to another task.

Cancelling the caller does not stop CPU work that has already started. Long-running work needs to check for a stop request itself.

Task counts and HTTP connection counts measure different things. See [Measurements](measurements.md) and [HTTP load tests](http-capacity.md) for results, including waiting time and memory use.
