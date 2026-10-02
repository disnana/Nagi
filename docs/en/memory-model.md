# Memory handling

Nagi distinguishes values that own their data from views that borrow it. Rust performs the final borrow checks and releases values.

| Value | Storage and release |
|---|---|
| Numbers, bools, and copyable classes | Stored directly and passed by copying |
| str, bytes, List, and classes containing owned data | Ownership is transferred; data is released when no longer needed |
| view | Borrows existing data and does not release it |
| shared | Shares data; it is released after the last reference is gone |
| Tasks, channels, and database jobs | Allocate runtime storage that is released when the work finishes or is dropped |

Ordinary values do not all carry reference counts. `shared` uses Rust's `Arc` to track references. See [Ownership](ownership.md) and [Views](view-and-zero-copy.md) for application examples.

Arenas that allocate and release request data together are experimental and are not applied automatically to applications. Their design must account for cleanup of strings and files, and for data lifetimes across async work.
