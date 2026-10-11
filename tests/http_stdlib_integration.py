"""Build Nagi's standard HTTP app and exercise it over real sockets."""
from __future__ import annotations

import argparse
import http.client
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
AUTH_EXAMPLE = ROOT / "test-nagi-code/library-examples/http-auth/main.nagi"

EXTRA_HANDLERS = '''
def operation_error(problem: Error) -> http.Response:
    return http.text(http.Status.INTERNAL_SERVER_ERROR, "operation failed")

def forbidden(problem: AuthError) -> http.Response:
    return http.text(http.Status.FORBIDDEN, "access denied")

async def echo(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    return ok(http.bytes(http.Status.CREATED, request.body))

async def cookies(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    output = http.text(http.Status.OK, "cookies")
    output = try http.append_header_text(output, "X-Trace", "first")
    output = try http.append_header_text(output, "X-Trace", "second")
    return ok(output)

async def text_headers(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    output = http.text(http.Status.OK, "text headers")
    output = try http.append_header_text(output, "X-Demo", "nagi")
    output = try http.append_header_text(output, "X-Trace", "first=demo")
    output = try http.append_header_text(output, "X-Trace", "second=demo")
    return ok(output)

async def reserved_header(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    output = http.text(http.Status.OK, "reserved header")
    return http.append_header_text(output, "Set-Cookie", "sid=demo")

async def invalid_header(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    output = http.empty(http.Status.OK)
    return http.append_header_text(output, "X-Demo", "demo\\r\\nX-Injected: yes")

async def invalid_framing(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    output = http.empty(http.Status.OK)
    return http.append_header_text(output, "Content-Length", "123")

async def panic_index(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    match request.query:
        case Some(value):
            index = try parse_i64(value)
            values = [7]
            await sleep(1)
            print(values[index])
            return ok(http.text(http.Status.OK, "unreachable"))
        case None:
            return error("missing index")

async def panic_divide(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    match request.query:
        case Some(value):
            divisor = try parse_i64(value)
            print(10 / divisor)
            return ok(http.text(http.Status.OK, "unreachable"))
        case None:
            return error("missing divisor")

async def no_content(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, AuthError]:
    return ok(http.text(http.Status.NO_CONTENT, "must not be sent"))

async def reset_content(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, AuthError]:
    return ok(http.text(http.Status.RESET_CONTENT, "must not be sent"))

async def not_modified(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, AuthError]:
    return ok(http.text(http.Status.NOT_MODIFIED, "must not be sent"))

async def conflict(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, AuthError]:
    return ok(http.text(http.Status.CONFLICT, "already exists"))

async def throttled(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, AuthError]:
    return ok(http.text(http.Status.TOO_MANY_REQUESTS, "slow down"))

async def query(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, AuthError]:
    match request.query:
        case Some(value):
            return ok(http.text(http.Status.OK, value))
        case None:
            return ok(http.text(http.Status.OK, "no query"))

async def explicit_head(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, AuthError]:
    return ok(http.text(http.Status.ACCEPTED, "head-only"))

def method_name(request: view[http.Request]) -> view[str]:
    return http.method_name(view(request.method))

def direct_method_name(request: view[http.Request]) -> view[str]:
    return http.method_name(request.method)

async def method(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, AuthError]:
    name = method_name(view(request))
    direct = direct_method_name(view(request))
    assert_true(name == direct)
    return ok(http.text(http.Status.OK, name))

def compare_strings(owned: str, borrowed: view[str]) -> bool:
    assert_true(owned == borrowed)
    assert_true(borrowed == owned)
    assert_true(not (owned != borrowed))
    assert_true(not (borrowed != owned))
    assert_true(not (owned < borrowed))
    assert_true(not (borrowed < owned))
    assert_true(owned <= borrowed)
    assert_true(borrowed <= owned)
    assert_true(not (owned > borrowed))
    assert_true(not (borrowed > owned))
    assert_true(owned >= borrowed)
    assert_true(borrowed >= owned)
    assert_true(borrowed == "/comparisons")
    assert_true("/comparisons" == borrowed)
    assert_true(borrowed != "/different")
    assert_true("/different" != borrowed)
    assert_true("/aaa" < borrowed)
    assert_true(borrowed > "/aaa")
    assert_true(borrowed < "/zzz")
    assert_true("/zzz" > borrowed)
    assert_true(borrowed <= "/comparisons")
    assert_true("/comparisons" >= borrowed)
    assert_true("/comparisons" <= borrowed)
    assert_true(borrowed >= "/comparisons")
    return True

def compare_bytes(owned: bytes, borrowed: view[bytes]) -> bool:
    assert_true(owned == borrowed)
    assert_true(borrowed == owned)
    assert_true(not (owned != borrowed))
    assert_true(not (borrowed != owned))
    assert_true(not (owned < borrowed))
    assert_true(not (borrowed < owned))
    assert_true(owned <= borrowed)
    assert_true(borrowed <= owned)
    assert_true(not (owned > borrowed))
    assert_true(not (borrowed > owned))
    assert_true(owned >= borrowed)
    assert_true(borrowed >= owned)
    return True

async def comparisons(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    assert_true(compare_strings(copy(request.path), request.path))
    assert_true(compare_bytes(copy(request.body), request.body))
    empty = try slice(request.body, 0, 0)
    owned = copy(request.body)
    assert_true(len(view(owned)) > 0)
    assert_true(empty < owned)
    assert_true(owned > empty)
    assert_true(empty <= owned)
    assert_true(owned >= empty)
    assert_true(empty != owned)
    assert_true(owned != empty)
    return ok(http.bytes(http.Status.OK, view(owned)))

async def launch(app: http.App[State, AuthError], port: i64, wire_authority: str) -> Result[unit, Error]:
    options = try http.authority(
        http.default_options(),
        "https://localhost",
        [wire_authority],
        1,
        256
    )
    return await http.serve(app, port, options)

async def main() -> Result[unit, Error]:
    state = State(authorization=env("NAGI_DEMO_AUTHORIZATION", ""), greeting="first app", subject=1)
    app = http.app[State, AuthError](state, auth_error)
    app = try http.route(app, http.Method.GET, "/health", http.public_policy[State](), health)
    app = try http.route(app, http.Method.GET, "/me", http.authenticated_policy[State](verify), profile)
    app = try http.route_mapped(app, http.Method.GET, "/restricted", http.public_policy[State](), restricted, forbidden)
    app = try http.route_mapped(app, http.Method.POST, "/echo", http.public_policy[State](), echo, operation_error)
    app = try http.route_mapped(app, http.Method.POST, "/cookies", http.public_policy[State](), cookies, operation_error)
    app = try http.route_mapped(app, http.Method.GET, "/text-headers", http.public_policy[State](), text_headers, operation_error)
    app = try http.route_mapped(app, http.Method.GET, "/reserved-header", http.public_policy[State](), reserved_header, operation_error)
    app = try http.route_mapped(app, http.Method.GET, "/invalid-header", http.public_policy[State](), invalid_header, operation_error)
    app = try http.route_mapped(app, http.Method.GET, "/invalid-framing", http.public_policy[State](), invalid_framing, operation_error)
    app = try http.route_mapped(app, http.Method.GET, "/panic-index", http.public_policy[State](), panic_index, operation_error)
    app = try http.route_mapped(app, http.Method.GET, "/panic-divide", http.public_policy[State](), panic_divide, operation_error)
    app = try http.route(app, http.Method.GET, "/no-content", http.public_policy[State](), no_content)
    app = try http.route(app, http.Method.GET, "/reset-content", http.public_policy[State](), reset_content)
    app = try http.route(app, http.Method.GET, "/not-modified", http.public_policy[State](), not_modified)
    app = try http.route(app, http.Method.GET, "/conflict", http.public_policy[State](), conflict)
    app = try http.route(app, http.Method.GET, "/throttled", http.public_policy[State](), throttled)
    app = try http.route(app, http.Method.GET, "/query", http.public_policy[State](), query)
    app = try http.route(app, http.Method.GET, "/head", http.public_policy[State](), health)
    app = try http.route(app, http.Method.HEAD, "/head", http.public_policy[State](), explicit_head)
    app = try http.route_mapped(app, http.Method.POST, "/comparisons", http.public_policy[State](), comparisons, operation_error)
    extended = try http.method("PROPFIND")
    app = try http.route(app, extended, "/method", http.public_policy[State](), method)
    second = http.app[State, AuthError](State(authorization=env("NAGI_SECOND_AUTHORIZATION", ""), greeting="second app", subject=2), auth_error)
    second = try http.route(second, http.Method.GET, "/health", http.public_policy[State](), health)
    second = try http.route(second, http.Method.GET, "/me", http.authenticated_policy[State](verify), profile)
    port = try parse_i64(env("NAGI_TEST_PORT", "0"))
    second_port = try parse_i64(env("NAGI_SECOND_PORT", "0"))
    wire_authority = env("NAGI_HTTP_AUTHORITY", "127.0.0.1:8080")
    second_wire_authority = env("NAGI_SECOND_HTTP_AUTHORITY", "127.0.0.1:8081")
    async with scope:
        spawn launch(app, port, wire_authority)
        spawn launch(second, second_port, second_wire_authority)
    return ok(assert_true(True))
'''


def request(port: int, method: str, path: str, *, body=None, headers=None):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
    try:
        connection.request(method, path, body=body, headers=headers or {})
        response = connection.getresponse()
        return response.status, response.read(), response.getheaders()
    finally:
        connection.close()


def duplicate_header(port: int):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
    try:
        connection.putrequest("GET", "/me")
        connection.putheader("Authorization", "Bearer first-fixture-value")
        connection.putheader("Authorization", "Bearer first-fixture-value")
        connection.endheaders()
        response = connection.getresponse()
        return response.status, response.read()
    finally:
        connection.close()


def invalid_utf8_header(port: int):
    # The HTTP auth policy rejects malformed credentials before the verifier
    # or application handler receives them.
    with socket.create_connection(("127.0.0.1", port), timeout=5) as connection:
        connection.sendall(f"GET /me HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n".encode("ascii") + b"Authorization: \xff\r\nConnection: close\r\n\r\n")
        response = http.client.HTTPResponse(connection)
        response.begin()
        return response.status, response.read()


def panic_response(port: int, method: str, path: str):
    # Read the complete HTTP message before asserting that the failed
    # request's connection closes. A server-side panic must not produce EOF
    # in place of the status, headers or promised response body.
    with socket.create_connection(("127.0.0.1", port), timeout=5) as connection:
        connection.sendall(f"{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n".encode())
        deadline = time.monotonic() + 5
        wire = bytearray()
        while True:
            remaining = deadline - time.monotonic()
            assert remaining > 0, "panicked request connection remained open"
            connection.settimeout(remaining)
            chunk = connection.recv(4096)
            if not chunk:
                break
            wire.extend(chunk)
            assert len(wire) <= 65536, "unexpectedly large panic response"
        header_block, separator, body = bytes(wire).partition(b"\r\n\r\n")
        assert separator, (method, path, "incomplete HTTP response", bytes(wire))
        lines = header_block.split(b"\r\n")
        status = int(lines[0].split()[1])
        headers = {}
        for line in lines[1:]:
            name, value = line.split(b":", 1)
            headers[name.decode().lower()] = value.strip().decode()
        assert status == 500, (method, path, status, body)
        assert body == (b"" if method == "HEAD" else b"Internal Server Error"), (method, path, body)
        assert headers.get("connection") == "close", headers
        assert headers.get("content-length") == str(len(b"Internal Server Error")), headers
        assert "transfer-encoding" not in headers, headers


def wait_for_server(port: int, process: subprocess.Popen, log: Path):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise AssertionError("Nagi HTTP server exited:\n" + log.read_text(encoding="utf-8"))
        try:
            if request(port, "GET", "/health")[:2] == (200, b"ok"):
                return
        except OSError:
            pass
        time.sleep(0.05)
    raise AssertionError("Nagi HTTP server did not start:\n" + log.read_text(encoding="utf-8"))


def free_ports():
    # Keep both reservations alive until both numbers have been selected.
    with socket.socket() as first, socket.socket() as second:
        first.bind(("127.0.0.1", 0))
        second.bind(("127.0.0.1", 0))
        return first.getsockname()[1], second.getsockname()[1]


def check_server(executable: Path, folder: Path, environment: dict[str, str], source: str) -> int:
    first, second = free_ports()
    environment = dict(environment, NAGI_TEST_PORT=str(first), NAGI_SECOND_PORT=str(second),
                       NAGI_HTTP_AUTHORITY=f"127.0.0.1:{first}",
                       NAGI_SECOND_HTTP_AUTHORITY=f"127.0.0.1:{second}",
                       NAGI_DEMO_AUTHORIZATION="Bearer first-fixture-value",
                       NAGI_SECOND_AUTHORIZATION="Bearer second-fixture-value")
    log_path = folder / (source + "-server.log")
    checked = 0

    def expect(port, method, path, status, expected_body, **kwargs):
        nonlocal checked
        actual, payload, headers = request(port, method, path, **kwargs)
        assert (actual, payload) == (status, expected_body), (source, method, path, actual, payload)
        assert b"fixture-value" not in payload, "credential leaked into a response"
        if status == 401:
            assert {name.lower(): value for name, value in headers}.get("www-authenticate") == "Bearer", headers
        checked += 1
        return headers

    with log_path.open("w", encoding="utf-8") as log:
        process = subprocess.Popen([str(executable)], cwd=folder, env=environment,
                                   stdin=subprocess.DEVNULL, stdout=log, stderr=log)
        try:
            wait_for_server(first, process, log_path)
            wait_for_server(second, process, log_path)
            expect(first, "GET", "/me", 401, b"invalid credential")
            expect(first, "GET", "/me", 401, b"invalid credential", headers={"Authorization": "Bearer wrong-value"})
            expect(first, "GET", "/me", 200, b"first app", headers={"Authorization": environment["NAGI_DEMO_AUTHORIZATION"]})
            expect(second, "GET", "/me", 200, b"second app", headers={"Authorization": environment["NAGI_SECOND_AUTHORIZATION"]})
            expect(second, "GET", "/me", 401, b"invalid credential", headers={"Authorization": environment["NAGI_DEMO_AUTHORIZATION"]})
            expect(first, "GET", "/restricted", 403, b"access denied")
            assert duplicate_header(first) == (400, b"invalid security request")
            checked += 1
            assert invalid_utf8_header(first) == (400, b"invalid security request")
            checked += 1
            expect(first, "POST", "/echo", 201, "日本語\x00body".encode(), body="日本語\x00body".encode())
            expect(first, "POST", "/echo", 201, b"", body=b"")
            cookies = expect(first, "POST", "/cookies", 200, b"cookies", body=b"ignored")
            assert [value for name, value in cookies if name.lower() == "x-trace"] == ["first", "second"], cookies
            headers = expect(first, "GET", "/reserved-header", 500, b"operation failed")
            assert all(name.lower() != "set-cookie" for name, _ in headers)
            headers = expect(first, "GET", "/text-headers", 200, b"text headers")
            assert {name.lower(): value for name, value in headers}.get("x-demo") == "nagi", headers
            assert [value for name, value in headers if name.lower() == "x-trace"] == ["first=demo", "second=demo"], headers
            headers = expect(first, "GET", "/invalid-header", 500, b"operation failed")
            assert all(name.lower() != "x-injected" for name, _ in headers)
            expect(first, "GET", "/invalid-framing", 500, b"operation failed")
            for path in ["/panic-index?1", "/panic-divide?0"]:
                for method in ["GET", "HEAD"]:
                    panic_response(first, method, path)
                    checked += 1
                    expect(first, "GET", "/health", 200, b"ok")
            # An ordinary Result error still uses the route mapper.
            expect(first, "GET", "/panic-index?invalid", 500, b"operation failed")
            for path, status in [("/no-content", 204), ("/reset-content", 205), ("/not-modified", 304)]:
                headers = expect(first, "GET", path, status, b"")
                values = {name.lower(): value for name, value in headers}
                assert values.get("content-length") in (None, "0"), (path, headers)
                assert "transfer-encoding" not in values, (path, headers)
            expect(first, "GET", "/conflict", 409, b"already exists")
            expect(first, "GET", "/throttled", 429, b"slow down")
            expect(first, "GET", "/query", 200, b"no query")
            expect(first, "GET", "/query?name=Nagi&empty=", 200, b"name=Nagi&empty=")
            head = expect(first, "HEAD", "/health", 200, b"")
            assert {name.lower(): value for name, value in head}.get("content-length") == "2", head
            expect(first, "HEAD", "/head", 202, b"")
            expect(first, "PROPFIND", "/method", 200, b"PROPFIND")
            expect(first, "POST", "/comparisons", 200, b"comparison bytes", body=b"comparison bytes")
            status, _, headers = request(first, "POST", "/health")
            assert status == 405, (status, headers)
            allow = {name.lower(): value for name, value in headers}.get("allow", "")
            assert "GET" in allow.split(", ") or "GET" in allow.split(","), headers
            checked += 1
        finally:
            if process.poll() is None:
                if os.name == "nt":
                    # TerminateProcess does not test graceful Windows Ctrl+C.
                    process.terminate()
                else:
                    process.send_signal(signal.SIGINT)
            try:
                code = process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
                raise AssertionError("Nagi HTTP app did not stop")
            if os.name != "nt":
                assert code == 0, log_path.read_text(encoding="utf-8")
    content = log_path.read_text(encoding="utf-8")
    assert "fixture-value" not in content, "credential leaked into server output"
    return checked


def verify(compiler: Path, target: Path) -> None:
    compiler, target = compiler.resolve(), target.resolve()
    environment = dict(os.environ, NAGI_ROOT=str(ROOT), NAGI_NATIVE_TARGET_DIR=str(target))
    with tempfile.TemporaryDirectory(prefix="nagi stdlib HTTP 凪 ") as temporary:
        folder = Path(temporary)
        auth = AUTH_EXAMPLE.read_text(encoding="utf-8")
        prefix, separator, _ = auth.partition("async def main()")
        assert separator, "the authentication example must retain its executable entry point"
        source = folder / "http-app.nagi"
        source.write_text(prefix + EXTRA_HANDLERS, encoding="utf-8")
        native_source = folder / "native.rs"
        native_source.write_text((AUTH_EXAMPLE.parent / "native.rs").read_text(encoding="utf-8"), encoding="utf-8")
        for command in ["check", "lower"]:
            result = subprocess.run([str(compiler), command, str(source), "--no-project", "--out", str(folder / "lowered")],
                                    cwd=folder, env=environment, capture_output=True, text=True, encoding="utf-8", timeout=60)
            assert result.returncode == 0, result.stdout + result.stderr
        generated = folder / "lowered/generated.low"
        assert generated.is_file(), list((folder / "lowered").rglob("*"))
        lowered = folder / "http-app.low"
        lowered.write_text(generated.read_text(encoding="utf-8"), encoding="utf-8")
        total = 0
        for input_file in [source, lowered]:
            if input_file == lowered:
                source.unlink()
            result = subprocess.run([str(compiler), "build", str(input_file), "--no-project", "--rust", str(native_source), "--out", str(folder / "build")],
                                    cwd=folder, env=environment, capture_output=True, text=True, encoding="utf-8", timeout=180)
            assert result.returncode == 0, result.stdout + result.stderr
            native = [line.removeprefix("native: ").strip()
                      for line in (result.stdout + "\n" + result.stderr).splitlines()
                      if line.startswith("native: ")]
            assert len(native) == 1, result.stdout + result.stderr
            executable = Path(native[0])
            assert executable.is_file(), (result.stdout, executable)
            total += check_server(executable, folder, environment, input_file.suffix[1:])
        print(f"Standard HTTP Nagi integration: {total} passed (High and independent Low)")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", type=Path,
                        default=Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "release" / ("nagic.exe" if os.name == "nt" else "nagic"))
    parser.add_argument("--target", type=Path,
                        default=Path(os.environ.get("NAGI_NATIVE_TARGET_DIR", ROOT / "native-target")))
    arguments = parser.parse_args()
    verify(arguments.compiler, arguments.target)
