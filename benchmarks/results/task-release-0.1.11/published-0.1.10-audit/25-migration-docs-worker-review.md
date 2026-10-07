# Nagi 0.1.11 migration-docs worker review

## Scope completed

Created the Japanese and English migration guides at `docs/migration-0.1.11.md` and `docs/en/migration-0.1.11.md`. The guides cover the narrow mandatory move change, Task result nesting and the S2 Supervisor-monitor migration, legacy statement-spawn behavior, sticky faults and discard limits, synchronous Drop/join limits, checked Rust emission, parser-based AST access and exhaustive enum matches, executable lookup through `native:`, the shared dependency-cache distinction, and the audited constant/diagnostic/experimental-auth changes. Both guides state that 0.1.11 has not been published.

Created `/tmp/nagi-release-task-audit/24-release-notes-final-draft.md` from the supplied 21 draft without replacing it. The new draft includes the S2 migration and weak-discard counterexample. It records the supplied local evidence (three source forms by seven native cases and both public examples in three forms), while stating that four-OS CI and main merge remain incomplete and the release is unpublished.

No existing public docs, CHANGELOG, version files, Task sources, or CI files were edited as part of this assignment.

## Evidence checked

The Rust example uses the APIs present in the working tree: `check::finalize` at `compiler/src/check.rs:4370`, `emit::rust(&CheckedProgram)` at `compiler/src/emit.rs:1161`, `emit::rust_with_lines` at `compiler/src/emit.rs:1185`, `SourceProvenance::user_low_unmapped()` at `compiler/src/source.rs:59`, and loader provenance at `compiler/src/source.rs:147`. `CheckedProgram` has private fields and a read-only `program()` accessor in `compiler/src/check/checked.rs`. The guide explicitly says its string assertion is not a native-build result.

AST and enum notes were checked against `compiler/src/ast.rs` and `compiler/src/stdlib.rs`. The monitor migration and limits follow `docs/internal/task-handles-s2-results.md`, including the parent `try inner` path, retained legacy HTTP spawn, non-primary guarantee under racing faults, and the discarded-monitor example where HTTP continues until independent stop.

## Checks

- `/tmp/nagi-site-venv/bin/python /tmp/nagi-s1-final-review/verify_markdown.py` passed for the repository: 217 Markdown files, 1,975 local links, zero errors; external links were not fetched.
- A focused check passed for both new guides: balanced code fences, no trailing whitespace, existing relative-link targets, and equal Japanese/English section counts.
- No code tests were run for this documentation-only change.

## Remaining release gates

Four-OS S2 CI, final-head independent readback, merge to main, and publication were not verified in this task. The migration guides and release-notes draft keep the unpublished status explicit. The root agent should perform the requested independent readback and continue the release gates separately.
