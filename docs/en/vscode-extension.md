# Nagi Language for VS Code

Development tools for Nagi High (`.nagi`) and Low (`.low`). Command labels and some extension messages are currently Japanese.

- Syntax highlighting, comments, bracket/quote handling, and four-space indentation
- Indentation on Enter, else/case alignment, and closing delimiters in multiline expressions
- Snippets for classes, functions, HTTP, borrowing, and Low replacements
- nagic check on opening/saving files, with diagnostics in Problems
- Command Palette actions for checking, lowering, building, and running
- A run button at the top right of the editor
- Project checking/execution using nagi.toml entry, Rust dependencies, and native Low
- F12/Go to Definition for functions, classes, imports, and local names
- Hovers for parameters, return types, async status, and class fields
- Type hovers for arguments, local variables, and case bindings
- Class field completion after value.
- Completion for project functions/classes/types and common built-ins
- Parameter hints and named arguments for class construction

## Installation

Search for "Nagi for VS Code" in VS Code's Extensions view, or install it from the [Marketplace](https://marketplace.visualstudio.com/items?itemName=Disnana.nagi-lang). From a terminal, run:

```bash
code --install-extension Disnana.nagi-lang
```

The extension ID is `Disnana.nagi-lang`. If you installed `nagi-local.nagi-language` or `Disnana.nagi-language`, disable or uninstall it first.

Formal VSIX downloads are published on [GitHub Releases](https://github.com/disnana/Nagi/releases) as nagi-language-VERSION.vsix. Updating the extension version on main publishes a release after CI succeeds.

From the repository root, run `python editors/vscode-nagi/scripts/package_vsix.py` to create `build/distribution/nagi-language-0.1.8.vsix`. Select it using **Extensions: Install from VSIX**, or run:

```powershell
code --install-extension build/distribution/nagi-language-0.1.8.vsix
```

Highlighting, snippets, and indentation support work without a compiler. Checks and execution require `nagic`. Install it using [Setup and first run](https://disnana.github.io/Nagi/en/docs/getting-started/). The extension checks the repository's release build, then debug build, then PATH.

The VSIX does not include the compiler. `spawn nagic.exe ENOENT` means it could not be found. Install the compiler, restart VS Code, then run **Nagi: 型検査** (Type Check). For a compiler elsewhere, set nagi.compilerPath. Startup failures and timeouts appear as warnings and in Nagi Output, without source-error squiggles.

## Indentation while typing

These improvements are planned for the next extension release. To try them earlier, build a VSIX from main.

Press Enter after a block header such as `def main():`, `async def`, `if`, `match`, or `case` to indent one level. Typing the final colon of `else:` or `case ...:` aligns the line with its enclosing `if` or `match`.

Enter inside parentheses or square brackets indents one level. A closing delimiter at the start of a line aligns with the line containing its opening delimiter. Low also handles braces. Characters inside strings and comments do not affect indentation. This works in new untitled files and untrusted workspaces without running the compiler.

The default is four spaces. The indentation width and tabs/spaces choice follow the editor's settings in the status bar. Disable **Editor: Format On Type** to stop this adjustment, or disable it for Nagi alone:

```json
{
  "[nagi]": { "editor.formatOnType": false },
  "[nagi-low]": { "editor.formatOnType": false }
}
```

## Settings

```json
{
  "nagi.compilerPath": "target/release/nagic.exe",
  "nagi.checkOnSave": true,
  "nagi.nativeFiles": [],
  "nagi.rustFile": "",
  "nagi.rustDependencies": [],
  "nagi.checkTimeoutMs": 15000
}
```

Outside Windows, the executable is nagic. Relative compilerPath/nativeFiles/rustFile paths use the workspace folder. Check-generated Low goes to project/source-specific build/vscode-nagi folders, separate from ordinary build output.

Automatic checks wait for saved code. Editing clears stale diagnostics; saving rechecks. Manual commands save edited files first. Untrusted workspaces do not run the compiler. Diagnostic locations currently follow the compiler's line-based output.

## Projects

Use [nagi.toml](projects.md) for entry, Rust dependencies, and native Low. The extension finds the nearest manifest above the open file and passes it with --project. Even with a helper open, entry is checked/run. Automatic checks wait for unsaved project sources; manual commands save that project's files. Saving, creating, or deleting configuration also refreshes checks.

Open test-nagi-code/rust-bridge/bridge.nagi to use its existing settings. Earlier nagi.rustFile and related settings become command arguments, following the CLI's override/addition rules. nagic check does not inspect Rust implementations; builds verify matching types. Imported errors appear in their own files' Problems.

Without a manifest, the open file is processed independently. Project support requires the latest compiler from this repository.

## Go to Definition

Press F12 on a function call, class annotation/construction, or variable name. On `import "models.nagi"`, it opens the file's beginning. High and Low are supported, covering files loaded from the project entry and native Low.

For example, read_item in test-nagi-code/result-api/server.nagi navigates to storage.nagi; Item navigates to models.nagi. Locations come from nagic symbols. Navigation remains available with type errors when syntax/imports can be read.

Since 0.1.5, this also covers arguments, assigned variables, for elements, and case bindings. Reassignment targets the first binding. For bindings reusing an outer name, navigation uses the loop definition inside and restores the outer definition afterward. New if/while/scope/case names apply within their blocks.

Files saved at least once support unsaved edits. Open imports and native Low are read in memory, so navigation uses edited positions. Save new files and nagi.toml first. When parsing falls back to saved information, stale position navigation is suppressed. Query results are also discarded if sources/settings change during the request.

Name references resolve separately from type checks, so identifiable bindings can still be found after moves or initializer type errors. Undefined/out-of-scope names do not navigate. Class fields, built-ins, and Rust implementation navigation are unsupported. Calls to replaced Low functions prefer the High declaration; Low locals navigate within Low.

## Hovers, completion, and parameter hints

Use the latest nagic and extension 0.1.8. Function hovers show parameters, returns, and async status; class hovers list fields. Types such as Result[Item?, Error] and view[str] retain their declared forms. See the [walkthrough](editor.md).

Variable hovers show confirmed types: count = 3 gives count: i64, and item from a class-returning call gives item: Item. Arguments, for elements, and Ok/Err bindings are supported at declarations and uses. Names outside their blocks do not receive those types.

Typing item. offers Item fields with types such as name: str; selecting inserts only the field name. Completion after item.na replaces the partial name. Calls, nested fields, and Copy-class list elements are supported. Result[Item, Error] and Item? are not implicitly Item. Extract Result through its Ok case or `(try fetch()).`.

Partial names or Ctrl+Space offer imported functions/classes, native Low functions, and common built-ins. Functions insert positional placeholders; classes insert named fields such as Item(id=..., name=...). Tab moves between placeholders. Type/return positions offer classes, types, Result, List, and view.

Typing ( or , shows parameter hints and selects the current argument. Completion avoids inserting duplicate parentheses. Use the displayed async/Result declaration to decide whether await, try, or match is needed.

Unsaved contents of previously saved Nagi/Low files and open imports/native Low are read in memory. Declarations remain available with type errors when syntax/imports are readable. Local types and fields appear only where established; uncertain, moved, or out-of-scope values get no guesses. Queries neither save nor build. Save nagi.toml edits first.

Unparseable fragments such as add(1, fall back to saved project declarations, marked “保存済み” (saved). Local types and fields are suppressed in that state. Names in comments/strings get no completion or hovers.

Local types and field completion were added in 0.1.4; local/unsaved definition navigation in 0.1.5. Update old compilers because they lack the new symbol information. Find References, rename, debugging, and Rust implementation analysis are not supported.

## Development

Build the compiler at the repository root and run `node --test editors/vscode-nagi/test/*.test.js`. The suite includes text-processing tests and tests using the actual nagic symbols command.

VS Code testing is separate. Launch an Extension Development Host with this extension folder as --extensionDevelopmentPath, test/host.js as --extensionTestsPath, and the repository root as workspace. It activates the real extension and verifies diagnostics, F12, hovers, completion, hints, and project execution. Passing Node tests alone does not verify behavior inside VS Code.
