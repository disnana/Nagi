# Views and zero-copy operations

A view borrows existing memory. `view(str)`, `view(List)`, and `slice(view, start, end)` use the original memory. Invalid ranges and UTF-8 boundaries return Result errors.

An HTTP handler's `body: view[bytes]` borrows a slice from the Bytes held by Axum. That borrow adds no body copy. Receiving data from the OS into HTTP buffers and collecting body frames still have their own costs.

Tests verify that the pointer offset between the source buffer and slice matches the requested offset and that allocations are zero. Borrowed JSON `&str` values are also checked to point within the input buffer.

Unescaping JSON strings can change the original bytes, requiring an owned String or copy. SQLite TEXT/BLOB values may become invalid on the next step, so values returned across the worker boundary must be owned.

```nagi
def identity(data: view[str]) -> view[str]:
    return data
```

A view derived from an input view can be returned. References that would outlive local Strings, owned arguments, or requests are rejected. Version 0.1 does not support borrowed class fields or lending request views to other tasks.
