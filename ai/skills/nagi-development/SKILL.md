---
name: nagi-development
description: Build, change, and debug Nagi applications using typed High code, project configuration, generated Low, and small Rust adapters. Use when working on .nagi/.low sources or nagi.toml, including JSON, HTTP, actors, and reuse of Rust libraries.
---

# Nagi development

Deliver readable application logic and a working executable. Prefer High (`.nagi`) for business rules; use a small Rust adapter for existing crates and handwritten Low only when the task calls for it. This skill works on its own; if the project contains `ai/README.md`, `ai/language.md`, and `ai/rust-interop.md`, use those for the repository's current contracts and example catalogue.

## Establish the project and contract

1. Read the user's goal, repository instructions, the nearest `nagi.toml`, and the entry source plus relevant imports. Preserve existing project structure and dependency constraints.
2. Run `nagic --version` and `nagic --help`. The accompanying repository guides describe compiler 0.1.10; confirm the installed compiler instead of assuming the VS Code extension's version is the language version.
3. Pick a small working example or inspect current builtin signatures/tests before adding an unfamiliar API. Registered unquoted imports are `std.http.server`, `std.actor`, and `std.result`; Rust crates require an adapter, not a new arbitrary Nagi import. Roadmap proposals are not available APIs.

## Implement a narrow, usable change

Annotate parameters and returned values. Represent records with classes, alternatives with enums, and absence with `T?`/`Option[T]`. Construct nullable values with `some(value)`/`None`; extract them with both `Some`/`None` cases. Give absent values and empty lists type context, such as `missing: i64? = None` and `values: List[i64] = []`.

Use `Result[T, E]` for expected failures: `ok(value)`, `fail(problem)`, or built-in `error("reason")`, which returns an Err containing an invalid Error. `try expression` propagates the same error type; use explicit conversion when E differs. Match both `Ok`/`Err` when recovering. This is not Python exception handling. Handle each Result deliberately, including Results assigned to locals.

Pass read-only strings/lists as `view[str]`/`view[T]` and pass `view(owner)` at the call. Owned arguments move. Copy with `copy(view(owner))` only when the caller needs an independent value. To store a view, first save its owner: `text = "Nagi"`, then `borrowed = view(text)`; `borrowed = view("Nagi")` borrows a temporary and is rejected. Keep borrowed owners valid, and do not move non-Copy fields from shared state or borrowed list elements. Use ordinary owned types rather than unfinished `owned[T]` wrappers.

Use plain functions and loops. Methods, inheritance, field assignment, tuples, dictionaries, comprehensions, lambdas, f-strings, string `+`, trailing commas, `elif`, `break`, `continue`, and `pass` are unsupported. Check exact builtin signatures instead of guessing Python or Rust syntax. Validate numeric and indexing limits before operations that can wrap or panic.

For Rust integration, pair `@rust("native::function")` with a typed `extern def` or `extern async def`; no declaration body or trailing colon. Put a matching `pub` implementation in `[rust].file`, and dependencies under `[rust.dependencies]` in `nagi.toml`. Adapt crate-specific types/errors into supported Nagi values. Named synchronous callbacks such as `fn[i64, i64]` map to Rust `fn(i64) -> i64`. Async extern calls must be awaited; arbitrary async callback parameters are unsupported. An existing Rust server can call a known generated Nagi function directly, such as `super::calculate(input).await`, with the adapter updated for that exact name and type.

## Check, build, and prove the behavior

From the application folder:

```sh
nagic check --project .
nagic build --project .
nagic run --project .
```

`run` executes the application. Choose temporary data and appropriate local resources for checks that touch files, databases, or listeners. A task involving an existing server may require starting it and sending requests instead of waiting for `run` to exit.

`check` validates saved Nagi declarations, ownership, and borrowing and writes generated Low for High input. It does not invoke Cargo or verify adapter bodies. `build` must also pass Rust checks and dependency resolution. Inspect source-mapped diagnostics first, then generated Rust when needed; edit the original source/adapter rather than generated output. Application stdout and compiler stderr are separate.

Use `--project` explicitly when invoking an entry source: `nagic check main.nagi --project .`. An explicit source without it does not discover a nearby config. Project paths are relative to `nagi.toml`; command-line paths are relative to the terminal.

After checking High, validate saved Low separately when changing lowering, imports, type identity, extern boundaries, or native Low integration. For `entry = "main.nagi"`:

```sh
nagic check build/main/generated.low --project . --out build/from-low
nagic build build/main/generated.low --project . --out build/from-low
```

For a different entry stem, use its generated directory. In the current unreleased repository compiler, stable application identity is separate from each successful build generation. Cooperating writers to one output directory use an OS lock; successful executables remain side by side. Read the emitted `native:` path. Set `NAGI_NATIVE_TARGET_DIR` to choose a shared dependency cache, not a successful generation. Confirm installed-release support before relying on this repository behavior. Retain the generated Cargo.lock. For a locked Rust resolution after generation, run `cargo build --release --locked --manifest-path build/main/Cargo.toml`; `nagic build --locked` is unsupported.

Exercise the concrete changed behavior: a successful input, a relevant rejected input, and affected state/file/HTTP behavior. Prefer an existing project smoke script to a duplicate test. In this repository, for example:

```sh
python scripts/verify_application_examples.py --compiler /path/to/nagic --only axum-service
```

This checks and builds the example from both High and saved Low and exercises real HTTP. Use another supported `--only` project when its behavior matches the change. Run focused compiler/runtime tests for compiler changes; documentation-only edits need snippet/link checks, not the full application suite.

## Report completion accurately

State the behavior delivered, relevant files, checks actually run, and any remaining build/environment limitation. Separate successful Nagi checks from successful Rust builds and executed behavior. Do not infer complete error handling, concurrency safety, rollback, server guarantees, or published release status from a successful `check` or a roadmap entry.
