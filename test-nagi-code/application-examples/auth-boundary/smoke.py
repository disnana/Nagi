"""Real HTTP checks for the same router with Rust or Nagi authorization policy."""
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
    results = []
    for mode in ("nagi", "rust"):
        with socket.socket() as available:
            available.bind(("127.0.0.1", 0))
            port = available.getsockname()[1]
        stdout_path = directory / f"{mode}.stdout"
        stderr_path = directory / f"{mode}.stderr"
        with stdout_path.open("w") as stdout, stderr_path.open("w") as stderr:
            process = subprocess.Popen([str(executable.resolve())], env=dict(env, NAGI_SAMPLE_PORT=str(port), NAGI_HTTP_AUTHORITY=f"127.0.0.1:{port}", NAGI_AUTH_POLICY_MODE=mode, NAGI_AUTH_PROBES="1"), stdout=stdout, stderr=stderr)

            def request(method, path, body=None, credential=None, extra=()):
                connection = http.client.HTTPConnection("127.0.0.1", port, timeout=4)
                try:
                    connection.putrequest(method, path)
                    if credential is not None:
                        connection.putheader("Authorization", credential)
                    for key, value in extra:
                        connection.putheader(key, value)
                    if body is not None:
                        connection.putheader("Content-Type", "application/json")
                        connection.putheader("Content-Length", str(len(body)))
                    connection.endheaders(body)
                    response = connection.getresponse()
                    return response.status, dict(response.getheaders()), response.read()
                finally:
                    connection.close()

            def check(name, method, path, status, *, credential=None, body=None, expected=None, expected_text=None, extra=()):
                actual, headers, output = request(method, path, body, credential, extra)
                assert actual == status, (mode, name, actual, output)
                if expected is not None:
                    assert json.loads(output) == expected, (mode, name, output)
                if expected_text is not None:
                    assert output == expected_text, (mode, name, output)
                if status == 401:
                    assert headers.get("www-authenticate") == "Bearer", (mode, name, headers)
                if status in (401, 403):
                    assert b"document" not in output.lower() and b"demo-" not in output, (mode, name, output)
                results.append({"mode": mode, "case": name, "status": actual})

            try:
                deadline = time.monotonic() + 10
                while True:
                    assert process.poll() is None, stderr_path.read_text()
                    try:
                        if request("GET", "/health")[0] == 200:
                            break
                    except (OSError, http.client.HTTPException):
                        pass
                    assert time.monotonic() < deadline, "server did not become ready"
                    time.sleep(0.05)
                check("public", "GET", "/health", 200)
                check("missing-auth", "GET", "/me", 401, expected_text=b"invalid credential")
                check("invalid-auth", "GET", "/me", 401, credential="Bearer invalid", expected_text=b"invalid credential")
                check("expired-fixture", "GET", "/me", 401, credential="Bearer demo-expired", expected_text=b"authority expired")
                check("authenticated", "GET", "/me", 200, credential="Bearer demo-alice", expected={"subject": 1})
                check("duplicate-header", "GET", "/me", 400, credential="Bearer demo-alice", expected_text=b"invalid security request", extra=(("Authorization", "Bearer demo-bob"),))
                check("invalid-header", "GET", "/me", 400, credential=b"Bearer \xff", expected_text=b"invalid security request")
                check("missing-principal", "GET", "/documents/1", 401)
                check("alice-own", "GET", "/documents/1", 200, credential="Bearer demo-alice", expected={"id": 1, "title": "Alice document"})
                check("bob-own", "GET", "/documents/2", 200, credential="Bearer demo-bob", expected={"id": 2, "title": "Bob document"})
                check("resource-substitution", "GET", "/documents/2", 403, credential="Bearer demo-alice", expected={"code": "access_denied"})
                check("other-subject", "GET", "/documents/1", 403, credential="Bearer demo-bob")
                check("custom-nagi-block-policy", "GET", "/documents/3", 403, credential="Bearer demo-alice")
                check("not-found", "GET", "/documents/9", 404, credential="Bearer demo-alice")
                check("typed-validation", "GET", "/documents/not-integer", 400, credential="Bearer demo-alice")
                check("json-success", "POST", "/documents/read", 200, credential="Bearer demo-alice", body=b'{"document_id":1}', expected={"id": 1, "title": "Alice document"})
                check("auth-before-body", "POST", "/documents/read", 401, body=b"{", expected_text=b"invalid credential")
                check("malformed-json", "POST", "/documents/read", 400, credential="Bearer demo-alice", body=b"{")
                check("wrong-field-type", "POST", "/documents/read", 422, credential="Bearer demo-alice", body=b'{"document_id":"1"}')
                check("no-bool-proof", "POST", "/documents/read", 422, credential="Bearer demo-alice", body=b'{"document_id":2,"authorized":true}')
                body = b'{"document_id":1}'
                check("body-limit-accepted", "POST", "/documents/read", 200, credential="Bearer demo-alice", body=body + b" " * (4096 - len(body)))
                check("body-limit-rejected", "POST", "/documents/read", 413, credential="Bearer demo-alice", body=body + b" " * (4097 - len(body)))
                check("handler-panic", "GET", "/probe/panic", 500, credential="Bearer demo-alice")
                check("handler-timeout", "GET", "/probe/timeout", 504, credential="Bearer demo-alice")
                with socket.create_connection(("127.0.0.1", port), timeout=4) as slow:
                    slow.sendall(
                        b"POST /documents/read HTTP/1.1\r\nHost: 127.0.0.1:"
                        + str(port).encode("ascii")
                        + b"\r\nAuthorization: Bearer demo-alice\r\nContent-Type: application/json\r\nContent-Length: 128\r\n\r\n{"
                    )
                    response = http.client.HTTPResponse(slow)
                    response.begin()
                    assert response.status == 408, (mode, "slow-body", response.status, response.read())
                    response.read()
                    results.append({"mode": mode, "case": "slow-body-timeout", "status": 408})
                check("recovery", "GET", "/documents/1", 200, credential="Bearer demo-alice")
            finally:
                if process.poll() is None:
                    process.send_signal(signal.SIGINT) if os.name != "nt" else process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)
                        raise AssertionError("shutdown exceeded five seconds") from None
            if os.name != "nt":
                assert process.returncode == 0, stderr_path.read_text()
        with socket.socket() as released:
            released.settimeout(1)
            assert released.connect_ex(("127.0.0.1", port)) != 0, "listener remained open"
    (directory / "smoke-results.json").write_text(json.dumps(results, indent=2) + "\n")
    return {"cases": len(results), "starts": 2, "result": "passed"}
