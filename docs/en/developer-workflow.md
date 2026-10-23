# Development and debugging quick reference

Start here for the Nagi 0.2.1 edit, check, run, and release loop. New users can follow [installation](getting-started.md); teams evaluating adoption can read [strengths and language comparisons](comparison.md).

## Three everyday commands

Run these from the application directory containing `nagi.toml`:

```sh
nagic check --project .
nagic run --project . --profile dev
nagic build --project . --profile release
```

| Operation | Purpose |
|---|---|
| `check` | Check Nagi syntax, types, ownership, and related rules without compiling Rust; use first after editing |
| `run --profile dev` | Build for development and execute; a separate preceding `build` is unnecessary if you just want to run |
| `build --profile release` | Produce an optimized build for distribution or performance measurements; verify runtime behavior separately |

CLI `build`/`run` default to release when the profile is omitted. The first dev build still compiles Rust dependencies. Ordinary integer overflow panics in dev and wraps in release, so verify production conditions too. [Profiles and incremental builds](development-builds.md)

## Find information by task

| Task | Start with |
|---|---|
| Find parameter and return types | [Built-in APIs](builtins.md), [types](types.md), [editor assistance](editor.md) |
| Write an HTTP API | [HTTP guide](http.md) → [server APIs](http-server.md) → [authentication and authorization](security.md) |
| Use a database | [SQLite Pools and Transactions](sqlite-pool.md), [SQL checks](sql-check.md) |
| Return or handle a failure | [Results and error handling](error-handling.md) |
| Fix move/borrow errors | [Ownership](ownership.md), [view examples](view-and-zero-copy.md) |
| Use a Rust crate | [Project configuration](projects.md), [Rust integration](modules-and-rust.md) |
| Start with working code | [Example catalogue](library-examples.md), [first CLI app](first-app.md) |
| Diagnose a slow build | [Timings and rebuild observations](development-builds.md), [measurement conditions](performance.md) |
| Set up application CI | [GitHub Actions generation](user-ci.md) |
| Delegate development/debugging to AI | [Agent entry point](../../ai/README.md), [development skill](../../ai/skills/nagi-development/SKILL.md) |
| Change Nagi itself | [Contributor guide](contributing.md), [repository instructions](../../AGENTS.md) |

## Locate the failing stage

| Symptom | Next step |
|---|---|
| `check` fails | Follow the first diagnostic to original source, types, imports, or move/view use |
| `check` passes, `build` fails | Read the first Cargo error; check Rust adapters, dependencies, and toolchain. A code-generation bug in accepted Nagi source is also possible |
| Unexpected runtime result | Reproduce with the same input, working directory, and profile; distinguish business Err, panic, and launch failure |
| Only the IDE fails | Compare the configured compiler's absolute path and `nagic --version`; compiler and plugin versions are separate |
| Configuration is missing | An explicit SOURCE does not discover nearby configuration; use `nagic check main.nagi --project .` |

A successful check, Rust build, and application test are separate results. Editing generated Low/Rust is temporary: fix the original source or adapter. See the [debugging procedure](../../ai/debugging.md) for reducing a reproduction and preparing information to share.

## Measure build time

```sh
nagic run --project . --profile dev --build-stats
```

The stderr `nagi-build-stats` record reports pre-Cargo time, Cargo time, and Fresh/compiled artifact counts. Measure initial, unchanged, and function-edit builds separately using the same profile. `--incremental`/`--incremental=units` are explicit dev-only options. They can be slower for small apps or particular changes; consult [measurements and supported cases](development-builds.md) before choosing them.

`NAGI_NATIVE_TARGET_DIR` selects shared dependency storage; `--out` selects generated output. Use the executable path printed after `native:`. Clearing all caches at the start loses evidence and reusable dependencies; begin with [read-only cache inspection](projects.md).

## Before releasing

Run complete `check`, a release build, and application-specific positive/negative tests. Nagi 0.2.1 has no generic `nagic test` command. Add the application's existing harness to CI, testing real HTTP requests or persisted database results where appropriate. Verify installed-version support using the [changelog](../../CHANGELOG.md) and [published releases](https://github.com/disnana/Nagi/releases). Version 0.2.2 is in development.
