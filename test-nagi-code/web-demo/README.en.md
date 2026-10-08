# Nagi Tasks JSON API

[日本語](README.md)

A task-management JSON API built with Nagi and SQLite. It supports create, update, completion, deletion, and aggregation. Routes use the standard HTTP API and each route has an explicit public policy. There is no browser UI or authentication.

## Run

Run from the repository root:

```powershell
cargo build --release --locked -p nagic
.\target\release\nagic.exe run --project test-nagi-code/web-demo
```

The default URL is [http://127.0.0.1:8091](http://127.0.0.1:8091). Set `NAGI_PORT` to change the port and `NAGI_DB` to change the database path. Running as a project uses `test-nagi-code/web-demo/nagi-tasks.sqlite` by default and reuses saved data after restart. Set `NAGI_DB` to an absolute path to use an existing database.

`GET /` returns a plain-text note about the JSON API. The former raw-HTML UI is not served through standard HTTP routes. `index.html` is an unconnected reference until typed HTML responses are available. The sample does not label HTML markup as plain text, and its API remains JSON.

To build a Windows x64 distributable executable:

```powershell
.\scripts\build_windows_demo.ps1 -Source test-nagi-code/web-demo/tasks.nagi
.\build\distribution\nagi-tasks.exe
```

Add `-Offline` when dependencies are cached. See the build script for the executable, SQLite, and VC++ runtime packaging. Runtime data stays in a separate SQLite file. Stop the server with Ctrl+C.

## API

| Method and path | Behavior |
|---|---|
| `GET /health` | Explicit public route returning `ok` |
| `GET /api/tasks` | List the latest 100 tasks |
| `GET /api/tasks/{id}` | Read one task |
| `POST /api/tasks` | Create `{"title":"something to do","done":false}` |
| `PUT /api/tasks/{id}` | Replace title and done state |
| `DELETE /api/tasks/{id}` | Delete and return the affected row count |
| `GET /api/stats` | Count all tasks and completed tasks |

Titles contain 1–240 UTF-8 bytes, with a matching database constraint. Authentication, a browser UI, task priority, and due dates are not included.

## Source files

| File | Contents |
|---|---|
| `nagi.toml` | Entry point shared by CLI and VS Code |
| `tasks.nagi` | Standard HTTP App, routes with public policies, and handlers |
| `models.nagi` | JSON and database types |
| `validation.nagi` | Input validation |
| `index.html` | Former UI reference, disconnected until SF04 typed HTML |

## Check a running server

```powershell
python test-nagi-code/web-demo/smoke_api.py --base-url http://127.0.0.1:8091
```

Python only checks HTTP requests and responses. It removes test rows it created through the API and leaves existing tasks alone. It does not start or stop the server or manipulate database files.
