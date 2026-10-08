# Library and Rust integration examples

Ten projects demonstrate shared code, custom errors, Rust crates, standard HTTP with explicit policies, a custom Axum host, Supervisors, and Low function replacement. Standard HTTP samples declare a public or authenticated policy on each route; the custom Axum sample is a separate trusted host boundary.

[Projects and run instructions](../../docs/en/library-examples.md) · [Shared code structure](../../docs/en/libraries.md) · [日本語](README.md)

Run from the repository root. You need Nagi 0.1.9, Rust/Cargo, and a toolchain that can build applications.

```sh
nagic run --project test-nagi-code/library-examples/rust-json
```

Both pricing applications import `shared/`, so keep this directory structure together. Each project's README explains its input, output, and limits.
