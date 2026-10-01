# Compiler internals

| Stage | Implementation |
|---|---|
| Lexer | Tokenizes line/column positions, indentation, strings, numbers, and operators |
| High parser | Indented blocks and a Pratt expression parser |
| High checker | Local inference, type consistency, moves, view origins, async, Result, and scopes |
| Lowering | Low text with inferred types and let declarations |
| Low parser | Independently parses braces and semicolons |
| Native integration | Adds ordinary functions and checks `@replace` signatures |
| Common IR | Uses the typed Low AST as the common representation |
| Code generation | Safe Rust and Serde, FromRow, and HTTP wrappers |
| Backend | Native code through rustc/Cargo; also checks borrows and Send |

The compiler is written in Rust. High and Low share a handwritten parser that switches between indentation and braces. Serde JSON is used for diagnostics, cost reports, and editor symbol information.

The AST retains original token ranges for expressions, arguments, and binding names. `symbols` returns types established by the usual checker rules alongside UTF-16 positions in the original files. Editor queries restore the variable environment after a failed statement and continue with subsequent statements; ordinary `check` stops at the first error. Completion information from invalid code does not mean it can be built.

Local names for definition navigation are resolved by walking the AST separately from type and ownership checks. The compiler records arguments, first assignments, for bindings, and case bindings; reassignment retains the first location. Child block names do not escape, and a for binding that reuses an outer name restores the original binding after the loop. Use and definition positions are emitted in `references`. Navigation can work after a move or a type error if the binding is identifiable. VS Code passes unsaved buffers, but F12 does not use stale positions when a query falls back to saved declarations.

Scopes generate a JoinSet wrapper. Classes generate native structs with typed JSON and database implementations. The common IR is currently neither SSA nor a separate optimizer. LLVM optimization comes from the Rust backend.

Each source file is limited to 2 MB. Expressions, types, and blocks also have nesting limits. Tests cover syntax mutations and invalid inputs. Coverage-guided fuzzing, incremental parsing, named modules, and precise source maps from Rust diagnostics back to High remain future work.
