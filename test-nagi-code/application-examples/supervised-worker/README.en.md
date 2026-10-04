# Manage independent workers with a Supervisor

This CLI runs small packing jobs and an audit counter in separate actors, with a resident connector supervised through `actor.task_with_ready`. It executes a fixed scenario, checks the results, and shuts down. It needs no network, database, or interactive input.

Run from this directory. Rust/Cargo is required.

```sh
nagic check
nagic run
```

The scenario checks the following behavior:

1. Accept five packing units and seven audit units.
2. Reject a packing quantity of `-1` with the business error `JobError.InvalidUnits`. Keep the packing total at five without restarting the actor.
3. Make the handler itself return `Error` through `Job.FailWorker`. The caller receives `REPLY_LOST`, and the Supervisor restarts packing as generation 2. Its factory creates fresh state, resetting the total to zero.
4. Add two audit units, keeping its independent total at nine. After the packing restart, process four packing units.
5. Confirm the connector's first-generation panic and active second generation. Call `shutdown`, then verify zero active connectors, two guard cleanups, three stopped children, and the end of the event stream.

The connector starts concurrently, so the order of its panic relative to packing jobs is not fixed. The scenario checks that audit starts once and preserves its state across the packing restart; it does not measure how long audit runs during a restart delay.

stdout contains three progress lines followed by JSON:

```text
job rejected: packing total stays 5
packing restarted: total reset to 0; audit continued at 9
connector panic recovered: generation 2 is active
```

The JSON includes `packing_total: 4`, `audit_total: 9`, `failed_events: 1`, `panicked_events: 1`, `restarts: 2`, `connector_active: 0`, and `connector_cleanups: 2`. Rust's panic hook writes caught panics to stderr too. The message `supervised-worker: controlled connector panic` is intentional; a successful run exits with status 0.

## Distinguish failure layers

| Failure | Result or observation | Meaning |
| --- | --- | --- |
| Invalid quantity | `Ok(Turn(state, Err(JobError)))` | Business rejection: save the state and continue processing |
| Packing worker failure | Handler `Err(Error)`, `FAILED`, `REPLY_LOST` | Actor failure: restart under the TRANSIENT policy |
| Connector panic | Task `PANICKED` | No ordinary business reply: start a fresh task under TRANSIENT |
| Call after shutdown | `Err(CallError)` with `CallKind.STOPPED` | Do not accept the job |

Neither state nor queued jobs are persisted. A restart does not save, roll back, or replay work. A production job processor needs state recovery in its factory and job IDs with durable result checks to avoid duplicate execution.

## Nagi and Rust responsibilities

[main.nagi](main.nagi) implements the actors, messages, business errors, restart checks, lifecycle aggregation, and explicit shutdown. [native.rs](native.rs) contains only the test connector. Its first invocation panics; the second waits on a cancellable future. Rust atomics count attempts and live tasks, and a local guard's Drop counts cleanup. There are no extra crates or additional Tokio runtimes. This connector does not contact an external service.

`shared[Context]` means shared ownership, rather than providing a mutable shared counter in Nagi. The small Rust adapter owns the mutable instrumentation needed for a single injected panic and Drop observations. These are adapter operations rather than implemented Nagi language features.

All Control handles are created before `run`. `clone_control` independently subscribes from its creation time; it does not copy earlier event history. Separate cursors wait for the packing restart and connector READY event; another aggregates the full lifecycle. No fixed sleep determines success. Actor readiness, calls, and each event wait have five-second deadlines. `WaitKind.TIMEOUT` is distinct from a closed event stream. [smoke.py](smoke.py) enforces a 15-second deadline for the whole process, including event waits.

`finish` saves the exercise result and awaits `shutdown` before returning it, attempting explicit cleanup even when the exercise returns `Error`. Successful shutdown and scope completion confirm cleanup of supervised children and shared context. This is a native implementation within one process. A VM, live code replacement, distribution, and persistent mailboxes are not implemented.

## Readiness and next steps

`STARTED` reports task startup; it does not imply completed initialization. The connector calls `mark_ready` after creating its cleanup guard, so generation 2 emits one `READY`. Nagi waits for that event through `next_event_timeout`; it no longer polls the native counters to determine readiness. The counters still verify cleanup.

A useful next sample would combine job IDs, durable results, and factory recovery. Resetting memory state alone cannot provide duplicate prevention or durable completion checks.

[Actor guide](../../../docs/en/actor.md) · [Supervisor](../../../docs/en/supervisor.md) · [日本語](README.md)
