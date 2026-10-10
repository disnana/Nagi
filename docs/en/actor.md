# Actors

## Send requests to update state in order

Think of a dedicated Python worker receiving work from `asyncio.Queue` and updating a counter. With an actor, callers similarly send requests as messages, and the actor updates its own state one message at a time. This is not an API for writing actor state directly from outside.

Available from Nagi 0.1.8, `std.actor` uses async functions for initialization and message handling. Nagi provides message, state, and reply types; Tokio provides execution and notification.

The following fragment defines a handler. A complete program with registration and startup is linked below.

```nagi
import std.actor as actor

class Counter:
    total: i64

async def add(state: Counter, amount: i64) -> Result[actor.Turn[Counter, i64, Error], Error]:
    total = state.total + amount
    next_state = Counter(total=total)
    return ok(actor.turn[Counter, i64, Error](next_state, ok(total)))
```

For example, a state with `total=3` and a message with `amount=2` produces a next state with `total=5` and a successful reply of `5`. Once registered as an actor, this handler is called in sequence for each message.

A common mistake is returning an input rejection as the handler's own `Err(Error)`. That is a worker failure subject to restart policy. Return an expected rejection in the Turn reply, as in `ok(actor.turn(..., fail(problem)))`, together with the next state.

This is not a syntax error; behavior depends on where the error is returned. If a registered handler should reject a negative amount as an ordinary business result, the version below is treated as a worker failure:

```nagi
async def add(state: Counter, amount: i64) -> Result[actor.Turn[Counter, i64, Error], Error]:
    if amount < 0:
        return error("amount must be non-negative")  # supervisor restart policy applies
    next_state = Counter(total=state.total + amount)
    return ok(actor.turn[Counter, i64, Error](next_state, ok(next_state.total)))
```

To send the rejection to the caller, wrap the Turn in the outer `ok(...)`:

```nagi
async def add(state: Counter, amount: i64) -> Result[actor.Turn[Counter, i64, Error], Error]:
    if amount < 0:
        return ok(actor.turn[Counter, i64, Error](state, error("amount must be non-negative")))
    next_state = Counter(total=state.total + amount)
    return ok(actor.turn[Counter, i64, Error](next_state, ok(next_state.total)))
```

The negative amount is now an `Err` reply, leaves the state unchanged, and does not restart the worker.

**In one sentence: request state updates with messages, and return business rejections in replies.** See the [actor reference](actor-reference.md) for APIs and the next section for failure distinctions.

`Turn` contains the next state and a reply. State does not need to be copied for each message. The [runnable example](../../test-nagi-code/library-examples/supervised-service/README.en.md) includes registration, startup, calls, and shutdown.

## Separate failures

| Where the failure is returned | Meaning |
| --- | --- |
| The reply inside `Turn` is `Err(E)` | An expected failure. Keep the next state and continue |
| The handler itself returns `Err(Error)`, or panics | Worker failure. The Supervisor applies its restart policy |
| The outer result of `call` is `Err(CallError)` | Not ready, full, stopped, reply timeout, or another call failure. Inspect `CallError.kind` |

Replies can use your own error class or enum. `call` returns `Result[Result[R, E], CallError]`, keeping business failures separate from call failures.

## Capacity and deadlines

An actor defaults to 64 accepted messages, including work in progress, and a 1MiB budget for accepted inputs. Numeric Lists are charged without visiting each element. Strings and nested values include retained buffer capacity. State and callers' waiting values are separate, so this is not a process memory limit.

`call` has a `mailbox_ms` admission deadline and a `reply_ms` deadline after acceptance. An accepted update may continue after its reply times out. Use idempotency keys or query the outcome before retrying a write.

Message and reply types cannot contain Map, views, shared values, or native resources. Initialization data and actor state can contain resources such as sqlite.Pool when ownership and Rust `Send`/`Sync` requirements are met. A public API for registering arbitrary Rust resource types in Nagi is not implemented.

Allowing shared messages that meet explicit conditions is an adopted future direction, not part of the current accepted types. Detailed design must cover thread safety, capacity charging, retained resources, and values escaping through replies. Distinguish the actor's own state from explicitly shared external resources such as a database. See [DESIGN](../../DESIGN.en.md) for the boundaries.

[Supervisor restart and shutdown](supervisor.md) · [API reference](actor-reference.md) · [Measurements](actor-performance.md)

The older [actor.nagi](../../examples/actor.nagi) tests a fixed counter. Its measurements are separate from this generic API.

See the [actor runtime](../../runtime/src/actor.rs), [compiler tests](../../compiler/tests/actor_stdlib.rs), and [runtime tests](../../runtime/src/actor/tests.rs) for type checking, state, restart, and capacity behavior.
