# Reuse Rust behind a small adapter

[Agent entry point](README.md) · [High reference](language.md)

Keep typed business decisions, records, and validation in Nagi. Let a Rust adapter own a crate's transport, resource management, and Rust-specific types. The independent crate can retain its API; only the adapter needs generated Nagi names.

## A complete borrowed call

`main.nagi`:

```nagi
@rust("native::text_bytes")
extern def text_bytes(text: view[str]) -> i64

def main():
    text = "Nagi"
    print(text_bytes(view(text)))
    print(text)
```

`native.rs`:

```rust
pub fn text_bytes(text: &str) -> i64 {
    text.len() as i64
}
```

`nagi.toml`:

```toml
entry = "main.nagi"

[rust]
file = "native.rs"
```

From this project folder, run `nagic check`, `nagic build`, then `nagic run`. Output is `4` and `Nagi`. The extern has no body or trailing colon. Its Rust path must begin with `native::`; the implementation is `pub`. An extern cannot return a view, so return owned data when a crate result borrows temporary adapter data.

## Match the generated types

| Nagi boundary | Rust boundary |
| --- | --- |
| `i64`, `bool` and other numeric primitives | Same Rust primitive |
| `str` / `view[str]` | `String` / `&str` |
| `bytes` / `view[bytes]` | `Vec<u8>` / `&[u8]` |
| `List[T]` / `view[T]` | `Vec<T>` / `&[T]` for ordinary element types |
| `T?` | `Option<T>` |
| `Result[T, Error]` | `Result<T, nagi_runtime::Error>` |
| Root class/enum `Quote` | `super::Quote` |
| Class from root module import `models.Quote` | `super::models::Quote` |
| `shared[T]` | `std::sync::Arc<T>` |
| `extern async def` | `pub async fn`, awaited by Nagi |

Convert a crate struct into a generated class, and a crate error into `nagi_runtime::Error` or a generated error class/enum. Field types match the Nagi declarations; Rust keywords and reserved generated names can be escaped or renamed, so inspect generated Rust when constructing records with those names. JSON field names remain the source names. Registered standard resources already have native representations. Arbitrary Rust structs, trait objects, lifetimes, and futures are not a general Nagi type declaration system. Integration is in the same Rust build; a stable C ABI and runtime DLL loading are unsupported.

Add dependencies to the existing `nagi.toml`:

```toml
[rust.dependencies]
serde_json = "1.0"
pricing = { version = "0.1", path = "engine", package = "nagi-pricing-engine", features = ["volume-discount"], default-features = false }
```

The `pricing` entry illustrates the supported local-crate table and requires an actual crate at `engine`; it is not an extra requirement for the borrowed-call example. Supported table fields are `version`, `path`, `package`, `features`, and `default-features`, with `version` or `path` required. `git`, `registry`, `workspace`, `optional`, and target/dev/build dependency sections are unsupported. Paths are relative to `nagi.toml`. Prefer retaining the application's existing constraints and features to inventing dependency versions.

`check` validates the extern declaration's Nagi contract but does not inspect the Rust file or crate API. `build` checks actual types, borrowing, required Clone traits for copies, and async `Send`/state `Sync` bounds. Read generated `src/main.rs` if an adapter path is unclear; fix the adapter or Nagi source, then rebuild.

The generated `Cargo.lock` survives rebuilds. For a fixed dependency resolution after generation, use `cargo build --release --locked --manifest-path build/main/Cargo.toml`. `nagic build --locked` is not supported. A lockfile does not pin the contents of a local path crate.

## Pass a synchronous callback

The same `nagi.toml` can use this `main.nagi` and `native.rs` instead. A named synchronous Nagi function crosses as a Rust function pointer; no closure or captured state is required.

```nagi
@rust("native::apply")
extern def apply(callback: fn[i64, i64], value: i64) -> i64

def add_one(value: i64) -> i64:
    return value + 1

def main():
    print(apply(add_one, 41))
```

```rust
pub fn apply(callback: fn(i64) -> i64, value: i64) -> i64 {
    callback(value)
}
```

`nagic run` prints `42`. `fn[input types..., output type]` describes synchronous functions; it does not enable arbitrary async callback parameters.

## Async and HTTP choices

For an existing Axum stack, copy the shape of the [Axum service](../test-nagi-code/application-examples/axum-service/README.en.md): Nagi declares an async `run_server` extern; Rust owns routes and JSON extraction. A Rust handler calls the known generated function `super::calculate(input).await`, receives `Result<super::Quote, super::QuoteError>`, and maps success or rejection to HTTP. Nagi's `calculate` also awaits Rust's `pause` operation. Changing Nagi function names or types requires updating this adapter.

This direct named call does not pass an arbitrary async callback value through an extern parameter. General async callback signatures are unsupported. A Nagi async app already runs on Tokio; an adapter should await its work rather than create a second Tokio runtime. See [rust-async](../test-nagi-code/library-examples/rust-async/README.en.md) for a timer and checked arithmetic adapter.

For a new service using Nagi's standard APIs, start from the [Quote API](../test-nagi-code/application-examples/quote-api/README.en.md) and `std.http.server`. Its server is implemented with Hyper's HTTP/1 transport; Axum types in the runtime do not make an external Axum Router inherit the standard server's behavior. Standard connection limits, deadlines, panic handling, and shutdown policies do not automatically apply to a custom Rust server. Configure the custom server at its Rust boundary. The Axum sample has a loopback listener, 4096-byte JSON body limit, and Ctrl+C shutdown; it does not add TLS or authentication.

## Validate the boundary you changed

Check and build the configured project, then exercise one successful call and a meaningful failure. For JSON, test malformed input and a wrong field type; for numeric operations, test the accepted limits; for file operations, test existing-file behavior using temporary storage. Run external effects with the scope the task authorizes: `run` executes the app, and a Rust adapter can access files, networks, and processes.

Use the [local Rust library](../test-nagi-code/rust-library/README.en.md), [Rust JSON](../test-nagi-code/library-examples/rust-json/README.en.md), and [Axum source](../test-nagi-code/application-examples/axum-service/native.rs) as executable evidence. Declaration/configuration tests are in [`compiler/tests/rust_dependencies.rs`](../compiler/tests/rust_dependencies.rs), and standard server behavior is implemented in [`runtime/src/http_server.rs`](../runtime/src/http_server.rs). A successful `check` is the first validation step; a successful Rust build and the relevant behavior check finish it.
