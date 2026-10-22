# Development builds and change observations (unreleased)

[日本語](../development-builds.md)

`--profile` and `--build-stats` belong to development source after the 0.2.0 release. They are absent from released 0.2.0. Build the compiler from the same source revision. Existing `build` and `run` still default to release.

```sh
nagic check --project my-app
nagic run --project my-app --profile dev
nagic build --project my-app --profile release --build-stats
```

`check` needs no Rust compilation. It checks loaded High/Low syntax, types, ownership and standard Policy type boundaries. Offline SQL schema checking remains opt-in. It does not prove application authorization, adapter correctness, final Send/Sync, linking or deployment state.

`dev` selects Cargo's development profile; `release` retains the existing optimized profile. Development compilation avoids optimization work and reuses development runtime dependencies. Ordinary integer arithmetic overflow normally panics in dev and wraps in release, as the existing language contract specifies. Cargo profile overrides and environment settings still apply. Measure production performance and distribute applications using release.

## Reusing the generated body (unreleased)

```sh
nagic run --project my-app --profile dev --incremental --build-stats
```

The generated Rust body and dependency manifest are hashed with SHA-256 to create an immutable Cargo library for the application. When generated Rust is identical, Cargo can reuse the compiled body and compile only the small generation-specific entry shim. Published executables are still copied into separate immutable generations.

Full checking, emission and Cargo invocation always run. Body changes create a new content identity; Cargo determines native freshness for runtime edits, Rust settings, toolchains and other inputs. Semantic hashes and previous analysis JSON never authorize executable reuse. This stage reuses one Rust crate for the body, rather than individual functions, types or modules as native units.

The initial mode is opt-in and requires `--profile dev`. Rust adapters, additional Rust dependencies and IDE-managed run retention fall back to the existing monolithic bin, with a reason on stderr and in `generated_body.reason`. Default builds and release remain unchanged.

Stats expose `generated_body.mode`, generated-source `source_cache` (`hit` / `miss`), `sha256`, and body artifact `cargo_fresh`. A source-cache hit alone does not imply native reuse. Cached files are compared byte-for-byte against the current generated content. Mismatches, links, invalid file types and unexpected extra files or directories fail without repair, preserving the previous latest and executable.

Bodies remain in `.nagi/apps/<app>/code/<sha256>/` under the output and are not automatically pruned. Their identities are not guaranteed portable across checkouts and do not authenticate shared caches. Noncooperating processes concurrently replacing directories or modifying Cargo caches are outside this guarantee.

## Rust units from function dependencies (unreleased)

```sh
nagic run --project my-app --profile dev --incremental=units --build-stats
```

The checked emission plan supplies direct-call dependencies. Closed function groups become at most eight Cargo libraries; mutually recursive functions stay together. An edit recompiles its unit and reverse dependents, while unrelated units can reuse their generated Rust and Cargo artifacts. This does not replace machine code for just one edited function. Small functions share compilation units.

The initial extraction supports synchronous functions whose parameters, results and internal values use `i32`, `i64`, `bool` or `unit`, with supported operators, direct calls, branches and loops. Functions using async, user types, standard APIs or function values, and callers depending on those functions, remain in the existing body. That facade depends on every unit, conservatively covering unknown callback relationships. HTTP and async applications can therefore reuse independent calculation functions. With no eligible group, the mode falls back to body reuse and reports `generated_body.partition_reason`. The existing monolithic fallback for adapters, additional Rust dependencies and IDE-managed runs still applies.

Stats report `generated_body.mode` as `cargo_units`, with each unit's functions, content identity, `source_cache` and `cargo_fresh` in `units`. Full checks, Rust emission and Cargo run every time. Selected functions use stable symbol order to avoid declaration-reordering noise. Adding functions or changing the call graph may change group membership and trigger a wider rebuild.

Receipt schema 3 verifies the complete referenced source-unit graph and requires its source caches. The verifier checks the closed generated manifest format, at most nine nodes, each content identity and file, and agreement of runtime and root dependencies. Schemas 1/2 remain supported. This adds neither shared native-cache authentication nor cross-checkout portability.

## Reading observations

On success, `--build-stats` writes one `nagi-build-stats: {JSON}` line to stderr. Application stdout stays pipeable. Times are milliseconds.

- `before_cargo_ms` includes loading, full checking, generation, output lock wait and observation writes before Cargo starts.
- `cargo_ms` covers Cargo startup through exit. `total_ms` also includes publication and receipt hashing, but excludes application execution.
- `cargo_fresh_artifacts` and `cargo_compiled_artifacts` count Cargo's `compiler-artifact.fresh` observations, including dependencies and the app. They are neither function-cache nor sccache statistics.
- `observed_change` compares the previous observation: missing/invalid record, profile, declaration shape, function implementation, generated Rust, or unchanged. It does not explain all Cargo invalidations or external dependency changes.

Successful generations contain `build-analysis.json` and `build-receipt.json`. Analysis records typed Program declaration shapes, SHA-256 of each function's canonical Low and generated Rust, and module/direct-call/type graphs. Function identities exclude physical line numbers. Declaration shape includes all loaded definitions, not a language export compatibility decision. Existing graph warnings retain unknown function-value/callback relationships.

The receipt hashes the published Low, Rust, manifest, lock, source/provenance snapshots, analysis and executable, with lengths and the compiler executable's SHA-256. Find it beside the published executable.

```sh
python3 scripts/verify_build_receipt.py path/to/generations/g-.../build-receipt.json --compiler path/to/nagic
```

Distribution candidates bundle the same verifier as `tools/verify_build_receipt.py`. It needs Python 3.10 or later, without a source checkout. Artifacts must be regular files; symlinks, Windows reparse points and FIFOs are rejected. The compiler argument may use the installer's executable link. This local verifier does not protect against another process maliciously replacing directories concurrently.

Full checking, emission and Cargo always run; every successful build publishes a fresh immutable generation. Corrupt observations cannot authorize reuse. Receipt-writing failure preserves the previous latest generation and does not execute the new app.

Receipt schema 2 adds the entry shim and body manifest for body reuse. The verifier recomputes the body identity from the snapshot, compares the body in the prescribed cache location and checks the generated dependency reference. Schema 1 remains supported for ordinary builds. Verifying schema 2 also requires the corresponding body cache.

Digest comparison detects corruption, not an attacker who can replace both files and receipt. The complete native input closure—including Rust module/include, path crates, build scripts, environment and toolchain—is not captured. Neither a receipt nor Nagi source hashes authorize execution of CI-restored binaries. Source snapshots can contain source-embedded secrets; do not share the whole generated directory as a cache.

## Local development and CI

The CI generator for published 0.2.0 does not enable this unreleased flag automatically. Use it explicitly in development jobs only when CI pins a compiler built from the matching source revision. Full pre-merge checks and production release builds remain necessary.

`NAGI_NATIVE_TARGET_DIR` still chooses the shared dependency cache. App IDs and immutable success generations remain separate, with `debug/` for dev and `release/` for release. Ordinary generations are not automatically pruned; monitor disk usage. IDE-internal retention settings are not general CLI garbage collection.

See the [project CI generator](user-ci.md), which works with released 0.2.0, the [implementation plan](../internal/smart-build-plan.md), and [measurements](../../benchmarks/results/smart-build-2026-10-20/README.md).

Module identities contain canonical paths. These initial semantic hashes are not portable cache keys across different checkouts.

The compiler digest reopens the `current_exe()` pathname during publication. Replacing that file during a build can make it differ from the executing image; it is not executing-image provenance.

`receipt` is a display path; `receipt_os` preserves OS units using the existing snapshot encoding (Unix bytes or Windows UTF-16).

[unit reuse measurements](../../benchmarks/results/function-unit-reuse-2026-10-21/README.md)
