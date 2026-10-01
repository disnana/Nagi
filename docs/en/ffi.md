# FFI and ABI plans

Low has no FFI syntax in 0.1. Existing rusqlite/libsqlite3-sys libraries handle SQLite's FFI. Calling arbitrary C functions from user-written Low is not implemented.

Equivalent primitives and classes in High and Low generate the same Rust types. Calls within one build need no serialization. Rust struct layout, String, Vec, and Result are not intended to be exposed directly as a C ABI.

A future C ABI needs fixed-width primitives, pointer-and-length slices, records with fixed layouts, tagged error payloads, and explicit allocator/deallocator functions. Ownership boundaries must specify who frees data and how long a library remains alive.

Currently, `@rust` and `extern def/fn` call Rust functions within the same generated crate. Use `--rust` for a Rust module and `--rust-dep` for Cargo dependencies; see [imports and Rust integration](modules-and-rust.md). These calls do not go through a C ABI. Borrowed buffers, async tasks, and error lifetimes and layouts must be settled before offering a stable ABI for uses such as Python extensions.
