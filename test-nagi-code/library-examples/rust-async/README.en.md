# Await Rust async work from Nagi

[日本語](README.md)

This project calls a Rust async adapter through `extern async def`. The adapter actually waits on a Tokio timer, then returns a doubled integer as `Result[i64, Error]`. Nagi receives the success value with `try await` and handles failures with `match await`.

## Run

Run these commands from the repository root with `nagic` on PATH, Rust/Cargo installed, and a working C build toolchain. The first dependency download needs network access; cached dependencies do not need downloading.

```sh
nagic check --project test-nagi-code/library-examples/rust-async
nagic run --project test-nagi-code/library-examples/rust-async
```

Apart from Rust build messages, the program prints:

```text
42
invalid delay -> invalid
overflow -> invalid
rust-async: OK
```

The success case awaits a 10 ms timer. A negative delay and multiplication outside the i64 range return failures before waiting. The program handles both failures and exits successfully. `assert_true` detects unexpected results.

## Files and limits

- [tokio-timer.nagi](tokio-timer.nagi): the async declaration and success/failure checks.
- [native.rs](native.rs): input validation, `checked_mul`, and `tokio::time::sleep`.
- [nagi.toml](nagi.toml): the entry file, adapter, and `tokio = "1.48"` dependency.

The adapter runs on the Tokio runtime supplied by Nagi. It does not construct another runtime or call `block_on` inside async work. It uses no blocking sleep or file I/O. This example limits delays to 0–1000 ms. A timer duration is not a guarantee of completion time or performance.

`check` validates Nagi types and await usage. The build performed by `run` checks the Rust implementation and Tokio APIs. This example covers timer integration; it does not introduce cancellation, task-management, or general I/O APIs.

See [typed Rust integration](../../../docs/en/modules-and-rust.md) for the interface rules.
