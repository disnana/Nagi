# Syntax reference

[Contents](README.md) · First program: [Language guide](language-guide.md) · [Built-in functions](builtins.md)

This reference describes High (`.nagi`): indented application code with typed functions and values. Some short examples are fragments for a function body. Use the [complete introductory program](../../examples/tutorial/basics.nagi) to run several features together.

To find a familiar Python operation, start here:

| Goal | Python form | Learn the current Nagi form |
|---|---|---|
| Update a value | `count += 1` | [Values and types](language-guide.md#1-values-and-types); reassignment keeps the type |
| Use a function | `def add(a, b):` | [Functions](language-guide.md#2-define-functions); annotate parameters and run from main |
| Append to a list | `items.append(value)` | [Lists and loops](language-guide.md#3-lists-classes-branches-and-loops); `append(items, value)` |
| Keep using a passed value | `b = a` refers to the same value | [Views and copies](language-guide.md#4-borrow-with-view-when-you-only-need-to-read), [sharing](ownership.md#retain-the-same-value-in-several-places) |
| Handle an absent value | `if value is None:` | [Some/None](error-handling.md#handle-an-absent-value) matching |
| Handle failure locally | `try/except` | [Result matching](error-handling.md#separate-success-and-failure); `try expression` propagates Err |
| Wait for async work | `await operation()` | [Await](language-guide.md#7-move-on-to-async-and-apis), [spawn and scopes](async.md) |

## Files and indentation

- Save as UTF-8 with a `.nagi` extension.
- Put `import`/`from` statements, classes, enums, functions, async functions, and external Rust declarations at the top level. Executable statements go inside functions.
- Introduce a block with `:` and indent with spaces. Four spaces are recommended; tabs are forbidden.
- Comments start with `#`. Identifiers use ASCII letters, digits, and `_`, and cannot begin with a digit. Unicode strings and comments are supported.
- Expressions inside `()` and `[]` can span lines. Trailing commas are not supported.

```nagi
def main():
    # A comment
    print("Hello, Nagi!")
```

## Values and variables

| Form | Meaning |
|---|---|
| `count = 10` | Inferred type; integers default to i64 |
| `count: i32 = 10` | Explicit type |
| `rate = 1.5` | Floats default to f64 |
| `enabled = True` / `False` | Boolean; lowercase true/false also accepted |
| `name = "Nagi"` / `'Nagi'` | UTF-8 string |
| `values = [1, 2, 3]` | List of elements with one type |
| `values: List[i64] = []` | Type annotation for an empty list |
| `missing: i64? = None` | Absent nullable value; null also accepted |
| `present: i64? = some(42)` | Present nullable value |
| `count = 11` | Reassignment with the same type |
| `count += 1` / `-= 1` / `*= 2` | Compound assignment; `/=` and `%=` unsupported |

Reassignment cannot change a variable’s type. The explicit move migration uses `std.ownership.move` when assigning an existing non-Copy local. Fresh values and Copy assignment do not require it. See [assignment rules](ownership.md#assignment-and-explicit-move) for implementation and release status.

String escapes are `\n`, `\r`, `\t`, `\"`, `\'`, and `\\`. String concatenation with `+`, f-strings, interpolation, and triple-quoted strings are unsupported. See [types](types.md).

## Functions and return

Calls resolve to a local function value, a user-defined function, then a builtin, in that order. Defining your own `len` makes `len(...)` call that function. Variables containing other values cannot be called.

Rust keywords such as `type` can be names wherever Nagi's grammar permits them. The compiler escapes names in generated Rust and preserves the original names in Low, JSON fields, and SQLite columns.

```nagi
def add(a: i64, b: i64) -> i64:
    return a + b

def show(value: i64):
    print(value)
    return
```

Parameter types are required. An omitted return type means unit. Value-returning functions must return on every path. Calls use positional arguments, such as `add(1, 2)`. Default/variadic arguments and user-defined generics are unavailable.

## Branches and loops

This is a function-body fragment:

```nagi
score = 80
if score >= 80:
    print("Passed")
else:
    print("Try again")

for index in range(3):
    print(index)

count = 0
while count < 3:
    count += 1
```

Conditions require bool. `range(n)` takes one argument and runs from zero up to but excluding n. List/view iteration copies Copy elements and borrows non-Copy elements for reading. See [ownership](ownership.md) for the restrictions. `elif`, `break`, `continue`, and `pass` are unavailable.

## Operators

The table runs from highest to lowest precedence. Binary operators on the same level parse left to right. Use parentheses where an expression is unclear.

| Precedence | Operator | Example |
|---|---|---|
| Highest | Call, field, index | `add(1, 2)`, `point.x`, `values[0]` |
| ↓ | Unary `-`, `not`, `try`, `await` | `-count`, `not enabled`, `try await db_open(...)` |
| ↓ | `*`, `/`, `%` | `count * 2` |
| ↓ | `+`, `-` | `count + 1` |
| ↓ | `<`, `>`, `<=`, `>=` | `count < 10` |
| ↓ | `==`, `!=` | `count == 10` |
| ↓ | `and` | `count > 0 and count < 10` |
| Lowest | `or` | `enabled or count == 0` |

Negation, `-value`, accepts signed integers (i8/i16/i32/i64) and floating-point numbers (f32/f64). It does not accept functions or class values.

| Values being compared | `==` / `!=` | `<` / `>` / `<=` / `>=` |
|---|---|---|
| Numbers, bool, or str of the same type | Supported | Supported |
| UUID or timestamp | Supported | Unsupported |
| view[str] or view[bytes] | Supported | Supported |
| view[T] | When T supports equality | When T supports ordering |
| Method or Status | Supported; Method comparison borrows | Unsupported; compare Status numbers through `.value` |
| Classes or owned Lists | Unsupported | Unsupported |

A borrowed list of classes, such as `view[Point]`, cannot be compared directly either. Compare the fields you need. List views compare their elements.

Numeric types do not convert implicitly. Use `i64(value)` to widen i32. `i32(an_i64)` returns `Result[i32, Error]`; use forms such as `try i32(value)` inside a Result-returning function.

Use `count > 0 and count < 10` rather than chained `0 < count < 10`. Integer `/` is integer division. `**`, `//`, and bitwise operations are unsupported.

## Classes, lists, and views

```nagi
class Point:
    x: f64
    y: f64

def main():
    point = Point(x=1.0, y=2.0)
    print(point.x)
    values = [10, 20]
    append(values, 30)
    print(values[0])
    borrowed = view(values)
    duplicate = copy(borrowed)
    print(len(duplicate))
```

Construct classes with every field named. Assigning fields/indices, methods, and inheritance are unsupported. Indices start at zero; negative or out-of-range indices panic at runtime. Strings cannot be indexed. String `len` counts UTF-8 bytes, unlike Python’s character count: `len("あ")` is `3`. List `len` counts elements. Borrow a string/list range with `try slice(view(data), start, end)`.

Passing owned strings/lists to user-defined functions moves them. For read-only arguments, accept `view[str]` or `view[i64]` and pass `view(value)`. See the [guide](language-guide.md) and [ownership](ownership.md).

## Result, async, and scopes

| Form | Meaning |
|---|---|
| `-> Result[i64, Error]` | Returns an integer or Error |
| `return ok(42)` | Returns success |
| `return error("reason")` | Returns failure |
| `return not_found("reason")` | Missing target; HTTP 404 |
| `return fail(problem)` | Returns an Error, custom class, or enum failure value |
| `value = try parse_i64("42")` | Extracts a value or returns failure to the caller |
| `async def work():` | Defines an async function |
| `await sleep(10)` | Waits for 10 milliseconds |
| `db = try await db_open(":memory:")` | Waits and propagates Result failure |

Use `try` in functions returning Result with the same error type, and `await` in async functions. `try` extracts success or returns Err to the caller; it does not catch Python-style exceptions. Handle Result locally with both cases. This is a function-body fragment:

```nagi
match parse_i64("42"):
    case Ok(number):
        print(number)
    case Err(problem):
        print(error_kind(problem))
```

An absent `T?` (Option) differs from Err and panic. Extract it with both `case Some(value):` and `case None:`; see the [absence example](error-handling.md#handle-an-absent-value). Use `Some(_)` to discard the value.

Use `_` for unused payloads. Matching consumes its owned subject; names exist only in their case and cannot reuse outer variable names. E in `Result[T, E]` can be a custom class or enum. Match every enum variant with cases such as `case Choice.Cancelled:` and `case Choice.Selected(id):`. See [enum definitions](types.md#distinguish-variants-with-an-enum) and [error handling](error-handling.md).

Spawn child tasks inside a scope, as in this complete program:

```nagi
async def main() -> Result[unit, Error]:
    async with scope:
        spawn sleep(10)
        spawn sleep(15)
    return ok(print("Done"))
```

This program prints `Done` after both sleeps finish. Normal scope exit waits for its children. With legacy statement spawn, detecting a child Err or panic after the body finishes stops and waits for the remaining children. Parent Future destruction or body panic requests termination without guaranteeing that termination has completed. Returning inside a scope and passing views to another task remain unsupported. Legacy statement spawn accepts only async calls returning unit or `Result[unit, Error]`. [Task result handles](task-handles.md) are implemented on the working branch and remain unreleased. See [async](async.md).

## Imports, HTTP, and Rust

| Purpose | Form | Details |
|---|---|---|
| Load a file | `import "models.nagi"` | [Imports](modules-and-rust.md); one shared namespace |
| Name a module | `import "orders.nagi" as orders` | Use that file's own definitions through `orders.Order` or `orders.score(...)` |
| Select a definition | `from "orders.nagi" import Order as SavedOrder` | Select several definitions with commas; each `as name` is optional |
| Import standard HTTP | `import std.http.server as http` | Use `http.Request` and `http.Status.OK`; `as` is required |
| Select standard types | `from std.http.server import Request, Response, Status as Code` | [Standard imports](modules-and-rust.md#import-the-standard-http-library) |
| Define a GET handler | `@get("/users/{id}")` before a function | [HTTP](http.md); post/put/delete also available |
| Return HTML | `return ok(html("<h1>Hello</h1>"))` | Return type `Result[Html, Error]` |
| Embed text | `include_text("index.html")` | Relative to source; embedded at compile time |
| Declare a Rust function | `@rust("native::crc32")`, then `extern def crc32(text: view[str]) -> i64` | [Rust integration](modules-and-rust.md); no body or trailing colon |

`orders.Order` and `SavedOrder` are the same type. Same-named classes from different files are different types. `from` and `as` are contextual import keywords and remain available as ordinary identifiers.

## Differences from Python

| Common Python form | In Nagi |
|---|---|
| `def add(a, b):` | Annotate parameters; add a return type when returning a value |
| `print(a, b)` | `print(a)` and `print(b)` separately |
| `items.append(x)` | `append(items, x)` |
| `try: ... except:` | `try expression` propagates failure; match branches on Result |
| `from models import User` | `from "models.nagi" import User`; quote the relative file path |
| Dictionaries, tuples, comprehensions, lambdas | Unsupported; use classes, lists, ordinary functions, and loops |
| `str(42)` or arbitrary casts | No general conversion; use forms such as `print(42)` directly |

Construct nullable values with `None` / `some(value)` and extract them with `case Some(value):` / `case None:`. There is no general unwrap API.

Release integer arithmetic follows the Rust backend's fixed-width behavior; overflow in operations such as addition wraps. Debug Rust builds may panic. A consistent language specification for checked/wrapping arithmetic is future work. See [Low](low-language.md) for its syntax/replacements and the [roadmap](roadmap.md) for plans.
