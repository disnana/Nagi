# Use serde_json through a Nagi record

[日本語](README.md)

This small project parses JSON with Rust's `serde_json` and returns a `JsonRecord` declared in Nagi. Nagi calls an `extern def`; the Rust adapter refers to the generated record as `super::JsonRecord`. Library-specific implementation details stay in the adapter without extending Nagi's public API.

The adapter borrows `view[str]`, so the program parses the same input twice and prints it afterward. The returned record owns its string field. Malformed JSON and a field with the wrong type become Result failures handled with Nagi's `match`.

## Run

Run these commands from the repository root with `nagic` on PATH, Rust/Cargo installed, and a working C build toolchain. The first dependency download needs network access; cached dependencies do not need downloading.

```sh
nagic check --project test-nagi-code/library-examples/rust-json
nagic run --project test-nagi-code/library-examples/rust-json
```

Apart from Rust build messages, the program prints:

```text
Nagi
2
{"label":"Nagi","count":2}
malformed -> invalid
wrong type -> invalid
rust-json: OK
```

The invalid inputs are deliberate checks. The program handles both failures and exits successfully. `assert_true` detects unexpected results.

## Files and limits

- [serde-record.nagi](serde-record.nagi): the record, external declaration, and borrow/Result checks.
- [native.rs](native.rs): `serde_json::from_str` and conversion of library errors into Nagi's `Error`.
- [nagi.toml](nagi.toml): the entry file, adapter, and `serde_json = "1.0"` dependency.

`check` validates Nagi declarations, types, and ownership. The build performed by `run` verifies the Rust implementation and crate APIs. Extra fields, missing required fields, and incorrect field types are rejected. This adapter joins the same Rust build; it does not provide dynamic library loading or a general JSON object API in Nagi.

See [typed Rust integration](../../../docs/en/modules-and-rust.md) for the interface rules.
