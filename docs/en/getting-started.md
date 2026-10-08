# Setup and your first run

[Contents](README.md) · Next: [Learn by writing code](language-guide.md)

Install a published Nagi compiler and run Hello World in High (`.nagi`).

## 1. Install the compiler

Install a prebuilt Nagi compiler. Building your own apps with `nagic build` or `nagic run` also requires [Rust / Cargo](https://www.rust-lang.org/tools/install) and C build tools.

### Use the installer

Run the command for your OS. It downloads the latest published release and verifies SHA-256 before installing. No administrator access is needed.

#### Windows (PowerShell)

```powershell
& ([scriptblock]::Create((Invoke-RestMethod 'https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.ps1')))
```

#### Linux and macOS (bash)

```bash
(set -o pipefail; curl -fsSL https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.sh | bash) && export PATH="$HOME/.local/bin:$PATH"
```

Check the installation in the same terminal. Restart VS Code so it picks up the new PATH.

```text
nagic --version
nagic --help
```

The compiler released on 2026-10-07 reports `nagic 0.1.11`. You can also use `nagic -V` or `nagic version`. Verify the installed compiler with `nagic --version`. Version and help work without Rust or project configuration.

### Install an editor extension

Get the VS Code extension from the [Marketplace](https://marketplace.visualstudio.com/items?itemName=Disnana.nagi-lang) or download `nagi-language-0.1.13.vsix` and its `.sha256` from [GitHub Releases](https://github.com/disnana/Nagi/releases). In VS Code, use **Extensions: Install from VSIX** and select the file.

The official destination for **Nagi for JetBrains** for IntelliJ IDEA and PyCharm is its [JetBrains Marketplace page](https://plugins.jetbrains.com/plugin/34891-nagi), which is currently under review. Until approval, download the ZIP from [GitHub Releases](https://github.com/disnana/Nagi/releases). The currently published 0.1.1 release has `nagi-jetbrains-IC-0.1.1.zip` for IDEA and `nagi-jetbrains-PC-0.1.1.zip` for PyCharm. In the IDE, choose **Settings → Plugins → ⚙ → Install Plugin from Disk**, select the ZIP, then restart. Neither editor extension includes the compiler.

### Tools for building applications

Along with Rust / Cargo, use the tools below. Your existing installation is fine.

- **Windows:** Rust's MSVC toolchain and Visual Studio C++ Build Tools.
- **Linux and WSL2:** a C compiler.
- **macOS:** Command Line Tools, installed with `xcode-select --install`.

The installer does not include build tools or the VS Code extension. The bundled SQLite C code is compiled when building your app. macOS distributions are verified on macOS 15 in CI.

## 2. Write your own file

Create `hello.nagi` in your own working folder and save this program:

```nagi
def main():
    print("Hello, Nagi!")
    count = 3
    print(count * 2)
```

`def main():` is the entry point. Indent its body with four spaces. `print` displays one value followed by a newline.

```powershell
nagic run hello.nagi
```

```text
Hello, Nagi!
6
```

The integer in `count = 3` has type `i64`. Save the file with a `.nagi` extension and run it with Nagi.

If the filename contains spaces, quote the path, as in `nagic run "hello world.nagi"`.

## 3. Choose between checking and building

| Command | What it does | When to use it |
|---|---|---|
| `check hello.nagi` | Checks Nagi syntax, types, and ownership | Find errors in saved code |
| `lower hello.nagi` | Checks and writes Low | Read the translated code |
| `build hello.nagi` | Generates an executable, including Rust backend checks | Build without running |
| `run hello.nagi` | Builds and runs | Try your program |

`run` leaves stdout for your application. Compiler progress and diagnostics go to stderr, so you can pipe a CLI's JSON output directly to another program.

Currently, both `check` and `lower` save the Low generated from High. A successful `check` can still be followed by a failed `build` if Rust's type or borrow checks reject the generated program.

When a source location can be identified, build errors show the corresponding Nagi or Low filename and line. Nagi 0.1.11 adds `--rust-diagnostics` to include generated Rust details. Errors in handwritten Rust, or errors without an identifiable source location, retain Rust's diagnostics in the normal output.

```powershell
nagic check hello.nagi
nagic build hello.nagi
```

Run the path shown in the `native:` line using PowerShell's `&` operator. To build and run together, use `nagic run hello.nagi`.

Default output locations:

| File | Contents |
|---|---|
| `build/hello/generated.low` | Low translated from High |
| `build/hello/src/main.rs` | Generated Rust during build/run |
| `build/hello/Cargo.toml` | Generated Rust project during build/run |
| `build/hello/.nagi/` | Executables and generated snapshots for each successful build generation in Nagi 0.1.11 |
| `native-target/` | Shared build cache |

Generated files go to `build/<source filename without its extension>/`. Use `--out build/my-hello` to choose another location. `NAGI_NATIVE_TARGET_DIR` changes the build cache location. Rebuilding changes the executable path; use the reported `native:` path. See [build generations and shared caches](projects.md).

Compilation also needs the bundled `runtime/`. If you copy `nagic.exe` elsewhere on its own, set `NAGI_ROOT` to the extracted folder containing `runtime/`. The [task management demo](web-demo.md) shows how to distribute a generated application executable.

For multiple-file applications and Rust dependencies, use [project configuration](projects.md). From a configured folder, `nagic run` and VS Code use the same entry file.

## 4. Use VS Code

Install the [Nagi extension](vscode-extension.md), then open your project folder in VS Code. Saving a `.nagi` file runs a check and shows errors in Problems. The top-right run button and Nagi commands in the Command Palette can run or build it. Hover names to see types and type `value.` for field completion. F12 navigates to functions, classes, imports, and local bindings, including unsaved edits to files saved at least once. See the [editor walkthrough](editor.md).

## Updates and other installation methods

### Update

Run the installation command above again. It checks the latest published release, downloads and verifies it, then switches the command. It never treats a main build or the VSIX as a release. Repeating an install of the same version does not add another copy.

Windows installs under `%LOCALAPPDATA%\Nagi\versions` and puts its `current` subdirectory on PATH. Linux/macOS use `~/.local/share/nagi`, with the command in `~/.local/bin/nagic`; the installer also adds PATH to bash/zsh configuration. These command locations stay the same across updates.

Stop Nagi builds before updating. After the new command starts successfully, the installer compares older distributions with their published archives and removes unchanged copies. Only the selected version remains; no rollback copy is kept permanently. Added or modified files are preserved. Older copies that cannot be verified or removed, including files locked by Windows, are also kept and their folder is reported. A failed update preserves the previous command and PATH.

Old URLs such as `nagi-v0.1.6/scripts/install.ps1` are pinned to 0.1.6. Use this page's `main/scripts/install.ps1` to update. On Windows, version-specific PATH entries from the original installer are replaced with the fixed `current` entry. If VS Code's `nagi.compilerPath` points to an old version, clear it to enable automatic discovery or set the new executable's absolute path, then restart VS Code.

To select a specific version, use the commands below. These examples select the published Nagi 0.1.11. The same one-version retention policy applies.

```powershell
& ([scriptblock]::Create((Invoke-RestMethod 'https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.ps1'))) -Version 0.1.11
```

```bash
(set -o pipefail; curl -fsSL https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.sh | bash -s -- --version 0.1.11) && export PATH="$HOME/.local/bin:$PATH"
```

If you used custom locations, pass the same `-InstallDir` (PowerShell) or `--prefix` and `--bin-dir` (bash) when updating. `-NoPath` / `--no-path` leave persistent PATH settings unchanged; register the fixed command location yourself in that case. Manually extracted distributions elsewhere are not removed.

### Uninstall

Remove the Nagi distributions installed by the installer, the command link, and the PATH settings added by the installer. On Windows, run:

```powershell
& ([scriptblock]::Create((Invoke-RestMethod 'https://raw.githubusercontent.com/disnana/Nagi/main/scripts/uninstall.ps1')))
```

On Linux and macOS, run:

```bash
(set -o pipefail; curl -fsSL https://raw.githubusercontent.com/disnana/Nagi/main/scripts/uninstall.sh | bash)
```

To preview removals, append `-WhatIf` to the PowerShell command, or replace `bash` with `bash -s -- --dry-run` in the bash command. If you used custom locations, pass the same `-InstallDir` or `--prefix` and `--bin-dir` as during installation. Also pass the same `--profile` if you specified a custom shell profile. `-NoPath` / `--no-path` skip persistent PATH cleanup.

Distributions are compared with their published archives before removal. Directories with added or modified files, and distributions that cannot be verified, remain intact; their locations are reported. Your projects and existing Rust/Cargo, C build tools, and VS Code extension are preserved. Restart your terminal and VS Code afterward.

### Extract the archive yourself

Download the file for your OS from [GitHub Releases](https://github.com/disnana/Nagi/releases). The names below are the published Nagi 0.1.11 filenames.

| Your system | File to download |
|---|---|
| Windows x64 | `nagi-0.1.11-windows-x86_64.zip` |
| Linux x86_64 | `nagi-0.1.11-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `nagi-0.1.11-macos-arm64.tar.gz` |
| macOS Intel | `nagi-0.1.11-macos-x86_64.tar.gz` |

Extract the whole archive and keep `runtime/` beside `nagic` or `nagic.exe`. Add **the extracted folder itself** to PATH to run `nagic` from any directory. `NAGI_ROOT` is normally unnecessary. GitHub's “Source code” downloads do not contain a prebuilt compiler.

### Build the compiler from source

Clone with [Git](https://git-scm.com/). You can also use GitHub's “Code → Download ZIP” and run the build in the extracted folder.

```bash
git clone https://github.com/disnana/Nagi.git
cd Nagi
cargo build --release --locked -p nagic
```

The compiler is `target/release/nagic` (`nagic.exe` on Windows). Add `target/release` to PATH, or replace `nagic` below with its executable path.

## When something goes wrong

| Symptom | What to check |
|---|---|
| Cargo is not found (`Cargoが見つかりません`) | Run `cargo --version`. Install Rust/Cargo, or check PATH if already installed; reopen your terminal and VS Code after changing PATH |
| Cargo cannot start (`Cargoを起動できません`) | Check the OS error shown, Cargo's execution permissions, and the executable file |
| `nagic.exe` is not found | Add the extracted folder to PATH and reopen your terminal |
| `runtime/Cargo.toml` is missing under `NAGI_ROOT` | Set `NAGI_ROOT` to the extracted folder containing `runtime/`, or unset it to use automatic discovery |
| VS Code reports `spawn nagic.exe ENOENT` or a missing compiler | The VSIX does not include the compiler. Install Nagi and restart VS Code, then run **Nagi: 型検査** (Type Check). For a compiler elsewhere, set `nagi.compilerPath` to its executable |
| The C compiler or linker is not found | Check your Windows C++ build tools or Linux C compiler |
| Tabs or indentation errors | Use four spaces consistently |
| A check succeeds but a build fails | Start with the Nagi or Low filename and line in the terminal; the following Rust diagnostic gives further details |
| `Build failed` / `Rust backend rejected program` | Read the diagnostics above it. Source errors point to Nagi/Low lines; dependency downloads or build tools can also cause a failure |
| A port is already in use | Stop the earlier server before restarting |
| Rebuilding a running exe fails on Windows | Stop the executable and rebuild |
| A server does not exit | `serve` keeps waiting for requests; press Ctrl+C in its terminal |

Continue with [Learn by writing code](language-guide.md) and [Your first small CLI app](first-app.md) to try values, input, failures, and a boundary check.
