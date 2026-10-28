---
name: nagi-development
description: Create, change, debug, and verify Nagi applications with the published 0.2.1 compiler. Use for .nagi or .low source, nagi.toml projects, High/Low diagnostics, Rust adapters, HTTP, JSON, SQLite, actors, async work, or generated-code failures. Do not use for changes to the Nagi compiler or editor implementation; follow the repository's contributor instructions for those.
---

# Nagi application development

Use High (`.nagi`) for typed business logic, a small Rust adapter for existing crates or Rust-specific operations, and Low (`.low`) only when the task requires it. Deliver a focused change and verify the behavior the user asked for. This skill can be used on its own. In a Nagi checkout, also read `ai/README.md`, `ai/language.md`, `ai/rust-interop.md`, and, for failures, `ai/debugging.md`.

If the skill was copied without the repository, use the public [agent guide](https://github.com/disnana/Nagi/blob/main/ai/README.md), [language guide](https://github.com/disnana/Nagi/blob/main/ai/language.md), [Rust adapters](https://github.com/disnana/Nagi/blob/main/ai/rust-interop.md), and [debugging guide](https://github.com/disnana/Nagi/blob/main/ai/debugging.md). The [official syntax reference](https://nagi.disnana.com/en/docs/syntax/) and [built-in signatures](https://nagi.disnana.com/en/docs/builtins/) are also available without a checkout. Check each page's version against the installed compiler; if a reference cannot be retrieved, use the inline rules below and a small `check` reproduction rather than inventing an API.

## Establish the version, project, and contract

1. Read the user's goal, repository guidance, nearest `nagi.toml`, entry source, and relevant imports. Preserve existing project structure and dependency constraints.
2. Run `nagic --version` and `nagic --help`. These guides target published Nagi compiler 0.2.1. Editor extensions have separate versions and binaries. Confirm the installed compiler before using a feature; do not assume unreleased repository changes or a plugin version are published. Check `cargo --version` when preparing for or diagnosing a native build, not for Nagi source checking.
3. Choose a nearby working example and read its source/configuration, or consult the relevant reference and compiler tests before using an unfamiliar API. Roadmap and design pages describe proposals, not shipped APIs.
4. Keep compiler implementation work out of this application workflow. For changes to Nagi itself, use the repository's `AGENTS.md`.

## Write code within the supported language

Annotate function parameters and value-returning functions. Use classes for named records, enums for alternatives, `T?`/`Option[T]` for absence, and `Result[T, E]` for expected failures. Match all `Some`/`None`, `Ok`/`Err`, or declared enum cases. `try` propagates an Err with the same error type; it is not exception handling. Give empty lists and absent values type context.

Use `view[T]` for read-only borrowing and pass `view(owner)` at the call. Keep the owner alive while a borrow is used. Passing an owned non-Copy value moves it; use `copy(view(value))` only when an independent copy is needed. For an existing non-Copy local assignment, the published move syntax is `from std.ownership import move` followed by `target = move(source)`. Do not wrap fresh values or every argument/return in `move`. Confirm the installed compiler supports the syntax.

Use plain functions and loops. Development High supports aligned `elif` clauses (unreleased); use nested `if`/`else` with a released compiler. Methods, inheritance, field reassignment, tuples, dictionaries, comprehensions, lambdas, f-strings, string `+`, trailing commas, `break`, `continue`, and `pass` are unsupported. Use exact builtin signatures rather than guessing Python or Rust syntax. Validate numeric and indexing limits before operations that may overflow or panic. Read `ai/language.md` when present in the checkout, or consult its linked syntax reference there. This skill does not require the surrounding repository to be installed.

For Rust integration, declare a typed `extern def` or `extern async def` with `@rust("native::function")`, and implement the matching `pub` function in the configured `[rust].file`. Put dependencies under `[rust.dependencies]` in `nagi.toml`. Adapt crate-specific types/errors into Nagi values; arbitrary Rust types and traits are not Nagi declarations. Async extern calls must be awaited. Read `ai/rust-interop.md` and the example matching the boundary.

## Check, build, and exercise the change

For a project with `entry = "main.nagi"`, run from its directory and specify the project explicitly:

```sh
nagic check --project .
nagic run --project . --profile dev
```

An explicit source file does not automatically load nearby project configuration. Use `nagic check main.nagi --project .` when invoking a file. `check` validates Nagi source and writes generated Low; it does not invoke Cargo or validate Rust adapter bodies. `run` performs native generation/build and executes the program in one command. Keep source checking, native build, and runtime outcomes distinct in diagnosis and final reporting.

The default profile is `release`; use `run --profile dev` for the edit/run loop. For a release build or pre-release validation, run `nagic build --project . --profile release` only when needed. Avoid running both `build` and `run` by default because `run` builds again. Published 0.2.1 supports `--profile dev|release` and `--build-stats` on `build` and `run`. `--incremental` and `--incremental=units` require `build`/`run --profile dev`; these options are opt-in. Check `nagic --help` if using another installed version. `nagic build --locked` is unsupported; after generation, use the generated Cargo manifest when a locked Cargo build is needed:

```sh
cargo build --release --locked --manifest-path build/main/Cargo.toml
```

If the change affects lowering, imports, type identity, extern boundaries, or native Low integration, validate saved Low separately while retaining project settings and using a distinct output directory. For `entry = "main.nagi"`:

```sh
nagic check build/main/generated.low --project . --out build/from-low
nagic build build/main/generated.low --project . --out build/from-low
```

For another entry name, use its generated directory. Do not edit generated Low/Rust to repair High source. Inspect generated `build/main/generated.low`, generated `src/main.rs`, and source-mapped diagnostics to locate the issue, then change the original source or adapter. The emitted `native:` path identifies the executable; do not construct its name from the entry stem. `NAGI_NATIVE_TARGET_DIR` selects a shared dependency cache, not the generated-source output directory.

Exercise the concrete behavior with a successful input and a relevant rejected input, plus affected file, database, or HTTP behavior when applicable. Prefer an existing sample or verification script over a duplicate harness. For this repository, an example is:

```sh
python scripts/verify_application_examples.py --compiler /path/to/nagic --only axum-service
```

This runner checks and builds the selected sample from High and saved Low, then exercises its HTTP behavior. Select an example related to the change. Do not run a full application suite for a documentation-only change.

## Debug failures without hiding them

Record compiler version, exact command, exit code, first diagnostic, and failing stage. Separate Nagi parse/check errors from generated-code preparation, output locking/publication, Rust/Cargo compilation, executable launch, runtime behavior, and IDE behavior using the actual output. A successful `check` only says that saved Nagi source passed that check; a later `build` failure is not automatically an adapter failure. Check project configuration, source path, current directory, target/toolchain, and adapter boundary before changing unrelated code.

Reduce the failure to a small source and input that preserve the failing stage. Retain needed `nagi.toml` settings and local adapter/crate only when the error crosses those boundaries. Preserve original source locations and generated artifacts as evidence. Before sharing logs or a reproduction, remove credentials, tokens, private URLs, customer data, usernames, secret environment values, and secrets from history or screenshots.

Do not patch generated files, disable checks, weaken the input, rerun repeatedly until a flaky case passes, or delete every build/Cargo cache to make the failure disappear. A controlled rerun may test a transient failure; compare exact commands and conditions. Target a specific cache only after identifying its role. The emitted `native:` path and stderr diagnostics help separate compiler/build failure from program output.

For an IDE mismatch, compare CLI and editor compiler paths and versions, project root, supported file type, and extension logs. Do not infer editor plugin availability from compiler publication. If this repository's guides are available, follow `ai/debugging.md` for the symptom-to-stage checklist and safe reproduction procedure.

## Report completion

State the delivered behavior and changed files, exact checks actually run, and any remaining limitation. Distinguish successful Nagi check, successful Rust build, and executed behavior. Do not infer complete error handling, concurrency safety, rollback, server guarantees, or publication status from a successful check, a roadmap entry, or a repository version number.
