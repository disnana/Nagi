# File imports and Rust integration

Load another Nagi file with `import "filename"`. To call Rust, declare the function's parameter and return types in Nagi and supply a Rust file when building.

## Split Nagi code into files

Create two files in the same directory. Put this class in `models.nagi`:

```nagi
class Item:
    name: str
    count: i64
```

Import it from `app.nagi`:

```nagi
import "models.nagi"

def main():
    item = Item(name="Nagi", count=42)
    print(item.name)
    print(item.count)
```

```sh
nagic run app.nagi
```

The program prints `Nagi` and `42`. Paths are relative to the file containing the import. High imports `.nagi`; Low imports `.low`. Low allows a semicolon after an import.

Each file is loaded once. Import cycles, missing files, mixed High/Low files, and duplicate names are errors. Imported definitions enter the same namespace. Named modules such as `import sqlite`, `as`, `from`, and visibility declarations are unsupported.

### Import limits and error locations

The limits are 128 files, depth 64, 8 MB total, and 2 MB per file. Type-checking and Low integration errors report original filenames and line numbers. Errors in generated Rust also show the corresponding Nagi or Low line first. If no source line can be identified, or an error is in handwritten Rust, the compiler reports the Rust location. Column mapping is not yet implemented.

### Embed a text file

`include_text("index.html")` reads a neighboring UTF-8 file at build time and embeds it as a `str`. Specify the path as a string literal. The original file is not needed when running the distributed application.

## Call Rust functions

Save this as `app.nagi`. `@rust` names the Rust function, and `extern def` declares its parameter and return types. The declaration has no body or trailing colon.

```nagi
@rust("native::text_bytes")
extern def text_bytes(text: view[str]) -> i64

def main():
    text = "Nagi"
    print(text_bytes(view(text)))
    print(text)
```

Put the Rust function in `native.rs` in the same directory:

```rust
pub fn text_bytes(text: &str) -> i64 {
    text.len() as i64
}
```

```sh
nagic run app.nagi --rust native.rs
```

The result is `4`, followed by `Nagi`. The function borrows the string through `view[str]`, so it remains available afterward. `--rust` includes the Rust file as the `native` module. Make the function `pub` and match the declared types. For example, Nagi's `str` is Rust's `String`, and `view[str]` is `&str`.

Low uses `extern fn ...;`. To call a Rust async function, use `extern async def` (`extern async fn` in Low) and await it at the call site. Rust function paths are sequences of identifiers beginning with `native::`. External functions cannot declare a view return type or carry HTTP attributes.

### Use a Rust crate

Add Cargo dependencies with `--rust-dep NAME=VERSION`. For an adapter using serde_json, pass:

```sh
nagic run app.nagi --rust native.rs --rust-dep serde_json=1.0
```

Cargo normally needs network access to download dependencies on the first build. See the [repository's Rust integration example](../../test-nagi-code/rust-bridge/) for a complete example using a crate. Save your entry file, Rust file, and dependencies in [nagi.toml](projects.md) to reuse them in the CLI and VS Code.

Nagi's `check` validates the declared types, calls, ownership, and borrowing. It does not inspect Rust bodies or crate APIs. A `build` checks that the Rust implementation matches its declaration. Adapt Rust-specific types to numbers, str, List, classes, or Result before passing them to Nagi. Refer to a generated Nagi class from Rust as `super::TypeName`.

To reuse the same dependency resolution, retain the generated Cargo.lock and run `cargo build --locked --manifest-path build/app/Cargo.toml`. A normal `nagic build` runs `cargo build --release` on the generated project.

This integration calls functions within the same Rust build. A stable C ABI and runtime DLL loading are unsupported.

See [libraries and Rust assets](libraries.md) and [sample projects](library-examples.md) for shared code and callbacks passed from Nagi to Rust. Dependency path/features settings and namespaces are covered in the [design proposal](library-design.md).
