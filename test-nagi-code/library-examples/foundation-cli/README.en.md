# Quote CLI with shared pricing

Enter a label, unit price, quantity, and discount to print one quote as JSON. This application shares a [Nagi module and Rust functions](../shared/README.en.md) with the [batch report](../foundation-report/README.en.md). It needs no database or server.

From the repository root, use an installed `nagic`:

```sh
nagic check --project test-nagi-code/library-examples/foundation-cli
nagic run --project test-nagi-code/library-examples/foundation-cli
```

Enter `Notebook`, `999`, `3`, and `1250` at the prompts. Prices use minor currency units such as cents. Discounts use basis points, so 1250 means 12.5%. The result has subtotal 2997, discount 374, and total 2623. The application also prints the original `Notebook` label after borrowing it for the calculation.

The default calculation is written in Nagi. To use the shared Rust implementation in bash:

```sh
printf 'Notebook\n999\n3\n1250\n' | NAGI_PRICING_ENGINE=rust nagic run --project test-nagi-code/library-examples/foundation-cli
```

In PowerShell:

```powershell
$env:NAGI_PRICING_ENGINE = "rust"
@("Notebook", "999", "3", "1250") | nagic run --project test-nagi-code/library-examples/foundation-cli
```

`NAGI_PRICING_ENGINE=nagi` and `rust` use identical amounts and rounding rules. Other names are errors. Invalid integer input or an out-of-range quantity propagates through `Result` and `try`, and the program exits unsuccessfully. See the [shared API](../shared/README.en.md) for input bounds and how to supply your own Rust function.

`nagi.toml` selects `foundation_cli.nagi` and the small `native.rs` adapter. The adapter includes `../shared/bridge.rs`, reached through `@rust("native::engine::foundation_rust_quote")`. No additional crates are required. Both implementations are compiled into the application; selection changes the function that runs.

Generated sources go to this folder's `build/foundation_cli/`. Successful executables are stored by build generation under the generated directory's `.nagi/`; read the actual path from the `native:` line instead of constructing a filename or generation path. This executable layout targets Nagi 0.1.11; check the official release record to confirm published availability. `NAGI_NATIVE_TARGET_DIR` selects a shared dependency cache and does not change the executable location. The `native:` path includes `.exe` on Windows. Building requires Rust/Cargo and a compatible C build environment.

[日本語](README.md)
