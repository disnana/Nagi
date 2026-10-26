"""Run the same HTTP checks against the deployed FastAPI and Quart apps."""

from contextlib import contextmanager
import importlib.metadata
import os
import secrets
import socket
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import httpx


VERSIONS = {
    "fastapi": "0.162.0",
    "hypercorn": "0.18.0",
    "httpx": "0.28.1",
    "quart": "0.23.1",
    "uvicorn": "0.54.0",
}


def verify_versions() -> None:
    for name, expected in VERSIONS.items():
        actual = importlib.metadata.version(name)
        assert actual == expected, f"{name}: {actual} != {expected}"


def free_port() -> int:
    with socket.socket() as server:
        server.bind(("127.0.0.1", 0))
        return int(server.getsockname()[1])


class ForeignHealthHandler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:
        if self.path != "/health":
            self.send_error(404)
            return
        body = b'"ok"'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args: object) -> None:
        pass


@contextmanager
def serve(module: str, server: str, database: Path):
    environment = os.environ.copy()
    environment["TASKS_DB"] = str(database)
    failures = []
    for _ in range(5):
        # The test servers do not expose a portable pre-bound-socket option. A
        # bounded retry closes the unavoidable allocate-then-bind race.
        port = free_port()
        command = [sys.executable, "-m", server]
        if server == "uvicorn":
            command += [f"{module}:app", "--host", "127.0.0.1", "--port", str(port)]
        else:
            command += [f"{module}:app", "--bind", f"127.0.0.1:{port}"]
        ready_token = secrets.token_urlsafe(32)
        environment["TASKS_TEST_READY_TOKEN"] = ready_token
        with tempfile.TemporaryFile(mode="w+", encoding="utf-8") as log:
            process = subprocess.Popen(
                command,
                cwd=Path(__file__).parent,
                env=environment,
                stdout=log,
                stderr=subprocess.STDOUT,
            )
            base_url = f"http://127.0.0.1:{port}"
            deadline = time.monotonic() + 15
            try:
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        break
                    ready = False
                    try:
                        response = httpx.get(f"{base_url}/health", timeout=0.5)
                        ready = (
                            response.status_code == 200
                            and response.json() == "ok"
                            and response.headers.get("X-Task-Ready") == ready_token
                        )
                    except (httpx.HTTPError, ValueError):
                        pass
                    if ready and process.poll() is None:
                        yield base_url
                        return
                    time.sleep(0.05)
                log.seek(0)
                failures.append(log.read())
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)
    raise RuntimeError(f"{server} did not start after 5 attempts:\n" + "\n".join(failures))


def verify_app(module: str, server: str, database: Path, invalid_status: int) -> None:
    with serve(module, server, database) as base_url, httpx.Client(base_url=base_url) as client:
        assert client.get("/health").json() == "ok"
        created = client.post("/api/tasks", json={"title": f"{module} task", "done": False})
        assert created.status_code == 200
        task = created.json()
        task_id = task["id"]
        assert client.get(f"/api/tasks/{task_id}").json() == task
        assert client.get("/api/tasks").json() == [task]
        updated = client.put(
            f"/api/tasks/{task_id}", json={"title": f"{module} done", "done": True}
        )
        assert updated.status_code == 200
        assert updated.json()["done"] is True
        assert client.get("/api/stats").json() == {"total": 1, "completed": 1}
        assert client.post("/api/tasks", json={"title": "", "done": False}).status_code == invalid_status
        assert client.post("/api/tasks", json={"title": "x", "done": 1}).status_code == invalid_status
        assert client.post(
            "/api/tasks", json={"title": "x", "done": False, "extra": 1}
        ).status_code == invalid_status
        assert client.post(
            "/api/tasks",
            content=b'{"title":"\\ud800","done":false}',
            headers={"Content-Type": "application/json"},
        ).status_code == invalid_status
        assert client.get("/api/tasks/999999").status_code == 404
        assert client.get("/api/tasks/9223372036854775808").status_code == 400
        disposable = client.post(
            "/api/tasks", json={"title": "delete me", "done": False}
        ).json()
        assert client.delete(f"/api/tasks/{disposable['id']}").json() == 1
        assert client.get(f"/api/tasks/{disposable['id']}").status_code == 404

    with serve(module, server, database) as base_url, httpx.Client(base_url=base_url) as restarted:
        assert restarted.get(f"/api/tasks/{task_id}").json() == updated.json()
        assert restarted.get("/api/stats").json() == {"total": 1, "completed": 1}


def verify_foreign_listener_is_rejected(database: Path) -> None:
    foreign = ThreadingHTTPServer(("127.0.0.1", 0), ForeignHealthHandler)
    thread = threading.Thread(target=foreign.serve_forever)
    thread.start()
    original_free_port = globals()["free_port"]
    candidates = iter([foreign.server_port])

    def first_foreign_then_free() -> int:
        try:
            return next(candidates)
        except StopIteration:
            return original_free_port()

    globals()["free_port"] = first_foreign_then_free
    try:
        with serve("fastapi_app", "uvicorn", database) as base_url:
            assert base_url != f"http://127.0.0.1:{foreign.server_port}"
    finally:
        globals()["free_port"] = original_free_port
        foreign.shutdown()
        foreign.server_close()
        thread.join(timeout=5)
        assert not thread.is_alive()


def main() -> None:
    verify_versions()
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        verify_foreign_listener_is_rejected(root / "foreign-race.sqlite")
        verify_app("fastapi_app", "uvicorn", root / "fastapi.sqlite", 422)
        verify_app("quart_app", "hypercorn", root / "quart.sqlite", 400)
    print("PASS: FastAPI and Quart CRUD, validation, and restart persistence")


if __name__ == "__main__":
    main()
