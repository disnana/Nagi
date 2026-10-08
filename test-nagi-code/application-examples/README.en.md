# Application examples

[日本語](README.md)

| Project | Contents |
| --- | --- |
| [Stock report CLI](stock-report/README.en.md) | JSON input including Japanese text, validation, custom errors and aggregation |
| [SQLite settings API](device-settings/README.en.md) | NULL, boolean, float and BLOB columns, HTTP responses and persistence across restarts |
| [Reservation workers](seat-reservations/README.en.md) | Business errors, supervised restarts and independent actor state |
| [JSON configuration file](file-json/README.en.md) | Typed JSON, Rust file operations and protection of existing files |
| [Quote API](quote-api/README.en.md) | HTTP without a database, shared configuration, custom errors and route-specific error mapping |
| [Supervised workers](supervised-worker/README.en.md) | Actor restarts, recovery from a task panic, shutdown and cleanup |
| [Byte inspector API](byte-inspector/README.en.md) | Borrowed request bodies, u8 iteration and indexing, nullable values and JSON responses |
| [Axum quote API](axum-service/README.en.md) | Rust HTTP calls Nagi async business logic and converts its Result to a response |
| [Authentication and authorization boundaries](auth-boundary/README.en.md) | Standard HTTP policy, dispatcher-created AuthScope, Nagi policy, and a consumed Grant for a protected DB operation |

To start with handwritten Low, use the [order quote CLI](../low-examples/order-quote/README.en.md). It combines Low-to-Low imports, typed JSON, input validation and integer price calculations.

The byte inspector requires Nagi 0.1.10 or later; the Axum quote API is verified with 0.1.10. The authentication example uses this branch's standard HTTP request-bound auth API. Other existing examples also run on Nagi 0.1.9. Check the official release record before relying on the new API in a published compiler. Download or clone the repository, then run each folder with its `nagi.toml` from the repository root. See [setup](../../docs/en/getting-started.md) for Rust/Cargo and your OS's build tools.

```sh
nagic run --project test-nagi-code/application-examples/stock-report
```

The script covers ten projects: nine High applications checked and built both from source and from saved generated Low, plus the order quote CLI built from handwritten Low. These nineteen verification runs compare input, file, HTTP, and actor behavior. The authentication example requires the compiler from this branch. Run from the repository root with Python 3.12 or later.

```sh
python scripts/verify_application_examples.py --compiler /path/to/nagic
python scripts/verify_application_examples.py --compiler /path/to/nagic --only device-settings
python scripts/verify_application_examples.py --compiler /path/to/nagic --only supervised-worker
python scripts/verify_application_examples.py --compiler /path/to/nagic --only order-quote
```

Build warnings count as failures. Results, build logs and execution logs are written to `build/application-example-verification/`. Database and file checks use temporary storage. Quote API checks start and stop a server on localhost. The supervised worker prints an intentional panic diagnostic to stderr, verifies recovery and cleanup, and exits with code 0.
