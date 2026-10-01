# Async functions and scopes

Async functions generate native futures. Await an async call or spawn it inside a scope rather than discarding it.

```nagi
async def main() -> Result[unit, Error]:
    async with scope:
        spawn sleep(10)
        spawn sleep(15)
    return ok(print("Done"))
```

A normal scope exit joins every child task. A Result error cancels the remaining tasks and awaits their cleanup. A child panic is detected through JoinError and also cancels the remaining tasks. In 0.1, `spawn` accepts only async functions returning `unit` or `Result[unit, Error]`.

Returning from inside a scope is rejected. If the parent future is dropped externally, or the scope body panics, JoinSet's Drop requests an abort. It cannot await every child's termination at that point; cooperative cancellation may take time.

Borrowed views cannot be passed to spawned tasks. Move owned data instead. Safely spawning tasks with arbitrary borrows remains future work.
