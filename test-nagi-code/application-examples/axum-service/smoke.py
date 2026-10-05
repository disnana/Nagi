"""Call an Axum server that awaits a named Nagi async business function."""
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
    directory = directory.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    with socket.socket() as available:
        available.bind(("127.0.0.1", 0))
        port = available.getsockname()[1]
    results = []
    stdout_path = directory / "server.stdout"
    stderr_path = directory / "server.stderr"
    with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open("w", encoding="utf-8") as stderr:
        process = subprocess.Popen(
            [str(executable.resolve())], cwd=directory,
            env=dict(env, NAGI_SAMPLE_PORT=str(port)), stdout=stdout, stderr=stderr,
        )

        def request(method, path, body=None, content_type="application/json"):
            connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
            headers = {} if content_type is None else {"Content-Type": content_type}
            try:
                connection.request(method, path, body=body, headers=headers)
                response = connection.getresponse()
                return response.status, dict(response.getheaders()), response.read()
            finally:
                connection.close()

        def check(name, method, path, status, body=None, *, expected=None, content_type="application/json"):
            actual, headers, output = request(method, path, body, content_type)
            assert actual == status, (name, actual, output)
            if expected is not None:
                assert headers.get("content-type", "").startswith("application/json"), (name, headers)
                assert json.loads(output) == expected, (name, output)
            results.append({"case": name, "status": actual})
            return output

        def check_split_missing_content_type():
            # Retain the ordinary request() cases above. This extra request sends
            # the finite body in two writes after the headers, without retries.
            body = b'{"quantity":1}'
            connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
            try:
                connection.putrequest("POST", "/quotes")
                connection.putheader("Content-Length", str(len(body)))
                connection.endheaders()
                connection.send(body[:6])
                connection.send(body[6:])
                response = connection.getresponse()
                output = response.read()
                assert response.status == 415, (response.status, output)
                assert output == b"Expected request with `Content-Type: application/json`", output
                results.append({"case": "split-missing-content-type", "status": response.status})
            finally:
                connection.close()

        try:
            deadline = time.monotonic() + 10
            while True:
                assert process.poll() is None, stderr_path.read_text(encoding="utf-8")
                try:
                    if request("GET", "/health")[0] == 200:
                        break
                except (OSError, http.client.HTTPException):
                    pass
                if time.monotonic() >= deadline:
                    raise AssertionError("Axum service did not become ready")
                time.sleep(0.05)
            assert check("health", "GET", "/health", 200) == b"ok\n"
            for quantity in (1, 2, 100):
                check(
                    f"quantity-{quantity}", "POST", "/quotes", 200,
                    json.dumps({"quantity": quantity}).encode(),
                    expected={"quantity": quantity, "total_minor": quantity * 1250},
                )
            failure = {"code": "invalid_quantity", "message": "Quantity must be between 1 and 100"}
            for quantity in (0, -1, 101, 9223372036854775807):
                check(
                    f"invalid-quantity-{quantity}", "POST", "/quotes", 422,
                    json.dumps({"quantity": quantity}).encode(), expected=failure,
                )
            for name, body in (
                ("missing-field", b"{}"),
                ("extra-field", b'{"quantity":1,"coupon":"demo"}'),
                ("wrong-type", b'{"quantity":"1"}'),
                ("outside-i64", b'{"quantity":9223372036854775808}'),
            ):
                check(name, "POST", "/quotes", 422, body)
            check("malformed-json", "POST", "/quotes", 400, b"{")
            check("missing-content-type", "POST", "/quotes", 415, b'{"quantity":1}', content_type=None)
            check_split_missing_content_type()
            # Valid JSON padded past the transport limit distinguishes the
            # body limit from a JSON syntax or field rejection.
            padded = b'{"quantity":1}' + b" " * (4097 - len(b'{"quantity":1}'))
            check("json-body-limit", "POST", "/quotes", 413, padded)
            check("method-not-allowed", "GET", "/quotes", 405)
            check("missing-route", "GET", "/missing", 404)
            check("server-remains-usable", "POST", "/quotes", 200, b'{"quantity":2}',
                  expected={"quantity": 2, "total_minor": 2500})
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
                    raise AssertionError("Axum service did not stop within five seconds") from None
        if os.name != "nt":
            assert process.returncode == 0, stderr_path.read_text(encoding="utf-8")
        assert stderr_path.read_text(encoding="utf-8") == ""
    with socket.socket() as released:
        released.settimeout(1)
        assert released.connect_ex(("127.0.0.1", port)) != 0, "listener remained open"
    for port in ("0", "65536", "not-a-number"):
        failure = subprocess.run(
            [str(executable.resolve())], cwd=directory,
            env=dict(env, NAGI_SAMPLE_PORT=port), capture_output=True,
            text=True, encoding="utf-8", timeout=5,
        )
        assert failure.returncode != 0 and failure.stderr, (port, failure)
        results.append({"case": "invalid-port", "value": port, "status": "rejected"})
    (directory / "smoke-results.json").write_text(
        json.dumps(results, ensure_ascii=False, indent=2) + "\n", encoding="utf-8",
    )
    return {"cases": len(results), "starts": 1, "invalid_configurations": 3, "result": "passed"}
