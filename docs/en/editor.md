# Types and completion in VS Code

[Contents](README.md) · [Setup](getting-started.md) · [Extension installation and settings](vscode-extension.md)

Install Nagi extension 0.1.7 and the latest nagic, then open the Nagi repository in VS Code. This walkthrough uses working code to try type hovers, field completion, and definition navigation. Both High and Low are supported.

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

## When editor information is unavailable

Previously saved Nagi/Low files are analyzed in memory with their unsaved edits, including open imports and native Low. Queries do not save or build sources. Save new files and nagi.toml first.

| State | Information shown or action |
|---|---|
| Only the field after value. is incomplete | Field candidates if the receiver's type is known |
| Unclosed parentheses elsewhere | Saved function/class declarations marked “保存済み” (saved); no local types, fields, or F12 |
| Undefined types/names, moved values, or names outside scope | No guessed types; check Problems |
| Local types, field completion, or local F12 unavailable | Update both compiler and extension |
| Helper functions missing | Check imports from [nagi.toml](projects.md)'s entry |
| All queries unavailable | Check workspace trust, nagi.compilerPath, and the Nagi Output channel |

Unsaved edits clear old Problems diagnostics. Save or run **Nagi: 型検査** (Type Check) to update them. Seeing a type in a hover or completion does not mean the whole program passed checking.
