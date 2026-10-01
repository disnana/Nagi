# Low and handwritten integration

Low is readable and writable text with its own parser.

```low
fn twice(x: i64) -> i64 {
    return x * 2;
}
```

Generated Low includes inferred types and let declarations. The compiler parses that text again, integrates ordinary and replacement functions from native Low, and checks the result. Native functions also participate in High name resolution.

```low
@replace generated::score
fn optimized_score(x: i64) -> i64 {
    return x * 6;
}
```

The compiler checks that the target exists, that argument and return types and async status match, and that replacements are unique. Replacement works on whole functions. Patching individual generated lines and `@override`/`@custom` are not implemented. Direct edits to generated files do not survive regeneration; move those changes to native Low.

Result matching replaces High's indentation with braces. This is a function example:

```low
fn number_or(text: view[str], fallback: i64) -> i64 {
    match parse_i64(text) {
        case Ok(number) { return number; }
        case Err(_) { return fallback; }
    }
}
```

Both cases are required. Payload types, ownership, and borrowing follow High's rules. See [error handling](error-handling.md).

Low 0.1 handles typed values, views, classes/records, functions, branches, Result matching, loops, async functions, and scopes. Raw pointers, layout/alignment declarations, allocation/free, SIMD instructions, unsafe operations, and FFI remain future work. Rust source is not accepted as Low.
