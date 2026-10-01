# Concurrency and scheduling

Nagi uses Tokio's M:N executor. It does not yet have its own scheduler. HTTP, timers, and channels wait asynchronously; SQLite work goes to a dedicated native thread. CPU work uses offloading with a concurrency limit of four.

Ordinary mutable state is not implicitly shared between tasks. The basic tools are moving owned values, immutable shared values, bounded channels, and state held inside actors. The Rust backend makes the final Send checks.

```nagi
async def main() -> Result[unit, Error]:
    result = try await cpu_sum(100000)
    print(result)
    return ok(print("Done"))
```

CPU offloading helps keep the event loop responsive. However, cancelling a caller cannot stop a `spawn_blocking` operation that has already started. Long CPU kernels still need cooperative cancellation and budgets.

The large-task test confirms that every task has been polled and holds completion behind a gate. It records RSS while all tasks are waiting, then releases the gate, joins them, and checks that none remain unfinished. A task count alone does not establish simultaneous waiting. Connection-count tests are separate.
