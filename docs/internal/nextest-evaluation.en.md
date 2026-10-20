# Opt-in nextest evaluation

This workflow compares only the existing `nagic` integration-test targets `enum_parser` and `builtin_type_contracts` on Linux x86_64, Windows x86_64, macOS ARM64, and macOS Intel. It starts only after a same-repository pull request receives the `ci-nextest-benchmark` label, or by manually dispatching it from `main`. It does not replace or gate the ordinary CI test commands.

The workflow first builds the selected targets with `cargo test --locked --no-run`, then compares each target’s complete libtest inventory (which includes ignored cases), its ignored-only subset, and nextest’s JSON inventory. It requires exact test-name and ignored-state equality before measuring. A mismatch fails the evaluation before test runs. The nextest runner is pinned to 0.9.148 and installed with checksum verification and no fallback.

The workflow measures four warm pairs, each containing one run of each runner, in alternating order. The matrix alternates which runner starts first across operating systems; within each runner the order reverses each pair. The commands retain Cargo’s locked default-feature build and default parallelism. Nextest explicitly disables fail-fast, sets retries to zero, and fails on an empty run. The helper records target-build preparation, inventory commands, and each test-run duration separately. No Cargo cache action is configured; nextest installation and toolchain setup remain separate Actions steps, with their durations visible in the workflow UI and outside the helper’s JSON artifact.

These runs cover only the named integration targets. Nextest does not execute doctests, so existing `cargo test --doc` coverage remains in place. Results are diagnostic timing evidence: a successful run does not establish a speedup or authorize a change to required CI. The selected targets were reviewed for this bounded comparison; `expression_contracts` remains excluded because it invokes rustc, and native/socket/browser suites are unchanged.

The helper and its unit tests use only Python’s standard library. The test file lives under `scripts/ci`, so the repository’s early `python -m unittest discover -s scripts/ci -p 'test_*.py'` check also runs it; the opt-in workflow runs this focused test directly. Local verification runs those fixtures and YAML parsing only; it does not compile Rust or install nextest. The four-OS measurements are recorded below.

The JSON schema was checked against TestListSummary, RustTestSuiteSummary, RustTestCaseSummary and FilterMatch in the [0.9.148 primary source](https://github.com/nextest-rs/nextest/blob/cargo-nextest-0.9.148/nextest-metadata/src/test_list.rs). Synthetic fixtures and observations of the installed runner remain distinct evidence. The helper requires the installed nextest version to match and records actual Cargo/rustc versions. Workflow concurrency cancels superseded experiments for the same PR/ref.

## Measurements on 2026-10-20

[Run 39514555992](https://github.com/disnana/Nagi/actions/runs/39514555992), HEAD `dde28e1`, passed on all four operating systems with identical test/ignored inventories. The [recorded data](../../benchmarks/results/ci-optimization-2026-10-20/nextest.json) contains four warm runs per runner. Medians below are seconds and include Cargo/process wall time.

| CI runner | Cargo/libtest median | nextest median |
|---|---:|---:|
| ubuntu-latest | 0.2951 | 0.4762 |
| windows-latest | 0.4070 | 0.7265 |
| macos-15 | 0.3418 | 0.6670 |
| macos-15-intel | 0.5332 | 1.0214 |

Nextest took about 1.6–2.0 times as long on these two lightweight targets. Required CI keeps Cargo; nextest remains an opt-in experiment. This does not establish results for full regressions, native/socket tests, or nextest in general. Build measurements include dependency resolution/downloads; setup, runner queue time, RSS and billing are outside these warm medians.
