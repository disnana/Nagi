# Library and Rust integration design

Common operations will use typed Nagi APIs backed by existing Rust libraries. Advanced operations and custom foundations will use Rust integration. This serves both developers writing applications without knowing Rust and developers combining Rust libraries.

This page includes unimplemented proposals. For current usage, see [imports and Rust](modules-and-rust.md), [nagi.toml](projects.md), and the [sample projects](library-examples.md).

## Available features and missing pieces

| Area | Available today | Unsupported or undecided |
|---|---|---|
| Imports | Relative files, `as`, multiple-name `from`, registered standard modules | Package management and visibility declarations |
| Standard APIs | `std.http.server`, `std.actor` | Separate JSON and database modules |
| Standard resources | App, Request, Response, Actor, Supervisor, and related types | User-defined resource registration |
| Rust integration | Sync/async extern functions, typed arguments/results, handwritten adapters | Direct use of arbitrary Rust types/traits and a stable external ABI |
| Cargo dependencies | Version, path, features, default-features, package | Generation that excludes unused runtime dependencies |
| Database | SQLite, fixed bind shapes, conversion of rows into classes | PostgreSQL, general parameters, pools, and transaction APIs |

`check` checks extern declarations and Nagi calls. `build` checks them against Rust implementations and crates. Extern functions cannot return views.

## Shared logic and adapters

Put types, validation, and calculations in Nagi modules and use them from CLI, HTTP, or other entry points. Rust adapters convert external types into Nagi-supported types. An independent Rust crate uses its own types; the adapter converts them to generated application classes.

The [HTTP authentication example](../../test-nagi-code/library-examples/http-auth/README.en.md) uses standard APIs for headers and business errors. [custom-http](../../test-nagi-code/library-examples/custom-http/README.en.md) calls synchronous Nagi callbacks from a Rust Axum server. Its Rust code owns capacities, deadlines, and shutdown; standard HTTP settings do not apply automatically.

## Modules and name resolution

Aliases of the same module refer to the same definitions; same-named classes in different modules are distinct types. Type checks, High/Low, Rust generation, and editors use these identities. Imported names are not automatically re-exported.

Types and operations in `std.http.server` and `std.actor` are registered by the compiler. Standard module names do not resolve to local lookalike files. Adding an ordinary library should not require adding language keywords.

## Cargo dependency settings

See [nagi.toml](projects.md) for supported settings. Paths resolve relative to the manifest, so changing the generated directory preserves the referenced crate. `check`, `lower`, and `symbols` do not invoke Cargo.

Cargo combines features across dependency paths. Rebuilding preserves the generated Cargo.lock, but it does not pin path dependency source contents. `nagic build --locked` is unsupported.

## Selecting runtime features

**Unimplemented proposal.** Separate HTTP, JSON, SQLite, and other dependencies according to application needs. Standard HTTP applications without a database currently still include the runtime's SQLite dependency.

Serde and row conversions, exported types, and error conversion generation also need conditional support. While all functions are emitted, dependencies cannot be removed by inspecting only direct calls from the entry point. Existing Rust adapter compatibility also needs checking.

## Opaque types and asynchronous work

**Proposed contracts for additional resource types.** Current classes represent data; they do not substitute for native clients or connections.

| Contract | Questions to resolve |
|---|---|
| Identity | Map Nagi types to Rust types; distinguish same-named types from different providers |
| Ownership and sharing | Move by default; explicitly permit borrowing, cloning, and sharing by type |
| Borrowing | Keep owners alive across await; distinguish read-only views from exclusive operations |
| Threads | rustc validates native Send/Sync and traits; do not grant them universally |
| Cleanup | Define explicit async close and who owns work that remains after drop |

Drop cannot await. Dropping a future stops the caller waiting without necessarily canceling accepted database work or external sends. Evaluate general traits and new variadic syntax only after establishing whether this boundary requires them.

## Requirements for a general database API

**Separate drivers with common conventions; new APIs are unimplemented.** SQLite and PostgreSQL will use separate modules and resource types, preserving SQL dialect, type, and transaction differences. Module names remain undecided.

| Area | Contract to define first |
|---|---|
| Parameters | Arbitrary typed values, typed NULLs, and ownership for str, bytes, and views |
| Rows | Field names, column order, NULLs, integer ranges, and decoding errors |
| Operations and errors | execute/one/all shapes, absence, connection/wait/SQL/decoding failures |
| Transactions | Hold one connection; consume the caller's handle on commit/rollback; assign cleanup ownership after cancellation |
| Pools | Closure state shared by clones, stopping new acquisitions, and waiting for operations and cleanup |

Serializing transaction operations inside a library differs from rejecting concurrent use in Nagi's `check`. The latter requires additional checking over future lifetimes. The extent of static guarantees remains undecided.

An unconfirmed commit outcome does not imply rollback or safe retry. Evaluate driver-side distinctions between unsent work, explicit database rejection, and response loss after sending.

Do not translate SQL automatically or send BEGIN/COMMIT through different pooled connections. Static SQL/schema checks and runtime checks of actual types/NULLs are separate. Ordinary `check` does not validate SQL. Nagi 0.1.10 supports explicit [SQLite checks](sql-check.md), which do not guarantee actual value types or NULL behavior.

## Comparing HTTP foundations

Evaluate Axum/Tower first against the current standard HTTP implementation. **Adoption is undecided and the comparison has not been run.** Axum also uses Hyper: compare routing and middleware within the same connection management, then evaluate listener and shutdown changes separately.

Match public APIs, admission capacity, body/header limits, deadlines, panic responses, and shutdown. Evaluate throughput, latency, CPU, memory, overload recovery, and maintenance costs. Existing [measurements](http-stdlib-performance.md) are not results of this adoption comparison.

An outgoing HTTP client is also unimplemented. Reuse of reqwest or similar libraries is under consideration; response status versus transport errors, receive limits, deadlines, and resource cleanup need contracts first.

## Implementation order

Address existing type/ownership consistency, native resource contracts, SQLite validation, and PostgreSQL support in that order. Treat HTTP foundation comparison as a separate evaluation and check public API compatibility.

Preserve existing imports, built-ins, Rust integration, and Low compatibility. Check existing examples and editors. Measure dependency reductions through Cargo features, build times, and output size.

References: [Cargo dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html), [features](https://doc.rust-lang.org/cargo/reference/features.html), [Send](https://doc.rust-lang.org/std/marker/trait.Send.html)/[Sync](https://doc.rust-lang.org/std/marker/trait.Sync.html), [Drop](https://doc.rust-lang.org/std/ops/trait.Drop.html), [Axum](https://docs.rs/axum/0.8.9/axum/), [Tower](https://docs.rs/tower/latest/tower/).
