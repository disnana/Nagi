# Supervisors

A Supervisor in `std.actor` manages startup, failure, restart, and shutdown of actors and async tasks registered in the same process. Its current restart strategy is one-for-one: only the failed child restarts, while other children keep working.

## Manage a worker from startup to completion

Python's TaskGroup provides a way to wait for related tasks. Restarting a failed worker from initialization requires a separate policy. In Nagi, register a factory and a policy with a Supervisor. Start with this complete example that runs once successfully.

```nagi
import std.actor as actor

class Context:
    message: str

async def worker(context: shared[Context]) -> Result[unit, Error]:
    return ok(print(context.message))

async def main() -> Result[unit, Error]:
    group = actor.supervisor[Context](Context(message="worker"), actor.default_options())
    try actor.task(view(group), "once", worker, actor.RestartPolicy.TEMPORARY)
    try await actor.run(group)
    return ok(print("Done"))
```

It prints `worker`, then `Done`. `run` starts the registered child and returns after its completion and cleanup. Registration alone does not start execution.

A common mistake is reading `PERMANENT` as "restart only on failure." It also restarts after normal completion. Use `TEMPORARY` for one run or `TRANSIENT` for restarting after failure; the next table gives their differences.

**In one sentence: a Supervisor owns children and manages restart and shutdown according to policy.** See the [reference](actor-reference.md) for registration, observation, and shutdown APIs.

## Restart policy

| Policy | Restart condition |
| --- | --- |
| `RestartPolicy.TEMPORARY` | Never restart |
| `RestartPolicy.TRANSIENT` | Child Error or panic; the actor default |
| `RestartPolicy.PERMANENT` | Error, panic, or normal completion |

A failure's effect depends on where the handler returns it.

| Handler return value | Effect |
| --- | --- |
| `Ok(Turn(state, Err(AuthError)))` | A business rejection, such as failed authentication. Saves the next state supplied to Turn without restarting |
| `Err(Error)` | Failure of the worker itself; applies the restart policy |

The caller's HTTP mapper chooses a status such as 401 for failed authentication. An `Err(AuthError)` reply does not set an HTTP status by itself.

A restart calls the factory again to create fresh state. In-flight and queued messages are not redelivered. If state needs recovery, load it from a database or another source in the factory.

The default delay is 10ms, with at most five restarts per ten seconds across the Supervisor. Exceeding the limit stops siblings and makes `run` return Error. Explicit shutdown or parent cancellation does not trigger restart.

A failed `TEMPORARY` child stops and its failure is retained. Other children keep running. When no running child or scheduled restart remains, the Supervisor ends; after cleanup, `run` returns the retained Error.

## Ownership and shutdown

Passing a Supervisor to `run` seals registration. A `Control` handle requests shutdown and observes events. Dropping Control does not stop an owned Supervisor. Dropping the Supervisor or its running `run` future requests child cancellation.

Once a [scope](async.md) body finishes and joins its children, an Error from a spawned `run` cancels the scope's remaining children and waits for them to stop. This includes an HTTP server spawned in the same scope. For example, failure of the last `TEMPORARY` worker can lead to HTTP shutdown. Return business rejections separately from worker failures.

This describes legacy statement spawn. When `run` is started through a [Task result handle](task-handles.md), its inner business Result Err alone does not fault the scope. The [service example](../../test-nagi-code/library-examples/supervised-service/README.en.md) awaits the monitor and propagates `Ok(inner)` through the parent's `try inner` as a body Err, requesting HTTP cancellation. Mechanically replacing receipt with discard loses this connection. Normal shutdown leaves HTTP running.

Successful `shutdown` means children and shared context cleanup have finished. An Error can mean either a retained child failure returned after cleanup, or an expired deadline with cleanup incomplete. The current API has no separate completion type, so an Error alone does not distinguish completed from incomplete cleanup.

After the deadline expires, ownership records remain available to track termination. HTTP or native work holding shared context must release it before cleanup can finish. If releasing that reference depends on Supervisor completion, both can wait for each other; callers must design ownership and shutdown order accordingly. When Rust integration uses another Tokio runtime, keep the runtime where `run` started alive through cleanup.

Tokio cancellation is cooperative. Non-yielding computation, blocking native calls, and blocking Drop cannot be forcibly interrupted. Split long computations with `await actor.yield_now()`, or use a separate process whose termination you can manage. External blocking jobs started without retaining this context are outside the group's completion guarantee.

## Observation

`next_event` reports startup, failure, restart, and shutdown. The default event capacity is 256, with diagnostics limited to 1024 UTF-8 bytes. A slow observer receives `EventKind.LAGGED` and a lost-event count. It does not block child work.

Use `next_event_timeout(view(control), 5000)` when monitoring needs a deadline. `Err(WaitError)` distinguishes `WaitKind.TIMEOUT` from `INVALID_TIMEOUT`; `Ok(None)` only means stream closure. Each call includes cursor-lock waiting, and canceled waits leave queued events available. The deadline must be 1..4,294,967,295 ms and fit the platform clock.

A task's `STARTED` is published before its factory body runs. For connection establishment or other initialization, register with `task_with_ready` and call `mark_ready(view(signal))` after setup. Observers then receive `EventKind.READY` with the child name and generation. Repeated or stale notifications are rejected. Actors keep their existing `ready` API.

[The worker sample](../../test-nagi-code/application-examples/supervised-worker/README.en.md) checks explicit task readiness, panic recovery, event deadlines, and shutdown.

This native implementation uses Tokio and does not provide the same process isolation or VM fault model as BEAM. A VM, hot code replacement, distributed actors, persistent mailboxes, dynamic child registration/removal, and one-for-all/rest-for-one restart strategies are not implemented.

[Writing actors](actor.md) · [API reference](actor-reference.md) · [Measurements](actor-performance.md) · [Runnable example](../../test-nagi-code/library-examples/supervised-service/README.en.md)

The older [supervisor.nagi](../../examples/supervisor.nagi) tests restarting fixed workers.

See the [Supervisor runtime](../../runtime/src/actor.rs), [lifecycle management](../../runtime/src/actor/lifecycle.rs), [restart tests](../../runtime/src/actor/tests.rs), and [shutdown and cleanup tests](../../runtime/src/actor/lifecycle_adversarial_tests.rs). They cover business errors, child failures, deadlines, and cleanup under specific conditions.
