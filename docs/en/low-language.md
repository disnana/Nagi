# High and Low

High uses indentation in `.nagi` files. Low uses braces and semicolons in `.low` files. Use High for everyday application code. Low lets you save and inspect generated code or replace selected function implementations. If you prefer braces, you can also write and run an entire application in Low.

Types, ownership, borrowing, and Result handling follow the same rules as High. High can [call Rust functions directly](modules-and-rust.md), so handwritten Low is not required for Rust integration. Moving code to Low alone does not make it faster or bypass Rust's borrow checker. Expanding Low into an independent systems language is outside the current development scope.

## Write and run a standalone Low program

After [setting up the compiler and build environment](getting-started.md), save this as `app.low` in your working folder:

```low
fn total(price: i64, quantity: i64) -> i64 {
    return price * quantity;
}

fn main() -> unit {
    let amount = total(120, 3);
    print(amount);
}
```

```sh
nagic check app.low
nagic run app.low
```

`check` checks types and ownership; `run` builds and executes the program. The result is `360`. No High file or `--native` option is needed.

Low declares functions with `fn`, encloses blocks in `{ }`, and separates statements with `;`. The type of `amount` is inferred from the call's result. You can also write it explicitly as `let amount: i64 = total(120, 3);`. Low uses `record` for the declaration called `class` in High.

To split a Low program across files, use relative imports such as `import "orders.low" as orders;`. Ordinary file imports load only `.nagi` files from High and `.low` files from Low. To connect High and Low, use the `--native` additions and replacements described below.

Try the [order quote CLI](../../test-nagi-code/low-examples/order-quote/README.en.md) for an application with multiple Low files, records, and Result handling.

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

Loading Low checks its types and ownership again. Saved Low retains module identity, but does not save diagnostic mappings back to the original High. See [compiler internals](compiler-internals.md).

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

Use `import "orders.low" as orders;` or `from "orders.low" import Order as SavedOrder;` for relative-file imports. A module name exposes functions, records, and enums defined in that file.

This function handles a Result:

```low
fn number_or(text: view[str], fallback: i64) -> i64 {
    match parse_i64(text) {
        case Ok(number) { return number; }
        case Err(_) { return fallback; }
    }
}
```

Both `Ok` and `Err` cases are required. See [error handling](error-handling.md).

Enums have the same variants and payloads as High. Match every variant:

```low
enum Choice {
    Cancelled;
    Selected(id: i64);
}
fn selected_or_zero(choice: Choice) -> i64 {
    match choice {
        case Choice.Cancelled { return 0; }
        case Choice.Selected(id) { return id; }
    }
}
```

Low currently supports typed variables, views, classes (called records in Low), enums, functions, branches, loops, async functions, and scopes. Raw pointers, memory layout declarations, manual allocation and freeing, SIMD instructions, unsafe syntax, and a C ABI are unsupported. See [file imports and Rust integration](modules-and-rust.md) to call Rust functions.
