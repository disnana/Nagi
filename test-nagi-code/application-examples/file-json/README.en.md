# JSON configuration file

Create typed JSON in Nagi, save it as UTF-8, and read it back. A short Rust adapter uses `std::fs` for file operations; Nagi defines and checks the configuration. No additional Rust crate is required.

Run from the repository root. Use `--project` so the compiler includes the Rust file specified in `nagi.toml`.

```sh
nagic run --project test-nagi-code/application-examples/file-json
```

Application output:

```text
{"site":"東京","enabled":true,"retries":3}
configuration.json
file-json: OK
```

Set `NAGI_SAMPLE_FILE` to choose the destination. The default is `configuration.json` in the application's working directory. `nagic run --project` creates it in the project directory.

The adapter uses Rust's `create_new` and does not overwrite existing files. Running again with the same destination writes a reason to stderr and exits with code 1. This example does not implement configuration updates or atomic writes.

`smoke.py` verifies reading and writing through paths containing Japanese text and spaces. It also checks that existing JSON and configuration files are preserved and that a directory cannot be used as the destination. The [shared verification command](../README.en.md) uses temporary directories.
