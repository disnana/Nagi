# Use a local Rust library from Nagi

This example includes a handwritten Rust library crate through a `path` dependency. Its package is `nagi-pricing-engine`; the adapter refers to it as `pricing`. The dependency table enables the volume discount feature and disables the default service fee feature.

```text
rust-library/
  nagi.toml
  library-pricing.nagi
  native.rs
  engine/
    Cargo.toml
    src/lib.rs
```

Run from the repository root with a `nagic` that supports dependency tables. Use the compiler's absolute path if it is not on PATH.

```sh
nagic check --project test-nagi-code/rust-library
nagic run --project test-nagi-code/rust-library
```

The program prints:

```text
Workshop notebook
Subtotal cents:
3000
Discount cents:
300
Service fee cents:
0
Total cents:
2700
unit price must be nonnegative and quantity must be positive
Local Rust library verified.
```

Twelve items at 250 cents each produce a subtotal of 3000, a discount of 300, no fee, and a total of 2700. The program asserts these values and checks the `Result` rejecting a zero quantity. The calculation crate uses only the standard library and needs no dependency downloads of its own. Cargo still downloads missing Nagi runtime dependencies on the first build.

## Configuration and type boundary

```toml
[rust.dependencies]
pricing = { version = "0.1", path = "engine", package = "nagi-pricing-engine", features = ["volume-discount"], default-features = false }
```

The path is relative to `nagi.toml`. Invoking the compiler from elsewhere or changing the generated directory with `--out` still selects `engine/`. Since `version` is also specified, Cargo checks that the local crate version meets that requirement.

| File | API and role |
|---|---|
| [engine/src/lib.rs](engine/src/lib.rs) | `quote(&str, i64, i64) -> Result<Quote, QuoteError>`; independent Rust pricing calculations |
| [native.rs](native.rs) | Calls `pricing::quote`, converts the crate's `Quote` to the generated Nagi `Quote`, and converts errors to `nagi_runtime::Error` |
| [library-pricing.nagi](library-pricing.nagi) | Declares types with `extern def`, passes a borrowed string, and receives a class and Result |

The Rust `Quote` holds the product name, subtotal, discount, fee, and total. Amounts use integer cents. Blank product names, negative unit prices, nonpositive quantities, and arithmetic overflow return `QuoteError`. The `volume-discount` feature discounts the subtotal by 10% for ten or more items, rounding down to whole cents. The default `service-fee` adds 100 cents; this application disables it.

The `[workspace]` in `engine/Cargo.toml` keeps the crate independent of the Nagi repository's workspace. It has no Nagi dependency and can be reused by another Rust application through an ordinary path dependency. The adapter owns the class conversion, so the calculation crate need not know generated code types.

`check` and `symbols` do not validate Rust bodies or behavior selected by features; `build`/`run` do. See [project configuration](../../docs/en/projects.md) for settings, Cargo feature unification, and generated lock handling. The sample includes no standalone crate `Cargo.lock` or `target/`.
