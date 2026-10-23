# Nagi guidance for coding agents

Use this directory for agents building or debugging Nagi applications. The guides target the published Nagi compiler **0.2.1**. Check `nagic --version` for the compiler actually in use; editor extensions have independent versions and release schedules. Repository development may contain unreleased work. Do not describe 0.2.2 or an editor plugin release as published unless its release record confirms it.

For work on the compiler, runtime, editor implementation, or this repository, follow the root [AGENTS.md](../AGENTS.md). These application guides do not set compiler contracts or contributor workflow.

## Route by task

| Task | Start here |
|---|---|
| Create, change, or review a Nagi application | Use the [nagi-development skill](skills/nagi-development/SKILL.md). It covers `.nagi`, `.low`, `nagi.toml`, Rust adapters, and checking behavior. |
| Diagnose a parse, check, build, run, generated-code, or IDE problem | Follow the [debugging guide](debugging.md), then return to the skill to implement and verify the fix. |
| Set up a repeatable build and development workflow | Read the [developer workflow](../docs/en/developer-workflow.md). |
| Compare Nagi with other implementation choices | Read the [language comparison](../docs/en/comparison.md). |
| Look up High syntax, types, ownership, Result, async, or supported imports | [High language guide](language.md), with links to the published [language reference](../docs/en/README.md). |
| Call a Rust crate or existing Rust service | [Rust adapter guide](rust-interop.md), then inspect a matching example. |
| Find a working application shape | Choose an example from the table below and read its README, source, and `nagi.toml`. |

The skill can be installed by pointing a skill loader at `ai/skills/nagi-development/` or copying that directory into the loader's skill location. It works without the repository guides; when working in this checkout, also read the guides linked above. Installing the skill does not install Nagi, Rust, or Cargo.

## Prepare and verify an application

Check the available tools and compiler before changing code:

```sh
nagic --version
nagic --help
```

`check`, `lower`, and `symbols` do not invoke Cargo. Check `cargo --version` when preparing to build or run; those commands invoke Rust/Cargo and need the compiler's bundled `runtime/` and platform C build tools. Dependency resolution may need network access. See the [installation guide](../docs/en/getting-started.md) if a required tool is missing.

For a project with `nagi.toml` containing `entry = "main.nagi"`, run from the project directory:

```sh
nagic check --project .
nagic run --project . --profile dev
```

`check` checks Nagi syntax, declared types, ownership, and borrowing, and writes generated Low. `run` then builds and executes the program in one command. A successful `check` does not establish that native generation, Rust/Cargo build, publication of the executable, or runtime behavior succeeds. Report the failing stage from its actual diagnostic.

The default native profile is `release`; use `--profile dev` for the normal edit/run loop. For an explicit release build or pre-release validation, run `nagic build --project . --profile release` when needed; do not also run `nagic run` unless you need a separate execution, since `run` builds again. The published 0.2.1 CLI accepts `--profile dev|release` and `--build-stats` on `build` and `run`; `--incremental` or `--incremental=units` requires `build`/`run --profile dev` and is opt-in. See the [development build guide](../docs/en/development-builds.md) for limits.

## Tested starting points

Run from the repository root. Inspect the selected sample's source and configuration before adapting it.

| Need | Example |
|---|---|
| JSON input, validation, and aggregation | [Stock report](../test-nagi-code/application-examples/stock-report/README.en.md), a High CLI with typed errors |
| HTTP with shared config and no database | [Quote API](../test-nagi-code/application-examples/quote-api/README.en.md), using `std.http.server` and explicit error responses |
| Existing Axum HTTP stack | [Axum service](../test-nagi-code/application-examples/axum-service/README.en.md), where Rust routes call a named Nagi async function |
| Typed JSON and Rust file access | [File JSON](../test-nagi-code/application-examples/file-json/README.en.md), with file operations behind an adapter |
| SQLite storage | [Device settings](../test-nagi-code/application-examples/device-settings/README.en.md), with typed rows and nullable fields |
| Workers and business errors | [Seat reservations](../test-nagi-code/application-examples/seat-reservations/README.en.md), using supervised actors |
| Local Rust crate | [Rust library](../test-nagi-code/rust-library/README.en.md), keeping crate types behind an adapter |
| Module identities and aliases | [Module imports](../test-nagi-code/library-examples/module-imports/README.en.md) |
| Handwritten Low | [Order quote](../test-nagi-code/low-examples/order-quote/README.en.md) |

For example, these commands check and run the configured Axum sample:

```sh
nagic check --project test-nagi-code/application-examples/axum-service
nagic run --project test-nagi-code/application-examples/axum-service --profile dev
```

The server listens on loopback port 8097 by default and stops with Ctrl+C. To exercise this sample from both High and saved Low, run:

```sh
python scripts/verify_application_examples.py --compiler /path/to/nagic --only axum-service
```

The runner checks, builds, starts, and stops the sample and writes results under `build/application-example-verification/`. Use a relevant sample for application behavior; documentation changes do not need the application suite.

## Check the contract

Use the [built-in reference](../docs/en/builtins.md) for signatures, [HTTP API reference](../docs/en/http-server.md) for standard HTTP APIs, and [actor reference](../docs/en/actor-reference.md) for actor types. For uncertain behavior, inspect the matching example and compiler tests, then check a small saved program with the installed compiler. The [roadmap](../docs/en/roadmap.md) and [library design](../docs/en/library-design.md) contain proposals, not shipped API guarantees.
