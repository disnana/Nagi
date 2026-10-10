# Internal improvement to retained disk source content

[日本語](compiler-source-read-retention.md) · [pipeline](compiler-pipeline.md)

2026-10-10 UTC. Source-only branch `fix/compiler-source-read-retention-pr` is based on main `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`. This introduces no public limit or language rule. SF05 `47dfc09` and the existing Task/SQLite/IDE worktrees are unchanged.
Results and open checks are summarized below. Detailed original evidence is retained locally in the frozen validation record and is excluded from this source-only branch. Source patch SHA-256 values and validation scope are listed below.

## Behavior and preserved boundaries

Previously the loader retained an entire disk file as UTF-8 before checking the 8,000,000-byte load limit and 2,000,000-byte file limit. The private reader now drains to EOF in 8KiB chunks, validates with the standard UTF-8 validator and retains at most the existing 2MB file limit. Excess content contributes to the byte count and validation but is not retained as source text. An incomplete UTF-8 suffix occupies at most three bytes. Buffer reservation uses the standard fallible API.

Completed-read I/O/UTF-8 errors still precede the aggregate 8MB limit, followed by the file 2MB/parser check. The reader drains after overflow or malformed UTF-8 so a later I/O error retains priority, and retries Interrupted. Existing diagnostic text, paths, parent import locations and successful source content are preserved.

Ordinary High, saved Low and handwritten Low keep the existing input limits. Private generated Low keeps its 64MB path; reopening saved Low is ordinary input. Native Low still uses separate loads and the existing append path, with no new compilation-wide aggregate cap. Import depth64/files128/cycle/duplicate/location behavior, overlays, include_text, manifests and external Rust paths remain unchanged. Only existing parser constants/messages become crate-private shared details; no public API or dependency is added.

This bounds retained disk source content, not read volume, work/time, OS read waits, total allocation or RSS. Successful source copies/ASTs, allocator rounding, changing files, non-terminating special files and universal physical-OOM recovery remain outside the guarantee.

## Evidence and limits

The original reviewed source patch SHA-256 is `d57fefc492fcd3e204d31be8ef13c5e829f68bd605139bd1f45a6379e8711fc5`. All-target clippy then identified test-module placement. Moving the identical test block to the end of source.rs preserves production-item and test-body bytes; the revised source patch is `47e679b5a7b9aaebb71941376e44c72720169ee6e8917295a79de89f1f9113bd`. Original patch/review/failures are preserved separately from revised results.

The original small retention-oracle RED had two failures and one pass. Its source snapshot was not saved beforehand and was reconstructed from the original edit afterwards. The reconstructed binary matches the preserved original byte for byte and exits101; this is explicitly post-hoc reconstruction.

Linux/Rust1.99 results: final reader/source unit7/7; new source integration3/3; compiler library135/135; existing frontend_contracts13/13 and modules32/32, both unfiltered. The latter harnesses contain nine actual native CLI runs across five test functions, covering High/generated saved Low/handwritten Low and the Low replacement/Rust JSON/SQLite bridge. Forty test functions do not run native programs. Run may build or reuse a generated binary; rebuild counts are not inferred. Child output is asserted inside the tests, while saved logs contain harness output. `check --native` is not counted as execution.

Limited compiler-lib/new-test clippy and changed-file formatting passed. Independent Sol High review READ-BUDGET-R01 found no required revisions, checked frozen source/artifacts/RED reconstruction, reran unit7/integration3 and added three finite differential checks. Existing 2/8MB fixtures ran; no new giant-input, load or resource-exhaustion experiment was added.

Revised workspace fmt and all-target clippy passed. Independent READ-BUDGET-R02 approved the unchanged test-block relocation and checked revised raw logs. The first workspace test stopped at the runtime library with191 pass/40 fail/1 ignored because the environment denied loopback binds; later targets were not reached. With revised source and loopback permission, runtime had230 pass/1 fail/1 ignored: the existing `oversized_body_declared_length_rejects_headers_and_incomplete_upload_promptly` failed with ConnectionReset while reading its response. Runtime source is unchanged from main; this does not establish a reader-induced failure. Original assertions, parallelism and the failure are preserved.

Exactly one `--no-fail-fast` continuation covered the remaining targets and exited0:98 Cargo-launched targets,1004 pass/0 fail/1 existing ignored/filter0. One nested graph_render image_child block contributes a separate1 pass/8 filtered, giving99 raw result occurrences/1005 passes including that child. The later runtime block passed, but the original ConnectionReset remains unresolved. No individual rerun, skip or relaxed oracle was used. Independent R02 addendum checked all98 launches/results and raw hashes.

Existing fuzz-smoke also passed with default seed305419896:1000 mutations (731 parse rejections,192 checker rejections,77 checked pipelines),16 bounded native cases,0 panics. It is not coverage-guided fuzzing.

A subsequent CI configuration adds `--test source_read_retention` to the existing four-OS package compiler checks. The existing `--lib` runs the reader units, and Linux workspace tests discover the integration harness. This is registration, not a remote CI run or four-OS success.

A successful command covered all targets, but this reader patch has not resolved the initial HTTP failure and full regression acceptance remains unmet. Four-OS CI, the real private64MB boundary, perf/RSS, allocation failure and changing files remain unverified. Review does not replace current CI acceptance. The initial HTTP failure occurred in the unchanged main runtime. PR #105 adds observations and diagnostics; PR #104 and #106 are validating a common connection shutdown fix. Keep the reader patch separate and check each PR's source head and latest CI independently. This is not completion of SF07 DoS protection.
