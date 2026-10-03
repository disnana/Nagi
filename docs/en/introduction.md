# About Nagi

Nagi is a programming language for writing readable applications and compiling them into executables. It uses indentation for functions and blocks, then compiles through Rust.

Its name comes from the Japanese word 凪, meaning calm. The idea is that even when the internals are busy, the surface stays calm.

## Writing applications

Use High, the `.nagi` format, for ordinary applications. High aims for Python-like readability, with a consistent style across authors. It supports variables, functions, lists, classes, HTTP, JSON, and SQLite. Start with [Setup and first run](getting-started.md), then try the [HTTP example](http.md) or [task management demo](web-demo.md).

Borrow data for reading with `view`, return failures with `Result`, and make owned copies explicitly with `copy`. The syntax resembles Python, but importing Python libraries is not supported.

## Adjusting generated code

Use Low to inspect generated code or replace a function. Low has explicit types and braces for blocks. Low and Rust remain available for optimization and custom foundations. See [High and Low](low-language.md) and [Rust integration](modules-and-rust.md).

## Development status

Nagi is currently in the 0.1 development series. Rust performs the final borrow checks, so a program accepted by Nagi's `check` can still fail to build. See the [syntax reference](syntax.md) for supported forms and the [roadmap](roadmap.md) for remaining work.
