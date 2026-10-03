# Types and inference

[Contents](README.md) · New to Nagi? [Language guide](language-guide.md)

Annotate variables with `count: i32 = 10`, parameters with `count: i32`, and return values with `-> i32`. Local annotations can be omitted, but reassignment cannot change an established type.

| Type | Purpose | Limits and value handling |
|---|---|---|
| `i8` / `i16` / `i32` / `i64` | Signed integers; the number is the bit width | Copyable; operations between numeric types require explicit conversion |
| `u8` / `u16` / `u32` / `u64` | Unsigned integers | Copyable; cannot store negative values |
| `f32` / `f64` | Floating-point numbers | Copyable |
| `bool` | `True` / `False` | Used in branch and loop conditions |
| `str` / `bytes` | UTF-8 strings / byte sequences | Owned; passing to a user-defined function moves the value |
| `List[T]` / `[T]` | Lists of elements of the same type | Owned; for iteration supports primitive elements and Copy classes/enums |
| `view[str]` / `view[bytes]` / `view[T]` | Read borrowed strings, byte sequences, or lists | Requires an owner for the source data; read-only |
| `T?` / `Option[T]` | A value or `None` | `None` requires type context |
| `Result[T, E]` | A success value or an error | `E` can be Error, a class, or an enum; propagate with `try` or handle with `match` |
| `shared[T]` | An owned value shared across multiple places | Create with `share`; duplicate its shared reference with `clone_shared` |
| `UUID` / `timestamp` | UUID / time values | Use `uuid_parse` / `uuid_format` for UUID text conversion |
| `fn[parameter types..., return type]` | Pass a function as a value | Supported in synchronous function signatures; the last type is the return type |
| `unit` | No return value | The return type when a function omits its return annotation |

`count = 10` infers i64; `rate = 1.5` infers f64. Annotated integer literals are checked against the type's range. Current conversions are lossless `i64(value)` from i8/i16/i32/u8/u16/u32 and range-checked `i32(value) -> Result[i32, Error]` from i64. There is no general cast to arbitrary types.

Floating-point literals are also checked against the `f32` or `f64` type determined by an annotation, parameter, return type, or other context. A value that would become infinity in that type fails type checking.

In the VS Code extension, hovering a variable shows its inferred type, such as `count: i64`. This also covers arguments and case bindings. Try the [editor walkthrough](editor.md).

Write `missing: i64? = None` for an absent nullable value and `present: i64? = some(42)` for a present value. `T?` abbreviates `Option[T]`; matching and a general unwrap API are not yet available. Annotate empty lists, for example `values: List[i64] = []`.

See [syntax](syntax.md) for parameter, return, and borrow annotations; [ownership](ownership.md) for copy and move rules; and [built-in functions](builtins.md) for accepted argument types.

## Distinguish variants with an enum

```nagi
enum Choice:
    Cancelled
    Selected(id: i64)
```

Construct `Choice.Cancelled` or `Choice.Selected(id=42)`, then handle every variant with `match`. Payload variants also accept positional arguments. An enum is copyable when every payload field is copyable; fields such as str, Error, or List make it move instead.

Import an enum from a file like a class. Enum type parameters, methods, and JSON conversion are unsupported. See [error handling](error-handling.md#define-your-own-error-type) for an example with Result.

## Pass a function as a value

A function name can be assigned to a variable or passed to another function. `fn[i64, i64]` means a function taking one i64 and returning i64. For a function with no parameters, write only its return type, such as `fn[i64]`.

Save this as `app.nagi`:

```nagi
def add_one(value: i64) -> i64:
    return value + 1

def apply(callback: fn[i64, i64], value: i64) -> i64:
    return callback(value)

def main():
    chosen: fn[i64, i64] = add_one
    print(apply(chosen, 41))
```

```sh
nagic run app.nagi
```

The result is `42`. Local types can also be inferred: `chosen = add_one`. A function that returns another function can declare a return type such as `def choose() -> fn[i64]:`.

You can also assign an async function with `selected = answer`, then call `await selected(...)` inside an async function. Type annotations for parameters receiving async functions, or functions returning them, are not yet supported. Lambdas and closures that capture surrounding local variables are also unsupported.

Storing async functions in lists or classes is also unsupported. You cannot store the unawaited result of async work in a variable. Write `await sleep(10)` rather than `pending = sleep(10)`.

## Unsupported type operations

`Map[K, V]` and `owned[T]` have type notation but incomplete operation APIs. User-defined generic functions and traits are unsupported.

## Rust representation

Numbers and bool map to the same Rust types. `str` maps to `String`, `bytes` and `List` to `Vec`, and `shared` to `Arc`. Views map to references, nullable values to `Option`, success/failure to `Result`, and enums to Rust enums. See [memory handling](memory-model.md).

## Minimum signed integers

The minimum values can be written directly as literals: `-128` for `i8`, `-32768` for `i16`, `-2147483648` for `i32`, and `-9223372036854775808` for `i64`. For example, `minimum: i8 = -128`. A literal outside the specified type's range fails type checking.
