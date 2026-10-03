# Development roadmap

The priority is to clarify the specification and strengthen safety while keeping the current working paths usable.

The latest source implements module aliases, custom class/enum errors, Result/Option/enum matching, `std.http.server`, and `std.actor` with arbitrary owned state. The new standard libraries are unreleased. See [actors](actor.md) and [Supervisors](supervisor.md) for their current scope.

1. Build on Result, Option, and enum matching to settle checked/wrapping arithmetic, string length, source spans, variable shadowing, and borrow origins.
2. Strengthen partial-move, branch, loop, and escape analysis in the High checker, reducing reliance on the Rust backend.
3. Build on registered standard modules and typed Rust integration to add user-defined generics and traits, general type annotations for passing and returning async functions, and a standard Map API.
4. Verify `std.actor` capacity, cancellation, and restart behavior, and define Supervisor trees and independent bounded queues. A VM, hot code replacement, and distributed actors are outside the current implementation.
5. Compare request arenas and borrowed classes, encoding during database stepping, buffer reuse, and streaming JSON.
6. Define typed SQL parameters, rows, and transactions, then add PostgreSQL using an existing Rust driver. Verify cancellation and pool shutdown against a real database.
7. Define Low layout, pointers, arenas, unsafe boundaries, and a C ABI; add sanitizers and coverage-guided fuzzing.
8. Introduce a backend independent of Rust code generation. Use measurements to decide whether to replace the scheduler as well.

## Self-hosting

| Stage | Work | Status |
|---|---|---|
| 0 | Low compiler in Rust | Small language subset implemented |
| 1 | High compiler in Rust | Translation to Low implemented |
| 2 | Rewrite the Low compiler in Low | Not started; needs String/Map/module/allocator API extensions |
| 3 | The Low compiler compiles itself | Not started; needs bootstrap comparison and determinism tests |

Practical use requires several stages of development. Passing prototype tests does not establish a finished production language/runtime or safety of every unsafe/FFI path. Accurate effort estimates depend on the specification and development team.

See the [working examples](library-examples.md) and [library design proposal](library-design.md) for reusable foundations and Rust assets. The proposal develops dependency settings, namespaces, resources, and database contracts while retaining existing code.
