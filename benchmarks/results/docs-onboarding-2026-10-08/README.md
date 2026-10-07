# Docs onboarding evidence

Evidence collected for draft [PR #98](https://github.com/disnana/Nagi/pull/98), based on `676576724829e45b077b58628bfe2417e6cf3673`. The last committed PR head recorded by the independent review is `2bae879a950e51ba9d3e38f4c515b0234f04b894`; review fixes are present as working-tree changes in the checkout. This record does not mark the PR or the latest-head verification complete.

## Result files

- [Native results for five inputs on High and saved Low](native10-results.json): both forms passed `check` and `build`, then ran `700`, `1000`, `1001`, `abc`, and `-1` (10 native executions total).
- [High and saved-Low harness summary](logs/onboarding-high-saved-low-success.log), [latest standalone harness run](logs/onboarding-harness-rerun.log), and [compiler build using an existing cache](logs/compiler-build-cache.log).
- [Boundary correction experiment](logs/boundary-regression-review.log): temporarily changed `<=` to `<`, observed `1000` return `over budget`, restored `<=`, and confirmed all three boundary values.
- [Nullable fixture check](logs/contribution-scratch-check.log), [targeted conformance test](logs/contribution-scratch-conformance-test.log), and [reviewed scratch diff](logs/contribution-scratch-diff.log). The fixture existed only in a temporary worktree, which has been removed.
- [Accepted/rejected language example checks](logs/check-fixtures.log), [site build](logs/website-build.log), and [Docs-only verifier output](logs/docs-only-verifier.log). The Actor handler shapes both type-check; the log does not claim that the restart behavior was executed.
- [Relative Markdown link scan](logs/relative-link-check.log) and [`git diff --check`](logs/diff-check.log).
- Independent reviews: [initial review](logs/independent-newcomer-review.md) and [follow-up review](logs/independent-newcomer-review-followup.md).

## Failures and limits

The first online tutorial run failed before app execution because Cargo could not reach the crates.io index; its raw output is preserved in [the network failure log](logs/tutorial-online-network-failure.log). That preliminary log resolves `nagi-runtime` from a sibling SQLite worktree, so it is not used as current-checkout source evidence. A later [run built from the Docs checkout](logs/tutorial-current-tree-run-700.log) succeeded, and the High/saved-Low harness covers all ten inputs. The initial project-mode harness invocation also failed because its temporary directory had no `nagi.toml`; see [that failure](logs/onboarding-project-mode-failure.log) and the successful standalone rerun above.

The Rust compiler and native build used pre-existing Cargo/native caches, so these are not clean-build results. `ci-routing-tests-prior-run.log` is an earlier recorded 59-test run read during review, not a rerun for the latest PR head. Latest-head four-OS CI remains **unverified / awaiting the parent’s check**. Windows/macOS manual commands, IDE and installer operations, actual fork push/PR flow, and every existing API sample were not tested here.

[Provenance](provenance.json) records source and evidence SHA-256 values, commands, cache usage, and the verified scope. No binaries, generated build trees, or dependency caches are included in this evidence directory.
