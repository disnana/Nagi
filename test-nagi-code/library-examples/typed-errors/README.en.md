# CLI with a custom error type

Validate a quantity between 1 and 1000000 and print twice its value on success. Failures use `QuantityError`; the caller chooses a message for each variant. This example uses neither a database nor HTTP.

Run from this directory with Nagi 0.1.9. `run` requires Rust/Cargo and a toolchain that can build applications on your OS.

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

Enums and values containing the built-in `Error` cannot be converted directly to JSON. For custom HTTP errors, use an App mapper to produce responses. The [authentication example](../http-auth/README.en.md) maps `AuthError` to 401 or 400.

[日本語](README.md) · [Error handling](../../../docs/en/error-handling.md)
