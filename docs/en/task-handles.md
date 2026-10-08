# Receive a concurrent result once

Task result handles and the S2 Supervisor-monitor migration are published in [Nagi 0.1.11](https://github.com/disnana/Nagi/releases/tag/nagi-v0.1.11). Confirm that the installed compiler is version 0.1.11 or later. The existing `spawn work()` statement remains available. See the [runnable High and handwritten Low example](../../test-nagi-code/library-examples/task-results/README.en.md).

```nagi
from std.task import discard

async def answer() -> i64:
    return 42

async def main() -> Result[unit, Error]:
    async with scope:
        task = spawn answer()
        result = await task
        match result:
            case Ok(value):
                print(value)
            case Err(failure):
                print("Child task failed")
        background = spawn sleep(10)
        discard(background)
    return ok(print("Done"))
```

`task = spawn answer()` starts concurrent work and returns `Task[i64]`. `await task` consumes the handle once and returns `Result[i64, TaskFailure]` after the child has actually terminated. Direct await of an async call remains available. Storing a Future itself is unsupported.

Task is non-Copy, non-Clone, and non-shared for every result type. It belongs to a local in its creating scope. Await or discard is required before normal binding exits, scope exits, and loop continuation. Both branches must satisfy this obligation. With `from std.ownership import move`, `alias = move(task)` transfers the handle and obligation. A bare `move(task)` cannot abandon it.

If a Task reaches scope exit without await or discard, `check` rejects the normal exit because the Task is still unhandled.

```nagi
async def answer() -> i64:
    return 42

async def main() -> Result[unit, Error]:
    async with scope:
        task = spawn answer()
    return ok(print("done"))
```

If you need the value, write `result = await task` and handle the result. If you do not need it, import `discard` from `std.task` and call `discard(task)`. The scope waits for the child to finish in either case.

The right side of short-circuit `and`/`or` can be skipped, and an `env` fallback is skipped when a value exists. Awaiting or discarding only there does not satisfy the normal-exit obligation. The checker does not waive this obligation based on constant conditions. Receive the Task first, then use its ordinary Result conditionally. Receipt in an always-evaluated position, such as the left operand or the `env` key, remains valid.

A Task cannot escape through a function argument or return, a field, a List, an Option or Result wrapper, or another task. An inner scope cannot receive an outer scope's Task. Receive it in its original scope after the inner scope finishes. Return inside a scope remains unsupported.

## Business results and task faults

When a child returns `Result[T, E]`, receiving yields `Result[Result[T, E], TaskFailure]`. An inner business Err alone does not stop siblings. Panic, unexpected cancellation, an Err from a legacy spawn in the same scope, and bridge protocol faults are outer failures.

Treating the outer and inner Err as one case confuses a Task fault with a business rejection. To inspect the business result, match the inner Result inside the outer `Ok`:

```nagi
async def save() -> Result[i64, Error]:
    return error("rejected")

async def main() -> Result[unit, Error]:
    async with scope:
        task = spawn save()
        received = await task
        match received:
            case Ok(operation_result):
                match operation_result:
                    case Ok(value):
                        print(value)
                    case Err(problem):
                        print("business operation rejected")
            case Err(failure):
                print("task failed")
    return ok(print("done"))
```

This example handles the inner Err and continues to print `done`. Receiving an outer `Err(TaskFailure)` leaves the scope failed, so scope exit returns Err.

Import `kind`, `message`, and `TaskFailureKind` from `std.task` to inspect a failure. `kind(failure)` returns a Copy enum with `Panicked`, `Cancelled`, `LegacyError`, and `Internal` constants. `message(failure)` returns `view[str]` borrowed from the failure. The failure cannot move while this borrow remains live. TaskFailure is an opaque non-Copy, non-Clone, non-shared value, with no implicit conversion to Error. Arbitrary panic payload text is not guaranteed to survive.

Receiving and matching a fault leaves the scope failed. The first observed fault is primary. The scope requests sibling cancellation, actually joins all children, then returns an Error at scope exit. Observation order need not match spawn order. An original body Err propagated by try is preserved even if a child faults later.

`discard(task)` returns unit and abandons receipt. It does not stop or detach the child, suppress a fault, omit joining, or confirm resource closure. The scope still waits for that child. A legacy `spawn work()` mixed into a Task scope still treats its `Result[unit, Error]` Err as a scope fault. Existing Supervisor/HTTP failure coupling is preserved.

## Migrating Supervisor and HTTP

The [service example](../../test-nagi-code/library-examples/supervised-service/README.en.md) migrates only its monitor to a Task and retains the legacy HTTP spawn. Handling the `Ok(inner)` from `await monitor_task` with the parent's `try inner` propagates a terminal Supervisor Err as the body Err. The scope requests HTTP cancellation, joins its direct children, and returns the original Error. Simply discarding the handle would abandon that inner Err and leave HTTP running. Normal Supervisor shutdown also leaves HTTP running. If an HTTP fault is observed first, it becomes the scope primary.

HTTP state does not hold the Supervisor context. External shared/native Arc owners can make Supervisor cleanup wait until they release the context; a Task does not collect reference cycles.

Dropping or panicking the parent Future requests cancellation; synchronous Drop cannot confirm actual joining. Termination of the scope's direct children does not confirm asynchronous closure of arbitrary nested Rust tasks or HTTP handlers. Forced termination of non-yielding code and rollback of external effects such as database work are not guaranteed. See [Async and scopes](async.md), [Concurrency](concurrency.md), and [implementation and validation status](../internal/task-handles-s1-results.md).
