# Configure applications with nagi.toml

For multiple-file applications, save the entry file and Rust integration arguments in `nagi.toml`. The CLI and VS Code use the same configuration.

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

For editors, `nagic symbols --project my-app` returns JSON on stdout with references and definition locations for loaded functions, classes, and local names. It does not build or write generated files. If syntax and imports can be read, definition positions remain available even with type errors.

The JSON includes function parameters, return types, async status, and class fields. Confirmed variable types are in `locals`; expression types, ranges, and matching fields are in `expressions`. Positions use one-based lines and UTF-16 columns in the original files. Uncertain types are omitted. This is editor information and does not establish a successful check.

`symbols --editor-input` accepts `{"files":[{"file":"main.nagi","text":"..."}]}` on stdin to supply edits to existing Nagi/Low files. Relative paths use the terminal's working directory. Sources are replaced in memory without writing to disk. This option is symbols-only; configuration files still use saved contents.

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
| `rust.dependencies` | Optional map of crate names to Cargo version strings |

All file paths are relative to nagi.toml. Only entry is required. Unknown settings, wrong value types, empty paths, and duplicate dependencies are errors. Rust dependencies currently accept version strings only, without Cargo path/git/features settings. Matching Rust types and implementations is checked by build/run.

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
| `--rust-dep NAME=VERSION` | Overrides the same dependency name or adds a new one |
| `--native FILE.low` | Adds to native |
| `--out DIR` | Changes the generated Low/Rust/Cargo.toml location |
| `--no-project` | Disables automatic search; requires SOURCE |

Command-line relative paths use the terminal's working directory. Duplicate SOURCE, project, rust, out, or same-name rust-dep arguments are errors. project and no-project cannot be combined.

Project-generated code goes to `build/<entry filename without extension>/`, and executables to `build/native-target/release/`. For main.nagi, the executable is nagi-main.exe on Windows or nagi-main on Linux. Separate build folders allow applications to reuse the same entry filename. `NAGI_NATIVE_TARGET_DIR` overrides the executable build directory.

## Use with VS Code

Since [extension](vscode-extension.md) 0.1.1, the nearest nagi.toml is found by searching upward from an open Nagi/Low file. Checks, lowering, builds, and runs use entry even when a helper is open. Imported errors appear in their original files.

Saving nagi.toml rechecks open Nagi files. Automatic checks wait while project sources are unsaved; manual commands save edited sources in that project first. Files not imported from entry are outside the project's check scope.

Configure the compiler location through `nagi.compilerPath` or PATH. Existing nagi.rustFile, nagi.rustDependencies, and nagi.nativeFiles settings are passed as command-line arguments using the precedence above. Project-specific nagi.toml settings keep the terminal and VS Code consistent.

Extension 0.1.5 and later, with the latest nagic, support F12 for project functions, classes, imports, local variables, arguments, for names, and case names. Queries read High/Low loaded from entry, including open unsaved edits in memory. If syntax or imports cannot be read, F12 does not navigate to stale saved positions. Save new files and nagi.toml edits first.

Declaration/local type hovers, function/class/type/field completion, and argument hints also read unsaved edits to previously saved files, including imports and native Low. Unparseable edits fall back to declarations marked “保存済み” (saved), without local types or field candidates. See the [walkthrough](editor.md) and [extension settings](vscode-extension.md).
