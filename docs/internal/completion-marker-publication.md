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

## #150 Windows follow-up (base `a529fab808e4dd4480ad0fc352981855e0b79e2d`)

CI run `39903958996`, Windows regression job `120251688291`, reported the new outer wrapper test failing while 68 other tests passed. The generic `success(output)` assertion printed only stderr; the failing nested Rust test's stdout was discarded when the fixture was dropped. The exact inner failure is therefore **unknown**. In particular, this record does not prove that the failure was the preexisting temporary-directory case. The parent also inspected artifact `12481165760` (eight files) and reported no completion/wrapper/generation evidence, so the discarded inner output could not be recovered there.

Two narrow corrections were approved and implemented, without changing the generic assertion helper or product code:

1. Before `create_new(done.tmp)`, apply the same `symlink_metadata` absence check already used for `done`. A known existing temporary file/directory/symlink now returns the helper's explicit `AlreadyExists` independently of the platform's open-error mapping. `create_new` is still required to reject a path created after that check. Permission/unknown metadata or open errors are propagated unchanged; no broad error conversion or error-kind assertion relaxation is used. The second `done` check, close-before-rename, exact marker values, original deadlines, and every conflict assertion remain.
2. Only the outer embedded-test invocation uses a diagnostic containing exit status, stdout, and stderr with the same success condition. A native regression executes the compiled nested test with its private root variable absent. That child panics before acquiring any directory or starting a publisher, exits unsuccessfully, and the Rust test harness records its panic on stdout with empty stderr. The old stderr-only formatter loses this message (deterministic RED); the new formatter retains its panic, test identity, and exit status (GREEN). The expected failure is checked privately by the parent test and is not printed as a failing CI run. Original normal nested tests still must both pass.

This normalizes a nonportable fixture boundary and fixes the proven diagnostic loss; it does **not** claim the unavailable original Windows inner failure has been identified or that Windows acceptance is complete. The exact revised Windows CI remains mandatory, and any new failure must be judged from its now-visible inner diagnostics without relaxing assertions or blindly rerunning.

Follow-up evidence is retained in `local-only/completion-marker/windows-followup/`, separate from the original RED/GREEN and failure logs. A fresh exclusive Cargo target is used because the previous audit target was later pruned; the retained original compiler and all previous evidence are untouched. Final validation and freeze hashes follow below.

Follow-up validation: the stderr-only diagnostic replay failed exactly with `nested failure stdout was discarded` after the native child had already been confirmed unsuccessful, stdout-containing-panic, and stderr-empty. The corrected outer test passed (1 selected / 73 filtered, 0.17 seconds), still executing both normal embedded tests and the expected-failure diagnostic probe. Final `cargo test --locked -p nagic --test build_generations` passed **all 74 Linux tests** with no ignored tests, 58.05 seconds test time / 58.551 seconds command time. Targeted clippy with `-D warnings` passed (6.003 seconds), as did fmt and diff checks. The new target's >=2 GiB workspace / >=1 GiB temporary-space guard never fired.

Only `compiler/tests/build_generations.rs` and this document are changed from the follow-up base. No product, workflow, generic helper, deadline, platform skip, or existing error-kind expectation changed. Source and evidence hashes are frozen in the follow-up `evidence-manifest.json`; the dedicated target is retained for the next independent High audit. No processes remain running and no commit/push was performed by the implementation worker. **Windows acceptance and the original unknown inner cause remain pending exact revised CI.**
