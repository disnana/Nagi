# Docs onboarding handoff — PR #98

PR #98 is currently a draft against `main`, based on `676576724829e45b077b58628bfe2417e6cf3673`. The independent review recorded committed head `2bae879a950e51ba9d3e38f4c515b0234f04b894`; the follow-up fixes described below are working-tree changes at the time of this handoff. This is a verification record, not a completion claim for the PR or its latest-head CI.

The public entry points lead to the bilingual [first CLI app](../../first-app.md) and [first contribution guide](../../contributing.md). The [Docs onboarding audit](../docs-onboarding-audit.md) records existing coverage, overlap, translation gaps, and per-page verification scope. Its evidence bundle is in [benchmarks/results/docs-onboarding-2026-10-08](../../../benchmarks/results/docs-onboarding-2026-10-08/README.md).

The docs add a small budget-checking CLI walkthrough with input, typed failure, boundary testing, and a correction; a concrete contribution path from setup and repository map through a nullable conformance fixture, test, diff review, fork push, and main-targeted PR; and a Japanese/English feature index. Ownership, nullable/Result, async/Task, Actor/Supervisor, and class references include beginner mistakes and corrections. The website has matching entry links. No compiler/runtime source was changed, and the database pages and SQLite internals remain outside this docs change.

## Verification recorded

| Check | Result and evidence |
|---|---|
| Compiler build | `cargo build --locked -p nagic` succeeded using an existing Cargo target cache; see [build log](../../../benchmarks/results/docs-onboarding-2026-10-08/logs/compiler-build-cache.log). This is not a clean build. |
| First-app source | Japanese and English code blocks match `examples/tutorial/first_app.nagi`. High and standalone saved Low each passed `check` and `build`, then passed five native inputs (10 executions); see [results](../../../benchmarks/results/docs-onboarding-2026-10-08/native10-results.json) and [latest harness run](../../../benchmarks/results/docs-onboarding-2026-10-08/logs/onboarding-harness-rerun.log). Existing Cargo/native caches were reused. |
| Boundary correction | In a scratch copy, `<=` was changed to `<`; input `1000` produced `over budget`. Restoring `<=` made `700`, `1000`, and `1001` match the expected table; see [log](../../../benchmarks/results/docs-onboarding-2026-10-08/logs/boundary-regression-review.log). |
| Error and language examples | Type-invalid forms were rejected and corrected examples were accepted by `nagic check`; both Actor handler shapes type-check, but their restart behavior was not run. See [fixture log](../../../benchmarks/results/docs-onboarding-2026-10-08/logs/check-fixtures.log). |
| Contribution example | A scratch worktree added `nullable_default.nagi` and its corpus case. Source check and `cargo test --locked -p nagic --test conformance corpus_and_bounded_generated_contracts_reach_native_execution -- --exact` passed (1 test); reviewed diff is preserved in the evidence bundle. The scratch worktree was removed without a commit. |
| Docs-only drift check | `python3 scripts/verify_onboarding_examples.py --docs-only` confirmed the Japanese/English tutorial blocks match the native-tested example; see [output](../../../benchmarks/results/docs-onboarding-2026-10-08/logs/docs-only-verifier.log). |
| Website and links | The site generated 96 pages and verified local routes, links, anchors, and assets; [build log](../../../benchmarks/results/docs-onboarding-2026-10-08/logs/website-build.log). `git diff --check` and a separate relative-link scan for the audit, handoff, and evidence index passed. |
| Independent review | The first review raised five findings. The [follow-up review](../../../benchmarks/results/docs-onboarding-2026-10-08/logs/independent-newcomer-review-followup.md) reports all five resolved: reproducible boundary error, Docs-only code drift check, runtime-error wording, internal-only audit visibility, and contribution-intro consistency. |

The first online tutorial build stopped before execution because Cargo could not reach crates.io. Its log also resolves `nagi-runtime` from a sibling SQLite worktree, so it is retained only as an infrastructure record, not current-checkout validation. See [network failure](../../../benchmarks/results/docs-onboarding-2026-10-08/logs/tutorial-online-network-failure.log) and the later [current-checkout run](../../../benchmarks/results/docs-onboarding-2026-10-08/logs/tutorial-current-tree-run-700.log). An initial project-mode verifier invocation also lacked a temporary `nagi.toml`; the standalone-file rerun succeeded. Both records are preserved in the evidence bundle.

## Not verified here

Latest-head four-OS CI is **unverified and awaiting the parent’s check**. Windows/macOS manual commands, IDE/installer operations, an actual fork push and PR flow, and all existing API sample programs were not exercised. The earlier 59-test CI-routing log is included only as prior evidence and was not rerun for this latest working diff. Do not treat these records as a release or merge report.
