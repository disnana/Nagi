# Replace a High implementation with Low

This project computes the sum of squares of an integer list. The application is written in High, while `kernel.low` replaces the body of `sum_squares` with the same parameter and return types.

Run these commands from this directory. You need Rust/Cargo and a toolchain that can build Nagi applications.

```sh
nagic check
nagic run
```

Expected output:

```text
30
4
```

The `native` field in `nagi.toml` selects the Low file. To run only the original High implementation, skip project discovery:

```sh
nagic run low_kernel.nagi --no-project
```

Both versions return the same result. The function borrows the list, so the application can still call `len(samples)` afterward.

Low uses the same type and ownership checker and ultimately generates Rust. This example demonstrates function replacement. It does not claim a measured speedup: the Low implementation performs the same calculation. Fixed-width integer overflow needs a separate policy for large inputs.

[日本語](README.md) · [High and Low](../../../docs/en/low-language.md)
