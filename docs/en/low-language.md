# High and Low

High uses indentation in `.nagi` files. Low uses braces and semicolons in `.low` files. Both are Nagi languages. Write ordinary code in High and use Low when you want to replace a function's implementation.

## Convert High to Low

Save this as `app.nagi`:

```nagi
def score(value: i64) -> i64:
    return value * 2

def main():
    print(score(7))
```

```sh
nagic run app.nagi
nagic lower app.nagi
```

The program prints `14`. `lower` checks types and ownership, then writes Low to `build/app/generated.low`. Low declares functions with `fn` and encloses blocks in `{ }`. Generated variable declarations include `let` and their inferred types.

## Replace a function with handwritten Low

Save this as `native.low` in the same directory:

```low
@replace generated::score
fn optimized_score(value: i64) -> i64 {
    return value * 6;
}
```

```sh
nagic check app.nagi --native native.low
nagic run app.nagi --native native.low
```

The result is now `42`. High still calls `score(7)`, but the body that runs comes from Low's `optimized_score`. `@replace generated::score` specifies the function to replace.

For a function imported at the High root with `import "orders.nagi" as orders`, use `@replace generated::orders::score`. A from function alias can be selected with `@replace generated::alias`. Type annotations such as `orders.Order` and from class aliases resolve to the same definition IDs. See [imports and Rust integration](modules-and-rust.md).

The replacement must match the original function's parameter count and types, return type, and async status. A missing target or multiple replacements of the same function is an error. Replacements affect whole functions.

Commands regenerate `generated.low`. Save changes in `native.low` to keep them. `--native` can also add ordinary Low functions that High can call. See [project configuration](projects.md#add-rust-and-handwritten-low) to save these options.

## Low syntax

Relative-file imports follow the same rules as High: `import "orders.low" as orders;` or `from "orders.low" import Order as SavedOrder;`. A module name exposes functions and records defined in that file.

Types, ownership, borrowing, and Result handling follow the same rules as High. This function handles a Result:

```low
fn number_or(text: view[str], fallback: i64) -> i64 {
    match parse_i64(text) {
        case Ok(number) { return number; }
        case Err(_) { return fallback; }
    }
}
```

Both `Ok` and `Err` cases are required. See [error handling](error-handling.md).

Low currently supports typed variables, views, classes (called records in Low), functions, branches, loops, async functions, and scopes. Raw pointers, memory layout declarations, manual allocation and freeing, SIMD instructions, unsafe syntax, and a C ABI are unsupported. See [file imports and Rust integration](modules-and-rust.md) to call Rust functions.
