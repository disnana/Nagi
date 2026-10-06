# High code that checks and builds

[Agent entry point](README.md) · [Rust adapters](rust-interop.md)

Use `.nagi` for application code and four spaces for blocks. Top level contains imports, classes, enums, functions, and extern declarations. Executable statements belong inside functions; `main` is the entry point. Annotate function parameters and value-returning functions. An omitted return annotation means `unit`. Classes are typed records constructed with every field named, rather than objects with methods.

## Values, ownership, and containers

| Intent | Write |
| --- | --- |
| Integer / boolean | `count = 42` (i64), `enabled = True` |
| Explicit numeric type | `small: i32 = 42`; numeric types do not mix implicitly |
| Typed empty list | `values: List[i64] = []` |
| Append to a list | `append(values, 42)` |
| Absent / present value | `missing: i64? = None`, `present: i64? = some(42)` |
| Read a string without moving it | Parameter `text: view[str]`, call with `view(text)` |
| Read an integer list | Parameter `values: view[i64]`, call with `view(values)` |
| Transfer an existing owned non-Copy local | `from std.ownership import move`, then `destination = move(source)` (unreleased repository compiler) |
| Own a separate string/list copy | `copy(view(value))` |
| Share a value | `share(value)`; duplicate the reference with `clone_shared` |

Passing `str`, lists, or non-Copy records to user functions moves them. Reading with `print` or `len` does not move them. A view borrows its owner; the owner must remain valid and cannot be moved or mutated while the borrow is live. To store a view, first save its owner: `text = "Nagi"`, then `borrowed = view(text)`. `borrowed = view("Nagi")` borrows a temporary and is rejected; an immediate call such as `read(view("Nagi"))` is allowed. `view[Point]` means a slice of Points; it is not a general reference to one ordinary class instance. Registered HTTP resource views are a separate supported case.

For list iteration, numeric elements copy and non-Copy elements borrow. Read a record's string field or copy it explicitly; do not take an owned record out of a borrowed iteration. Append to another output list, then mutate the original after the loop. `view[bytes]` iteration/indexing yields `u8`. String character iteration and indexing are unavailable; `slice` uses checked UTF-8 byte boundaries.

For shared state, borrow a non-Copy field with `view(state.label)` rather than moving it out. `shared[T]` does not establish that T is safe across threads; Rust checks the required `Send`/`Sync` traits at build time.

The work branch implements unreleased `std.ownership.move`: import `move`, then use `destination = move(source)` to transfer an existing owned non-Copy local. Bare local RHS assignment is rejected, including declarations, annotations, reassignment, and parentheses. Fresh construction and existing argument/return/field/index/try/match consumption rules are unchanged; do not wrap every transfer.

Copy policy is unchanged, including supported local async function aliases; shared and Result remain non-Copy even with Copy payloads. The operation evaluates its input once, preserving view origins and cleanup responsibility without cloning or allocating. Qualified/aliased imports use canonical identity; a user function named `move` is ordinary. One inferred argument is required; explicit type arguments and first-class standard operation values are unsupported. Futures/nested Futures remain unsupported; a supported awaited result can be moved. Borrowing restrictions still apply.

Confirm the compiler version before assuming an installed release has these rules. Spawn result handles and shared actor messages remain future directions. See [ownership](../docs/en/ownership.md), [async](../docs/en/async.md), and [ADR 011](../docs/internal/adr/011-language-behavior-and-docs.md).

## Result and Option are values

`try expression` extracts `Ok` or returns `Err` to the caller. The caller must return `Result[..., E]` with the same E; it does not catch exceptions or convert error types. Use `try await operation(...)` for an async Result. `ok` constructs success, `fail` wraps an Error/class/enum in a failure Result, and `error("message")` constructs an Err containing a built-in `invalid` Error. Convert another error type explicitly, for example with `std.result.map_error` and a synchronous named mapper.

This complete program shows propagation, a custom error, and nullable matching:

```nagi
enum InputError:
    InvalidNumber
    OutOfRange

def quantity(text: view[str]) -> Result[i64, InputError]:
    match parse_i64(text):
        case Ok(number):
            if number < 1 or number > 100:
                return fail(InputError.OutOfRange)
            return ok(number)
        case Err(_):
            return fail(InputError.InvalidNumber)

def doubled(text: view[str]) -> Result[i64, InputError]:
    value = try quantity(text)
    return ok(value * 2)

def main():
    match doubled(view("21")):
        case Ok(answer):
            print(answer)
        case Err(_):
            print("Invalid input")
    selected: i64? = some(42)
    match selected:
        case Some(value):
            print(value)
        case None:
            print("No value")
```

The 1–100 quantity limit keeps this multiplication within i64. Save as `main.nagi`; `nagic check main.nagi` succeeds and `nagic run main.nagi` prints `42` twice.

Match Result with exactly `Ok` and `Err`, Option with `Some` and `None`, and an enum with every declared variant. Constructors are lowercase `ok`, `fail`, `some`; patterns are uppercase `Ok`, `Err`, `Some`. Match consumes owned payloads. A case binding exists only in that case and cannot shadow an outer variable. Guards, nested patterns, match expressions, and catch-all cases are unavailable.

Handle every Result deliberately. The checker rejects discarding a Result expression and unawaited async calls, but does not reject an unused Result assigned to a variable. `check` alone does not establish that every error is handled. Out-of-bounds indexing and an integer divisor that becomes zero at runtime panic rather than return Result; there is no general Python-style `raise`/`except`. Release arithmetic follows Rust fixed-width behavior; ordinary overflow can wrap.

For integer `/` and `%`, the repository's unreleased common typed constant validator rejects proven zero divisors, including `1 - 1` and supported scalar aliases, and signed MIN division/remainder by `-1` across eight integer widths. It does not evaluate arbitrary function calls or propagate profile-dependent overflow values. Runtime-dependent divisors still need validation. Released 0.1.10's earlier literal-zero rule is narrower; see [constant validation](../compiler/tests/constant_validation.rs), [language contracts](../docs/internal/language-invariants.md), and [CHANGELOG](../CHANGELOG.md) before relying on the repository's additions.

## Async and imports

Named synchronous functions can be passed as values. `fn[i64, i64]` is a function taking an i64 and returning an i64; assign a function name with `chosen = add_one` and call `chosen(41)`. These synchronous function pointers can also cross a Rust extern boundary; see the [callback example](rust-interop.md#pass-a-synchronous-callback).

Use `async def`, then `await` its calls. You can store a named async function with `selected = answer` and call `await selected(...)`. Do not store an unawaited call result. General async callback parameters/return signatures, async functions in containers, and capturing closures are unsupported. The standard HTTP/actor registration functions support their documented named async handlers; this is not a general extern callback mechanism.

`async with scope` and `spawn` wait for children when the body ends. Children must return `unit` or `Result[unit, Error]`; passing borrowed views or returning from inside the scope is unsupported. Current child Err/panic cancels siblings; business Result handling is different in actor replies. Parent Future destruction requests abort without synchronously awaiting child completion. Async CPU work does not automatically run on another thread. Cancellation is not rollback of external work or shared state.

Use quoted paths for local files:

```nagi
import "models.nagi" as models
from "models.nagi" import Item as SavedItem
```

Paths are relative to the importing file. High imports High; Low imports Low. The aliased module exposes that file's own definitions. Definitions imported into it are not re-exported automatically. `models.Item` and `SavedItem` identify the same type; same-named classes in different files identify different types. Cycles are rejected.

Only registered standard modules use unquoted imports: `std.http.server`, `std.actor`, and `std.result`; the unreleased repository also registers `std.auth` and `std.ownership`. Module imports require an alias, for example `import std.result as result`. There is no general package discovery or arbitrary `import serde_json`; declare a Rust adapter for crates.

## Common generated-code corrections

| Tempting Python/Rust form | Supported approach |
| --- | --- |
| `print(a, b)` | One value per `print` |
| `items.append(x)` | `append(items, x)` |
| Methods, field assignment, inheritance | Plain functions; construct a new class value |
| Dictionaries, tuples, comprehensions, lambdas | Typed classes/lists, loops, named functions |
| `elif`, `break`, `continue`, `pass` | Nested `if`/`else`, explicit loop conditions and returns |
| f-strings, `"a" + "b"`, `str(number)` | Follow available builtin signatures or use a Rust formatting adapter |
| Trailing commas or tabs | Omit trailing commas; indent with spaces |
| Arbitrary numeric casts | Current `i64` widening or `i32(...) -> Result[i32, Error]` narrowing; use an adapter for other conversions |
| `Option.unwrap()`, `Result.unwrap()` | Exhaustive `match` or `try` for Result |
| `Map` construction/lookup | No Map APIs; use another supported representation or a Rust adapter |
| `owned[T]` wrappers | Use `str`, `List[T]`, and other ordinary owned types directly |

See [syntax](../docs/en/syntax.md), [types](../docs/en/types.md), and [ownership](../docs/en/ownership.md) for exact forms. Evidence: [Result checks](../compiler/tests/result_discard.rs), [Result matching](../compiler/tests/result_match.rs), [Option matching](../compiler/tests/option_match.rs), [view origins](../compiler/tests/view_origins.rs), [async values](../compiler/tests/async_value_types.rs), [iteration borrows](../compiler/tests/iterator_borrows.rs), and [registered imports](../compiler/tests/stdlib_imports.rs). [View-rebinding execution tests](../compiler/tests/view_branch_rebinding.rs) cover temporary local borrows restored to an input view inside return-terminated blocks. The current repository adds [checked storage tests](../compiler/tests/view_flow_completion.rs) for List/Result/Option restoration, nested values, branches, loops, and async. [Cleanup tests](../compiler/tests/view_container_drop.rs) cover RHS failure, replacement Drop panic, and Future cancellation; [real scope tests](../compiler/tests/scope_runtime_contract.rs) cover lexical borrowing and nested error cleanup. Local views still cannot escape their owner. These unreleased changes require the current repository compiler; check [CHANGELOG](../CHANGELOG.md) before assuming they are in an installed release. Rust adapter types and trait bounds still require a successful `build` after `check`.
