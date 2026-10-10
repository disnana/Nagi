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
| `List[T]` / `[T]` | Lists of elements of the same type | Owned; for copies Copy elements and borrows non-Copy elements for reading |
| `view[str]` / `view[bytes]` / `view[T]` | Read borrowed strings, byte sequences, or lists | Requires an owner for the source data; read-only |
| `T?` / `Option[T]` | A value or `None` | `None` requires type context; extract with a Some/None match |
| `Result[T, E]` | A success value or an error | `E` can be Error, a class, or an enum; propagate with `try` or handle with `match` |
| `shared[T]` | An owned value shared across multiple places | Create with `share`; duplicate its shared reference with `clone_shared` |
| `UUID` / `timestamp` | UUID / time values | Use `uuid_parse` / `uuid_format` for UUID text conversion |
| `fn[parameter types..., return type]` | Pass a function as a value | Supported in synchronous function signatures; the last type is the return type |
| `unit` | No return value | The return type when a function omits its return annotation |

`count = 10` infers i64; `rate = 1.5` infers f64. Annotated integer literals are checked against the type's range. Current conversions are lossless `i64(value)` from i8/i16/i32/u8/u16/u32 and range-checked `i32(value) -> Result[i32, Error]` from i64. There is no general cast to arbitrary types.

Floating-point literals are also checked against the `f32` or `f64` type determined by an annotation, parameter, return type, or other context. A value that would become infinity in that type fails type checking.

In the VS Code extension, hovering a variable shows its inferred type, such as `count: i64`. This also covers arguments and case bindings. Try the [editor walkthrough](editor.md).

Write `missing: i64? = None` for an absent nullable value and `present: i64? = some(42)` for a present value. `T?` abbreviates `Option[T]`. Write both cases to extract the value:

```nagi
match present:
    case Some(value):
        print(value)
    case None:
        print("No value")
```

Use `Some(_)` to discard a value. There is no general unwrap API. Annotate empty lists, for example `values: List[i64] = []`.

See [syntax](syntax.md) for parameter, return, and borrow annotations; [ownership](ownership.md) for copy and move rules; and [built-in functions](builtins.md) for accepted argument types.

In Nagi 0.1.10, `view[bytes]` iteration and indexing yield `u8` values. Character iteration and integer indexing of `view[str]` are unsupported. Use `slice` to borrow part of a string with checked UTF-8 byte boundaries.

Nagi 0.1.10 supports rebinding a view parameter to local data and adding local views to lists that hold views. These views cannot outlive their local owners or be returned as borrowed input data. The Nagi 0.1.11 target adds checked flow plans and regression cases for restoring view-containing List/Result/Option values across nested values, branches, loops, and async code. This is not blanket support for every view combination. If supported Nagi code passes check and generated Rust is then rejected for a Nagi-originated type, move, or lifetime problem, treat that as a compiler defect. Rust adapter crate APIs and traits, targets, and dependency environments still require a Rust build.

## Distinguish variants with an enum

```nagi
enum Choice:
    Cancelled
    Selected(id: i64)
```

Construct `Choice.Cancelled` or `Choice.Selected(id=42)`, then handle every variant with `match`. Payload variants also accept positional arguments. An enum is copyable when every payload field is copyable; fields such as str, Error, or List make it move instead.

Import an enum from a file like a class. Enum type parameters, methods, and JSON conversion are unsupported. See [error handling](error-handling.md#define-your-own-error-type) for an example with Result.

## Standard HTTP resource types

Import `Request`, `Response`, `Method`, `Status`, `Options`, and `App[State, E]` from `std.http.server`. These are native values created by the library, rather than ordinary classes. They cannot be constructed with named fields or converted to JSON. Only `Status` is Copy. Comparing Methods borrows them; assigning a Request's method to another variable moves that field.

A view of a registered HTTP resource, such as `view[Request]`, refers to the value itself. An ordinary `view[Point]` still means a slice of Points, rather than a reference to one Point. Views of a Request's body, path, or headers borrow their owner and cannot outlive it.

Resource views do not support `len`, `slice`, indexing, or `for`. You can index or iterate a `List[Status]` directly, but creating a view from a List of resources is currently unsupported.

An App handler receives Request by move and reads state through `shared[State]`. Copy fields can be read, but non-Copy fields such as str cannot be moved out of shared state. Borrow them with `view(state.label)`. State fields may contain sqlite.Pool; classes containing sqlite.Pool or HTTP resources do not support JSON conversion. App's State and E type arguments cannot retain views. See [HTTP](http.md) and [standard imports](modules-and-rust.md#import-the-standard-http-library).

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

When a function type has an input holding a view, such as `fn[view[str], view[str]]`, a returned view can be used only while the borrowed input remains valid. With no borrowing input, `fn[view[str]]` and `fn[i64, view[str]]` can only return views of data valid until the program exits. The same rule applies to views inside return types such as `List[view[str]]` and `Option[view[str]]`.

For example, an HTTP status's `phrase` is a static string. This code prints `OK`:

```nagi
import std.http.server as http

def phrase() -> view[str]:
    return http.Status.OK.phrase

def apply(factory: fn[view[str]]) -> view[str]:
    return factory()

def main():
    chosen = phrase
    print(apply(chosen))
```

A function cannot return a view of a string it owns. A static view can be stored in a local variable and returned. The example borrows a static string, so returning it does not copy the string.

You can also assign an async function with `selected = answer`, then call `await selected(...)` inside an async function. HTTP `route` and `route_mapped` can register these named async functions or local aliases. General type annotations for parameters receiving async functions, or functions returning them, are not yet supported. Lambdas and closures that capture surrounding local variables are also unsupported.

Storing async functions in lists or classes is also unsupported. You cannot store the unawaited result of async work in a variable. Write `await sleep(10)` rather than `pending = sleep(10)`.

## Unsupported type operations

`Map[K, V]` is accepted as type notation, but has no dedicated APIs for construction, lookup, or updates. User-defined generic functions and traits are unsupported.

`owned[T]` is an unfinished type form. It generates Rust's `T`, but Nagi's checker distinguishes it from `T`. Operations such as `try` and `view` do not consistently treat the two alike. Use `str` directly for owned strings and `List[T]` for owned lists. See [error handling](error-handling.md) for the limits of Result checking.

## Rust representation

Numbers and bool map to the same Rust types. `str` maps to `String`, `bytes` and `List` to `Vec`, and `shared` to `Arc`. Views map to references, nullable values to `Option`, success/failure to `Result`, and enums to Rust enums. See [memory handling](memory-model.md).

## Minimum signed integers

The minimum values can be written directly as literals: `-128` for `i8`, `-32768` for `i16`, `-2147483648` for `i32`, and `-9223372036854775808` for `i64`. For example, `minimum: i8 = -128`. A literal outside the specified type's range fails type checking.
