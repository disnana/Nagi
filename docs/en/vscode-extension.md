# Nagi for VS Code

Edit, check, and run Nagi High (`.nagi`) and Low (`.low`). Command labels and some messages are currently Japanese.

This page describes extension 0.1.13, which includes fixes to execution preparation and standard API source display. See the [Changelog](../../CHANGELOG.md) for versioned changes.

| Feature | Compiler required |
| --- | --- |
| Highlighting, snippets, bracket completion, indentation, Low folding | No |
| Keyword, type, and built-in completion, hover, and parameter hints | No |
| Project type hovers, field completion, and definition navigation | Yes |
| Checking, lowering, building, and running | Yes |

## Installation

Install from the [Marketplace](https://marketplace.visualstudio.com/items?itemName=Disnana.nagi-lang).

```sh
code --install-extension Disnana.nagi-lang
```

Disable or remove the old `nagi-local.nagi-language` or `Disnana.nagi-language` extensions. The current public asset verified in GitHub Releases is `nagi-language-0.1.13.vsix`, with a matching `.sha256` file. In VS Code, choose **Extensions: Install from VSIX** and select the downloaded file.

The extension does not include the compiler. [Install Nagi](getting-started.md), then restart VS Code. Type checking needs `nagic`; building and running also need Rust/Cargo and your OS build tools.

`nagi.compilerPath` takes precedence. When empty, the extension looks for repository builds in `target/release`, then `target/debug`, then PATH. `spawn nagic.exe ENOENT` means the executable was not found. Check the setting and **Nagi** in Output. Startup failures are not displayed as source type errors.

## Indentation while typing

Enter after `def main():`, `async def`, `if`, `match`, or `case` indents one level. Typing the final colon of `else:` or `case ...:` aligns it with the corresponding block.

Newlines inside brackets and closing delimiters are also adjusted. Low includes braces. Delimiters in strings and comments are ignored. The default is four spaces; width and tabs/spaces follow the editor settings.

Disable these adjustments with:

```json
{
  "[nagi]": { "editor.formatOnType": false },
  "[nagi-low]": { "editor.formatOnType": false }
}
```

Whole-file formatting is not supported.

## Handwritten Low

Low is an alternative brace-based syntax. In `.low` files, hovers and completion use `fn` and `record`, and brace-delimited blocks can be folded.

Try Low imports, records, and enums in the [order quote CLI](https://github.com/disnana/Nagi/tree/main/test-nagi-code/low-examples/order-quote). For replacing a High function, see [High and Low](low-language.md).

## Settings

Enable automatic compiler discovery with:

```json
{
  "nagi.compilerPath": "",
  "nagi.checkOnSave": true,
  "nagi.checkTimeoutMs": 15000
}
```

Relative `compilerPath` values resolve from the workspace folder. `nagi.nativeFiles`, `nagi.rustFile`, and `nagi.rustDependencies` can add CLI arguments. Use `nagi.toml` below for application settings.

Automatic checks run on opening and saving a file. Editing clears stale diagnostics without checking on every keystroke. **Nagi: 型検査** (Type Check) reads unsaved edits to previously saved files without saving or building. Save new files and `nagi.toml` first.

Untrusted workspaces do not run the compiler. Highlighting, indentation, and built-in assistance remain available.

## Projects

Set the entry file, Rust dependencies, and native Low in [nagi.toml](projects.md). The extension searches upward from the open file for the nearest manifest. Even from a helper file, it checks and runs `entry`. Without a manifest, it processes the open file alone.

Use the top-right run button or **Nagi: 実行** (Run) in the Command Palette. Lower, build, and run first save edited project files and loaded imports. In extension 0.1.13, preparation is canceled if saving fails or sources or settings change during preparation.

Diagnostics appear in Problems, compiler output in **Nagi** under Output, and build/run output in the terminal. Read Rust and dependency diagnostics in the terminal. A successful Nagi check does not mean Rust's checks will succeed.

## Go to Definition

F12 navigates to functions, classes, enums, enum variants, local bindings, and imports. Qualified module names and from aliases are supported. Reassigned variables navigate to their first binding.

Queries read unsaved edits to previously saved High/Low files and open imports. If unreadable syntax or imports force a fallback to saved declarations, navigation to stale positions is disabled.

Standard API names open a read-only reference supplied by the compiler. Class fields, ordinary built-ins, and Rust implementations do not support definition navigation.

## Hovers, completion, and parameter hints

| Input or action | Result |
| --- | --- |
| Hover a variable | Its compiler-confirmed type |
| `item.` | Class fields and their types |
| `orders.` | Definitions declared by the imported module |
| `AuthError.` | Enum variants |
| `http.Status.` or `http.Method.` | Standard HTTP constants |
| Complete a function or class | Argument placeholders; named fields for classes |
| Type `(` or `,` | Parameter hints |

Types, functions, and constants from `std.http.server` and `std.actor` are supported. Identically named types are distinguished by their imports. Extract nullable values with `Some`/`None`, and Results with `Ok`/`Err`.

Unknown types and moved values do not receive guessed field candidates. Unparseable edits fall back to declarations marked “保存済み” (saved), without local types, fields, or F12. Completion is suppressed in comments and strings. See the [walkthrough](editor.md).

Find References, rename, debugging, and Rust implementation analysis are not supported.

## Development

Build the compiler at the repository root, then run:

```sh
node --test editors/vscode-nagi/test/*.test.js
python editors/vscode-nagi/scripts/package_vsix.py
```

Node tests cover assistance logic and `nagic symbols` integration. Test VS Code behavior separately in an Extension Development Host: set `--extensionDevelopmentPath` to the extension folder, `--extensionTestsPath` to `test/host.js`, and the workspace to the repository root.

Use `test/indentation-host.js` for indentation or `test/static-assistance-host.js` for compiler-free assistance. Passing Node tests in ordinary CI alone does not verify input and display behavior inside VS Code.
