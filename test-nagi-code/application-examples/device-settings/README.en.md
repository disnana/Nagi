# SQLite settings API

Read SQLite settings into a typed class and return them as JSON. Startup inserts three sample rows containing NULL, booleans, floating-point numbers, text, and BLOBs. Existing rows are preserved.

Build the compiler from unreleased development source with SF05 Query/Pool/Tx and run from the repository root. Published 0.1.x binaries do not provide these APIs.

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
