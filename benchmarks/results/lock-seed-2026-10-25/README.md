# Lock-seed benchmark record (2026-10-25)

## Scope and status

- **良かった点:** 別の新規アプリを同じCargo targetでビルドする時間は、測定した各構成でseedの中央値が短くなりました。
- **悪かった点:** clean full buildはPoint/Stockともseedが遅くなりました。warm-cache・offline以外は測定していません。
- **判断:** seedは既定で無効のままです。条件をまたいで一律に速いとは言えず、性能改善を主張しません。


This is a reviewer-facing record for a local Linux benchmark of the opt-in seed-lock candidate. The raw product identifies itself as version `0.2.1`, but these measurements describe an unreleased candidate for 0.2.2 review; they are not a published 0.2.1 or 0.2.2 result. The measured compiler is pinned by SHA-256 in [`summary.json`](summary.json). Its source tree is represented by the published-code-only commit [b5eb5c7](https://github.com/disnana/Nagi/tree/b5eb5c7aeda5888bffdad6e2571a41d21c83c203), whose tree matches the measured product tree.

The experiment has 18 configurations and 54 build samples: Point/full, Point/auto, and Stock/full; ordinary and seed modes; clean, unchanged-input, and new-application stages; three repeats per combination. For new-application samples, the target is shared with the preceding configuration stage as recorded. Individual sample rows retain measured wall and CPU time, maximum child RSS, sizes, compiler-reported build statistics, runtime/lock metadata, smoke evidence, and log/executable hashes. Dependency package tuples are deduplicated into four graphs in [`dependency-graphs.json`](dependency-graphs.json), addressed by their canonical tuple-list SHA-256. The JSONL rows refer to those graph hashes.

## Results

Times below are median seconds (min–max across the three repeats), computed from the recorded samples. A lower value is faster.

| Case / stage | Ordinary | Seed |
| --- | ---: | ---: |
| Point/full, clean | 46.885 (45.544–47.231) | 58.501 (57.664–59.661) |
| Point/auto, clean | 11.724 (11.308–12.297) | 11.441 (11.295–11.679) |
| Stock/full, clean | 46.058 (45.772–47.919) | 58.201 (58.034–58.865) |
| Point/full, unchanged input | 0.273 (0.265–0.279) | 0.291 (0.286–0.296) |
| Point/auto, unchanged input | 0.122 (0.121–0.129) | 0.129 (0.121–0.130) |
| Stock/full, unchanged input | 0.344 (0.333–0.353) | 0.342 (0.327–0.348) |
| Point/full, new application | 0.438 (0.413–0.472) | 0.277 (0.275–0.282) |
| Point/auto, new application | 0.181 (0.180–0.207) | 0.126 (0.119–0.134) |
| Stock/full, new application | 0.492 (0.487–0.495) | 0.348 (0.335–0.364) |

The clean full Point and Stock seed runs are slower in this sample. Point/auto clean is a small difference with overlapping observed ranges. Point/full unchanged-input is slightly slower under seed; Point/auto and Stock unchanged-input ranges overlap. New-application medians are lower for seed in these configurations, but the raw counters report the same Fresh/Compiled artifact counts between modes for each path (Point/full 197/1; Point/auto 49/1; Stock/full counts are preserved in `summary.json`). These observations do not establish a cache-hit improvement.

The package graphs differ between tested paths. This dataset therefore cannot isolate the lock-seeding algorithm as the cause of the observed timings, nor support a general speedup ratio. Seed mode is opt-in and default-off, intended for reproducibility. The clean full-build numbers are not a basis for advertising faster cold builds.

A separate default-release native execution probe used the same full runtime and Rust output, matched release profile, checksum/root-Rust equality checks, two warmups, and 15 interleaved samples per compiler. Median (range) native wall time was 0.02018 s (0.01981–0.02105) for baseline compiler SHA `49aa30fb…` and 0.02039 s (0.01996–0.02731) for candidate SHA `2351cc29…`; median child RSS was 22,736 KiB for both. CPU and RSS details are in `summary.json`. This is an application-specific native execution observation, not a general native throughput or core-performance claim. The associated build timings are omitted from before/after comparisons because a shared target was used in fixed order.

## Method and evidence

The recorded environment is Linux x86_64 (kernel 6.18.55, glibc 2.41 observed), Rust/Cargo 1.99.1, 4-vCPU quota, 16-GiB cgroup memory limit observed, two Cargo jobs, dev profile with debug info disabled, `CARGO_INCREMENTAL=0`, offline mode with warm registry and OS caches, and a fresh target directory per configuration. Mode order alternated. The report preserves full three-repeat min/median/max aggregates for wall time, CPU time, child RSS, target/output bytes, Cargo Fresh/Compiled artifact counts, and native startup time in `summary.json`.

The raw record reports all 54 builds complete, successful build exits, Point native smoke exits, 12 Stock smoke-case executions per Stock sample (216 executions across the 18 initial Stock samples), and all 18 owned targets pruned. Receipts and executable hashes are retained as hashes/metadata, not local paths. The post-prune audit rehashed all 54 executables, verified their receipts, reran the executables after all 18 owned targets were absent, and confirmed the distribution file hashes were unchanged. This included 36 Point single-case reruns and 18 Stock runs of the 12-case smoke set (216 case executions). The detailed sanitized observations are in [`verification.json`](verification.json).

A separate test-only timing measured 18 Stock smoke suites (nine per mode; 12 cases each) with no Cargo or build in the timed stage. It includes a fresh Python launcher, driver, assertions, and JSON result I/O. Median wall time was 79.099 ms (76.656–96.865 ms) for ordinary and 80.041 ms (77.292–112.403 ms) for seed; both had a 25,816 KiB median one-child high-water RSS. The small difference and overlapping ranges do not support an improvement claim. These 18 timed suite replays (216 case executions) are a separate measurement stage, not additional unique correctness coverage. Raw sanitized sample values and hashes are in [`verification.json`](verification.json).

A known-version audit checked the four exact dependency graphs against frozen RustSec database commit `21d909e4c8bd4e14c7aea58bf980c881a0053ab1`, with no ignores, target filters, or stale allowances. It reported zero known-version matches and no warnings. This is only a result against that database snapshot; it is not proof that the graphs have no vulnerabilities. Per-graph hashes, lock hashes, and flags are in [`verification.json`](verification.json).

The measured sample is offline and warm-cache only. It does not measure online resolution, index access, downloads, CI behavior, whole-backend throughput, or native application throughput. `peak_child_rss_kib` is a one-child high-water measurement that can reflect launcher/fork/exec behavior; it is not steady-state Rust heap use or a whole-process-tree/cgroup peak. The package graph versions differ across the compared paths. Original compiler, harness, measurement helper, application helper, fixture, distribution, build-log, and native-smoke hashes are kept in the JSON artifacts; machine-private absolute paths are omitted.

A separate interrupted Phase 1 trial was excluded from these results and contributes no samples. The follow-up P2 FIFO behavior is a safety refusal for abnormal input, not a performance comparison: the parent-reported observation was an old 3-second timeout versus an explicit regular-file error in 2.394 ms (independent run 14.8 ms). It must not be generalized to normal workloads.

## Reproduction and artifact map

From a source checkout containing `scripts/benchmark_lock_seeds.py`, use a validated plain extracted development distribution for both compiler and runtime root, and choose a new work directory:

```sh
python3 scripts/benchmark_lock_seeds.py \
  --compiler path/to/development-distribution/nagic \
  --runtime-root path/to/development-distribution \
  --work-dir /tmp/nagi-lock-seed-benchmark-new \
  --repeat 3 --jobs 2 --debug-info 0 --prune-owned-targets
```

The work directory must not already exist or overlap the distribution/compiler inputs. Before an offline run, warm both dependency graphs: run `cargo fetch --locked` in the matching source checkout after confirming its root `Cargo.lock` SHA-256 matches the distribution seed, and materialize the ordinary graph with a normal CLI dev build from the same distribution into a separate primer output. The extracted `runtime-seed/` directory is not a standalone Cargo project. Registry and OS caches must be warm before timing. The benchmark creates fresh Cargo targets per configuration, but this does not mean the registry/OS caches are cold; online index, download, and network time are not measured. The harness SHA-256 for this exact run is in `summary.json`.

For a separate default-release native execution probe, the existing [probe script](../runtime-profiles-2026-10-24/release_cpu_probe.py) and its [documented invocation](../runtime-profiles-2026-10-24/README.md#再現) show the input form (`--baseline`, `--compiler`, `--runtime-root`, `--work-dir`, `--output`, `--repeat 15`). The current probe data here is in `summary.json`; shared-target build timings are not compared.

The input artifact archive SHA-256 is `d5fc3868a740f3fd90a30b4268b9199fb7abad6eaf59dd7fa4f721d7cc12ed4b`. Harness and helper hashes are included in `summary.json`; individual build logs are represented by SHA-256 only because local log files are not included here. `samples.jsonl` contains one sanitized row per recorded sample. `summary.json` contains experiment metadata, the full three-repeat aggregate table, native probe summaries, verification status, and limitations. `dependency-graphs.json` contains the four unique external-package graphs used by the samples. `verification.json` contains the after-prune integrity/rerun audit, Stock test-only timing rows, and scoped known-version database audit.
