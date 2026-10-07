# JSON report with shared pricing

Calculate several rows and print successful quotes, rejected rows, and the total in one JSON object. This application uses the same [Nagi module and Rust functions](../shared/README.en.md) as the [interactive CLI](../foundation-cli/README.en.md). The CLI stops on a failed quote; this report uses `match` to collect each failure and continue with the next row.

Run from the repository root:

```sh
nagic check --project test-nagi-code/library-examples/foundation-report
nagic run --project test-nagi-code/library-examples/foundation-report
```

The default calculation is the Rust implementation. [report-input.json](report-input.json) is embedded into the executable at build time using `include_text`. Rebuild after editing it. A distributed executable does not read the original JSON file.

| Input ID | Result |
| --- | --- |
| 1 | Unit price 999 × quantity 3 with 12.5% discount; total 2623 |
| 2 | Unit price 250 × quantity 2 without discount; total 500 |
| 3 | Quantity 0 is rejected: `quantity must be between 1 and 10000` |

Output `quotes` contains IDs 1 and 2 with their `quote`; `rejected` contains ID 3 with its `reason`; `total_cents` is 3123. Row-level pricing errors are report data, so the supplied batch exits successfully. Invalid JSON structure, unknown fields, more than 1000 rows, or an invalid engine name fails the entire report with a nonzero exit. IDs are returned as supplied; uniqueness is not checked.

The Nagi implementation produces the same report:

```sh
NAGI_PRICING_ENGINE=nagi nagic run --project test-nagi-code/library-examples/foundation-report
```

In PowerShell:

```powershell
$env:NAGI_PRICING_ENGINE = "nagi"
nagic run --project test-nagi-code/library-examples/foundation-report
```

Input records contain numeric fields, so they can be copied during iteration. Output classes own their labels and are appended to explicitly typed Lists. The 1000-row limit and shared price bounds keep the total within i64. See the [shared API](../shared/README.en.md) for units, rounding, validation, and supplying a custom Rust implementation.

`nagi.toml` selects `foundation_report.nagi` and `native.rs`. The Rust source is the same `../shared/bridge.rs` and `pricing.rs` used by the CLI. No database, HTTP server, or additional crates are used. Generated sources go to this folder's `build/foundation_report/`. Successful executables are stored by build generation under the generated directory's `.nagi/`; read the actual path from the `native:` line instead of constructing a filename or generation path. This executable layout targets Nagi 0.1.11; check the official release record to confirm published availability. `NAGI_NATIVE_TARGET_DIR` selects a shared dependency cache and does not change the executable location. The `native:` path includes `.exe` on Windows. Building requires Rust/Cargo and a compatible C build environment.

[日本語](README.md)
