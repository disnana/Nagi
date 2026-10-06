# Continued development

The immediate goal is to expand the range of common backends that can be written in readable High, with consistent rules for types, ownership, failures, and resource cleanup. Rust native code generation and existing libraries remain the foundation.

This page describes the priorities for future development. Release dates remain undecided. See the [reference](README.md) for current APIs and [CHANGELOG](../../CHANGELOG.md) for changes by version.

## Priorities

1. **Make existing language rules consistent.** Review `owned`, `view`, moves, branch and loop checks, arithmetic failures, and source mapping. Track check-success/build-failure cases with reproductions and distinguish Nagi diagnostics from checks delegated to Rust.
2. **Define native resource boundaries.** Specify opaque type identity, borrowing, sharing, asynchronous cleanup, and work that remains after cancellation. Evaluate general traits and new syntax only when concrete APIs establish a need.
3. **Organize the standard library.** Provide HTTP, JSON, and database APIs through modules and separate unused runtime dependencies. Generated Serde and row conversions and exported types also need support; making Cargo dependencies optional is insufficient.
4. **Generalize database access.** Use separate modules and resource types for SQLite and PostgreSQL while aligning parameter, row, and error rules. The initial SQLite Pool/Tx API and cleanup policy have been adopted. A private prototype has tested capacity, cancellation, and shutdown with two connections, but the public API remains unimplemented. Pool/Tx is not yet available through the standard library. Nagi 0.1.10's [explicit SQL/schema checks](sql-check.md) cover SQLite names, result columns, and bind counts, but not value types, NULL behavior, or other databases.
5. **Compare HTTP foundations.** Evaluate Axum/Tower first against the current implementation with matching APIs, limits, failures, and shutdown conditions. Compare normal load, overload, sustained operation, and maintenance costs before deciding on adoption. Replacement is undecided.
6. **Validate boundaries through examples and documentation.** Show Nagi and Rust checks, main/release differences, and measurement conditions. Evaluate diagnostics and usability through implementations of the same tasks.

See the [library and Rust integration proposal](library-design.md). [ADR 010](../internal/adr/010-sqlite-transaction-boundary.md) records the adopted initial SQLite contract. API names and cleanup contracts for other resources still have unresolved details.

The narrow migration requiring explicit move for assignment of an existing owned value, spawn result handles, and business Err/task fault separation are implemented on the working branch and remain unreleased. S2 Supervisor/HTTP migration and conditional shared actor messages remain later design work. See [DESIGN](../../DESIGN.en.md) and [ADR 011](../internal/adr/011-language-behavior-and-docs.md) for differences from published releases, unresolved details, and migration conditions.

Compiler verification combines known pass/fail examples with bounded generated programs through High, saved Low, and Rust, plus mutation tests. Keep tested cases separate from unsupported combinations and preserve accepted-then-rejected cases as regressions.

## Low scope

Preserve existing Low code, brace syntax, inspection of generated output, and function replacement. Prioritize High and Rust integration. Plans to expand Low into an independent systems language with pointers, layout, unsafe syntax, C ABI, and SIMD are paused.

See [High and Low](low-language.md) for these limits and current usage.

## self-hosting

An independent backend, self-hosting, a custom VM or scheduler, hot code replacement, and distributed actors have not been started. Whether to pursue them remains undecided.

Preserving semantics with another backend requires contracts and implementations for types, ownership, cleanup, failures, asynchronous work, and runtime integration. Rewriting the compiler in Low is not a requirement for the current development stage.

S1 Task result handles are connected on the working branch and remain unreleased. See [usage](task-handles.md) and [validation status](../internal/task-handles-s1-results.md). S2 Supervisor/HTTP migration and public Pool/Tx remain later work.
