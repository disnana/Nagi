# Learn by writing code

[Contents](README.md) · Previous: [Setup and first run](getting-started.md) · Reference: [Syntax](syntax.md)

If you have written Python functions and lists, this guide shows how to do similar things in current Nagi. High (`.nagi`) uses indentation for blocks, with differences in passing values and handling failures. [Install Nagi](getting-started.md), then run `nagic run filename.nagi` in your working folder.

## 1. Values and types

To increase a number and print it, Python can use:

```python
count = 10
count += 1
print(count)
```

Nagi also starts with assignment. Place this fragment inside a function such as `main`:

```nagi
count = 10            # i64, inferred from the value
rate = 1.5            # f64
enabled = True        # bool: use True / False
name = "Nagi"         # str: Unicode text is supported
age: i32 = 18         # An annotation: name: type = value
count += 1
print(count)
```

This fragment prints `11`. You can reassign variables but cannot change their types. `count = "ten"` is a type error. Conditions require `bool`; you cannot use an integer as a condition with `if count:`.

Different integer types do not mix implicitly. `i64(age)` widens i32 to i64. The reverse, `i32(count)`, can exceed the range and returns `Result[i32, Error]`. Failure handling comes later in this guide.

Assignment determines a type; updates keep that type. See [types](types.md) and [variable syntax](syntax.md#values-and-variables).

## 2. Define functions

Put reusable addition in a function. Python allows parameters without type annotations:

```python
def add(a, b):
    return a + b

print(add(20, 22))
```

In Nagi, annotate the parameters and returned value, and put executable statements in `main`:

```nagi
def add(a: i64, b: i64) -> i64:
    return a + b

def main():
    print(add(20, 22))
```

Save and run this complete program to print `42`. Parameters use `name: type`; return types use `-> type`. An omitted return type means `unit`, with no returned value. `main` is the entry point.

Putting `print(add(...))` directly at file level, as in Python, is rejected. Put it inside `main`, as above. Define typed functions and call them from `main`; see [functions and return](syntax.md#functions-and-return).

## 3. Lists, classes, branches, and loops

To add up several numbers and branch on the result, start with a list. Python appends and iterates like this:

```python
values = [1, 2, 3]
values.append(4)
for value in values:
    print(value)
```

A Nagi list literal is `[1, 2, 3]`, and its type annotation is `List[i64]`. `append(values, 4)` adds an element. A class groups named fields into a type.

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
- `for value in values` reads elements in order. Elements such as numbers are copied; strings and classes containing them are borrowed for reading. The property that permits implicit copying is called Copy. The source list cannot change during the borrow. See [ownership](ownership.md) for examples.
- `range(3)` gives `0, 1, 2`. It takes one argument and excludes the end.
- `while` repeats while its condition is `True`. `break`/`continue` are not supported.
- Combine conditions with `and`/`or`/`not`. There is no `elif`; put another `if` inside `else` when needed.

Nagi does not support `values.append(4)`; use `append(values, 4)`. Read lists with loops and group named values with classes. See [branches and loops](syntax.md#branches-and-loops) and [types](types.md).

`view(values)` borrows the list for reading instead of giving it away. The next section explains this.

## 4. Borrow with view when you only need to read

Suppose a function reads your string and you want to use it afterward. In Python, assignment does not create a new list; both names refer to the same list:

```python
a = [1, 2]
b = a
b.append(3)
print(a)  # [1, 2, 3]
```

In Nagi, a function that only reads accepts `view[str]`, and the caller lends the string with `view(name)`. This complete program is in [borrowing.nagi](../../examples/tutorial/borrowing.nagi):

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

Passing a string or list as an owned argument to a user-defined function gives the value away. This is called a **move**.

To transfer a string to another local, make the operation explicit. This assignment rule is published in Nagi 0.1.11; confirm that the installed compiler is version 0.1.11 or later:

```nagi
from std.ownership import move

def main():
    name = "Nagi"
    destination = move(name)
    print(destination)
```

Output: `Nagi`. Bare `destination = name` is rejected for an owned non-Copy local, and `name` is unavailable after `move(name)`. To keep reading the original, replace the assignment with `destination = copy(view(name))`; do not add the copy after moving the value. Copy values such as numbers and freshly constructed values use ordinary assignment; existing argument, return, and field/index rules are unchanged. See [assignment and explicit move](ownership.md#assignment-and-explicit-move) for import aliases and the exact scope.

This complete program intentionally fails `check`:

```nagi
# This program is rejected
def take_name(name: str):
    print(name)

def main():
    name = "Nagi"
    take_name(name)
    print(name)       # name was moved to take_name
```

The original `name` is unavailable afterward. For reading, change the parameter to `view[str]` and pass `view(name)`, as in the first example. Lend with view when you only need to read.

Built-ins such as `print` and `len` read their input and do not move a string merely because you pass it. Functions do not all handle arguments in the same way.

`len(str)` counts UTF-8 bytes rather than characters: `len("あ")` is `3`. Modifying the original data is also restricted while a view is alive. See [ownership](ownership.md).

## 5. Return failures with Result

To convert text to a number, Python uses `int(text)`, which raises an exception on failure. You can let it propagate or handle it locally:

```python
try:
    number = int("abc")
except ValueError:
    number = 0
```

Nagi returns processing failures as values. Use `T?` for an absent value, `Result` for success or failure, and distinguish panics from ordinary input failures. Try extracting `T?` in [handling absent values](error-handling.md#handle-an-absent-value).

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

Unlike Python's int, i64 has a fixed range. This example assumes small integers and omits a range check before doubling. For large inputs, also check the [integer arithmetic rules](syntax.md#operators).

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

This function returns `21` for `"21"`, or the supplied fallback for `"abc"`. For Result, write both `Ok` and `Err` cases. Each bound name is available only in its case; use `_` for an unused value. `match` also supports nullable `Some`/`None` and user-defined enums. Calling `parse_i64(text)` and discarding the Result is rejected by `check`. Use `try` to propagate it or `match` to recover, as above. Receive and handle failures as returned values. See [error handling](error-handling.md) and [types](types.md).

## 6. Split code into files

To put a function in another file, Python imports a module by name, for example `from math_helpers import add`. Nagi uses a quoted relative file path. Create two files in the same directory.

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

This form loads one namespace, so call `add(...)`. Change the import to `import "math.nagi" as math` to call `math.add(...)`. A from statement can select a class or function. Bare `import math` does not load this file; use `import "math.nagi"`. Imports resolve relative to the importing file. See [imports and Rust integration](modules-and-rust.md) to distinguish same-named definitions or use Rust libraries.

## 7. Move on to async and APIs

To wait for a timer or I/O result, Python also uses `await` inside `async def`:

```python
import asyncio

async def wait_once():
    await asyncio.sleep(0.01)  # Seconds
```

Nagi uses `async def` and `await` too. This is a complete program:

```nagi
async def main() -> Result[unit, Error]:
    await sleep(10)
    print("Waited 10 milliseconds")
    return ok(print("Done"))
```

It prints `Waited 10 milliseconds`, then `Done`. Python’s `asyncio.sleep` takes seconds; Nagi’s `sleep` takes milliseconds. `sleep` returns `unit` and needs only `await`. Fallible database calls use forms such as `try await db_open(...)`. `await` waits for the operation; `try` propagates a failure in the returned result.

A bare `sleep(10)` is rejected because it is not awaited; use `await sleep(10)`. Awaiting does not itself create another task. Use await to wait for a result. Try `spawn` for concurrent work and scopes for managing its lifetime in [async](async.md).

Continue with [Your first small CLI app](first-app.md) to validate input and try boundary values. Then build an API you can call from a browser with [HTTP and HTML](http.md). Use the [syntax reference](syntax.md) for notation and [built-in functions](builtins.md) for arguments.
