# LTS candidate observations, 2026-10-25

These observations assess a candidate; they do not designate Nagi 0.2.2 as LTS or publish a release. [Japanese validation](../../../docs/internal/lts-candidate-validation.md), [English validation](../../../docs/internal/lts-candidate-validation.en.md), [proposed policy](../../../docs/internal/lts-candidate.en.md).

`summary.json` records source/compiler identity, all 30 configuration identities and runtime selections, input hashes, application smoke facts, separate check/build/smoke totals, three native execution samples per mode as median/range, three VS Code test trials and limits. It contains no private output paths or credentials. Raw logs, manifests, receipts, the private replay driver and retained executables remain local evidence; this JSON is a bounded export, not an independent test runner.

## Conditions and repetition

Source main `22183d7233950bdb575b62a72393dac9b4aa0767`, tree `a7a2e3985847a1bbd2fca8be22b05f5c69018322`. The existing optimized compiler executable matches 17 frozen product inputs; its version still reports 0.2.1 because there was no version bump. This candidate's `build --help` supports `--runtime-profile`. The published 0.2.1 executable does not.

Linux x86_64/Rust1.99.1, `CARGO_NET_OFFLINE=true`, `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_INCREMENTAL=0`. Registry cache was available. A new owned target was shared sequentially by all configurations, first full then auto, each High then saved Low. The first full build paid dependency compilation. This is a compatibility replay, **not a balanced cold-build speed comparison**. All build and execution failures were fatal; existing smoke assertions and 180-second command deadlines were retained.

To reproduce a sample's default-profile behavior with the existing public harness:

```sh
python scripts/verify_application_examples.py --compiler /absolute/path/to/candidate/nagic --only device-settings
```

That harness uses its existing default release/full behavior; it is not the dev/auto replay above. For the measured selection matrix, the private driver reused that module's `run`, `verifier`, `verify_smoke`, `LOW_PROJECTS` and `native_artifacts.native_executable`, rather than copying or reducing smoke assertions. For each example and mode it performed these operations, with distinct output directories and the shared target:

```sh
nagic check --project test-nagi-code/application-examples/device-settings --out /owned/output/full/high
nagic build --project test-nagi-code/application-examples/device-settings --out /owned/output/full/high --profile dev --runtime-profile full --build-stats
nagic check /owned/output/full/high/generated.low --project test-nagi-code/application-examples/device-settings --out /owned/output/full/low
nagic build /owned/output/full/high/generated.low --project test-nagi-code/application-examples/device-settings --out /owned/output/full/low --profile dev --runtime-profile full --build-stats
```

Repeat with auto and independent auto outputs; use the existing handwritten Low project directly for order-quote. Hello uses `examples/hello.nagi` without a project and checks exact stdout `Hello, Nagi!\n4\n`. For every published executable, the driver ran that project's unchanged `smoke.py` verifier with an independent execution directory, verified its build receipt, compared full/auto and High/Low facts, and checked all input hashes again. It removed only its completed owned Cargo target, then reverified the retained executables' SHA256 and receipts. Never delete a shared or active target to reproduce these numbers.

Native timing ran only the seat-reservations executable after building, three times per mode; it includes startup and the existing actor scenario, not build time or steady-state throughput. Both modes selected full, and the observed ranges overlap. Application smoke timing includes waits and negative assertions and is a separate test observation.

For the VS Code trials, a private copy of the same compiler was named `nagic` on PATH and `npm test` ran unchanged in `editors/vscode-nagi`, three times with ordinary warm filesystem state and execution permission for snippet subprocesses. Node24.19.0,196 passing subtests across22 files each time. Initial ENOENT/EPERM environment failures were preserved and diagnosed; no assertions, limits or skips changed. These are headless Node/compiler tests, not GUI latency.

The row counts in `authoring` include physical blank/comment lines of Nagi/Rust sources and exclude smoke.py. Rust adapter source can contain its own unit-test code. This is a setup/maintenance observation, not a condition-matched Python comparison. A single type-error check confirmed the original line, expected/actual types and explicit-conversion advice; it is not a full diagnostic-quality audit.

## Limits

No new release-profile app replay, cross-OS local replay, long-running load/soak, real credentials, external TLS deployment, current IDE GUI/IME/rendering measurement or general security scan. A separate public RustSec lockfile audit found six known-version matches; static triage and the separate dependency-update requirement are recorded in the validation pages, alongside L3 status. CodeQL/test success does not establish absence of vulnerabilities. Historical Windows SF06 failure still lacks a confirmed root cause. See the validation pages for remaining designation decisions.
