# Library and Rust integration design

This page is a proposal. Module namespaces, detailed Cargo dependency settings, opaque resource types, and a general database API are not implemented. The proposed syntax below is separate from the examples that run today.

[Contents](README.md) · Available features: [imports and Rust integration](modules-and-rust.md), [nagi.toml](projects.md)

The aim is to share Nagi types, validation, and calculations between applications while using existing Rust libraries for networking and storage. CLI tools, HTTP servers, and batch jobs should be able to use the same business rules.

## Available features and missing pieces

| Area | Available today | Proposed addition |
| --- | --- | --- |
| Imports | Relative files loaded into one namespace | Modules, `from`, `as`, and separate definitions with the same name |
| Rust integration | Sync/async extern functions with typed arguments and results | Opaque types representing connections and clients |
| Rust files | One native module selected with `rust.file` | Compatible settings that also support shared crates |
| Cargo dependencies | Crate names and version strings | path, features, default-features, and package |
| Runtime | HTTP, JSON, SQLite, and other dependencies always included | Dependencies and generated code selected by use |
| Database | SQLite with fixed bind argument shapes | Arbitrary typed arguments, row decoding, and transactions |

Extern declarations do not automatically import Rust APIs. Rust-specific types must be converted to supported numbers, strings, classes, lists, or results. Nagi's `check` checks declarations and calls; `build` checks them against Rust implementations. Extern functions cannot currently return views.

## Shared logic and adapters

Keep data types, validation, and calculations in shared Nagi files. Application entry points combine inputs and outputs. Rust adapters convert library types to Nagi data types. An independent Rust crate uses its own types, with conversions to generated application classes kept in the adapter.

The six current examples explore this structure using existing APIs.

| Project | Reuse and integration |
| --- | --- |
| [foundation-cli](../../test-nagi-code/library-examples/foundation-cli/README.en.md) | Uses shared `foundation.nagi` and Rust `pricing.rs` from a CLI |
| [foundation-report](../../test-nagi-code/library-examples/foundation-report/README.en.md) | Uses the same validation and calculations for a JSON report |
| [rust-json](../../test-nagi-code/library-examples/rust-json/README.en.md) | Converts serde_json results to a Nagi class |
| [rust-async](../../test-nagi-code/library-examples/rust-async/README.en.md) | Awaits a Rust Tokio timer on Nagi's runtime |
| [custom-http](../../test-nagi-code/library-examples/custom-http/README.en.md) | Passes a synchronous Nagi callback to a Rust Axum/Tokio server |
| [low-kernel](../../test-nagi-code/library-examples/low-kernel/README.en.md) | Calls handwritten Low calculations from application logic |

One `rust.file` can include multiple Rust files through `mod` or `#[path]`. custom-http does not use the built-in serve or Db. Its Rust code manages HTTP limits and shutdown; built-in HTTP settings do not apply automatically.

## Modules and name resolution

The following syntax is proposed and is not implemented.

```nagi
import json
import sqlite as storage
from json import decode as decode_json
import "domain/orders.nagi" as orders
from "domain/orders.nagi" import Order as SavedOrder
```

`orders.Order` and `SavedOrder` refer to the same definition. Classes with the same name in different files remain different types. Module and definition IDs must be shared by type checking, High-to-Low conversion, Rust generation, and editor tools. Aliases must not be implemented as text replacement.

Standard modules refer to bundled definitions; quoted imports refer to relative files. Importing does not open a database or start networking. Initially, a module exposes functions and classes defined in that file without automatically re-exporting imported names. Existing flat imports and built-ins remain compatibility entry points.

## Cargo dependency settings

The table values below are proposed. Today, dependency values must be version strings such as `serde_json = "1.0"`.

```toml
[rust]
file = "adapters/native.rs"

[rust.dependencies]
serde_json = "1.0"
foundation = { package = "my-foundation", path = "../my-foundation" }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
```

Keep string values and add tables accepting version, path, features, default-features, and package. Resolve paths relative to nagi.toml so changing the generated output directory does not change the referenced crate. Cargo resolves dependencies. Define how CLI version settings override table entries.

Cargo combines features enabled through different dependency paths. Disabling default features on one direct dependency does not disable features enabled elsewhere. Check the actual dependency tree. Versions specify allowed ranges; Cargo.lock records resolved versions. Preserve generated locks and support locked builds. Current nagic build does not pass `--locked`.

## Selecting runtime features

Split the runtime into core, async, json, http, sqlite, and postgres features. Collect requirements from emitted functions, types, and resolved built-ins. While all functions are emitted, inspecting only direct calls from the entry point is insufficient to remove dependencies.

Serde and row implementations, exported types, and error conversions also need conditional generation. Making dependencies optional is only part of the work. Preserve compatibility settings for Rust integration and let new adapters declare runtime requirements. A dependency's feature does not automatically enable a feature with the same name in the generated application.

For built-in HTTP, add `serve(port)` while retaining `serve(db, port)`. Reject the one-argument form if any registered route requires Db. This is separate from existing custom servers such as custom-http.

## Opaque types and asynchronous work

Exposing a client or connection pool requires registered Nagi/Rust type mappings, operations, and ownership rules. Current classes represent data and do not serve as these resource types.

| Contract | Required behavior |
| --- | --- |
| Ownership and sharing | Move by default; no Copy. Explicitly permit sharing or cloning where supported |
| Borrowing | Separate read access from exclusive operations; keep the owner alive across await |
| Identity and conversion | Distinguish same-named types from different providers; do not add JSON/row derives automatically |
| Threads | Send permits transfer between threads; Sync permits shared references. Neither is universal |
| Cleanup | Define close, commit, rollback, and the remaining work on drop |

Ordinary Drop cannot await. Provide explicit operations for required asynchronous cleanup. Dropping a future can cancel the caller without undoing work on another worker or a database write already sent. Define cleanup, retries, and duplicate-write handling for each operation.

## Requirements for a general database API

Current db_insert takes str/i32; db_update takes i64/str/i32. Retain them for compatibility, without describing them as an API for arbitrary tables and conditions.

| Area | Contract to define first |
| --- | --- |
| Parameters | Any number of typed values, typed NULLs, and ownership for str/bytes/views |
| Operations | execute, one, and all; Option for absence; the same rules for queries without binds |
| Rows | Match original field names and handle column order, NULLs, integer ranges, and type errors |
| Transactions | Pin one connection; consume on commit/rollback; reject use after completion and concurrent operations |
| Errors | Distinguish connection, waiting, SQL, decoding, and absence |

Typed variadic arguments are the first option to evaluate. The backend also checks SQL and schema compatibility; dynamic SQL is not guaranteed correct at compile time. Exclusive transaction borrowing needs rules beyond the current read-only view.

Keep SQLite's `?1`, PostgreSQL's `$1`, BIGINT for i64, identity columns, and other dialect differences in the storage layer. Do not translate SQL automatically or send BEGIN/COMMIT through separate pool calls. General PostgreSQL support requires parameter, row, and transaction contracts plus real database tests.

## Implementation order

1. Validate the shared logic and adapter boundaries with current examples, using both Nagi checking and Rust builds.
2. Add detailed dependency settings, runtime selection, conditional derives, and lock preservation.
3. Use one name-resolution system for modules, from/as, High-to-Low conversion, and editors.
4. Separate built-in HTTP startup from databases, removing both workers and SQLite dependencies when unused.
5. Validate resource ownership, borrowing, cancellation, and general database contracts with SQLite.
6. Test the same contracts, TLS, pooling, timeouts, and shutdown against PostgreSQL; demonstrate unchanged business rules with different storage providers.

Retain old imports, built-ins, and Rust integration so applications can migrate incrementally. At each stage, check existing examples, High/Low, editors, and supported operating systems. Measure dependency reductions through Cargo features, build times, and output size.

References: [Cargo dependency settings](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html), [feature unification](https://doc.rust-lang.org/cargo/reference/features.html), [Cargo.lock](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html), [Rust modules](https://doc.rust-lang.org/book/ch07-02-defining-modules-to-control-scope-and-privacy.html), [Send](https://doc.rust-lang.org/std/marker/trait.Send.html)/[Sync](https://doc.rust-lang.org/std/marker/trait.Sync.html), [Future](https://doc.rust-lang.org/std/future/trait.Future.html), [Drop](https://doc.rust-lang.org/std/ops/trait.Drop.html), [Tokio cancellation](https://tokio.rs/tokio/tutorial/select).
