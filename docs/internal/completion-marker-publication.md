# Cargo-wrapper completion marker publication

2026-10-23; base main `9fe5e079eb23ed4aef515fabd19ed86dc67958f7`.

Sol 6.1 High performed the scoped design, implementation, and validation after parent approval. A separate High will audit the frozen diff. Model-only time, token usage, and cost were not measured. All task history is kept here to avoid overlapping the plugin PR’s global progress/routing edits.

## Approved test-only design

Main CI run `39890332445`, macOS ARM job `120208428025`, observed an empty `orphan/done` where the existing test requires exactly `0`. The wrapper's `fs::write` creates/truncates the pathname before writing its bytes; the reader waits only for existence. The same fixture writes `42` on forced failure and `timeout` after killing and waiting for its direct Cargo child. This is a fixture publication race; it is not evidence of a compiler/runtime generation failure.

All three wrapper branches will share a completion publisher. A same-directory `done.tmp` is opened with `create_new`, completely written with `write_all`, closed, then renamed to `done`. Existing `done` is rejected using `symlink_metadata`, before creating the temporary file and immediately before publication; existing files, directories, dangling symlinks, and temporary paths fail closed. Cooperating publishers must acquire the same exclusive temporary pathname, in the fixture-owned directory. This does not claim protection against an unrelated noncooperating writer racing the final check/rename. The marker has no crash-durability contract, so no fsync or product storage policy is added.

The original exact values, deadlines, child polling/kill/wait, immutable-source/native-executable assertions, and test parallelism remain. Production compiler/runtime code is outside this change.

## Required proof

A finite, test-only gate pauses the publisher immediately after creation and again after close, before publication. Observations are saved, both gates released, the publisher joined, and its owned directory cleaned before assertions. The old create-before-write behavior must expose empty `done` (RED). The new publisher must keep `done` absent at both gates, retain the exact complete temporary bytes after close, then publish exact `0`, `42`, and `timeout` (GREEN). Conflict tests retain existing bytes and reject temporary files/directories and a destination created at the prepublication gate. Unix additionally exercises a dangling symlink.

The embedded std-only wrapper tests execute the same helper that all three real branches call. Existing native generation tests continue to exercise real success/failure and the orphan path; the timeout branch's 45-second child ownership logic is unchanged rather than shortened for a new test. Validation and independent audit results will be appended after execution. A successful rerun of an unfixed head would not demonstrate this repair.

## Executed validation

Evidence is retained privately under `local-only/completion-marker/` in the original workspace (excluded from Git).

- `red-wrapper.rs` instruments the former `fs::write` operations (`File::create` then `write_all`) at the actual creation boundary. The finite native gate observed `Some("")` instead of absence. Both releases, thread join, and owned signal-directory cleanup completed before the assertion failed; `red.json` records exit 101 and zero leftover children in that directory. This deterministic instrumented replay is separate from the original macOS CI failure log.
- `green-wrapper.rs` executes the final shared publisher. Both embedded native tests passed with normal test parallelism: all exact `0`/`42`/`timeout` values, prepublication absence, complete closed temporary bytes, final temporary removal, and the existing-path conflicts above. `green.json` records exit 0 and zero leftover children. The original three call sites, 45-second limit, direct-child kill/wait, and exit values are also checked in `branch-source-proof.txt`.
- `cargo test --locked -p nagic --test build_generations`: **74 passed, zero failed/ignored** (the base has 73 tests, plus one outer test that compiles/runs the two embedded wrapper tests). The full suite includes the original orphan Cargo test and all old native/source/cache/old-executable assertions. Test execution took 58.30 seconds; the command including the fresh compiler build took 76.566 seconds. The command used normal test parallelism and original deadlines.
- `cargo clippy --locked -p nagic --test build_generations -- -D warnings`: passed, 6.503 seconds overall. `cargo fmt --all -- --check` and `git diff --check`: passed.

A new exclusive Cargo target was created for this task; it did not reuse a different checkout's compiler target. Cargo registry state was the existing offline toolchain cache. A guard checked workspace free space >=2 GiB and temporary space >=1 GiB while Cargo ran; it never triggered. The small dedicated compiler/test target is retained for the independent reviewer, alongside raw logs, replay sources/binaries, hashes, and the original failure evidence. No shared target, existing CLI, old application executable, or unrelated evidence was deleted. No Docker, child agent, commit, or push was used.

The implementation/test tree is frozen for a separate High's independent audit and exact-head CI, including macOS ARM. Local Linux success does not substitute for that OS gate. A one-time rerun of unchanged main, if performed separately, is diagnostic only and is not evidence that this fixture repair was present in that head.
