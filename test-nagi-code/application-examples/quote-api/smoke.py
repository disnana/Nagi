"""Exercise typed JSON, custom HTTP errors, request IDs and shared configuration."""
from __future__ import annotations

import http.client
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import time


REQUEST_ID = "123e4567-e89b-12d3-a456-426614174000"
FAILURE_LOG_BYTES = 32768


def failure_log(path):
    # Keep the tail of each owned child's log without loading an unbounded file.
    with path.open("rb") as source:
        size = source.seek(0, os.SEEK_END)
        source.seek(max(0, size - FAILURE_LOG_BYTES))
        return {"bytes": size, "truncated": size > FAILURE_LOG_BYTES,
                "tail": source.read(FAILURE_LOG_BYTES).decode("utf-8", errors="replace")}


def verify(executable: Path, env: dict, directory: Path) -> dict:
    executable = executable.resolve()
    directory = directory.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    failure_path = directory / "smoke-failure.json"
    failure_path.unlink(missing_ok=True)
    results = []

    def expected_quote(quantity):
        shipping = 500 if quantity < 5 else 0
        return {
            "sku": "NOTEBOOK", "quantity": quantity, "currency": "USD",
            "unit_price_minor": 1250, "subtotal_minor": 1250 * quantity,
            "shipping_minor": shipping, "total_minor": 1250 * quantity + shipping,
        }

    for maximum in (1000, 3):
        with socket.socket() as available:
            available.bind(("127.0.0.1", 0))
            port = available.getsockname()[1]
        stdout_path = directory / f"server-{maximum}.stdout"
        stderr_path = directory / f"server-{maximum}.stderr"
        child_env = dict(env, NAGI_SAMPLE_PORT=str(port), NAGI_HTTP_AUTHORITY=f"127.0.0.1:{port}", NAGI_SAMPLE_MAX_QUANTITY=str(maximum))
        with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open("w", encoding="utf-8") as stderr:
            process = subprocess.Popen(
                [str(executable)], cwd=directory, env=child_env,
                stdout=stdout, stderr=stderr,
            )
            request_context = {"case": "readiness"}

            def request(method, path, body=b"", headers=()):
                request_context.update(method=method, path=path, body_bytes=len(body), stage="prepare-headers")
                connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
                try:
                    # putheader preserves duplicate fields for the error cases.
                    connection.putrequest(method, path)
                    for name, value in headers:
                        connection.putheader(name, value)
                    connection.putheader("Content-Length", str(len(body)))
                    request_context["stage"] = "send-headers-and-body"
                    connection.endheaders(body)
                    request_context["stage"] = "response-status-and-headers"
                    response = connection.getresponse()
                    output_headers = dict((name.lower(), value) for name, value in response.getheaders())
                    request_context.update(stage="response-body", status=response.status)
                    output = response.read()
                    request_context["stage"] = "response-assertions"
                    return response.status, output_headers, output
                finally:
                    connection.close()

            def check(name, method, path, expected_status, *, body=b"", headers=(), expected=None, request_id=None):
                request_context.clear()
                request_context.update(case=name, expected_status=expected_status)
                status, output_headers, output = request(method, path, body, headers)
                assert status == expected_status, (name, status, output)
                assert output_headers.get("x-request-id") == request_id, (name, output_headers)
                if expected is not None:
                    assert output_headers.get("content-type") == "application/json", (name, output_headers)
                    assert json.loads(output) == expected, (name, output)
                results.append({"configuration_maximum": maximum, "case": name, "status": status})
                return output_headers, output

            def post(name, quantity, expected_status=200, *, sku="NOTEBOOK", request_id=REQUEST_ID, code=None):
                body = json.dumps({"sku": sku, "quantity": quantity}).encode("utf-8")
                headers = (("Content-Type", "application/json"), ("X-Request-ID", request_id))
                expected = expected_quote(quantity) if expected_status == 200 else errors[code]
                return check(name, "POST", "/quotes", expected_status, body=body, headers=headers,
                             expected=expected, request_id=request_id)

            errors = {
                "invalid_request_id": {"code": "invalid_request_id", "message": "X-Request-ID must contain one UUID"},
                "invalid_header": {"code": "invalid_header", "message": "Content-Type must contain one valid media type"},
                "unsupported_media_type": {"code": "unsupported_media_type", "message": "Content-Type must be application/json"},
                "invalid_json": {"code": "invalid_json", "message": "Expected exactly sku:string and quantity:integer"},
                "invalid_quantity": {"code": "invalid_quantity", "message": "Quantity is outside the configured range"},
                "unknown_sku": {"code": "unknown_sku", "message": "Only NOTEBOOK is available"},
            }
            valid = b'{"sku":"NOTEBOOK","quantity":1}'
            json_headers = (("Content-Type", "application/json"), ("X-Request-ID", REQUEST_ID))
            try:
                deadline = time.monotonic() + 10
                while True:
                    assert process.poll() is None, stderr_path.read_text(encoding="utf-8")
                    try:
                        ready = request("GET", "/health")
                        break
                    except (OSError, http.client.HTTPException):
                        if time.monotonic() >= deadline:
                            raise AssertionError("quote API did not become ready") from None
                        time.sleep(0.05)
                assert ready[0] == 200, ready
                health = {"status": "ok", "currency": "USD", "maximum_quantity": maximum}
                check("health", "GET", "/health", 200, expected=health)
                headers, output = check("head-health", "HEAD", "/health", 200)
                assert output == b"" and int(headers["content-length"]) > 0, (headers, output)
                post("one-item", 1)
                post("configured-maximum", maximum)
                post("over-configured-maximum", maximum + 1, 422, code="invalid_quantity")

                if maximum == 1000:
                    post("below-free-shipping", 4)
                    post("free-shipping", 5)
                    check("no-request-id", "POST", "/quotes", 200, body=valid,
                          headers=(("Content-Type", "application/json"),), expected=expected_quote(1))
                    check("canonical-request-id", "POST", "/quotes", 200, body=valid,
                          headers=(("Content-Type", "application/json"), ("X-Request-ID", REQUEST_ID.upper())),
                          expected=expected_quote(1), request_id=REQUEST_ID)
                    check("escaped-owned-string", "POST", "/quotes", 200,
                          body=b'{"sku":"\\u004eOTEBOOK","quantity":2}', headers=json_headers,
                          expected=expected_quote(2), request_id=REQUEST_ID)
                    post("zero-quantity", 0, 422, code="invalid_quantity")
                    post("negative-quantity", -1, 422, code="invalid_quantity")
                    post("unknown-sku", 1, 422, sku="PEN", code="unknown_sku")
                    invalid_json = (
                        ("malformed-json", b'{'),
                        ("missing-field", b'{"sku":"NOTEBOOK"}'),
                        ("extra-field", b'{"sku":"NOTEBOOK","quantity":1,"coupon":"demo"}'),
                        ("string-quantity", b'{"sku":"NOTEBOOK","quantity":"1"}'),
                        ("boolean-quantity", b'{"sku":"NOTEBOOK","quantity":true}'),
                        ("float-quantity", b'{"sku":"NOTEBOOK","quantity":1.0}'),
                        ("out-of-i64-range", b'{"sku":"NOTEBOOK","quantity":9223372036854775808}'),
                        ("duplicate-json-field", b'{"sku":"NOTEBOOK","quantity":1,"quantity":2}'),
                        ("invalid-utf8-json", b'{"sku":"\xff","quantity":1}'),
                    )
                    for name, body in invalid_json:
                        check(name, "POST", "/quotes", 400, body=body, headers=json_headers,
                              expected=errors["invalid_json"], request_id=REQUEST_ID)
                    for name, content_type in (("missing-content-type", None), ("text-content-type", "text/plain"),
                                               ("suffix-content-type", "application/problem+json")):
                        headers = (("X-Request-ID", REQUEST_ID),)
                        if content_type is not None:
                            headers += (("Content-Type", content_type),)
                        check(name, "POST", "/quotes", 415, body=valid, headers=headers,
                              expected=errors["unsupported_media_type"], request_id=REQUEST_ID)
                    for name, content_type in (
                        ("parameterized-content-type", "application/json; charset=utf-8"),
                        ("case-insensitive-content-type", "Application/JSON"),
                        ("whitespace-content-type", " \tapplication/json\t ; charset=\"UTF-8\" \t"),
                        ("quoted-parameter-content-type", 'application/json; profile="a;b=\\\"c"'),
                        ("charset-does-not-select-decoder", "application/json; charset=iso-8859-1"),
                    ):
                        check(name, "POST", "/quotes", 200, body=valid,
                              headers=(("X-Request-ID", REQUEST_ID), ("Content-Type", content_type)),
                              expected=expected_quote(1), request_id=REQUEST_ID)
                    for name, content_type in (
                        ("comma-content-type", "application/json, text/plain"),
                        ("missing-subtype-content-type", "application/"),
                        ("invalid-parameter-content-type", "application/json; charset="),
                        ("unclosed-parameter-content-type", 'application/json; charset="utf-8'),
                    ):
                        check(name, "POST", "/quotes", 400, body=valid,
                              headers=(("X-Request-ID", REQUEST_ID), ("Content-Type", content_type)),
                              expected=errors["invalid_header"], request_id=REQUEST_ID)
                    check("charset-still-requires-utf8-json", "POST", "/quotes", 400,
                          body=b'{"sku":"\xff","quantity":1}',
                          headers=(("X-Request-ID", REQUEST_ID),
                                   ("Content-Type", "application/json; charset=iso-8859-1")),
                          expected=errors["invalid_json"], request_id=REQUEST_ID)
                    check("duplicate-content-type", "POST", "/quotes", 400, body=valid,
                          headers=json_headers + (("Content-Type", "application/json"),),
                          expected=errors["invalid_header"], request_id=REQUEST_ID)
                    check("invalid-content-type-text", "POST", "/quotes", 400, body=valid,
                          headers=(("X-Request-ID", REQUEST_ID), ("Content-Type", b"\xff")),
                          expected=errors["invalid_header"], request_id=REQUEST_ID)
                    check("invalid-request-id", "POST", "/quotes", 400, body=valid,
                          headers=(("Content-Type", "application/json"), ("X-Request-ID", "not-a-uuid")),
                          expected=errors["invalid_request_id"])
                    check("duplicate-request-id", "POST", "/quotes", 400, body=valid,
                          headers=json_headers + (("X-Request-ID", REQUEST_ID),), expected=errors["invalid_request_id"])
                    check("invalid-request-id-text", "POST", "/quotes", 400, body=valid,
                          headers=(("Content-Type", "application/json"), ("X-Request-ID", b"\xff")),
                          expected=errors["invalid_request_id"])
                    headers, _ = check("method-not-allowed", "GET", "/quotes", 405)
                    assert headers.get("allow") == "POST", headers
                    check("missing-route", "GET", "/missing", 404)
                    check("body-limit", "POST", "/quotes", 413, body=b" " * 4097, headers=json_headers)
            except Exception as error:
                # Observe before finally stops the child; a receive failure
                # must not be mistaken for a child failure or a successful 413.
                try:
                    stdout.flush()
                    stderr.flush()
                    failure = {
                        "configuration_maximum": maximum,
                        "request": dict(request_context),
                        "child_pid": process.pid,
                        "child_returncode_before_cleanup": process.poll(),
                        "exception_type": type(error).__name__,
                        "exception": str(error)[:4096],
                        "stdout": failure_log(stdout_path),
                        "stderr": failure_log(stderr_path),
                    }
                    failure_path.write_text(json.dumps(failure, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
                except OSError as capture_error:
                    error.add_note(f"Unable to save quote API failure evidence: {capture_error}")
                raise
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
                        raise AssertionError("quote API did not stop within five seconds") from None
            if os.name != "nt":
                assert process.returncode == 0, stderr_path.read_text(encoding="utf-8")
            assert stderr_path.read_text(encoding="utf-8") == ""
            with socket.socket() as released:
                released.settimeout(1)
                assert released.connect_ex(("127.0.0.1", port)) != 0, "listener remained open"

    for maximum in ("0", "1001", "not-a-number"):
        failure = subprocess.run(
            [str(executable)], cwd=directory,
            env=dict(env, NAGI_SAMPLE_PORT="0", NAGI_SAMPLE_MAX_QUANTITY=maximum),
            capture_output=True, text=True, encoding="utf-8", timeout=5,
        )
        assert failure.returncode != 0, (maximum, failure.stdout, failure.stderr)
        assert failure.stderr, (maximum, failure.stdout)
        results.append({"case": "invalid-configuration", "maximum": maximum, "status": "rejected"})
    (directory / "smoke-results.json").write_text(
        json.dumps(results, ensure_ascii=False, indent=2) + "\n", encoding="utf-8",
    )
    return {"cases": len(results), "starts": 2, "invalid_configurations": 3, "result": "passed"}
