# Build libraries and use the Rust ecosystem

You can write application logic in Nagi, share functions across files, and call Rust libraries. The direction is to expose common operations through Nagi APIs and use Rust adapters for advanced features. This page describes file imports and Rust integration that are available today.

## Share a foundation between applications

The [pricing CLI](../../test-nagi-code/library-examples/foundation-cli/README.en.md) and [JSON report](../../test-nagi-code/library-examples/foundation-report/README.en.md) import the same pricing library. A shared Nagi file defines validation and public data types; Rust provides a calculation implementation.

```text
library-examples/
  shared/              Shared Nagi declarations and Rust implementation
  foundation-cli/      An application that reads input and produces a quote
  foundation-report/   An application that produces several quotes as JSON
```

Import the shared file with a relative path such as `import "../shared/foundation.nagi"`. Each application's `nagi.toml` selects a Rust entry file, which loads the shared Rust module. Keep the sample directory structure together.

This traditional import loads definitions into one namespace. Use `import "../shared/foundation.nagi" as foundation` to call the file's own functions as `foundation.function_name`. A from statement can also alias a class or function. The [module example](../../test-nagi-code/library-examples/module-imports/README.en.md) distinguishes same-named classes from different files. This file-based reuse is separate from publishing a Cargo crate or using a Nagi package manager.

## Keep the Rust boundary small

The [Rust integration reference](modules-and-rust.md) describes `@rust`, `extern def`, and corresponding types.

To use a Rust crate, provide Rust functions that Nagi can call. Convert crate-specific types to Nagi numbers, strings, classes, or Result at that boundary. Application code can then use the library without depending on its internal Rust types.

The [JSON reader](../../test-nagi-code/library-examples/rust-json/README.en.md) uses `serde_json` to read a Nagi class from borrowed text and returns failures through Result. The [async example](../../test-nagi-code/library-examples/rust-async/README.en.md) awaits a Tokio timer from Nagi.

Nagi's `check` validates declarations and calls. A `build` checks that the Rust implementation matches those declarations. Arbitrary Rust types, traits, and generics are not exposed directly to Nagi, and users cannot register their own opaque resource types. The library author manages blocking operations and custom shared state on the Rust side.

## Pass application logic to a Rust foundation

A synchronous function type lets Nagi pass a function to Rust. The [custom HTTP foundation](../../test-nagi-code/library-examples/custom-http/README.en.md) uses this approach.

Axum/Tokio handles accepting HTTP requests and stopping the server; a Nagi function produces the response. The example uses a Rust foundation without calling Nagi's built-in `serve` or passing a Db.

Nagi's built-in HTTP server limits do not automatically apply to this server. The sample README lists the limits it sets. Generated applications still depend on the common runtime, so SQLite remains a build dependency even if the application never uses Db.

## Replace High code with Low

The [Low calculation kernel](../../test-nagi-code/library-examples/low-kernel/README.en.md) replaces a High function with a Low implementation of the same type. Calls from application code stay the same, and no Rust adapter is required.

Moving code to Low does not guarantee a speedup. Check that results match, then measure performance or allocations where needed.

## Further library design

The following items need design work; they are not available configuration or syntax:

- Package discovery and visibility declarations for user libraries. Relative-file `as` and `from` imports, and unquoted imports of registered standard modules, are already available.
- Optional HTTP and DB standard libraries, to omit unused build dependencies.
- Ownership, sharing, shutdown, and async cancellation rules for custom connections and clients.
- Separate modules and connection types for SQLite and PostgreSQL, with consistent rules for typed SQL arguments, row decoding, and errors. SQL dialect and transaction differences remain explicit.

These changes should preserve existing file imports, extern declarations, and SQLite calls. Read the [design proposal](library-design.md) or browse the [sample projects](library-examples.md) for programs that work with current features.
