# Debug a Nagi application

Use this guide when a Nagi application fails to parse, check, build, run, or receive editor assistance. It targets published compiler 0.2.2. Start by recording `nagic --version`, the command that failed, its exit status, and the first useful diagnostic. Editor extensions have separate versions and may launch a different compiler binary. Check their configured path and version independently. Do not infer that an `Unreleased` compiler or plugin change is available from the repository source alone.

## Find the failing stage

Start with Nagi's saved-source check from the application directory. Preserve the project's configuration by using `--project`:

```sh
nagic --version
nagic --help
nagic check --project .
```

An explicit source file does not discover a nearby `nagi.toml`; pass both the source and `--project .` when project settings, Rust dependencies, or native Low are required. `--project` paths and paths in `nagi.toml` have different bases: command-line paths are relative to the current directory, while configured paths are relative to `nagi.toml`.

For the usual development loop, run `nagic run --project . --profile dev` after `check`; `run` builds and executes in one command. Run a separate `nagic build --project . --profile release` for a release build or pre-release validation. Do not run both build and run by default when run would repeat the build. Check `cargo --version` when investigating a native build failure, not as a prerequisite for Nagi's `check`.

| Symptom | Check next |
|---|---|
| Parse or syntax diagnostic | Confirm the file is `.nagi` or `.low`, inspect the reported source location and surrounding block, then compare the construct with [supported syntax](../docs/en/syntax.md). Nagi resembles Python but does not support every Python form. |
| Type, ownership, import, or borrow diagnostic | Treat the first Nagi diagnostic as the current failure. Check the original `.nagi`/`.low` location, imports and types, then consult [language guide](language.md), [ownership](../docs/en/ownership.md), [modules](../docs/en/modules-and-rust.md), or the relevant reference. Do not edit generated Low to silence it. |
| `check` succeeds but `build` fails | `check` validates Nagi source but does not establish that later build stages succeed. Use the actual diagnostic and emitted stage information to distinguish source loading/checking, generated-code preparation, output locking or publication, Cargo/rustc, and native executable creation. Only inspect adapter types/dependencies when evidence points to that boundary; generated-code defects can be compiler defects. |
| `build` succeeds but `run` fails | Separate executable selection/launch failure from application behavior. Read stderr for build diagnostics and the emitted `native:` executable path; inspect stdout for application output. Reproduce with the same project directory, inputs, environment, and local resources. |
| Generated Rust or Low looks wrong | Preserve the source-mapped diagnostic, inspect generated `build/main/generated.low` and `build/main/src/main.rs`, and use the failure evidence to decide whether the source, adapter, or compiler-generated output is responsible. Fix the original source or report a compiler defect; generated output is evidence, not the source to patch. If lowering/import/type identity is implicated, check saved Low separately with the same project and a distinct output directory. |
| Editor check/completion differs from CLI | Run `nagic --version` and the CLI check on the same file. Confirm the editor's configured compiler path points to that binary, then inspect the extension's output/log panel and its own version. Check support for the active file type and project root. Compiler and editor extension versions are independent. |
| Compiler cannot start or dependencies cannot resolve | Capture the exact executable path and error. Check Rust/Cargo and platform build tools for `build`/`run`; determine whether the failure is toolchain, target, filesystem, or network access before changing code. |

## Make a small safe reproduction

Reduce the failing case while preserving the stage and relevant boundary. Keep a copy of the original command and project configuration. Prefer a temporary copy of the project or a new directory under a task-specific temporary path; do not overwrite the user's output or application data. Include the smallest source file, required imports, relevant `nagi.toml` sections, and one input that reproduces the problem. Keep Rust adapters and local crates only when the failure crosses that boundary.

Preserve the first diagnostic verbatim with its file, line/column, command, exit code, and whether it came from Nagi, rustc/Cargo, the native program, or the IDE. If a generated artifact matters, note its path and the source revision/compiler version that created it. Compare High checking and saved-Low checking only when the suspected change involves lowering or High/Low consistency. To check saved Low independently, keep project settings and select a separate output directory:

```sh
nagic check build/main/generated.low --project . --out build/from-low
nagic build build/main/generated.low --project . --out build/from-low
```

Use these commands only for an entry named `main.nagi`; another entry stem has a different generated directory. The `--out` option chooses generated output, not a dependency cache. For native commands, `--profile dev|release` and `--build-stats` are supported only by `build`/`run`; `--incremental` and `--incremental=units` require `build`/`run --profile dev`. Do not add options based on another compiler's CLI.

## Protect user data and keep evidence

Before sharing a reproduction, remove credentials, tokens, private URLs, customer data, local usernames, and secret environment values from source, configuration, logs, shell history, and screenshots. Replace them with inert placeholders while keeping the same shape and failure. Do not print secrets to test whether they are configured.

Do not mask an intermittent failure by repeatedly rerunning until it passes, deleting all Cargo/build caches, disabling checks, weakening inputs, or switching to a different compiler without recording that change. A single controlled rerun is useful when the failure may be transient; compare the exact command and output, then investigate the changed condition. Preserve existing lockfiles, generated evidence, and successful outputs unless the task specifically calls for their removal. Cache cleanup is a targeted diagnostic only after identifying which cache and state are implicated.

When reporting the fix, state the failing stage, root cause supported by evidence, source/configuration change, exact checks run, and any unverified behavior. Keep `check`, `build`, and executed behavior as separate results. A passing check alone does not establish a successful Rust build or correct runtime behavior.
