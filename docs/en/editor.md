# Types and completion in VS Code

[Contents](README.md) · [Setup](getting-started.md) · [Extension installation and settings](vscode-extension.md)

Install the VS Code extension from the Marketplace or a VSIX on GitHub Releases. The official destination for the [Nagi for JetBrains](https://github.com/disnana/Nagi/blob/main/editors/jetbrains-nagi/README.en.md) plugin for IntelliJ IDEA and PyCharm is its [JetBrains Marketplace page](https://plugins.jetbrains.com/plugin/34891-nagi), currently under review. Until approval, install it manually from a ZIP on [GitHub Releases](https://github.com/disnana/Nagi/releases). See the [setup guide](getting-started.md) for the current asset names.

Use Nagi extension 0.1.13 and `nagic` 0.1.11 after its availability is confirmed in the official 0.1.11 release record. This guide targets the 0.1.11 release. Open the repository in VS Code, or save the example below in your own folder.

## Indentation support

Extension 0.1.12 and later support indentation after headers such as `def main():`, else/case alignment, and closing delimiter alignment in multiline expressions.

Typing the final colon of `else:` or `case ...:` aligns it with its enclosing `if` or `match`. The default is four spaces; the editor's indentation settings are respected. This runs without the compiler, including in new untitled files. See [indentation while typing](vscode-extension.md#indentation-while-typing) for behavior and settings.

## Open the example

Open [examples/tutorial/editor_types.nagi](../../examples/tutorial/editor_types.nagi), which contains this complete program:

```nagi
class Count:
    value: i64

def parse_count(text: str) -> Result[Count, Error]:
    number = try parse_i64(text)
    return ok(Count(value=number))

def show(count: Count):
    print(count.value)

def main():
    result = parse_count("42")
    match result:
        case Ok(count):
            show(count)
        case Err(problem):
            print(error_message(problem))
```

Use the top-right run button or **Nagi: 実行** (Run) in the Command Palette to print 42. Execution saves the file first. Extension command labels are currently Japanese.

## Inspect variable types

Hover these names. The compiler's inferred types appear even without explicit annotations.

| Name and location | Displayed type | Why |
|---|---|---|
| text in parse_count | `text: str` | Parameter annotation |
| number | `number: i64` | try extracts parse_i64's success value |
| count in show | `count: Count` | Parameter annotation |
| result in main | `result: Result[Count, Error]` | parse_count's return type |
| count in the Ok case | `count: Count` | Result success payload |
| problem in the Err case | `problem: Error` | Result failure payload |

Case names exist only in that case. Typing count after the match does not show the Ok binding's type. New variables inside if/while/for/scope blocks also stop being available outside the block. Unannotated integers normally infer i64 and floats f64. See [types and inference](types.md).

## Complete fields

In show's `print(count.value)`, remove value to get `print(count.)`. If candidates do not appear, press Ctrl+Space. Selecting `value: i64` restores count.value. You can also complete after typing count.va.

Calls returning classes (`make().`), nested fields (`container.item.`), and Copy-class list elements (`items[0].`) offer fields for the expression's type. Selecting a field inserts only its name.

Result has type `Result[Count, Error]`, so `result.` does not offer Count fields. Extract Count through match as above. In a Result-returning function, use `count = try parse_count("42")` or `(try parse_count("42")).value`. Fields on awaited values similarly use `(await fetch()).field`.

## Call functions and find definitions

Type part of a name or press Ctrl+Space for project functions, classes, and built-ins. Function completion inserts argument placeholders; Tab moves between them. Class completion inserts named arguments such as `Count(value=...)`. Typing `(` or `,` shows parameter order and types.

Press F12 on parse_count or Count to reach its definition. On an import string, F12 opens the file. Previously saved files support unsaved edits and changes in open imports.

Try F12 on these local names:

| Name under the cursor | Destination |
|---|---|
| text in `parse_i64(text)` | parse_count parameter `text: str` |
| number in `Count(value=number)` | `number = try parse_i64(text)` |
| count in `print(count.value)` | show parameter `count: Count` |
| result in `match result` | `result = parse_count("42")` |
| count in `show(count)` | `case Ok(count)` |
| problem in `error_message(problem)` | `case Err(problem)` |

Reassigned names navigate to their first binding. A for binding reusing an outer name targets the loop binding inside the loop and the original afterward. New if/while/scope/case bindings apply only in their blocks. Navigation can work after moves or initializer type errors if a binding is identifiable. F12 for class field names and built-ins is not supported.

### Try module candidates and definitions

Open `module_imports.nagi` from the [module example](../../test-nagi-code/library-examples/module-imports/README.en.md). Press Ctrl+Space after `orders.` to see `Order` and `total`, defined in `orders.nagi` itself. After `current.`, the candidates are class fields such as `amount`. The compiler's name resolution and type information distinguish module definitions from class fields.

Hover and parameter hints on `orders.total` show its function declaration; hovering `SavedOrder` shows the original class's fields. Press F12 on `total` in `orders.total` or the from alias `SavedOrder` to reach the original definition in `orders.nagi`. Open and edit `orders.nagi` without saving to query the edited buffer. If a local shadows a module name, assistance follows that scope's resolution.

## When editor information is unavailable

Previously saved Nagi/Low files are analyzed in memory with their unsaved edits, including open imports and native Low. Queries do not save or build sources. Save new files and nagi.toml to use project information.

Extension 0.1.12 and later also offer keyword/type completion and built-in completion, hover, and argument hints in new unsaved files, without a compiler, and in untrusted workspaces. When the source cannot be analyzed, unresolved imports or same-name bindings in the file suppress built-in information that could refer to another definition. Local types, field candidates, and F12 require the compiler and a trusted workspace.

| State | Information shown or action |
|---|---|
| Only the field after value. is incomplete | Field candidates if the receiver's type is known |
| Unclosed parentheses elsewhere | Saved function/class declarations marked “保存済み” (saved); no local types, fields, or F12 |
| Undefined types/names, moved values, or names outside scope | No guessed types; check Problems |
| Local types, field completion, or local F12 unavailable | Update both compiler and extension |
| Helper functions missing | Check imports from [nagi.toml](projects.md)'s entry |
| Project declarations, local types, or F12 unavailable | Check workspace trust, nagi.compilerPath, and the Nagi Output channel |

Edits clear stale Problems diagnostics. Automatic checks run on opening and saving files, not on every keystroke. **Nagi: 型検査** (Type Check) also reads unsaved edits to previously saved files without saving or building. Save new files and `nagi.toml` first. Lower, build, and run save project files before executing. Seeing a type in a hover or completion does not mean the whole program passed checking.
