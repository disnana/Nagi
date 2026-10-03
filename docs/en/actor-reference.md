# std.actor reference

```nagi
import std.actor as actor
```

[Writing actors](actor.md) · [Restart and shutdown](supervisor.md)

## Types

| Type | Purpose |
| --- | --- |
| `Supervisor[C]` | Owns shared initialization data C and child registrations |
| `Control` | Shutdown and event observation handle |
| `Actor[M, R, E]` | Handle with message M, reply R, and business error E |
| `Turn[S, R, E]` | Next state S and `Result[R, E]` |
| `Options` / `ActorOptions` | Group / actor configuration |
| `RestartPolicy` / `CallKind` / `EventKind` | Comparable constant types |
| `CallError` / `Event` | Call failure / lifecycle event |

Factories receive C as `shared[C]`. Each actor owns S. M, R, and E must be owned values with allocation accounting. Classes, enums, Lists, Option, and Result are supported; Map, views, shared graphs, and opaque resources are excluded.

## Registration and execution

| Call | Return |
| --- | --- |
| `supervisor[C](context, options)` | `Supervisor[C]` |
| `control(view(group))` | `Control` |
| `clone_control(view(control))` | `Control` with an independent event cursor |
| `register[S, M, R, E](view(group), name, factory, handler, options)` | `Result[Actor[M, R, E], Error]` |
| `task(view(group), name, factory, policy)` | `Result[unit, Error]` |
| `turn[S, R, E](next_state, reply)` | `Turn[S, R, E]` |
| `await run(group)` | `Result[unit, Error]`; consumes group |
| `await shutdown(view(control))` | `Result[unit, Error]` |
| `await next_event(view(control))` | `Result[Option[Event], Error]` |
| `await yield_now()` | `unit` |

Factories and handlers are named async functions. Actor factories have shape `shared[C] -> Result[S, Error]`; handlers have `(S, M) -> Result[Turn[S, R, E], Error]`. Task factories have `shared[C] -> Result[unit, Error]`. Child names must be unique and contain 1..128 UTF-8 bytes.

## Calls

| Call | Return |
| --- | --- |
| `clone_actor(view(handle))` | Handle to the same actor |
| `await ready(view(handle), timeout_ms)` | `Result[unit, CallError]` |
| `await call(view(handle), message, mailbox_ms, reply_ms)` | `Result[Result[R, E], CallError]` |

A zero ready timeout probes the current state. A zero mailbox timeout attempts immediate admission. The reply timeout must be positive and starts at acceptance. A reply produced before its deadline remains valid if the caller resumes late. Timing out after acceptance does not cancel an update. To spawn an operation borrowing a handle, wrap it in a named async function that owns the handle.

`CallError.kind` is `NOT_READY`, `MAILBOX_FULL`, `MAILBOX_TIMEOUT`, `MESSAGE_TOO_LARGE`, `REPLY_TOO_LARGE`, `STOPPED`, `RESTARTING`, `REPLY_LOST`, or `REPLY_TIMEOUT`. Its `.message` is `view[str]`.

## Configuration

| Call | Purpose / defaults |
| --- | --- |
| `default_options()` | 64 children, 256 events, five restarts per ten seconds, ten-second shutdown deadline |
| `options(children, events, restarts, window_ms, shutdown_ms)` | `Result[Options, Error]` |
| `restart_delay(options, milliseconds)` | `Result[Options, Error]`; default 10ms |
| `default_actor_options()` | 64 messages, 1MiB inputs, 1MiB reply, five-second startup, TRANSIENT |
| `actor_options(messages, message_bytes, reply_bytes, startup_ms, policy)` | `Result[ActorOptions, Error]` |

Capacity includes accepted work in progress. An oversized reply returns `REPLY_TOO_LARGE` while preserving the next state. Charging counts inline storage and retained buffer capacity, with depth 64 and at most 65,536 visits. Destruction of rejected values and caller/C/S memory are separate.

## Events

`Event.kind` is `STARTING`, `STARTED`, `FAILED`, `PANICKED`, `RESTART_SCHEDULED`, `STOPPED`, `INTENSITY_EXCEEDED`, `SHUTDOWN`, or `LAGGED`.

`.child_id`, `.generation`, and `.lost_events` are i64; `.truncated` is bool; `.child_name` and `.message` are `view[str]`. Each Control has its own cursor. Reads on one Control are serialized. After shutdown, buffered events are drained before None is returned.
