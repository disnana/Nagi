"""Verify SQLite rows over HTTP, then reopen the same database."""

import http.client
import json
import os
from pathlib import Path
import signal
import socket
import sqlite3
import subprocess
import tempfile
import time


def verify(executable: Path, env: dict, output: Path) -> dict:
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    initial = [
        {"id": 1, "enabled": True, "gain": 1.25, "label": None, "calibration": [0, 1, 255]},
        {"id": 2, "enabled": None, "gain": None, "label": "front-door", "calibration": None},
        {"id": 3, "enabled": False, "gain": 0.5, "label": "side-door", "calibration": []},
    ]
    results = []
    with tempfile.TemporaryDirectory(prefix="settings-", dir=output) as temporary:
        database = Path(temporary) / "settings.sqlite"
        for run_number in (1, 2):
            with socket.socket() as available:
                available.bind(("127.0.0.1", 0))
                port = available.getsockname()[1]
            child_env = dict(env, NAGI_SAMPLE_DB=str(database), NAGI_SAMPLE_PORT=str(port))
            stdout_path = output / f"server-{run_number}.stdout"
            stderr_path = output / f"server-{run_number}.stderr"
            with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open("w", encoding="utf-8") as stderr:
                process = subprocess.Popen(
                    [str(executable.resolve())], env=child_env, cwd=temporary,
                    stdout=stdout, stderr=stderr,
                )

                def request(path):
                    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=2)
                    try:
                        connection.request("GET", path)
                        response = connection.getresponse()
                        body = response.read()
                        return response.status, response.getheader("Content-Type"), body
                    finally:
                        connection.close()

                try:
                    deadline = time.monotonic() + 10
                    while True:
                        assert process.poll() is None, stderr_path.read_text(encoding="utf-8")
                        try:
                            ready = request("/devices")
                            break
                        except (OSError, http.client.HTTPException):
                            if time.monotonic() >= deadline:
                                raise AssertionError("device server did not become ready")
                            time.sleep(0.05)
                    expected = initial if run_number == 1 else [
                        dict(initial[0], enabled=False, gain=2.5, label="東京", calibration=[255, 0, 128]),
                        *initial[1:],
                    ]
                    assert ready[0] == 200 and ready[1] == "application/json", ready
                    rows = json.loads(ready[2])
                    assert rows == expected, rows
                    assert all(type(row["enabled"]) is bool for row in rows if row["enabled"] is not None), rows
                    assert all(type(row["gain"]) is float for row in rows if row["gain"] is not None), rows
                    assert all(type(byte) is int for byte in rows[0]["calibration"]), rows
                    for row in expected:
                        status, content_type, body = request(f"/devices/{row['id']}")
                        assert status == 200 and content_type == "application/json", (status, body)
                        assert json.loads(body) == row, body
                    status, content_type, body = request("/devices/999")
                    assert status == 404 and content_type == "application/json", (status, body)
                    assert json.loads(body) == {"error": "not found"}, body
                    status, _, body = request("/devices/not-a-number")
                    assert status == 400, (status, body)
                    results.append({"run": run_number, "rows": rows, "missing_id": 404, "invalid_id": 400})
                finally:
                    if process.poll() is None:
                        if os.name == "nt":
                            process.terminate()
                        else:
                            process.send_signal(signal.SIGINT)
                        try:
                            process.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            process.kill()
                            process.wait(timeout=5)
                            raise AssertionError("device server did not stop within five seconds")
                if os.name != "nt":
                    assert process.returncode == 0, stderr_path.read_text(encoding="utf-8")
                assert stderr_path.read_text(encoding="utf-8") == ""
                with socket.socket() as released:
                    released.settimeout(1)
                    assert released.connect_ex(("127.0.0.1", port)) != 0, "listener remained open"
            if run_number == 1:
                with sqlite3.connect(database) as connection:
                    connection.execute(
                        "UPDATE devices SET enabled=?, gain=?, label=?, calibration=? WHERE id=?",
                        (False, 2.5, "東京", bytes([255, 0, 128]), 1),
                    )
        (output / "results.json").write_text(
            json.dumps(results, ensure_ascii=False, indent=2), encoding="utf-8",
        )
    return {"cases": 12, "starts": 2, "result": "passed"}
