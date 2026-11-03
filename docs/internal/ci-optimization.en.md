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

Six failure-independent jobs use `always() && !cancelled()`. [GitHub cancellation](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-cancellation) reevaluates job conditions and continues jobs whose condition stays true; publication must also refuse cancellation. Cleanup-step `always()` stays. This cannot roll back a remote publication already accepted or change an old run retroactively.

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

Actual [L1 run 39511768690](https://github.com/disnana/Nagi/actions/runs/39511768690), HEAD `44b9d13`, changed only a lexer comment in temporary PR #137, now closed unmerged. Critical Rust, browser, four IDE paths and Ready passed: 584 s wall and 31.38 unweighted job-minutes. Comprehensive tests were not selected for this path; its workload differs from the baseline, so this is not a controlled speedup percentage.

The [four-OS nextest measurements](nextest-evaluation.en.md#measurements-on-2026-10-20) took about 1.6–2.0 times longer than Cargo on these two lightweight targets; the mandatory runner stays unchanged. The PR validation section records final L2 results and wall/job/cache observations with the exact HEAD. Manual execution of the new four-platform L3 wiring and actual platform cancellation remain unverified, distinct from strict local matrix/rejection tests. L3 remains mandatory before publication and incomplete verification cannot publish. No billing saving or general performance guarantee is claimed.

Sources checked 2026-10-20: [GitHub cache scope](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching), [cache restore/save](https://github.com/actions/cache/tree/0057852bfaa89a56745cba8c7296529d2fc39830), [rust-cache](https://github.com/Swatinem/rust-cache), [sccache](https://github.com/mozilla/sccache), [uv GitHub guide](https://docs.astral.sh/uv/guides/integration/github/), [nextest](https://nexte.st/docs/).

## Linux critical-path sharding (batch 29)

PR #218 recorded Linux at 26m55s, including CSRF semantic mutants 6m29s, samples 4m51s, Session diagnostics 3m16s, release compilation 2m18s and HTTP registry checks 2m04s. PR #217 recorded Linux at 22m43s and the workspace runner near 23 minutes. Its workspace command took 22m51s: approximately 2m03s compilation and 20m48s execution. The six observed heavy binaries were `build_generations` (179.11s), `security_sf01_native` (101.55s), `http_stdlib` (95.35s), `error_routes` (94.32s) `go_backend` (88.54s) and `auth_boundaries` (83.43s). These are observations from those runs, including cache misses, rather than predictions for the changed workflow. The Windows package remains a longer overall gate; this change does not establish a halving of whole-CI duration.

`linux-workspace-shards` uses four runners and discovers the complete default-member target inventory through locked, dependency-free Cargo metadata. Deterministic longest-first balancing uses the observed heavy-binary weights and explicit fallback estimates. A complete validated source diff prioritizes the affected integration target or source package inside its assigned shard. Unknown or missing mappings prioritize the full inventory. Change awareness never removes a target, changes the four-platform release matrix, or weakens the existing L1/L2/L3 selector.

Each shard compiles only its assigned selectors with locked Cargo and `--workspace`, preserving workspace feature unification. All workspace libraries form one atomic `--lib` selector. Each other selector has an exact metadata target set; existing shared names such as `security_sf01` and `task_handles` retain both package executables. Mixed build/test modes, duplicate identities, or a default-member graph that differs from `--workspace` fail closed. The runner reconciles every emitted executable by kind, name and source path, lists all and ignored cases in pretty format (which retains count summaries), and compares complete named results and summaries for each Cargo execution block. Exact libtest ` - should panic` annotations are normalized before name comparison. Cargo remains the execution driver, retaining package working directories and binary environments. The CLI's reviewed zero-test harness is allowed explicitly; unexpected empty test targets and shards with zero passing cases fail. Default build-only examples compile once with `cargo build --locked --workspace --profile test --example NAME`; the aggregate requires that exact command and a successful exit. Shard 0 also retains the authoritative `cargo test --locked --doc` command once, recording its exact command, successful exit, passed/ignored/failed totals, summaries and metadata set of doctest-enabled libraries. Doctest summary count must cover that library set and the existing nonempty doctest workload must execute. Individual doctest names remain an observability limit; the receipt does not claim name-level doctest reconciliation.

The `linux-workspace` aggregate freshly discovers the expected inventory and checks exactly four receipts, each target's exact assignment, ignored accounting, a nonzero executed workload, and matching HEAD, complete rustc identity, default flags, lock and workflow hashes. Failed, skipped, missing, duplicate, unknown or partial execution cannot satisfy Ready. The complete aggregate and publication gates also require `linux-csrf-mutants` and `linux-samples`. The mutant command and its artifact step move unchanged together. The self-contained release-build, tutorial, sample, socket and fuzz block moves unchanged and in its original internal order. Session diagnostics stay in Linux with their existing cross-step failure conditions and enforcement.

New Linux cache keys bind the role/shard, OS, architecture, toolchain, default compiler/doc flags, Cargo lock and workflow/runner source. Restores never fall back to the prior mixed diagnostic build namespace. Only a successful main push saves the cache; a hit still executes the full assigned workload. Release version planning, main/tag publication policy and all platform contracts remain unchanged. The exact outer workflow inverse recovers the pre-change source hash; the older command-body proofs and frozen historical hashes remain intact.

This CI-only branch is isolated from main `0b21a1d` and does not depend on PR #218's feature changes. The outer inverse matches that main workflow byte for byte; only the new outer base hash is bound to that source, while older historical hashes remain unchanged. Independent Python reconciliation of main's manifests and tracked source finds 133 non-doc targets, 129 selectors plus one doc entry, and retains Go platform counts 34/33/34/34. This is source-inventory validation, not actual Cargo metadata or native execution. Local checks cover selector/receipt rejection, mocked assigned-only execution, cache/gate mutations and the complete Python CI policy suite.

Earlier batch 29 validation on a different source observed real metadata with 134 targets/130 selectors, eight offline `enum_parser` cases, unified features and a small native workspace; those results are not execution evidence for this isolated main patch. Full workspace/native execution, artifact transfer and cold/warm critical-path measurements remain unverified. Compare identical sources and inventories with separate cold/warm observations, recording dependency wait, queue, setup, cache restore/save, compilation, execution and artifact transfer alongside the Linux gate, overall completion and runner-minutes. The #217 Windows package dominated overall completion at 34m52s. Summed shard weights are scheduling estimates, not measured Actions durations. Four dependency builds and setup/transfer overhead can offset execution savings. A successful exact-source CI run and independent review are required before reporting the optimization as measured or Ready.


### Separate native child stdout from outer libtest results

`graph_render` launches its own test binary with `current_exe --exact image_child --nocapture` and inherited stdout. The same case is also listed in the outer harness, so counting every `test ... ok` line incorrectly rejected legitimate nested execution as an outer duplicate. The Cargo selectors, workspace features, cwd and Nagi environment remain unchanged. A scoped Cargo host runner adds a separate stable libtest `--logfile` for each outer binary, removes its own runner settings from the environment, and execs the actual binary. Nested Cargo and self subprocesses inherit the original environment. An exclusive start file and per-binary boundaries reject genuine repeated outer execution. Every outer case, ignored state, result and final count summary is checked against the independent event file; nested output and summaries remain diagnostic. No filters or serialization are added.

`--logfile` is stable but deprecated. A future toolchain rejecting it fails the Cargo command; there is no fallback to weaker stdout-only checks. Orchestration logs preserve successful Cargo output too, and parser exceptions print the relevant execution output. Failure artifacts retain raw logs, boundaries and event files even when compiler failure artifacts are empty. The small native fixture and Python checks for this repair do not establish full workspace Actions success or a performance improvement.

## Independent platform contracts (2026-11-02, local implementation)

The baseline is main `2cb7c33aff2ccc3763fa80e6d0f07240ad09570f` (PR #219). Replace the existing `windows-contracts` role with one `platform-contracts` job matrix using `ci-plan.outputs.matrix` unchanged: three ordinary platforms and all four L3 platforms, including macOS Intel. Keep the level/matrix producer, both broad `nagi-regression` Cargo steps, and release/package output contracts unchanged.

| Relocated unit | Dependencies retained on the same runner |
| --- | --- |
| SF06 signature fake check, public client, native artifact, signature classifier/artifact (5 steps) | The fake check precedes the public client; its raw/native evidence and `steps.sf06_public_client.outputs.closed_failure_diagnostic` remain local to the classifier/uploads. Cargo builds its own test binaries and `CARGO_BIN_EXE`; no release executable is required |
| Windows stalled-send/join (1 step) | Locked Cargo and the exact two named cases/summary; unchanged Windows guard and log |
| Session runtime, auth-stage capture/artifact, deadline capture/artifact, two final enforcers (7 steps) | Raw/sanitized logs and `auth_stage_diag`/`session_deadline_diag` outcomes stay together. Preserve continue-on-error, always conditions, TMP/TEMP validation, capture bounds, cleanup, diagnostic cfgs and enforced failure |
| Four common SF01 observer/policy/lease and Host contracts | Cargo owns the test/native compiler. Remove package's guarded copies and run the existing Windows bodies once on every selected platform |
| Existing Windows native-send/private Session pair | Preserve Windows guards, 30-minute budget, source digests, exact three-case assertion, TMP/TEMP and private diagnostic |

SF07 checkpoint/prelogin observer/artifact, B1 acceptance/observer and BrowserLogin business witnesses remain in package. The release compiler, archive/distribution checks, PowerShell 5.1/7 demos, High/Low applications, installer/upgrades/uninstaller and User PATH checks consume actual release artifacts and remain on their package runner. No relocated bundle depends on hidden predecessor `GITHUB_ENV` data. SF06 outputs and Session outcomes do not cross jobs.

The new `nagi-platform-contracts-v1` cache role includes platform/OS/architecture, toolchain, Cargo lock, workflow/diagnostic sources, manifests/compiler/runtime/runtime-core sources, and a versioned configuration-set identity for default/auth/browser/SF01 oracle/SF06 no-debug builds. Cache both target directories without package/regression fallbacks. Save only after the whole job succeeds on a main push. Preserve inline/step RUSTFLAGS; do not introduce empty CARGO_ENCODED_RUSTFLAGS that would override diagnostic cfgs.

Ready and the independent publication gate require the new job ID: success when selected and skipped when unselected. Missing, failed, cancelled, unknown/partial results and reduced/duplicate/invalid matrices fail closed. This uses GitHub's matrix aggregate result and the complete fixed matrix; it does not add a separate per-platform execution receipt. Existing named-case/count checks and failure artifacts remain intact.

The new outer inverse restores the baseline workflow byte-for-byte, SHA256 `127de03b03d887ea04265454409568f39d3ab8a754c1a511db293bd0388b62f0`, and composes with the frozen historical hashes, action pins, Windows and cache proofs. Mutations cover lost/duplicate/reordered bodies, OS guards, cfgs, counts, source pins, cache provenance, failure artifacts, matrix and required jobs. All 254 CI policy tests passed with `python -B -m unittest discover -s scripts/ci -p 'test_*.py'`. The eight relocated diagnostic fake/harness suites also passed (201 tests, no Cargo/native execution). YAML parsing, bash syntax for every explicit bash step, and parsing 34 inline Python heredocs also passed. Rust/Go/native and actual OS runners were not run; independent final review and exact-source CI remain outstanding. This does not fix the unknown Session 504 cause.

Baseline #219, run `41449198678`, exact source `7a05a1d` (same tree as the merged baseline), passed all selected checks and Ready. Ready completed at 13:42:20; overall wall time was 42m28s and Windows package 39m15s. Observed Session auth 4m52s + SF06 client 4m35s + stalled-send/join 2m30s sum to 11m57s of package-lane step time being relocated, not a speedup forecast. Do not count the partial Session runtime observation of 1m28s as completed duration. Distribution 3m26s, High/Low apps 2m53s, HTTP registry 2m35s, PowerShell demos about two minutes each, and release build 1m39s remain in package. Linux aggregation completed after 26m34s, then waited; all Linux shard caches missed.

Overall wall time is bounded by the maximum of the remaining package lane, platform contracts, regression and other required jobs. Distinguish warm-cache overlap, cold compilation/setup offset, and a regression/I/O tail becoming dominant. The change adds two runner executions normally and three at L3; duplicate setup/compilation can increase runner-minutes. Baseline #219 used 279.47 runner-minutes versus #217's 250.42, but different source/cache/failed refs prevent a performance conclusion. Measure matched source and matrix separately for cold/warm runs, recording queue/dependency wait, setup, cache hit/key/restore/save, compile, test, artifacts, packaging, Ready/overall wall time and runner-minutes. Local wiring tests establish no measured speedup.
