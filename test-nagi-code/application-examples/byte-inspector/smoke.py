"""Verify byte iteration and indexing through real HTTP requests."""
from __future__ import annotations

import http.client
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import time


def verify(executable: Path, env: dict, directory: Path) -> dict:
    directory.mkdir(parents=True, exist_ok=True)
    with socket.socket() as available:
        available.bind(("127.0.0.1", 0))
        port = available.getsockname()[1]
    errors = directory / "server.stderr"
    cases = []
    with (directory / "server.stdout").open("w", encoding="utf-8") as stdout, errors.open("w", encoding="utf-8") as stderr:
        process = subprocess.Popen(
            [str(executable.resolve())], cwd=directory,
            env=dict(env, NAGI_SAMPLE_PORT=str(port), NAGI_HTTP_AUTHORITY=f"127.0.0.1:{port}"), stdout=stdout, stderr=stderr,
        )

        def request(method, path, body=b""):
            connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
            try:
                connection.request(method, path, body=body,
                                   headers={"Content-Type": "application/octet-stream"})
                response = connection.getresponse()
                return response.status, dict(response.getheaders()), response.read()
            finally:
                connection.close()

        try:
            deadline = time.monotonic() + 10
            while True:
                assert process.poll() is None, errors.read_text(encoding="utf-8")
                try:
                    status, _, body = request("GET", "/health")
                    assert status == 200 and body == b"byte inspector"
                    break
                except (OSError, http.client.HTTPException):
                    if time.monotonic() >= deadline:
                        raise AssertionError("byte inspector did not become ready") from None
                    time.sleep(0.05)
            inputs = {
                "empty": b"",
                "ascii": b"ABC",
                "zero-and-high-bytes": bytes([0, 128, 255]),
                "all-byte-values": bytes(range(256)),
                "utf8": "東京・凪".encode("utf-8"),
                "invalid-utf8": b"\xff\xfe\x00",
                "maximum-body": b"\xff" * 4096,
            }
            for name, value in inputs.items():
                status, _, body = request("POST", "/bytes", value)
                assert status == 200, (name, status, body)
                expected = {"length": len(value), "total": sum(value),
                            "first": value[0] if value else None}
                assert json.loads(body) == expected, (name, body, expected)
                cases.append(name)
            status, headers, body = request("HEAD", "/health")
            assert status == 200 and body == b"" and int(headers["content-length"]) > 0
            cases.append("head")
            status, _, _ = request("GET", "/bytes")
            assert status == 405
            cases.append("method")
            status, _, _ = request("GET", "/missing")
            assert status == 404
            cases.append("missing-route")
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
                    raise AssertionError("byte inspector did not stop") from None
        if os.name != "nt":
            assert process.returncode == 0, errors.read_text(encoding="utf-8")
        assert errors.read_text(encoding="utf-8") == ""
    return {"cases": len(cases), "checks": cases}
