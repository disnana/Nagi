# Pricing code shared by two applications

The [interactive CLI](../foundation-cli/README.en.md) and [JSON report](../foundation-report/README.en.md) use the same Nagi module and Rust calculation code. The calculation function is an argument, so callers can choose a Nagi or Rust implementation while keeping input validation and data types in one place.

| File | Purpose |
| --- | --- |
| `foundation.nagi` | `FoundationQuote`, validation, engine selection, and the Nagi calculation |
| `pricing.rs` | Ordinary Rust arithmetic with no Nagi types or third-party crates |
| `bridge.rs` | Converts Rust `Totals` into the generated `FoundationQuote` class |

Each application uses `import "../shared/foundation.nagi"`. Its `native.rs` includes `#[path = "../shared/bridge.rs"] pub mod engine;`. The shared sources are compiled into each executable.

## Shared APIs and contract

`foundation_select_engine(name: view[str])` returns `foundation_nagi_quote` for `nagi` and `foundation_rust_quote` for `rust`. Its return type is `Result[fn[view[str], i64, i64, i64, Result[FoundationQuote, Error]], Error]`; other names are input errors.

`foundation_quote(calculate, label, unit_cents, quantity, discount_bps)` checks the following conditions, then calls the supplied calculation function. It borrows `label`, so the caller can use the original string afterward. A successful `FoundationQuote` owns a copy of the label and contains the inputs, subtotal, discount, and total. The CLI propagates failures with `try`; the report handles each row with `match`.

| Value | Contract |
| --- | --- |
| `label` | 1–80 UTF-8 bytes; surrounding spaces are preserved |
| `unit_cents` | Integer from 0 to 100,000,000 |
| `quantity` | Integer from 1 to 10,000 |
| `discount_bps` | Integer from 0 to 10,000; 1000 means 10% |

Subtotal is unit price × quantity. Discount is `subtotal × discount_bps / 10000`, rounded down by integer division. Total is subtotal minus discount. The bounds keep both products within i64. All amounts must use the minor units of one currency. Taxes, shipping, and currency conversion are outside this example.

`foundation_nagi_quote` assumes inputs have passed those checks; normally call it through `foundation_quote`. Rust `pricing::calculate` also validates numeric bounds so ordinary Rust consumers can use it independently. `bridge.rs` validates the label and maps Rust failures into `nagi_runtime::Error::invalid`.

## Substitute your own Rust implementation

Declare another function with the same inputs and return type using `@rust` and `extern def`, then pass it to `foundation_quote`. Each application's `native.rs` can expose that function without changing the shared Nagi source.

```nagi
@rust("native::custom_quote")
extern def custom_quote(label: view[str], unit_cents: i64, quantity: i64, discount_bps: i64) -> Result[FoundationQuote, Error]
```

For example, call `quote = try foundation_quote(custom_quote, view(label), 999, 3, 1250)` inside `main`. The corresponding Rust types are `&str`, `i64`, and `Result<crate::FoundationQuote, nagi_runtime::Error>`. The returned class owns its fields and stores no references into borrowed input. Calculation functions are trusted application code; the shared function does not recompute amounts returned by a custom implementation.

## What this reuse provides today

This example shares source files compiled into each application; it does not use a Nagi package download or publishing system. Its string imports place definitions in one namespace, so names use the `foundation_` or `Foundation` prefix. The [module example](../module-imports/README.en.md) shows module names and `from` aliases instead.

`rust.file` names one adapter. Other Rust files are ordinary modules beneath it. Dependencies accept version strings or tables with `path`, `package`, `features`, and `default-features`. Git dependencies are unsupported. See the [local Rust crate example](../../rust-library/README.en.md) for configuration. `build` and `run` check whether Rust implementations match their Nagi declarations. Stable C ABI and runtime DLL loading are unsupported.

The applications open no database and start no HTTP server. Building still includes the standard runtime dependencies and needs Rust/Cargo plus a compatible C build environment. Rebuild both applications when shared source changes.

[日本語](README.md)
