# Supervisors

A Supervisor in `std.actor` manages startup, failure, restart, and shutdown of registered actors and async tasks. Other children continue working while one child restarts.

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

Successful `shutdown` means children and shared context cleanup have finished. If its deadline expires, it returns an incomplete-cleanup Error and retains ownership records. HTTP or native work holding the shared context must release it before cleanup can finish. When Rust integration uses another Tokio runtime, keep the runtime where `run` started alive through cleanup.

Tokio cancellation is cooperative. Non-yielding computation, blocking native calls, and blocking Drop cannot be forcibly interrupted. Split long computations with `await actor.yield_now()`, or use a separate process whose termination you can manage. External blocking jobs started without retaining this context are outside the group's completion guarantee.

## Observation

`next_event` reports startup, failure, restart, and shutdown. The default event capacity is 256, with diagnostics limited to 1024 UTF-8 bytes. A slow observer receives `EventKind.LAGGED` and a lost-event count. It does not block child work.

Use `next_event_timeout(view(control), 5000)` when monitoring needs a deadline. `Err(WaitError)` distinguishes `WaitKind.TIMEOUT` from `INVALID_TIMEOUT`; `Ok(None)` only means stream closure. Each call includes cursor-lock waiting, and canceled waits leave queued events available. The deadline must be 1..4,294,967,295 ms and fit the platform clock.

A task's `STARTED` is published before its factory body runs. For connection establishment or other initialization, register with `task_with_ready` and call `mark_ready(view(signal))` after setup. Observers then receive `EventKind.READY` with the child name and generation. Repeated or stale notifications are rejected. Actors keep their existing `ready` API.

[The worker sample](../../test-nagi-code/application-examples/supervised-worker/README.en.md) checks explicit task readiness, panic recovery, event deadlines, and shutdown.

This is a native implementation within one process. A VM, hot code replacement, distributed actors, persistent mailboxes, and dynamic child registration/removal are not implemented.

[Writing actors](actor.md) · [API reference](actor-reference.md) · [Measurements](actor-performance.md) · [Runnable example](../../test-nagi-code/library-examples/supervised-service/README.en.md)

The older [supervisor.nagi](../../examples/supervisor.nagi) tests restarting fixed workers.
