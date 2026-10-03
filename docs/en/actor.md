# Actors

An actor handles one message at a time and updates its own state. Available from Nagi 0.1.8, `std.actor` uses ordinary async functions to define initialization and message handling.

```nagi
import std.actor as actor

class Counter:
    total: i64

async def add(state: Counter, amount: i64) -> Result[actor.Turn[Counter, i64, Error], Error]:
    total = state.total + amount
    next_state = Counter(total=total)
    return ok(actor.turn[Counter, i64, Error](next_state, ok(total)))
```

`Turn` contains the next state and a reply. State does not need to be copied for each message. The [runnable example](../../test-nagi-code/library-examples/supervised-service/README.en.md) includes registration, startup, calls, and shutdown.

## Separate failures

| Where the failure is returned | Meaning |
| --- | --- |
| The reply inside `Turn` is `Err(E)` | An expected failure. Keep the next state and continue |
| The handler itself returns `Err(Error)` | Actor failure. The Supervisor applies its restart policy |
| The outer result of `call` is `Err(CallError)` | Not ready, full, stopped, reply timeout, or another call failure. Inspect `CallError.kind` |

Replies can use your own error class or enum. `call` returns `Result[Result[R, E], CallError]`, keeping business failures separate from call failures.

## Capacity and deadlines

An actor defaults to 64 accepted messages, including work in progress, and a 1MiB budget for accepted inputs. Numeric Lists are charged without visiting each element. Strings and nested values include retained buffer capacity. State and callers' waiting values are separate, so this is not a process memory limit.

`call` has a `mailbox_ms` admission deadline and a `reply_ms` deadline after acceptance. An accepted update may continue after its reply times out. Use idempotency keys or query the outcome before retrying a write.

The initial API excludes Map, views, shared graphs, and native resources without allocation accounting from message and reply types. Shared initialization data and actor state may contain Db and other resources when their ownership requirements are met.

[Supervisor restart and shutdown](supervisor.md) · [API reference](actor-reference.md) · [Measurements](actor-performance.md)

The older [actor.nagi](../../examples/actor.nagi) tests a fixed counter. Its measurements are separate from this generic API.
