# Migrating from Nagi 0.1.10 to 0.1.11

This guide describes migration to Nagi 0.1.11. Check the official release record to confirm availability; repository changes alone do not establish publication.

## Assigning an existing owned value to another local

Assigning an existing owned non-Copy value from one local to another now requires explicit `move`.

Before:

```nagi
destination = source
```

After:

```nagi
from std.ownership import move

destination = move(source)
```

The bare existing-value RHS is rejected in declarations with or without type annotations, in reassignments, and when parenthesized. This change applies only to assignment from an existing owned local to another local. Fresh values and Copy assignments remain unchanged. Existing consumption rules for arguments, returns, fields and indexes, try, and match also remain unchanged, so you do not need to add `move` to every argument or return.

After the move, `source` cannot be reused. Moving an owner while it is borrowed, or taking a non-Copy field from a borrowed/shared parent, is rejected. Use `view(source)` to read, `copy(view(source))` for an independent copy, or `share` and `clone_shared` for safe shared ownership. The compiler does not insert implicit clones or sharing. The standard `move` performs no clone or allocation. An unimported user function named `move` remains an ordinary function. High and Low follow the same rules.

## Task result handles and migrating a Supervisor monitor

The new spawn binding creates a result handle inside a scope. `await task` consumes the handle once and returns `Result[T, TaskFailure]` after the child actually finishes. If the child returns `Result[T, E]`, the received type is `Result[Result[T, E], TaskFailure]`. The inner business Err is not a TaskFailure and does not cancel siblings by itself.

When migrating a Supervisor monitor to a typed Task, explicitly pass its inner business Err through the existing parent-body Result/try path.

Before (legacy statement spawn):

```nagi
async with scope:
    spawn monitor(group)
    spawn serve_web(...)
```

After (propagate the monitor's inner Result to the parent):

```nagi
from std.task import message

async with scope:
    monitor_task = spawn monitor(group)
    spawn serve_web(...)
    received = await monitor_task
    match received:
        case Ok(inner):
            try inner
        case Err(failure):
            print(message(failure))
```

`try inner` makes the Supervisor's terminal Err the parent body Err. The scope then requests child cancellation and proceeds to actual join. If faults race, an HTTP fault observed first can remain primary; the monitor Error is not guaranteed to win every race. Inspect an outer TaskFailure with `std.task.kind`, which returns a Copy `TaskFailureKind`, or `std.task.message`, which returns a `view[str]` tied to that failure. There is no implicit TaskFailure-to-Error conversion. Displaying or handling it does not clear the scope fault.

The existing statement form `spawn work()` remains supported. In this legacy form, an Err from a child returning `Result[unit, Error]` remains a scope fault. Do not mechanically replace old Supervisor/HTTP monitoring with the binding form. Legacy HTTP spawn keeps its fail-on-Err behavior.

`discard(task)` returns unit and abandons receipt. It does not stop or detach the Task, suppress faults, skip actual join, or complete resource close. However, an inner business Err is not a scope fault by itself, so discarding a monitor without awaiting it loses that Err at the parent. In the verified example, HTTP continues returning 503 until an independent stop, and the scope exits normally. To stop HTTP after a monitor Err, await the Task and propagate the inner Err to the parent instead of discarding it.

Task is non-Copy, non-Clone, and non-shared; it cannot escape through another scope, an argument, return, field, container, wrapper, or another Task. Move transfers the handle and its receipt obligation together. Await or discard each Task before a normal binding/scope exit or loop continuation. There is no per-Task close or cancel operation. Synchronous parent Future Drop/unwind requests cancellation but cannot wait for actual join. Forced stopping of non-yielding work, rollback of external side effects, and universal recovery from arbitrary Rust Drop/panic payloads are not guaranteed. See the [Task guide](task-handles.md), [Task results example](../../test-nagi-code/library-examples/task-results/README.en.md), and [supervised service example](../../test-nagi-code/library-examples/supervised-service/README.en.md).

## Embedding the compiler from Rust

The Rust emitter now accepts `&checked::CheckedProgram` instead of `&Program`. Pass the final Low AST, native fragment, and source provenance to `check::finalize(primary, native, provenance)`, then emit from its result. `emit::rust_with_lines` accepts the same checked input.

Minimal example for an in-memory handwritten Low unit without a physical file:

```rust
use nagic::{ast::Program, check, emit, parser, source::SourceProvenance};

fn main() {
    let primary = parser::parse("fn main() { print(42); }\n", false).unwrap();
    let checked = check::finalize(
        primary,
        Program::default(),
        SourceProvenance::user_low_unmapped(),
    )
    .unwrap();
    let generated = emit::rust(&checked).unwrap();
    assert!(generated.contains("fn main"));
}
```

`CheckedProgram`'s fields and constructor are private; `.program()` is read-only. If you modify an AST, run finalize again after the changes. `check::check` or `integrate` alone does not produce a checked value for Rust emission. For file inputs, use loader-derived `Sources::provenance()`. High inputs retain the normal High→Low reparse→final-check pipeline; do not label High origins as `user_low_unmapped()`.

This example checks the emitted Rust string. It does not prove a native build of the generated Rust. rustc remains responsible for Rust adapter crate APIs and traits, final Send/Sync/Clone constraints, linking, targets, and dependency environments.

## Rust callers that inspect the AST or exhaustively match enums

`ast::Stmt` now has private checker facts, so external Rust crates can no longer construct it with a struct literal. Obtain ASTs through `parser::parse` or `source::load`, make any transformations, then finalize. Public `ast::S` gains a `SpawnBind` variant. `stdlib::StandardModule`, `stdlib::Resource`, and `stdlib::Operation` also gain Ownership/Task/Auth variants. Callers with exhaustive matches need to handle the new variants.

## Finding the executable in build scripts

Get the executable path from the `native:` line printed to standard error by `build` or `run`. Each successful build stores its executable under the generated directory's `.nagi/`, so rebuilding does not overwrite an older running executable. Do not assemble paths from fixed internal filenames, application IDs, or generation names.

`NAGI_NATIVE_TARGET_DIR` selects a shared dependency cache; it is not the successful generation's executable path. Scripts that looked in the cache or used the Cargo package name to locate the executable should use the path from `native:`. See the [project guide](projects.md).

## Other acceptance and diagnostic changes

Typed constant validation rejects proven zero divisors from compound expressions or supported scalar aliases, and signed MIN division/remainder by -1. It also checks proven constants in unreachable branches. Arbitrary function/extern/field/index results and profile-dependent overflow values are not propagated as proven constants.

Mapped Rust errors are concise by default. Use `build/run --rust-diagnostics` to include generated-Rust notes and suggestions. Native, dependency, and unmapped errors remain visible. See the [error handling guide](error-handling.md).

`std.auth.Principal` and `Grant[P]` are experimental APIs. A trusted Rust adapter creates proofs after authentication; Nagi cannot construct, JSON-decode, copy, or share them. The authentication example's fixed credential is not a production authenticator. Public SQLite Pool/Transaction and shared actor messages are outside this Task contract.

