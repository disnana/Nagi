# CLI with a custom error type

Validate a quantity between 1 and 1000000 and print twice its value on success. Failures use `QuantityError`; the caller chooses a message for each variant. This example uses neither a database nor HTTP.

Run from this directory with a compiler supporting enums. `run` requires Rust/Cargo and a toolchain that can build applications on your OS.

```sh
nagic check
nagic lower
nagic run
```

```text
42
quantity must be between 1 and 1000000
quantity must be a number
```

`validation.nagi` explicitly wraps the built-in parse error in `NotNumber(cause: Error)`, preserving its cause. `doubled` propagates the same error type with `try`. `message` matches every variant and omits the private cause from displayed text. `InputError` and `validation.QuantityError` are the same type.

JSON conversion of enums or types containing built-in Error is unsupported. Standard HTTP handlers still return `Result[..., Error]`.

[日本語](README.md) · [Error handling](../../../docs/en/error-handling.md)
