# SQLite settings API

Read SQLite settings into a typed class and return them as JSON. Startup inserts three sample rows containing NULL, booleans, floating-point numbers, text, and BLOBs. Existing rows are preserved.

Use Nagi 0.1.9 and run from the repository root. The 0.1.8 release cannot read this example's `bool?`, `f64?`, and `bytes?` fields from SQLite rows.

```sh
nagic run --project test-nagi-code/application-examples/device-settings/nagi.toml
```

```sh
curl http://127.0.0.1:8091/devices
curl http://127.0.0.1:8091/devices/1
```

`GET /devices` returns all rows. `GET /devices/{id}` returns one row, with 404 for an unknown ID and 400 for a non-integer ID. BLOBs become JSON integer arrays; SQL NULL becomes JSON `null`.

The server binds to localhost. Set `NAGI_SAMPLE_PORT` or `NAGI_SAMPLE_DB` to choose a port or SQLite file. Defaults are port 8091 and `settings.sqlite` in the application's working directory.

This is a read-only sample without authentication. It does not include settings update routes or transactions spanning multiple operations.

`smoke.py` checks HTTP values and types, stops the server, modifies one SQLite row, and restarts with the same file. It verifies that startup preserves saved data and shutdown releases the listener. See the [parent README](../README.en.md) for the shared verification command.
