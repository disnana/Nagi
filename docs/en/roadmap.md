# Development roadmap

The priority is to clarify the specification and strengthen safety while keeping the current working paths usable.

1. Build on Result matching to settle nullable matching, checked/wrapping arithmetic, string length, source spans, variable shadowing, and borrow origins.
2. Strengthen partial-move, branch, loop, and escape analysis in the High checker, reducing reliance on the Rust backend.
3. Build on relative file imports and typed Rust integration to add named modules and aliases, general generics, traits, function/async function types, and a standard Map API.
4. Lower arbitrary-state actor declarations, supervisor trees, and bounded queue declarations into the current runtime.
5. Compare request arenas and borrowed classes, encoding during database stepping, buffer reuse, and streaming JSON.
6. Implement PostgreSQL's binary protocol, general typed SQL bindings, schema validation, transactions, and cancellation.
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
