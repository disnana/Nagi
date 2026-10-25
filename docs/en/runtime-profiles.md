# Runtime dependency selection (0.2.2 development)

[日本語](../runtime-profiles.md)

This feature targets **unreleased development sources for 0.2.2**. Published 0.2.1 does not support it. Check that `nagic build --help` lists `--runtime-profile full|auto`. Compiler and editor plugin versions are independent.

Opt in to dependency selection to reduce the communication and cryptography dependencies compiled for a small first program:

```sh
nagic run --project my-app --profile dev --runtime-profile auto --build-stats
```

The default `full` retains all existing runtime APIs. After complete checking, `auto` selects `core` only if every generated function, type and operation belongs to the supported IR subset. The initial subset covers synchronous Hello/Point-style programs: primitive values, simple records, direct calls, print, supported arithmetic and field access. Unused functions are checked too. This does not make HTTP or database applications generally lightweight.

Core retains Serde and SQLite's `FromRow` support needed by generated records. SQL sessions, HTTP, authentication and TLS use full APIs and select the ordinary configuration. Async and unknown syntax, types or operations also select full in this initial implementation.

## Read the selection

`--build-stats` reports the requested profile, selected configuration and reason in `runtime`. This illustrates the format, not a timing measurement:

```json
{"runtime":{"requested":"auto","selected":"core","reason":"eligible_core"},"checks":"full","cargo_always_invoked":true}
```

| reason | Meaning |
|---|---|
| `full_requested` | Default or explicit full |
| `eligible_core` | Supported IR and runtime capability confirmed |
| `unsupported_ir` | Unsupported types, operations, syntax, or async |
| `incremental_requested` | Body/function-unit CAS retains full |
| `native_input` / `rust_adapter` / `rust_dependencies` | Additional native inputs, adapters or Rust dependencies retain full |
| `runtime_unsupported` | The core declaration, expected layout or shared-source identity cannot be confirmed |

Combining auto with `--incremental` or `--incremental=units` retains full runtime dependencies, while existing source reuse follows its normal rules. There is no force-core option that removes unsupported APIs.

## Checks and caches

Complete High/Low syntax, type, ownership and Policy checks, Rust emission and Cargo invocation run every time. Dependency selection uses checked IR rather than import names or generated Rust text. Cargo decides native freshness using features, manifests, locks, toolchains, profiles and other inputs. Nagi hashes alone never authorize native reuse.

This does not retry a failed build with full to conceal errors. Corrupt locks and Cargo failures remain failures and preserve previous successful generations. The default release profile and its optimization settings stay unchanged.

Switching full/core changes the Cargo dependency package and goes through Cargo's freshness checks. Cargo also handles changes to sources, the shared runtime, manifests, locks, features, toolchains, targets, profiles, build scripts and environment inputs it tracks. This change leaves Nagi's body/unit CAS and its invalidation rules intact by retaining full when incremental reuse is requested. It adds no independent restoration of native artifacts across projects.

## Compare configurations

The measurement script requires Python 3.11 or later. Run `cargo fetch --locked` in the repository first to populate the same Cargo cache with the reviewed dependencies. Measurements then run offline.

```sh
python scripts/benchmark_runtime_profiles.py --compiler path/to/nagic --work-dir build/runtime-bench --output build/runtime-bench.json --repeat 3 --prune-owned-targets
```

The harness alternates ordinary and auto configurations using the same compiler, source paths and seed lock. For Hello and Point, it covers cold, unchanged, body/API-shape edits, runtime edits, and corrupt-lock rejection/recovery. Cold means a fresh Cargo target with warm registry/OS caches. Default measurement settings are dev, debug information 0 and 2 jobs; these are explicit conditions, not the unmodified dev profile.

The measurement runner places the seed lock in the output directory. Ordinary builds inherit an existing output Cargo.lock; otherwise Cargo resolves dependencies. Initial index updates and downloads are outside this offline measurement.

Reports retain wall/CPU time, largest-child RSS, Cargo Fresh/Compiled counts, capacity, medians/ranges and native output checks. RSS is not a sum across concurrent processes. Process startup is not production throughput. `--prune-owned-targets` removes only completed targets owned by this measurement and retains logs, sources and published executables.

The [2026-10-24 measurements](../../benchmarks/results/runtime-profiles-2026-10-24/README.md) report Point cold-build medians of 60.68 seconds for full and 11.41 seconds for auto over three trials under the same stated conditions. These results are specific to the supported subset; other applications retain full.

See [development builds](development-builds.md) and the [technical design](../internal/runtime-profile-selection.md).

[Initial dependency lock selection](lock-seeds.md) is an independent opt-in.
