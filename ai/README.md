# Nagi for coding agents

This directory contains agent documentation and a reusable development skill. Human tutorials remain in [`docs/`](../docs/README.md). Start here when generating, changing, or reviewing a Nagi application.

For changes to the Nagi compiler, runtime, editors, or repository itself, read the root [AGENTS.md](../AGENTS.md) instead. These application guides do not define compiler invariants.

Nagi is useful for typed application logic with concise, indented High code (`.nagi`), existing Rust libraries behind small adapters, and handwritten Low (`.low`) where needed. High lowers to Low, then Rust builds the executable. This guide targets the Nagi **0.1.11** compiler release; the VS Code extension has its own **0.1.13** version. Check the official release record and installed compiler before relying on a feature; a repository version alone does not establish publication.

## Load only what the task needs

| Task | Read |
| --- | --- |
| Write or fix High code | [Language and error handling](language.md) |
| Call a crate or reuse a Rust service | [Rust adapter boundary](rust-interop.md) |
| Follow a repeatable implementation workflow | [nagi-development skill](skills/nagi-development/SKILL.md) |
| Find a working starting point | The examples below, including their source and `nagi.toml` |

The skill has a self-contained workflow. Point a skill loader at `ai/skills/nagi-development/`, or copy that directory into the loader's skill location. Its references to this repository's `ai/` guides are optional when used elsewhere. Installing this skill does not install the compiler.

## Check the environment, then work in a project

```sh
nagic --version
nagic --help
cargo --version
```

`check`, `lower`, and `symbols` work without invoking Cargo. `build` and `run` require Rust/Cargo, platform C build tools, and the compiler's bundled `runtime/`. Initial dependency resolution may need network access. See [installation](../docs/en/getting-started.md) if the compiler or build tools are missing.

For a new application, create `my-app/nagi.toml` with:

```toml
entry = "main.nagi"
```

Save this as `my-app/main.nagi`:

```nagi
def main():
    print("Hello, Nagi!")
```

From the directory containing `my-app`:

```sh
nagic check --project my-app
nagic build --project my-app
nagic run --project my-app
```

`check` checks saved source syntax, declared types, ownership, and borrowing, and writes `my-app/build/main/generated.low`. `build` adds Rust type/borrow checking and produces an executable. `run` builds and executes the app with the project directory as its working directory. Both build diagnostics and the actual executable path appear on stderr; application output goes to stdout.

When validating generated Low separately, keep project settings and use a separate output directory:

```sh
nagic check my-app/build/main/generated.low --project my-app --out my-app/build/from-low
nagic build my-app/build/main/generated.low --project my-app --out my-app/build/from-low
```

Omitting SOURCE searches upward for `nagi.toml`. An explicit source such as `nagic check my-app/main.nagi` does **not** load nearby project configuration. Use `--project` whenever the application depends on a Rust adapter, Cargo dependencies, or native Low. Paths passed on the command line are relative to the current terminal directory; paths inside `nagi.toml` are relative to that file.

Do not edit generated Low/Rust to repair High source. Inspect `generated.low`, generated `src/main.rs`, and backend diagnostics to identify the source or adapter change. In the compiler targeted for Nagi 0.1.11, stable application identity is separate from each successful build generation. Cooperating writers to the same output directory use an OS lock; successful executables remain side by side and old ones are not overwritten. Read the emitted `native:` path instead of constructing a filename from the entry stem. [`NAGI_NATIVE_TARGET_DIR`](../docs/en/projects.md) selects a shared dependency cache, not a successful generation. See [ADR 007](../docs/internal/adr/007-build-generations.md) and [CHANGELOG](../CHANGELOG.md) for scope and release status.

## Reuse tested applications and assets

Run the following commands from the repository root. Read the chosen example's source before adapting its API.

| Need | Working project and boundary |
| --- | --- |
| JSON input, validation, aggregation | [Stock report](../test-nagi-code/application-examples/stock-report/README.en.md): High CLI with typed errors |
| HTTP with shared config, no database | [Quote API](../test-nagi-code/application-examples/quote-api/README.en.md): `std.http.server` and explicit error responses |
| Existing Axum HTTP stack | [Axum service](../test-nagi-code/application-examples/axum-service/README.en.md): Rust routes call a named Nagi async function and map its Result |
| Typed JSON and Rust file access | [File JSON](../test-nagi-code/application-examples/file-json/README.en.md): adapter owns file operations |
| SQLite storage | [Device settings](../test-nagi-code/application-examples/device-settings/README.en.md): typed rows, NULL, booleans, floats, BLOBs |
| Workers and business errors | [Seat reservations](../test-nagi-code/application-examples/seat-reservations/README.en.md): supervised actors |
| Independent local Rust crate | [Rust library](../test-nagi-code/rust-library/README.en.md): crate types remain behind an adapter |
| Module identities and aliases | [Module imports](../test-nagi-code/library-examples/module-imports/README.en.md): same-named classes in separate files |
| Handwritten Low | [Order quote](../test-nagi-code/low-examples/order-quote/README.en.md): Low imports, JSON, validation |

For example:

```sh
nagic check --project test-nagi-code/application-examples/axum-service
nagic run --project test-nagi-code/application-examples/axum-service
```

The server listens on loopback port 8097 by default and stops with Ctrl+C. To check the existing application's real HTTP behavior from both High and saved Low:

```sh
python scripts/verify_application_examples.py --compiler /path/to/nagic --only axum-service
```

The runner checks, builds, starts, and stops the sample, then writes results to `build/application-example-verification/`. It includes successful requests and rejected input. Select a sample relevant to the change; there is no need to run every application for a documentation edit.

A useful starting request names the inputs, behavior, and proof of completion:

> Use the nagi-development skill to create a JSON stock-report CLI. Start from the stock-report example, keep typed validation and totals in High, reject negative quantities, and preserve Japanese item names. Check and build the configured project, then verify one valid input and one rejected input. Report the commands and results.

## Find the exact contract

Use [`docs/en/builtins.md`](../docs/en/builtins.md) for builtin signatures, [`docs/en/http-server.md`](../docs/en/http-server.md) for standard HTTP APIs, and [`docs/en/actor-reference.md`](../docs/en/actor-reference.md) for actors. For uncertain behavior, inspect [`compiler/tests/`](../compiler/tests/) and the relevant runtime source, then check a small saved program with the installed compiler. The [roadmap](../docs/en/roadmap.md) and [library design](../docs/en/library-design.md) contain proposals; they are not an API reference for shipped support.
