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

These records are an integrity foundation for future content-addressed reuse. This implementation does not skip Nagi checks, emission, tests or native builds based on semantic hashes. Full checking and Cargo always run; every successful build publishes a fresh immutable generation. Corrupt observations cannot authorize reuse. Receipt-writing failure preserves the previous latest generation and does not execute the new app.

Digest comparison detects corruption, not an attacker who can replace both files and receipt. The complete native input closure—including Rust module/include, path crates, build scripts, environment and toolchain—is not captured. Neither a receipt nor Nagi source hashes authorize execution of CI-restored binaries. Source snapshots can contain source-embedded secrets; do not share the whole generated directory as a cache.

## Local development and CI

`NAGI_NATIVE_TARGET_DIR` still chooses the shared dependency cache. App IDs and immutable success generations remain separate, with `debug/` for dev and `release/` for release. Ordinary generations are not automatically pruned; monitor disk usage. IDE-internal retention settings are not general CLI garbage collection.

See the [project CI generator](user-ci.md), which works with released 0.2.0, the [implementation plan](../internal/smart-build-plan.md), and [measurements](../../benchmarks/results/smart-build-2026-10-20/README.md).

Module identities contain canonical paths. These initial semantic hashes are not portable cache keys across different checkouts.

The compiler digest reopens the `current_exe()` pathname during publication. Replacing that file during a build can make it differ from the executing image; it is not executing-image provenance.

`receipt` is a display path; `receipt_os` preserves OS units using the existing snapshot encoding (Unix bytes or Windows UTF-16).
