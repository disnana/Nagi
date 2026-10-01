# Memory model

Version 0.1 uses native values, owned values, and non-owning views. The Rust backend performs the final borrow/Send checks and drops values. There is no custom GC or atomic reference count attached to every value.

| Value | Storage and release |
|---|---|
| Primitives and Copy classes | Stack, registers, or array elements |
| str, bytes, List, and owned classes | Ownership transferred by move; released by Rust drop |
| Views | Borrow within the original data's lifetime; no ownership or responsibility for release |
| shared | Explicit Arc; clone/drop changes an atomic reference count |
| Tasks, channels, and database jobs | Allocated by the runtime; released on completion, cancellation, or drop |

Request arenas are a design candidate, but are not implicitly used by High. There are scoped bumpalo experiments. Values containing String or file descriptors also need destructors; freeing an arena's memory alone does not run them.

Introducing request arenas requires separate rules for borrowed data escaping, lifetimes across async waits, moves to database workers, retries, and storage in shared caches. For 0.1, ownership conversions stay explicit while measurements establish whether allocations actually decrease.
