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

It calls CRC32, JSON formatting, and an async Rust function. The [task management demo](web-demo.md) also has an entry configured.

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

Project-generated code goes to `build/<entry filename without extension>/`, and executables to `build/native-target/release/`. For `main.nagi`, the executable is `nagi-main.exe` on Windows or `nagi-main` on Linux. Separate build folders allow applications to reuse the same entry filename.

Set `NAGI_NATIVE_TARGET_DIR` to share dependency builds across applications. On main, only with this override, executable names include an identifier for the source and generated output directory, such as `nagi-main-0123456789abcdef.exe`. This collision fix is not included in the published 0.1.9 compiler. The `native:` line that `build` and `run` print to stderr gives the actual path. Concurrent compilation into the same generated output directory is not supported.

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
