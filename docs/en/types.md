# Types and inference

[Contents](README.md) · New to Nagi? [Language guide](language-guide.md)

Annotate variables with `count: i32 = 10`, parameters with `count: i32`, and return values with `-> i32`. Local annotations can be omitted, but reassignment cannot change an established type.

| Type | Representation | Status |
|---|---|---|
| i8/i16/i32/i64, u8/u16/u32/u64 | Native fixed-width integers | Implemented; no implicit conversion between types |
| f32/f64, bool | Native floating-point and Boolean values | Implemented |
| str, bytes | String / Vec<u8> | Owned values; str is UTF-8 |
| List[T], [T] | Contiguous Vec<T> | Iteration over primitives and Copy classes |
| view[str], view[bytes], view[T] | &str, &[u8], &[T] | Non-owning; lifetimes checked |
| shared[T] | Explicit Arc<T> | Implemented |
| Result[T, Error] | Tagged Result | try propagation and Ok/Err matching implemented |
| T? | Option<T> | Implemented; None requires type context |
| UUID, timestamp | u128/i64 newtypes | Native representation; explicit UUID parse/format |
| Map[K,V], owned[T] | HashMap and ownership type foundations | Type notation exists; complete operation APIs not implemented |
| Function/async function types | Static specialization is the goal | General generics and function-type annotations not implemented |

`count = 10` infers i64; `rate = 1.5` infers f64. Annotated integer literals are checked against the type's range. Current conversions are lossless `i64(value)` from i8/i16/i32/u8/u16/u32 and range-checked `i32(value) -> Result[i32, Error]` from i64. There is no general cast to arbitrary types.

Floating-point literals are also checked against the `f32` or `f64` type determined by an annotation, parameter, return type, or other context. A value that would become infinity in that type fails type checking.

Since VS Code extension 0.1.4, hovering a variable shows its inferred type, such as `count: i64`. This also covers arguments and case bindings. Try the [editor walkthrough](editor.md).

Write `missing: i64? = None` for an absent nullable value and `present: i64? = some(42)` for a present value. `T?` abbreviates `Option[T]`; matching and a general unwrap API are not yet available. Annotate empty lists, for example `values: List[i64] = []`.

Values are not all boxed. String, Vec, tasks, and channels still have internal allocations. Nagi uses Rust monomorphization and LLVM optimization. User-defined generic functions and traits remain future work.

## Minimum signed integers

The minimum values can be written directly as literals: `-128` for `i8`, `-32768` for `i16`, `-2147483648` for `i32`, and `-9223372036854775808` for `i64`. For example, `minimum: i8 = -128`. A literal outside the specified type's range fails type checking.
