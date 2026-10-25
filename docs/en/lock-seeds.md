# Selecting the initial dependency lock (0.2.2 development)

[日本語](../lock-seeds.md)

This is an unreleased development feature for 0.2.2; published 0.2.1 does not support it. The default remains ordinary Cargo dependency resolution. Supporting distributions include an initial lock, checked against their runtime, under `runtime-seed/`.

```sh
nagic build --project my-app --seed-lock --build-stats
nagic run --project my-app --seed-lock --runtime-profile auto --profile dev --build-stats
```

`--seed-lock` is an explicit build/run option. If the output has no `Cargo.lock`, a supporting distribution supplies the initial Cargo lock input. An existing output lock always wins, preserving comments under the existing rules. A corrupt existing lock is never replaced or retried with the bundled lock. Cargo runs every time and revalidates the lock against the actual generated manifest. Complete High/saved-Low checks, the default release profile, generations and old executable protection remain unchanged.

This is not vendoring. Registry index access, crate downloads and Rust/C compilation may still be necessary; offline operation or a build speedup is not guaranteed. The distribution lock can contain packages the generated application does not ultimately use. It does not automatically track future dependency updates.

## Read the selection

Inspect `lockfile` in `--build-stats`:

```json
{"lockfile":{"requested":true,"source":"bundled","reason":"bundled_seed"}}
```

| source / reason | Meaning |
|---|---|
| `output` / `existing_output_lock` | Use the existing output lock |
| `bundled` / `bundled_seed` | Use the verified bundled seed as initial input |
| `cargo` / `not_requested` | No opt-in; ordinary resolution |
| `cargo` / `runtime_unsupported` | Older distribution/source checkout without a seed, unsupported schema/runtime version |
| `cargo` / `incremental_requested` | Ordinary resolution for generated-body/function-unit CAS |
| `cargo` / `native_input`, `rust_adapter`, `rust_dependencies` | Ordinary resolution with additional native input, Rust adapter or Rust dependencies |

After selecting a supported seed schema, invalid contents, fixed layout, hashes or runtime identity cause failure. Cargo failure is never retried without the seed. A source checkout without a seed does not reproduce distribution seeding conditions.

When disabled or superseded by an output lock, unused seed contents and hashes are not checked. Output aliases that would overwrite installation seed inputs are still rejected. An existing file whose identity cannot be established is not assumed safe to overwrite. Preserve the `runtime/`, `runtime-core/` and `runtime-seed/` layout.

[Runtime dependency selection](runtime-profiles.md) is a separate option. Selecting core through auto does not override the seed eligibility rules above.
