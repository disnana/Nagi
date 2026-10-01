# File imports and Rust integration

Use [projects and nagi.toml](projects.md) to save your entry file, Rust file, and crate dependencies. The CLI and VS Code share this configuration.

## Split Nagi code into files

```nagi
import "models.nagi"
import "validation.nagi"
```

Paths are relative to the source file containing the import. High imports `.nagi`; Low imports `.low`. Low permits a terminating `;`. Dependencies are read first, and each file is loaded once. Cycles, missing files, mixed languages, and duplicate names are errors. All files currently enter one namespace; named modules, aliases, selective imports, and visibility are not implemented.

Limits are 128 files, depth 64, and 8 MB total. Each parser also retains its 2 MB per-file limit. Type checking and Low integration report original imported filenames and lines. Rust backend diagnostics also show the original Nagi or Low location first when a corresponding statement or definition line is known. Unmapped diagnostics and handwritten Rust retain Rust's output. Precise column mappings are not implemented.

`include_text("index.html") -> str` embeds a neighboring UTF-8 file at compile time. Its path must be a string literal. It does not read the file again on the machine running the distributed program.

## Call Rust functions

Declare a typed external function in Nagi. A High `extern def` has no body or trailing `:`.

```nagi
@rust("native::crc32")
extern def crc32(text: view[str]) -> i64

@rust("native::pretty_json")
extern def pretty_json(text: view[str]) -> Result[str, Error]
```

Low uses `extern fn ...;`. `extern async def`/`extern async fn` call Rust async functions; await them like other async functions. Rust paths are identifier sequences beginning with `native::`. External functions currently cannot return views or carry HTTP attributes.

Write an ordinary Rust function:

```rust
pub fn pretty_json(text: &str) -> Result<String, nagi_runtime::Error> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| nagi_runtime::Error::invalid(e.to_string()))?;
    serde_json::to_string_pretty(&value)
        .map_err(|e| nagi_runtime::Error::invalid(e.to_string()))
}
```

```powershell
.\target\release\nagic.exe run test-nagi-code/rust-bridge/bridge.nagi `
  --rust test-nagi-code/rust-bridge/native.rs --rust-dep serde_json=1.0
```

`--rust` adds one Rust file as the `native` module. `--rust-dep NAME=VERSION` adds a Cargo dependency. Cargo resolves these dependencies, normally requiring network access on the first build. Retain the generated Cargo.lock and use `cargo build --locked --manifest-path build/bridge/Cargo.toml` to reuse the resolution. The CLI itself invokes `cargo build --release` for the generated crate.

Nagi checks parameter/return declarations and move/view rules. During a build, rustc checks that the Rust implementation matches them. `nagic check` does not inspect Rust bodies or crate APIs. Rust adapters can use the standard library, crates, ordinary modules, and `unsafe`. Refer to a generated Nagi value type as `super::TypeName`.

These calls happen within the same Rust build. They do not provide a stable C ABI or runtime DLL loading. Direct exposure of Rust-specific types and Nagi raw-pointer/unsafe syntax are not implemented. Adapt values to primitives, str, List, classes, and Result in Rust first.
