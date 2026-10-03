# Supervisors

A Supervisor in `std.actor` manages startup, failure, restart, and shutdown of registered actors and async tasks. Other children continue working while one child restarts.

## Restart policy

| Policy | Restart condition |
| --- | --- |
| `RestartPolicy.TEMPORARY` | Never restart |
| `RestartPolicy.TRANSIENT` | Child Error or panic; the actor default |
| `RestartPolicy.PERMANENT` | Error, panic, or normal completion |

A restart calls the factory again to create fresh state. In-flight and queued messages are not redelivered. If state needs recovery, load it from a database or another source in the factory.

The default delay is 10ms, with at most five restarts per ten seconds across the Supervisor. Exceeding the limit stops siblings and makes `run` return Error. Explicit shutdown or parent cancellation does not trigger restart.

## Ownership and shutdown

Passing a Supervisor to `run` seals registration. A `Control` handle requests shutdown and observes events. Dropping Control does not stop an owned Supervisor. Dropping the Supervisor or its running `run` future requests child cancellation.

Successful `shutdown` means children and shared context cleanup have finished. If its deadline expires, it returns an incomplete-cleanup Error and retains ownership records. HTTP or native work holding the shared context must release it before cleanup can finish. When Rust integration uses another Tokio runtime, keep the runtime where `run` started alive through cleanup.

Tokio cancellation is cooperative. Non-yielding computation, blocking native calls, and blocking Drop cannot be forcibly interrupted. Split long computations with `await actor.yield_now()`, or use a separate process whose termination you can manage. External blocking jobs started without retaining this context are outside the group's completion guarantee.

## Observation

`next_event` reports startup, failure, restart, and shutdown. The default event capacity is 256, with diagnostics limited to 1024 UTF-8 bytes. A slow observer receives `EventKind.LAGGED` and a lost-event count. It does not block child work.

This is a native implementation within one process. A VM, hot code replacement, distributed actors, persistent mailboxes, and dynamic child registration/removal are not implemented.

[Writing actors](actor.md) · [API reference](actor-reference.md) · [Measurements](actor-performance.md) · [Runnable example](../../test-nagi-code/library-examples/supervised-service/README.en.md)

The older [supervisor.nagi](../../examples/supervisor.nagi) tests restarting fixed workers.
