# Nagi Tasks: a small browser-based task manager

This example embeds HTML/CSS/JavaScript and uses a Nagi JSON API with SQLite. It supports adding, editing, completing, deleting, filtering, and aggregate counts. The browser interface uses no UI library or CDN. HTTP and database operations use Rust libraries inside the Nagi runtime.

## Start

Run from the repository root:

```powershell
cargo build --release --locked -p nagic
.\target\release\nagic.exe run --project test-nagi-code/web-demo
```

Open [http://127.0.0.1:8091](http://127.0.0.1:8091) in your browser. `NAGI_PORT` changes the port; `NAGI_DB` changes the database path. Project mode defaults to `test-nagi-code/web-demo/nagi-tasks.sqlite` and retains data across restarts. The `run tasks.nagi` form and direct executable execution use the starting working directory. To reuse an existing database, give its absolute path in `NAGI_DB`.

For a distributable Windows x64 executable:

```powershell
.\scripts\build_windows_demo.ps1 -Source test-nagi-code/web-demo/tasks.nagi
.\build\distribution\nagi-tasks.exe
```

Use `-Offline` if dependencies are cached. HTML, SQLite, and the VC++ runtime are built into the exe; distribute that one executable. Saved user data lives in a separate SQLite file. The receiving machine does not need Rust, Nagi, or Python. Stop with Ctrl+C in the terminal.

## Read the code

| File | Purpose |
|---|---|
| [nagi.toml](../../test-nagi-code/web-demo/nagi.toml) | Entry configuration shared by CLI and VS Code |
| [tasks.nagi](../../test-nagi-code/web-demo/tasks.nagi) | HTTP handlers and startup |
| [models.nagi](../../test-nagi-code/web-demo/models.nagi) | JSON and database types |
| [validation.nagi](../../test-nagi-code/web-demo/validation.nagi) | Input validation |
| [index.html](../../test-nagi-code/web-demo/index.html) | Browser UI and API calls |

The UI is `GET /`. APIs are `GET/POST /api/tasks`, `GET/PUT/DELETE /api/tasks/{id}`, and `GET /api/stats`. Create/update bodies look like `{"title":"A task","done":false}`. Titles must be 1–240 UTF-8 bytes; the list shows the latest 100 tasks, while counts cover all tasks.

The example is a local loopback application. It does not include authentication or production hosting configuration.

## Check an already running server

```powershell
python test-nagi-code/web-demo/smoke_api.py --base-url http://127.0.0.1:8091
```

Python only sends HTTP requests and checks responses. It deletes the test data it created through the API and retains existing tasks. It neither starts/stops the server nor accesses database files.
