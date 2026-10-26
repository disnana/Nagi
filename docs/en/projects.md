# Configure applications with nagi.toml

`nagi.toml` configures the entry file, Rust dependencies, and native Low. The CLI, VS Code, and JetBrains plugin use it.

## A minimal project

Create these two files in a new folder:

```text
my-app/
  nagi.toml
  main.nagi
```

`nagi.toml`:

```toml
entry = "main.nagi"
```

`main.nagi`, a complete program:

```nagi
def main():
    print("Hello, Nagi project!")
```

Run from `my-app`. If nagic is not on PATH, use the absolute path to your built compiler.

```powershell
nagic check
nagic run
```

`check` checks types; `run` builds and runs. `lower` and `build` use the same settings. Omitting the source argument searches from the current directory upward for the nearest nagi.toml. This also works from subfolders such as `my-app/src`.

To select a project from elsewhere:

```powershell
nagic run --project my-app
nagic check --project my-app/nagi.toml
```

The application runs with the nagi.toml directory as its working directory. For example, relative `data.sqlite` is created there regardless of the calling terminal's location. Running an executable directly uses that process's working directory.

## Add Rust and handwritten Low

```toml
entry = "src/main.nagi"
native = ["native/math.low"]

[rust]
file = "native/bridge.rs"

[rust.dependencies]
serde_json = "1.0"
```

| Setting | Meaning |
|---|---|
| `entry` | Required `.nagi` or `.low` entry file for execution/checking |
| `native` | Optional array of handwritten Low files to integrate |
| `rust.file` | Optional Rust file included as the native module |
| `rust.dependencies` | Optional map of dependency names to Cargo version strings or tables |

All file paths are relative to nagi.toml. Only entry is required. Unknown settings, wrong value types, empty paths, and duplicate dependencies are errors. Existing version strings remain supported. Matching Rust types and implementations is checked by build/run.

### Select local crates and features

Each dependency can also use a table. This is the configuration of the [local Rust library example](../../test-nagi-code/rust-library/README.en.md):

```toml
entry = "library-pricing.nagi"

[rust]
file = "native.rs"

[rust.dependencies]
pricing = { version = "0.1", path = "engine", package = "nagi-pricing-engine", features = ["volume-discount"], default-features = false }
```

| Table field | Meaning |
|---|---|
| `version` | Cargo version requirement; Cargo also checks it when combined with `path` |
| `path` | Local crate directory, resolved relative to nagi.toml |
| `package` | Actual Cargo package name; Rust refers to the example dependency as `pricing::` |
| `features` | Array of features to enable; an empty array is allowed |
| `default-features` | Whether this declaration enables default features; omission uses Cargo's default of `true` |

At least one of `version` or `path` is required. `git`, `registry`, `workspace`, target-specific dependencies, dev/build dependencies, and `optional` are unsupported.

Paths remain relative to the configuration file regardless of the generated directory or terminal location. `check`, `lower`, and `symbols` do not invoke Cargo, fetch dependencies, or require the dependency crate to exist. You can check the Nagi code before obtaining the Rust crate. Cargo checks the actual path, crate APIs, and dependency resolution during `build`/`run`.

Cargo combines features requested through dependency paths to the same package. Setting `default-features = false` does not disable default features requested through another path.

Rebuilding retains the existing generated `Cargo.lock`. The lock records resolved versions; it does not pin local crate source contents. To build with a fixed resolution, run `cargo build --release --locked --manifest-path build/library-pricing/Cargo.toml` from the example directory. `nagic build --locked` is unsupported.

Try the [Rust integration example](../../test-nagi-code/rust-bridge/nagi.toml) from the repository root:

```powershell
.\target\release\nagic.exe run --project test-nagi-code/rust-bridge
```

It calls CRC32, JSON formatting, and an async Rust function. The [task management JSON API](web-demo.md) also has an entry configured.

## Command-line precedence

An explicit source alone, such as `nagic run main.nagi`, processes that file independently. It does not automatically read a nearby nagi.toml.

| Argument | Behavior |
|---|---|
| `--project DIR` / `--project FILE` | Uses that configuration |
| `SOURCE --project DIR` | Uses the config but replaces its entry with SOURCE, relative to the terminal |
| `--rust FILE` | Overrides rust.file |
| `--rust-dep NAME=VERSION` | Replaces the entire same-name dependency with a version string, or adds a new one |
| `--native FILE.low` | Adds to native |
| `--out DIR` | Changes the generated Low/Rust/Cargo.toml location |
| `--no-project` | Disables automatic search; requires SOURCE |

Replacing a same-name table with `--rust-dep` removes its `path`, `package`, `features`, and `default-features`. The CLI accepts version strings only.

Command-line relative paths use the terminal's working directory. Duplicate SOURCE, project, rust, out, or same-name rust-dep arguments are errors. project and no-project cannot be combined.

Generated Low, Rust, and Cargo.toml go to `build/<entry filename without extension>/`. The build cache remains in `build/native-target/`. The `native:` line that `build` and `run` print to stderr gives the executable's actual path.

Nagi 0.2.1 provides `nagic cache inspect` to read explicitly supplied output and shared Cargo cache roots and print a versioned JSON observation. The command is `nagic cache inspect --out-root ABS --cargo-root ABS --max-entries N --max-depth N`. Specify both absolute roots and both per-root bounds exactly once: entries from 1 to 4096 and depth from 1 to 64. Each root is limited to 4096 OS-encoded path bytes. It does not discover roots from project settings or environment variables, create files or locks, or invoke Cargo.

JSON reports each root's structural classes, logical file lengths, bounded samples, and reason counters for links, errors, and limit hits. Missing roots and incomplete traversals still produce JSON and exit with status 1. Syntax errors stop before scanning and produce no JSON. Classification is only a structural hint. Hardlinks are counted by path and can make logical byte counts overlap; physical allocation, free space, and bytes recoverable by deletion are not measured. Roots can point to aliased locations even when lexical overlap is false, so their results cannot be added as unique disk usage. Traversal, including skipped or rejected links/reparse points, is not an atomic snapshot and does not prevent path check/use races. Do not use the report to decide deletion. `root_os` and `relative_path_os` preserve OS path units for paths accepted by the current CLI string argument interface; the CLI does not accept arbitrary non-UTF-8 root arguments.

The Nagi 0.1.11 target stores an executable for each build generation under the generated directory's `.nagi/`. The same entry source and output directory keep their application ID, while rebuilding changes the executable path. Builds do not overwrite, delete, or stop an older executable. A failed build does not run its new executable and leaves the previous successful metadata in place.

The published `nagic` 0.1.11 CLI does not automatically delete successful generations or enforce a retention limit. These files are separate from the shared Cargo cache in `build/native-target/` or `NAGI_NATIVE_TARGET_DIR`. There is no generation cleanup command for ordinary CLI use. JetBrains plugin 0.1.3 provides a retention mode that passes `NAGI_RUN_RETENTION=latest` only to plugin-managed runs using compiler 0.2.0. Published compiler 0.1.11 does not implement this mode. The temporary compiler 0.1.12 candidate was cancelled; this feature requires JetBrains plugin 0.1.3 and compiler 0.2.0.

In IDE-managed mode, a `latest` generation is protected only after its native run has a success record; compile success alone is not runtime success, and old successful runs are retained while the latest run has no success record. Active generations, generations referenced by input snapshots, and the last-good generation after a failed run are also protected. Every successful generation, including ordinary builds, records canonical namespace references and input file identities. This protects generations referenced by another app or a non-latest active run. If an external hard-link input shares the identity of a run lease, that lease and generation are preserved too. A dependency may remain until one additional successful sweep after its owner is reclaimed, and there is no strict total generation or disk-capacity limit. Missing input metadata on a managed ready generation is treated as unknown and stops safe reclamation. Unknown files, links/reparse points, failed staging, and the shared Cargo target cache are left alone. A recovery journal outside the generation tree is checked by a later plugin-managed run; unknown or oversized journal records are preserved while recovery continues for valid records. Do not manually delete a generation that is running or referenced by `latest`. The retention limits above still apply.

Writes to the same generated directory are serialized by an OS lock. Contention prints `waiting for output lock:` and waits until generation, Cargo, and successful publication finish. The application runs after releasing the lock. Writing `check`, `lower`, and cost reports use the same lock; an editor check that writes no files does not need it.

Explicit Rust path dependencies also protect their containing generation. A generation used as input from another output directory is retained like an ordinary CLI artifact and is not automatically reclaimed. A run journal with remaining hardlinks is also retained. Nagi does not discover every arbitrary Rust include or build-script dependency.

`NAGI_NATIVE_TARGET_DIR` shares dependency builds across applications. The default cache also distinguishes applications. Nagi 0.1.10 added identifiers only with this environment variable; both paths now follow the same rule. Scripts that looked up binaries using the cache or Cargo package name should use `native:`. Identifiers and internal paths are not fixed across compiler versions. Choose distribution filenames when copying executables.

The ordinary generated Low, Rust, and Cargo.toml remain available for inspection and direct Cargo use. A failed build may update these files; they are separate from the successful generation record. This does not freeze concurrent edits to handwritten Rust or path dependencies.

## Use with VS Code

Since [extension](vscode-extension.md) 0.1.1, the nearest nagi.toml is found by searching upward from an open Nagi/Low file. Checks, lowering, builds, and runs use entry even when a helper is open. Imported errors appear in their original files.

Automatic checks run on opening and saving files, reading unsaved edits to previously saved sources in memory. Checks wait while `nagi.toml` is unsaved; saving it rechecks open Nagi files. Manual type checks do not save sources. Lower, build, and run save edited files first. Files not imported from `entry` are outside the project's check scope.

Configure the compiler location through `nagi.compilerPath` or PATH. Existing nagi.rustFile, nagi.rustDependencies, and nagi.nativeFiles settings are passed as command-line arguments using the precedence above. Values in `nagi.rustDependencies` are version strings only; write dependency tables in `nagi.toml`. Project-specific nagi.toml settings keep the terminal and VS Code consistent.

Extension 0.1.5 and later, with the latest nagic, support F12 for project functions, classes, imports, local variables, arguments, for names, and case names. Queries read High/Low loaded from entry, including open unsaved edits in memory. If syntax or imports cannot be read, F12 does not navigate to stale saved positions. Save new files and nagi.toml edits first.

Declaration/local type hovers, function/class/type/field completion, and argument hints also read unsaved edits to previously saved files, including imports and native Low. Unparseable edits fall back to declarations marked “保存済み” (saved), without local types or field candidates. See the [walkthrough](editor.md) and [extension settings](vscode-extension.md).

## symbols for editor integrations

`nagic symbols --project my-app` returns JSON on stdout with references and definition locations for loaded functions, classes, and local names. It does not build or write generated files. If syntax and imports can be read, definition positions remain available even with type errors.

The JSON includes function parameters, return types, async status, and class fields. Confirmed variable types are in `locals`; expression types, ranges, and matching fields are in `expressions`. Positions use one-based lines and UTF-16 columns in the original files. Uncertain types are omitted. This is editor information and does not establish a successful check.

`symbols --editor-input` accepts `{"files":[{"file":"main.nagi","text":"..."}]}` on stdin to supply edits to existing Nagi/Low files. Relative paths use the terminal's working directory. Sources are replaced in memory without writing to disk. The option is available for `symbols` and `check`; configuration files use saved contents. `check --editor-input` does not write generated files.

## Shortening development and CI feedback

See [CI configuration generation](user-ci.md) in the 0.2.1 distribution and [development profiles and build observations](development-builds.md).
