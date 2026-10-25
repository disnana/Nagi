# LTS candidate validation (2026-10-25)

[日本語](lts-candidate-validation.md) · [Proposed policy](lts-candidate.en.md)

Validated main `22183d7233950bdb575b62a72393dac9b4aa0767`, after [#160](https://github.com/disnana/Nagi/pull/160), tree `a7a2e3985847a1bbd2fca8be22b05f5c69018322`, identical to the final PR tree. Compiler SHA256: `49aa30fbf2b0eec6067755f1c1b72408e9a0d650d6862f2652b3fd4ecf887f9c`. Adding this record changes no implementation. No version bump, LTS designation or publication has occurred.

## L3 is separate from publication

Following the [existing coverage levels](ci-levels.en.md), executed `gh workflow run ci.yml --repo disnana/Nagi --ref main -f level=release`. A manual run does not meet publishing's `event=push` condition; versions and CHANGELOG are unchanged.

- [L3 run 40152653289](https://github.com/disnana/Nagi/actions/runs/40152653289) targets the main SHA above and runs the existing four-platform/MSRV, browser/TLS, install/upgrade/uninstall, distribution and Ready gates. Check the completed results in that run and the corresponding PR's validation summary. Dispatch success is not test success.
- [Main L2 run 40152458417](https://github.com/disnana/Nagi/actions/runs/40152458417) succeeded as a separate run. Neither that run nor #160's three-platform success substitutes for L3 macOS Intel coverage.

## Representative selection and compatibility

Linux x86_64, Rust/Cargo 1.99.1, dev/debug0/jobs2/offline, one owned shared target. Reused `verify_application_examples.py` check/build helpers, unchanged existing smoke assertions and receipt verification. Seven High examples also passed independently loaded saved Low; one handwritten Low example completed the set. Both full/auto configurations passed: 30 check/build/execution configurations, matching smoke facts across modes and High/Low. Every build's stats reported `checks=full` and `cargo_always_invoked=true`.

| Existing example | Configurations | Auto selection / reason | Smoke cases per configuration |
| --- | ---: | --- | ---: |
| Hello/Point | 4 | core / eligible_core | 1 |
| stock-report | 4 | full / unsupported_ir | 12 |
| device-settings | 4 | full / unsupported_ir | 12 |
| seat-reservations | 4 | full / unsupported_ir | 1 |
| supervised-worker | 4 | full / rust_adapter | 12 |
| file-json | 4 | full / rust_adapter | 4 |
| auth-boundary | 4 | full / rust_adapter | 52 |
| order-quote (handwritten Low) | 2 | full / unsupported_ir | 23 |

Existing assertions covered Unicode and 10,000-row JSON aggregation, SQLite NULL/BLOB/reopening, actor business errors/restart, task-panic cleanup, file adapters and positive/negative authorization cases. Case counts are each smoke's own units, not comparable measures of test strength. This adds no core support for HTTP, databases, authentication or async. Unsupported unused functions also select full. Do not generalize Point's speedup to applications generally.

See [runtime profiles](../en/runtime-profiles.md) for supported IR, fallback and cache invalidation. Complete type/ownership/Policy checks remain; Cargo failures are not hidden by retrying full. All 17 frozen compiler/runtime inputs and application input hashes were unchanged. Removed only the completed owned target, whose observed maximum was 711.2MiB; rechecked all 30 retained executable hashes and receipts.

## Separate speed observations

Compatibility replay totals were check **0.261s**, build **73.837s**, smoke **17.523s**. Builds warmed a shared target in fixed order, so no full/auto speedup ratio is calculated. Smoke includes HTTP waits and negative cases; it is not program execution throughput.

After building, ran only the same seat-reservations native scenario three times per mode: full median14.689ms (14.513–14.822), auto14.776ms (14.570–14.963). Both actually selected full. These include process startup and self-checking; overlapping ranges and small samples do not establish a performance ranking or steady-state throughput.

The earlier balanced full/core build comparison and default-release CPU probe are in [separate measurements](../../benchmarks/results/runtime-profiles-2026-10-24/README.md). Treat supported small-example dev builds, default full release execution and this fallback compatibility replay as separate evidence. No overall CI speedup or cost saving was measured.

With Node24.19.0 and the candidate compiler explicitly on PATH, existing VS Code `npm test` passed three times:196 passed,0 failed,0 skipped, across22 files each time. Wall times1.286/1.130/1.171s, median1.171s, range1.130–1.286s. Preserved initial missing-compiler ENOENT and ordinary-sandbox snippet EPERM results, then reran unchanged tests with appropriate execution permission. No assertion or timeout changed. This measures Node/compiler tests, not GUI responsiveness.

[Machine-readable measurements](../../benchmarks/results/lts-candidate-2026-10-25/summary.json) separate execution/build/smoke. No current before/after full-workspace test-time comparison was performed. #160's2,029 successful Rust tests and exact-source L3 provide correctness evidence; test removal or skipping is not presented as acceleration.

## Clarity and setup burden

Use the [published installation steps](../en/getting-started.md), then `nagic check --project .` and `nagic run --project . --profile dev`. Native execution still requires Rust/Cargo, platform build tools and runtime, including initial dependency download/compilation. Feature-detect and explicitly select auto on a candidate compiler; do not send the flag to published0.2.1.

Physical source lines, including blank/comments but excluding smoke code: stock-report Nagi71/Rust0; device-settings132/0; seat-reservations142/0; supervised-worker210/39; file-json22/16; auth-boundary188/160. These are not equivalent Python comparisons. Do not compare full authorization logic with Hello or omit Rust-adapter burden.

A minimal `x: i32 = "wrong"` example exited1 during check, identified original line2 and expected i32/actual str, and suggested explicit conversion. This is one diagnostic observation, not a promise of automatic fix suggestions for every error. Concision does not justify removing explicit type/ownership/authorization boundaries.

## Editor scope

| Area | VS Code | JetBrains |
| --- | --- | --- |
| Authoring | Compiler-backed semantic completion/hover/signatures/definitions, snippets, High/Low syntax | Compiler-backed completion/docs/parameter info, exact-target navigation, High/Low colors |
| Diagnostics | On-save and manual check; default15s timeout is not measured latency | Background snapshot analysis and manual Check; stale type/diagnostic facts are not reused for new snapshots |
| Execution | Check/lower/build/run tasks and workspace trust | Standard Run/Stop, save-before-run, project trust, default dev/explicit release, compiler-capability flag detection |
| Tests | No dedicated Nagi test explorer; existing application harnesses/CI | No dedicated Nagi test runner; Check/Run and external harnesses |

Current JetBrains source exposes dev/release selection; standard VS Code tasks use the CLI's default profile and have no dedicated dev setting. Neither plugin exposes a dedicated `--runtime-profile auto` selector. These core-selection measurements therefore do not describe default IDE Run speed. Explicitly use a capable compiler's CLI to try auto. Dedicated selectors and test explorers remain future proposals, not candidate features.

This Node run is not VS Code GUI acceptance. JetBrains0.1.6 has been published on GitHub and accepted by the user, but was not newly GUI-tested here. Rechecked its release record and corrected the stale0.1.6 candidate/unpublished wording in both READMEs in this change. GitHub tags do not establish Marketplace publication. Deprecated/experimental API warnings accompany compatible Verifier results and remain a separate investigation; this task does not resolve them.

Earlier [JetBrains typing measurements](../../editors/jetbrains-nagi/benchmarks/python-editing-2026-10-22/README.md) used an older commit/IDEA2025.1.1,3 warmups/20 action samples, not current-head GUI/IME/rendering/end-to-end latency. Compiler query timing also excludes IDE debounce/PSI/rendering. New GUI/long-session stability, future IDEs and untested compiler combinations remain unverified.

## Security and remaining work

Tie existing security/MSRV/browser/TLS regressions and CodeQL to exact commits. The lock uses Tokio1.54.1; one patched dependency does not establish safety of the rest. Reading `repos/disnana/Nagi/dependabot/alerts` was denied HTTP403 because the integration lacks permission; alerts were not obtained. No bypass or same-credential retry was attempted.

Separately compared the original Cargo.lock with public RustSec DB `21d909e4c8bd4e14c7aea58bf980c881a0053ab1` (updated2026-10-24), using cargo-audit0.22.2 with no ignored advisories, OS/architecture filtering or stale allowance. Exit1 reported six known TLS dependency version matches. Static triage found four missing necessary conditions in the current fixed profile and two requiring confirmation of configured trust roots/certificate paths; none was confirmed exploitable in Nagi. Arbitrary Rust feature-unified graphs need separate reassessment. A minimal update to known patched dependencies is handled in [separate Draft #162](https://github.com/disnana/Nagi/pull/162). Do not approve LTS before its post-update audit, compatibility checks and L3. This was not a new general security scan or dynamic attack validation; do not claim zero vulnerabilities.

The fix candidate is `55d785a145437be773cc139e57da3fbbdae636ff`: only the runtime manifest and two locks require rustls0.23.46/WebPKI0.103.16/roots1.0.10. An ordinary build migrated all three versions in an inherited application lock while retaining its old executable/receipt. Both locks changed6→0 matches in the same DB. Existing HTTP client221 (including15 TLS-specific controls)/public API1/registry3/contracts11/native2, Clippy, existing Rust1.88 Ed25519 core14/public1 and full workspace2,029 passed/0 failed/4 existing ignored. Its949.59s is one full-regression observation, not a before/after comparison. Fresh independent final review reported no concrete findings. For the candidate completion status and results, see [four-platform L3](https://github.com/disnana/Nagi/actions/runs/40154996082) and [#162](https://github.com/disnana/Nagi/pull/162); assess them separately from original main L3. Use actual completed jobs for the exact SHA rather than document timestamps or historical status text.

Also matched30 published crate library/data files to official WebPKI VCS `8ecdcc76f56b68fda67a6adf7976f5eed5006a4c` and passed its unchanged existing `distrusted_spki` regression (one passed,119 filtered; not all upstream tests). AWS-LC versions/features match, but the upstream dev/test graph uses a separate validation lock. This is not an attack reproduction against a Nagi deployment. The30-configuration replay and speed observations above belong to original main and must not be relabeled as post-TLS-update performance.

The original Windows SF06 failure remains unexplained because its original detailed log was not retained. Better diagnostics and later passing runs do not prove a root-cause fix; flakiness remains possible. The [policy proposal](lts-candidate.en.md) requires ownership and a residual-risk decision. SECURITY.md also has stale date/release/incomplete-status statements; a separate proposed diff is prepared for review before application. The actual policy is unchanged and needs reconciliation against releases/contracts before LTS designation. This proposal adds or removes no existing security guarantee.

Local replay covered Linux/dev/loopback/synthetic credentials only. Distinguish L3 evidence from this replay for release applications, other OSes, external TLS/production setups, soak/load, all Axum/quote-api/byte-inspector smokes and extracted distributions. Four-platform CI alone does not establish every deployment or long-term stability.
