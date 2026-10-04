# Actors

An actor handles one message at a time and updates its own state. Available from Nagi 0.1.8, `std.actor` uses async functions for initialization and message handling. Nagi provides message, state, and reply types; Tokio provides execution and notification.

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

Message and reply types cannot contain Map, views, shared values, or native resources. Initialization data and actor state can contain resources such as Db when ownership and Rust `Send`/`Sync` requirements are met. A public API for registering arbitrary Rust resource types in Nagi is not implemented.

[Supervisor restart and shutdown](supervisor.md) · [API reference](actor-reference.md) · [Measurements](actor-performance.md)

The older [actor.nagi](../../examples/actor.nagi) tests a fixed counter. Its measurements are separate from this generic API.

See the [actor runtime](../../runtime/src/actor.rs), [compiler tests](../../compiler/tests/actor_stdlib.rs), and [runtime tests](../../runtime/src/actor/tests.rs) for type checking, state, restart, and capacity behavior.
