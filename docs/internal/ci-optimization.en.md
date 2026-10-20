# CI decisions and measurements (2026-10-20)

This is a post-0.2.0 development change, separate from the frozen release. See [coverage levels](ci-levels.en.md) and [nextest evaluation](nextest-evaluation.en.md). Timeouts, assertions, seeds, features and release build profiles are unchanged.

## Baseline, not a speedup claim

[Run 39415520849](https://github.com/disnana/Nagi/actions/runs/39415520849), HEAD `467f8c9c791b4e98d1d33e15f8b9fc5515dafe0a`, provides one completed observation using GitHub job/step timestamps.

| Observation | Elapsed time |
|---|---:|
| First job start to last check completion | 3,463 s |
| Sum of 21 successful job intervals | 364.53 runner-minutes |
| Linux workspace cargo test | 1,492 s |
| CLI regression: Linux / Windows / ARM Mac / Intel Mac | 727 / 1,415 / 1,041 / 1,538 s |
| Intel package + regression jobs | 96.30 runner-minutes |
| Windows regression cache restore | About 3.53 GB, 160 s |
| Intel package cache save | 203 s |

Runner-minutes are unweighted job elapsed intervals, not billed minutes, OS price multipliers, model time or runner queue time. Pre-job delay can include dependencies and must not be called queue time. A Linux cache reservation collision was observed. These results do not establish a nextest speedup or a general SLA.

## Implemented selection and trust boundaries

After change/release planning, independent comprehensive Rust, platform package/regression, browser and IDE jobs run on separate runners. Existing native/socket fixture isolation, port ownership and Cargo locks remain; heavy commands are not indiscriminately launched together on one runner. L1 is limited to reviewed front-end paths; runtime/security/control-plane and unknown paths escalate. Main/weekly use three platforms; explicit RC and release use all four including Intel. This saves selected jobs, not time within each retained test.

Ready verifies every selected job. Before publication, an independent executable gate rechecks job results, matrix, release/package consistency and the exact workflow SHA. PR/manual RC/tag events cannot publish. Existing version/CHANGELOG release planning remains; no configured external approval environment is assumed. Later main commits do not cancel a prior release run.

External Actions use reviewed commit SHAs; checkouts do not retain credentials. Diagnostic artifacts default to 7 days and release candidates to 14 days; existing explicit retention stays. No secrets, private keys or extra user data are added to caches.

Extra historical BrowserLogin and editor-stage observers are manual only. Required runtime and actual-browser coverage remains in ordinary CI. Editor-stage remains bound to its historical source bundle, not a general benchmark for the current compiler; source mismatches still refuse execution.

## Cache and Rust alternatives

| Option | Decision |
|---|---|
| Existing actions/cache | Separate restore/save. Keep heavy restore keys/paths; save only on a successful main push after all ordinary steps. Reduce PR uploads/reservation collisions; restoration cost remains to measure |
| L1 Rust build/dependency cache | Successful PR-local saves under a separate nagi-critical-v1 namespace because L1 does not run on main. GitHub PR ref isolation applies; publication producers never consume this namespace |
| Swatinem/rust-cache | Its default workspace-crate exclusion and retention differ from generated native-target/cfg builds. Replacement cost and cache savings are unmeasured; not adopted |
| sccache | Rust incremental/linking restrictions and transfer overhead require measurement. Not installed or claimed effective in this patch |
| cargo-nextest | Opt-in checksum-verified evaluation of two existing compiler targets with exact inventories/ignored states and separate build/list/counterbalanced warm timing. No migration of libtest-dependent diagnostics or full socket/native suites; doctests stay with Cargo |

Keep existing disabled dev/test incremental and release opt-level=3, lto=false, codegen-units=1, panic=unwind. Do not change debug information or linkers without measurements. Check, build and test serve distinct type-check, distribution and execution purposes. Different RUSTFLAGS diagnostic builds are not interchangeable artifacts.

## Python

All seven existing workflows were inspected. Only Linux tests and website install pip dependencies; both already use requirements-keyed pip caches. Other Python work is mainly standard-library tests, diagnostics and distribution checks. Available predecessor run `39358061068`, HEAD `1229d412d535c3bb4b87ae27b3959fc01cec2eff`, measured setup-python at 0–1 s and pip installation at 3 s. The 277 s build_examples step largely runs native compilation.

No uv or pytest-xdist is added without evidence of a benefit. Parallel socket/native safety is not proven merely by changing Python runners. Tests use ranged requirements; website pins direct dependencies, not a fully hashed transitive lock. Caches do not lock dependencies.

## Validation and remaining measurements

From the repository root run `python -B -m unittest discover -s scripts/ci -p 'test_*.py'` and `python -B -m unittest discover -s scripts/ci -p 'test_nextest_evaluation.py'`. Existing source/mutation guards remain required. Reviewed wiring reverses exactly to inherited commands; mutated pins/matrices/publication policy and command removal must be rejected. Historical expected digests are not replaced.

Validate L2 at the final PR HEAD, then opt-in nextest after release CI finishes. A manual L3 RC verifies all four platforms without publication. Update after-change wall/job duration, unweighted runner-minutes, cache hit/miss, transfer and restore/save time using that exact HEAD. Speedup percentages, nextest runtime gains and billing savings remain unverified until those runs complete.

Sources checked 2026-10-20: [GitHub cache scope](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching), [cache restore/save](https://github.com/actions/cache/tree/0057852bfaa89a56745cba8c7296529d2fc39830), [rust-cache](https://github.com/Swatinem/rust-cache), [sccache](https://github.com/mozilla/sccache), [uv GitHub guide](https://docs.astral.sh/uv/guides/integration/github/), [nextest](https://nexte.st/docs/).
