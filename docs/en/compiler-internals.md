# Compiler internals

| Stage | Implementation |
|---|---|
| Lexer | Tokenizes line/column positions, indentation, strings, numbers, and operators |
| High parser | Indented blocks and a Pratt expression parser |
| High checker | Local inference, type consistency, moves, view origins, async, Result, and scopes |
| Lowering | Low text with inferred types and let declarations |
| Low parser / checker | Reparses braces and semicolons, then checks types and ownership again |
| Native integration | Adds ordinary functions and checks `@replace` signatures |
| Common IR | Uses the Program AST shared by High and Low |
| Code generation | Rust and Serde, FromRow, and HTTP wrappers for supported types |
| Backend | Native code through rustc/Cargo; also checks borrows and Send |

The compiler is written in Rust. Nagi handles parsing, type checking, and Rust code generation. Cargo/rustc builds dependencies, performs the final borrow and trait checks, and generates machine code. There is no independent machine-code backend or VM.

High and Low share a handwritten parser and Program AST, switching between indentation and braces. The usual High path checks High, emits Low text, reparses it, integrates handwritten Low, and checks the result again. Low does not introduce a separate memory model.

Low retains type annotations, control flow, and module and definition IDs. Inferred expression types, name resolution, and move/borrow checking state are reconstructed after parsing, rather than carried forward as proofs. JSON module metadata preserves alias and type identity. Serde JSON is also used for diagnostics, cost reports, and editor symbol information.

The AST retains original token ranges for expressions, arguments, and binding names. `symbols` returns types established by the usual checker rules alongside UTF-16 positions in the original files. Editor queries restore the variable environment after a failed statement and continue with subsequent statements; ordinary `check` stops at the first error. Completion information from invalid code does not mean it can be built.

Local names for definition navigation are resolved by walking the AST separately from type and ownership checks. The compiler records arguments, first assignments, for bindings, and case bindings; reassignment retains the first location. Child block names do not escape, and a for binding that reuses an outer name restores the original binding after the loop. Use and definition positions are emitted in `references`. Navigation can work after a move or a type error if the binding is identifiable. VS Code passes unsaved buffers, but F12 does not use stale positions when a query falls back to saved declarations.

Scopes generate a wrapper around Tokio's JoinSet. Classes generate Rust structs, with JSON and database implementations when their field types support them. Low is a compile-time common representation, not a runtime VM. The common IR is currently neither SSA nor a separate optimizer. Optimization comes from the Rust backend.

Within one compilation, lowering and code generation retain a mapping from generated lines to the original statement, definition, or field line. During a build, the compiler reads Cargo's JSON diagnostics and first shows the corresponding Nagi or Low file and line. Rust's notes and edit suggestions retain generated Rust coordinates. Handwritten Rust and unmapped diagnostics are not rewritten.

Saved `generated.low` comes from High before handwritten Low is integrated. It is not a dump of the entire program executed after replacement. Use `map` for integrated function relationships and generated Rust for the final output.

When a saved `generated.low` is loaded by a separate command, diagnostics use Low line numbers. Module IDs survive, but the source map back to High is not saved.

`check` reports diagnostics using Nagi's rules and source locations without invoking Cargo. It does not replace Rust's borrow and trait checks, so a successful check can still be followed by a failed build. Rust-side borrow and Send requirements, handwritten Rust bodies, and crate APIs are checked during the build. See [ownership](ownership.md#borrowing-and-the-limits-of-checking).

[SQL checks](sql-check.md) are enabled only when `check` is given a schema and dialect. Ordinary `check` does not validate SQL contents against a schema. Inspected application queries are not executed; generated Rust and application database operations are unchanged.

Each source file is limited to 2 MB. Expressions, types, and blocks also have nesting limits. Tests cover syntax mutations and invalid inputs. Coverage-guided fuzzing, incremental parsing, precise expression columns, and mappings for every Rust diagnostic are not supported. Unquoted imports of registered standard modules [are available](modules-and-rust.md).

See the [parser](../../compiler/src/parser.rs), [checker](../../compiler/src/check.rs), and [code generator](../../compiler/src/emit.rs). The [ownership boundary tests](../../compiler/tests/ownership_boundaries.rs) check High and reparsed Low, including identical generated Rust for selected examples. The [standard import tests](../../compiler/tests/stdlib_imports.rs) check definition IDs in saved Low. These tests do not establish equivalence for every program.
