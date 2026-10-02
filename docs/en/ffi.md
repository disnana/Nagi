# Calling other languages

Nagi can currently call Rust functions. Declare them with `@rust` and `extern def`, then include the Rust file and any Cargo dependencies in the build. See [Imports and Rust](modules-and-rust.md).

Equivalent High and Low types compile to the same Rust types. Calls within one build do not need to convert values through JSON or another serialization format.

## C interfaces

Direct calls to arbitrary C functions from Nagi or Low are not supported. The built-in SQLite implementation connects to C through Rust's `rusqlite` and `libsqlite3-sys` libraries.

A fixed C-facing type layout and an ABI for transferring memory allocation and release are not defined. Nagi classes and strings cannot be treated directly as C values.
