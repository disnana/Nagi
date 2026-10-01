# Setup and your first run

[Contents](README.md) → Setup and first run → [Learn by writing code](language-guide.md)

This page helps you prepare the Nagi compiler and run a program in one file. Run the commands from **the root of this repository**.

A compiler turns the code you write into an executable. Enter the commands below in a terminal: PowerShell on Windows, or a terminal application on Linux or macOS. The repository root means the Nagi folder you downloaded.

Download and extract the Nagi archive for your OS from [GitHub Releases](https://github.com/disnana/Nagi/releases).

| Your system | File to download |
|---|---|
| Windows x64 | `nagi-0.1.4-windows-x86_64.zip` |
| Linux x86_64 | `nagi-0.1.4-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `nagi-0.1.4-macos-arm64.tar.gz` |
| macOS Intel | `nagi-0.1.4-macos-x86_64.tar.gz` |

These archives include the compiler and source. Keep the folder structure, including `runtime/`. GitHub's **Source code** downloads contain only source. The VS Code extension is a separate `nagi-language-0.1.8.vsix` file.

To build the compiler from source instead, install [Git](https://git-scm.com/) and run:

```bash
git clone https://github.com/disnana/Nagi.git
cd Nagi
```

You can also use **Code → Download ZIP** on GitHub. Extract the ZIP and open that folder in your terminal.

## 1. Prepare the compiler

You need [Rust/Cargo](https://www.rust-lang.org/tools/install) and a C build environment. Rust builds the compiler and the generated code. Cargo manages Rust builds and dependencies. The bundled SQLite C code also needs a C compiler. The first build downloads dependencies through Cargo.

On Windows, use the Rust MSVC toolchain and the C++ build tools from Visual Studio Build Tools. On Linux, install a C compiler. WSL2 follows the Linux instructions. On macOS, install Command Line Tools with `xcode-select --install`. The macOS archives are built and run in CI on macOS 15.

With a compiler archive, skip the `cargo build` command below. Rust/Cargo and a C build environment are still needed to build your Nagi applications.

```powershell
# Windows / PowerShell
cargo build --release --locked -p nagic
.\target\release\nagic.exe run examples/hello.nagi
```

```bash
# Linux / WSL2 / macOS
cargo build --release --locked -p nagic
./target/release/nagic run examples/hello.nagi
```

After the build logs, the program prints:

```text
Hello, Nagi!
4
```

## 2. Write your own file

Create `hello.nagi` in the repository root and save this complete program:

```nagi
def main():
    print("Hello, Nagi!")
    count = 3
    print(count * 2)
```

`def main():` is the entry point. Indent its body with four spaces. `print` displays one value followed by a newline.

```powershell
.\target\release\nagic.exe run hello.nagi
```

On Linux/WSL2, replace `.\target\release\nagic.exe` in the following commands with `./target/release/nagic`.

```text
Hello, Nagi!
6
```

The integer in `count = 3` has type `i64`. Save the file with a `.nagi` extension and run it with Nagi.

## 3. Choose between checking and building

| Command | What it does | When to use it |
|---|---|---|
| `check hello.nagi` | Checks Nagi syntax, types, and ownership | Find errors in saved code |
| `lower hello.nagi` | Checks and writes Low | Read the translated code |
| `build hello.nagi` | Generates an executable, including Rust backend checks | Build without running |
| `run hello.nagi` | Builds and runs | Try your program |

Currently, both `check` and `lower` save the Low generated from High. A successful `check` can still be followed by a failed `build` if Rust's type or borrow checks reject the generated program.

Build errors first show the original Nagi or Low filename, the line of the corresponding statement or definition, and its source text. This includes imports and handwritten Low used with `@replace`. The following `Rust backend details` preserves the full diagnostic for the generated Rust. Errors in handwritten Rust, or errors without an identifiable source location, retain Rust's diagnostics.

```powershell
.\target\release\nagic.exe check hello.nagi
.\target\release\nagic.exe build hello.nagi
.\native-target\release\nagi-hello.exe
```

Default output locations:

| File | Contents |
|---|---|
| `build/hello/generated.low` | Low translated from High |
| `build/hello/src/main.rs` | Generated Rust during build/run |
| `build/hello/Cargo.toml` | Generated Rust project during build/run |
| `native-target/release/nagi-hello.exe` | Windows executable |
| `native-target/release/nagi-hello` | Linux executable |

Generated files go to `build/<source filename without its extension>/`. Use `--out build/my-hello` to choose another location. Set `NAGI_NATIVE_TARGET_DIR` to change the executable build location.

Compilation also needs this repository's `runtime/`. If you copy `nagic.exe` elsewhere on its own, set `NAGI_ROOT` to the Nagi repository location. The [task management demo](web-demo.md) shows how to distribute a generated application executable.

For multiple-file applications and Rust dependencies, use [project configuration](projects.md). From a configured folder, `nagic run` and VS Code use the same entry file.

## 4. Use VS Code

Install the [Nagi extension](vscode-extension.md), then open this repository folder in VS Code. Saving a `.nagi` file runs a check and shows errors in Problems. The top-right run button and Nagi commands in the Command Palette can run or build it. Hover names to see types and type `value.` for field completion. F12 navigates to functions, classes, imports, and local bindings, including unsaved edits to files saved at least once. See the [editor walkthrough](editor.md).

## When something goes wrong

| Symptom | What to check |
|---|---|
| `cargo` is not found | Install Rust/Cargo and reopen your terminal |
| `nagic.exe` is not found | Run the compiler build from the repository root |
| VS Code reports `spawn nagic.exe ENOENT` or a missing compiler | The VSIX does not include the compiler. Build it in step 1, then run **Nagi: 型検査** (Type Check). For a compiler elsewhere, set `nagi.compilerPath` to its executable |
| The C compiler or linker is not found | Check your Windows C++ build tools or Linux C compiler |
| Tabs or indentation errors | Use four spaces consistently |
| A check succeeds but a build fails | Start with the Nagi or Low filename and line in the terminal; the following Rust diagnostic gives further details |
| A port is already in use | Stop the earlier server before restarting |
| Rebuilding a running exe fails on Windows | Stop the executable and rebuild |
| A server does not exit | `serve` keeps waiting for requests; press Ctrl+C in its terminal |

Continue with [Learn by writing code](language-guide.md), from variables through APIs.
