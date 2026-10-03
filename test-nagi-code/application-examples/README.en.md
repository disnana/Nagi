# Application examples

[日本語](README.md)

| Project | Contents |
| --- | --- |
| [Stock report CLI](stock-report/README.en.md) | JSON input including Japanese text, validation, custom errors and aggregation |
| [JSON configuration file](file-json/README.en.md) | Save and read UTF-8 through Rust's standard library, preserving existing files |
| [SQLite settings API](device-settings/README.en.md) | NULL, boolean, float and BLOB columns, HTTP responses and persistence across restarts |
| [Reservation workers](seat-reservations/README.en.md) | Business errors, supervised restarts and independent actor state |

Run each folder with its `nagi.toml`. The settings API requires Nagi built from the latest source. See [setup](../../docs/en/getting-started.md) for Rust/Cargo and your OS's build tools.

```sh
nagic run --project test-nagi-code/application-examples/stock-report
```

From the repository root, check and build all four apps independently from High and saved Low, then repeat the same input, file, HTTP and actor checks for each. Python 3.12 or later is required.

```sh
python scripts/verify_application_examples.py --compiler /path/to/nagic
python scripts/verify_application_examples.py --compiler /path/to/nagic --only device-settings
```

Results, build logs and execution logs are written to `build/application-example-verification/`. SQLite verification uses a temporary database rather than existing application data.
