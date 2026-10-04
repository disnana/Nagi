# Learn by writing code

[Contents](README.md) · Previous: [Setup and first run](getting-started.md) · Reference: [Syntax](syntax.md)

High (`.nagi`) uses indentation for blocks. This guide covers values, functions, lists, and failures with short examples. [Install Nagi](getting-started.md), then run `nagic run filename.nagi` in your working folder.

## 1. Values and types

Place this fragment inside a function such as `main`:

```nagi
count = 10            # i64, inferred from the value
rate = 1.5            # f64
enabled = True        # bool: use True / False
name = "Nagi"         # str: Unicode text is supported
age: i32 = 18         # An annotation: name: type = value
count += 1
print(count)
```

You can reassign variables but cannot change their types. `count = "ten"` is a type error. Conditions require `bool`; you cannot use an integer as a condition with `if count:`.

Different integer types do not mix implicitly. `i64(age)` widens i32 to i64. The reverse, `i32(count)`, can exceed the range and returns `Result[i32, Error]`. Failure handling comes later in this guide.

## 2. Define functions

```nagi
def add(a: i64, b: i64) -> i64:
    return a + b

def main():
    print(add(20, 22))
```

Save and run this complete program to print `42`. Parameters use `name: type`; return types use `-> type`. An omitted return type means `unit`, with no returned value. `main` is the entry point.

## 3. Lists, classes, branches, and loops

A list literal is `[1, 2, 3]`, and its type annotation is `List[i64]`. `append(values, 4)` adds an element. A class groups named fields into a type.

Save this complete program as `basics.nagi`. It is also available as a [sample](../../examples/tutorial/basics.nagi):

```nagi
class Point:
    x: f64
    y: f64

def sum_numbers(values: view[i64]) -> i64:
    total = 0
    for value in values:
        total += value
    return total

def main():
    values = [1, 2, 3]
    append(values, 4)
    total = sum_numbers(view(values))
    print(total)

    point = Point(x=3.0, y=4.0)
    print(point.x + point.y)

    if total >= 10 and len(values) == 4:
        print("OK")
    else:
        print("NG")

    for index in range(3):
        print(index)

    count = 0
    while count < 2:
        count += 1
    print(count)
```

```powershell
nagic run basics.nagi
```

Output, in order: `10`, `7`, `OK`, `0`, `1`, `2`, `2`.

- Construct a Point with all fields named: `Point(x=..., y=...)`. Positional `Point(3.0, 4.0)` is not supported.
- Read fields with `point.x`. Classes do not yet have methods or inheritance.
- `for value in values` reads elements in order. Copy elements, such as numbers, are copied; strings and classes containing them are borrowed for reading. The source list cannot change during the borrow. See [ownership](ownership.md) for examples.
- `range(3)` gives `0, 1, 2`. It takes one argument and excludes the end.
- `while` repeats while its condition is `True`. `break`/`continue` are not supported.
- Combine conditions with `and`/`or`/`not`. There is no `elif`; put another `if` inside `else` when needed.

`view(values)` borrows the list for reading instead of giving it away. The next section explains this.

## 4. Borrow with view when you only need to read

Passing a string or list directly to a user-defined function moves ownership. Using the variable afterward is an error.

```nagi
# This program is rejected
def take_name(name: str):
    print(name)

def main():
    name = "Nagi"
    take_name(name)
    print(name)       # name was moved to take_name
```

A function that only reads can accept `view[str]`. Call it with `view(name)`. This complete program is in [borrowing.nagi](../../examples/tutorial/borrowing.nagi):

```nagi
def name_size(name: view[str]) -> i64:
    return len(name)

def main():
    name = "Nagi"
    print(name_size(view(name)))
    print(name_size(view(name)))
    duplicate = copy(view(name))
    print(duplicate)
    print(name)
```

Output: `4`, `4`, `Nagi`, `Nagi`. `copy(view(name))` creates a separate owned string. The copy has its own storage.

Built-ins such as `print` and `len` read their input and do not move a string merely because you pass it. Functions do not all handle arguments in the same way.

`len(str)` counts UTF-8 bytes rather than characters: `len("あ")` is `3`. Modifying the original data is also restricted while a view is alive. See [ownership](ownership.md).

## 5. Return failures with Result

`Result[T, Error]` returns `T` on success or `Error` on failure. Use `ok(value)` and `error("reason")` to construct them.

`try operation` extracts the success value, or immediately returns the failure to the caller. The function using `try` must itself return Result.

Save this complete example as `input.nagi`. It doubles a nonnegative integer. The [sample](../../examples/tutorial/input.nagi) contains the same logic with Japanese prompts.

```nagi
def double_nonnegative(text: view[str]) -> Result[i64, Error]:
    value = try parse_i64(text)
    if value < 0:
        return error("Enter a nonnegative integer")
    return ok(value * 2)

def main() -> Result[unit, Error]:
    write("Enter an integer > ")
    text = try read_line()
    doubled = try double_nonnegative(view(text))
    print(doubled)
    return ok(print("Done"))
```

```powershell
nagic run input.nagi
```

Enter `21` and press Enter to print `42`, followed by the completion message. `abc` fails number conversion; `-1` triggers the error you wrote. Both propagate to `main` and exit with failure.

`write` prints without a newline; `read_line` reads one line. `return ok(print("Done"))` wraps the `unit` returned by printing as a success.

Nagi's `try` differs from Python's `try: ... except:`. To separate success and failure, for example to return a default, use `match`. This is a function example:

```nagi
def number_or(text: view[str], fallback: i64) -> i64:
    match parse_i64(text):
        case Ok(number):
            return number
        case Err(_):
            return fallback
```

For Result, write both `Ok` and `Err` cases. Each bound name is available only in its case; use `_` for an unused value. `match` also supports nullable `Some`/`None` and user-defined enums. See [error handling](error-handling.md) and [types](types.md).

## 6. Split code into files

Create two files in the same directory.

`math.nagi`:

```nagi
def add(a: i64, b: i64) -> i64:
    return a + b
```

`imports.nagi`, the entry file:

```nagi
import "math.nagi"

def main():
    print(add(20, 22))
```

```powershell
nagic run imports.nagi
```

The [complete example](../../examples/tutorial/imports.nagi) prints `42`. Import paths are relative to **the file containing the import**, not your terminal's working directory. Imported `math.nagi` does not need a `main`.

This form loads one namespace, so call `add(...)`. Change the import to `import "math.nagi" as math` to call `math.add(...)`. A from statement can select a class or function. See [imports and Rust integration](modules-and-rust.md) to distinguish same-named definitions or use Rust libraries.

## 7. Move on to async and APIs

Define functions that wait for timers or I/O with `async def`, and call them with `await`.

```nagi
async def main() -> Result[unit, Error]:
    await sleep(10)
    print("Waited 10 milliseconds")
    return ok(print("Done"))
```

This is a complete program. `sleep` returns `unit` and needs only `await`. Fallible database calls use forms such as `try await db_open(...)`. `await` waits for the operation; `try` propagates a failure in the returned result.

Continue to [HTTP and HTML](http.md) to build an API you can call from a browser. Use the [syntax reference](syntax.md) for notation and [built-in functions](builtins.md) for arguments.
