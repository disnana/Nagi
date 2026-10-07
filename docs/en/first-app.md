# Your first small CLI app

[Docs contents](README.md) · Previous: [Setup and first run](getting-started.md) · Next: [Learn by writing code](language-guide.md)

Build a CLI app that checks whether an amount is at most 1,000 yen. Install a released Nagi compiler, Rust/Cargo, and C build tools. Follow [setup](getting-started.md) first if you have not prepared them.

This example reads one line and prints a result. It does not save data to a file or database. Before adding persistence, read the current API and limits in [SQLite](database.md).

## 1. Create a working folder and file

Choose a location in your terminal. Create a folder named `nagi-budget` and make it the current directory:

```sh
mkdir nagi-budget
cd nagi-budget
```

Create `first_app.nagi` in this folder and save the following code. The same code is in the [runnable repository example](../../examples/tutorial/first_app.nagi).

```nagi
def within_budget(amount_text: view[str], limit: i64) -> Result[bool, Error]:
    amount = try parse_i64(amount_text)
    if amount < 0:
        return error("amount must be non-negative")
    return ok(amount <= limit)

def main() -> Result[unit, Error]:
    print("Enter a whole-number amount:")
    text = try read_line()
    fits = try within_budget(view(text), 1000)
    if fits:
        print("within budget")
    else:
        print("over budget")
    return ok(print("done"))
```

When `mkdir` and `cd` succeed, they print nothing. Confirm that the editor saved the file as `nagi-budget/first_app.nagi`. If the folder already exists, skip `mkdir` and move into it.

`within_budget` converts the input string to an integer. `view[str]` borrows the string for reading, and `parse_i64` returns a `Result[i64, Error]`. `try` propagates failure to the caller. After rejecting negative amounts, the function returns the comparison inside `ok`. `main` also uses `try`, so its return type is `Result`.

For these first terms, see [views and ownership](ownership.md#borrowing-and-the-limits-of-checking) and [Result, try, and match](error-handling.md). `unit` is the type for a computation with no useful value; here, `print` returns it inside `ok(...)`.

## 2. Check and run it

From the same working folder, check the compiler version and source:

```sh
nagic --version
nagic check first_app.nagi
```

For Nagi 0.1.11, the version command prints `nagic 0.1.11`. A successful `check` exits with code 0 and reports no Nagi syntax, type, or ownership diagnostics. `check` does not run the Rust/Cargo build.

On success, it prints `checked` followed by the path to the checked file. If it fails, inspect the source line, confirm `nagic --version` is 0.1.11 or later, and check that `first_app.nagi` was saved in the working folder.

On Linux and macOS, pipe an amount into the app:

```sh
printf '700\n' | nagic run first_app.nagi
```

In PowerShell, run the same check with:

```powershell
"700" | nagic run .\first_app.nagi
```

The app's standard output is:

```text
Enter a whole-number amount:
within budget
done
```

`run` builds and executes the app through Rust/Cargo. Even after `check` succeeds, the app can fail before execution if a C compiler or linker is missing, or if a dependency cannot be downloaded. See [build tools](getting-started.md#tools-for-building-applications) and [check versus build](getting-started.md#3-choose-between-checking-and-building).

## 3. Try boundary values and fix a mistake

Suppose the last comparison in `within_budget` were `amount < limit`. These three inputs expose the difference at the boundary:

| Input | Expected output | What it checks |
|---|---|---|
| `700` | `within budget` | Below the limit |
| `1000` | `within budget` | Equal to the limit |
| `1001` | `over budget` | Above the limit |

If `1000` prints `over budget`, change the comparison to `amount <= limit`. Run `nagic check first_app.nagi` again and compare all three inputs with the table.

On Linux and macOS, run these three commands. In PowerShell, use the corresponding commands below.

```sh
printf '%s\n' '700' | nagic run first_app.nagi
printf '%s\n' '1000' | nagic run first_app.nagi
printf '%s\n' '1001' | nagic run first_app.nagi
```

```powershell
"700" | nagic run .\first_app.nagi
"1000" | nagic run .\first_app.nagi
"1001" | nagic run .\first_app.nagi
```

Each command prints the prompt, the result in the table, and `done`. `abc` fails number conversion; `-1` returns the app's own error. Both exit with status 1. Nagi 0.1.11 prints:

| Input | Error output | Exit code |
|---|---|---:|
| `abc` | `Invalid: invalid digit found in string` | 1 |
| `-1` | `Invalid: amount must be non-negative` | 1 |

On Linux/macOS, pipe `printf '%s\n' 'abc'` or `printf '%s\n' '-1'` into `nagic run first_app.nagi`; in PowerShell, pipe `"abc"` or `"-1"` the same way. If an error is shown, see [propagating and recovering from Result](error-handling.md#propagate-failure-to-the-caller).

A negative amount returns the `Error` created by the app; `abc` returns the `Error` from `parse_i64`. `try` propagates both to `main`. See [Results and error handling](error-handling.md) to recover from a failure locally.

Nagi does not currently provide a built-in unit test framework. This tutorial uses a small input table that you run and compare manually. For automated regression tests when changing the compiler, see the [contribution guide](contributing.md) and [compiler test guide](../internal/compiler-testing.md).

## 4. Extend the example

Continue with the [language guide](language-guide.md) and [language feature index](README.md#language-feature-index) for functions, Lists, classes, loops, enums, nullable values, `Result`, and `match`. Read [imports and modules](modules-and-rust.md) to split files, [ownership and borrowing](ownership.md) to pass values, and [Rust integration](modules-and-rust.md#call-rust-functions) to call a Rust library.
