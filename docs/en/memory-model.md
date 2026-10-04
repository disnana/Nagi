# Memory handling

Nagi distinguishes values that own their data from views that borrow it. High and Low share these rules. Nagi currently compiles to Rust, which performs the final borrow checks and releases values. There is no GC or Nagi-specific VM.

| Value | Storage and release |
|---|---|
| Numbers, bools, and copyable classes | Stored directly and passed by copying |
| str, bytes, List, and classes containing owned data | Ownership is transferred; data is released when no longer needed |
| view | Borrows existing data and does not release it |
| shared | Shares data; it is released after the last reference is gone |
| Tasks, channels, and database jobs | The runtime allocates storage; cancelling a waiter does not necessarily end the work or release its resources immediately |

Ordinary values do not all carry reference counts. `shared[T]` uses Rust's `Arc` to track references. Runtime resources such as Db may also use reference counting internally. `shared` does not collect reference cycles or automatically provide mutation or thread safety for its contents.

If you stop waiting for async work, a job already sent to a DB worker or a blocking Rust operation may continue. Cancellation does not imply rollback. See [ownership](ownership.md), [views](view-and-zero-copy.md), and [databases](database.md) for examples and limits.

There is an experimental test for allocating and releasing data through an arena, but arenas are not automatically applied to applications. There is no standard API for request-scoped arenas.
