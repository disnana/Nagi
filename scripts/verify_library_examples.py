"""Build and exercise the library projects, including real HTTP servers."""
from __future__ import annotations

import argparse
from collections import Counter
import http.client
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
PROJECTS = ROOT / "test-nagi-code/library-examples"
EXE = ".exe" if os.name == "nt" else ""
OUTPUTS = {
    "typed-errors": (
        "main",
        "42\nquantity must be between 1 and 1000000\nquantity must be a number\n",
    ),
    "module-imports": ("module_imports", "42\n42\n7\n"),
    "low-kernel": ("low_kernel", "30\n4\n"),
    "rust-json": (
        "serde-record",
        'Nagi\n2\n{"label":"Nagi","count":2}\nmalformed -> invalid\nwrong type -> invalid\nrust-json: OK\n',
    ),
    "rust-async": (
        "tokio-timer",
        "42\ninvalid delay -> invalid\noverflow -> invalid\nrust-async: OK\n",
    ),
}


def run(args, *, cwd=ROOT, env=None, input=None, timeout=180, diagnostics=False):
    result = subprocess.run(
        [str(arg) for arg in args], cwd=cwd, env=env, input=input,
        text=True, capture_output=True, timeout=timeout,
    )
    if result.returncode:
        raise RuntimeError(f"Command failed: {args}\n{result.stdout}{result.stderr}")
    return result.stdout + result.stderr if diagnostics else result.stdout


def native_executable(output):
    for line in output.splitlines():
        if line.startswith("native: "):
            return Path(line.removeprefix("native: "))
    raise RuntimeError("Compiler did not report a native executable")


def response(port, path="/health", body=None, headers=None, *, method="GET"):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
    try:
        connection.request(method, path, body=body, headers=headers or {})
        result = connection.getresponse()
        return result.status, result.read(), dict(result.getheaders())
    finally:
        connection.close()


def wait_for_health(port, process, expected=b"ok\n"):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"HTTP sample exited early: {process.returncode}")
        try:
            if response(port)[:2] == (200, expected):
                return
        except OSError:
            pass
        time.sleep(0.05)
    raise RuntimeError("HTTP sample did not become ready")


def check_http(executable, env, output):
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    server_env = dict(env, NAGI_SAMPLE_PORT=str(port))
    with (output / "custom-http.log").open("w", encoding="utf-8") as log:
        process = subprocess.Popen(
            [str(executable)], cwd=PROJECTS / "custom-http", env=server_env,
            stdin=subprocess.DEVNULL, stdout=log, stderr=log,
        )
        try:
            wait_for_health(port, process)
            for path, expected in [
                ("/hello/7", (200, b"Hello, Nagi! 7\n")),
                ("/hello/42", (200, b"Hello from the Nagi callback! 42\n")),
                ("/missing", (404, b"Route not found\n")),
            ]:
                assert response(port, path)[:2] == expected, path
            for path in ["/hello/not-a-number", "/hello/9223372036854775808"]:
                assert response(port, path)[0] == 400, path
            assert response(port, "/hello/7", b"x" * 65537)[0] == 413

            # Hold eight requests while their bodies are incomplete.
            held = []
            try:
                for _ in range(8):
                    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
                    held.append(connection)
                    connection.putrequest("GET", "/hello/7")
                    connection.putheader("Content-Length", "1")
                    connection.endheaders()
                deadline = time.monotonic() + 2
                while time.monotonic() < deadline:
                    status, body, headers = response(port)
                    if status == 503:
                        assert headers.get("retry-after") == "1"
                        break
                    time.sleep(0.01)
                else:
                    raise AssertionError("HTTP request slots were not enforced")
            finally:
                for connection in held:
                    connection.close()
            wait_for_health(port, process)

            connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
            try:
                connection.putrequest("GET", "/hello/7")
                connection.putheader("Content-Length", "1")
                connection.endheaders()
                result = connection.getresponse()
                assert result.status == 408
                assert result.read() == b"Request timed out\n"
            finally:
                connection.close()
            wait_for_health(port, process)
        finally:
            if process.poll() is None:
                if os.name == "nt":
                    # Termination verifies cleanup here. Ctrl+C is a manual
                    # console check on Windows; this is not a graceful signal.
                    process.terminate()
                else:
                    process.send_signal(signal.SIGINT)
            try:
                code = process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
                raise RuntimeError("HTTP sample did not stop")
            if os.name != "nt":
                assert code == 0, f"HTTP shutdown failed: {code}"
    assert response_after_stop(port), "HTTP listener remains after shutdown"
    return "terminated" if os.name == "nt" else "graceful"


def response_after_stop(port):
    try:
        response(port)
    except OSError:
        return True
    return False


def check_http_auth(executable, env, output):
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    # This is an ephemeral test fixture, not a credential for a deployed app.
    authorization = "Bearer library-example-fixture"
    server_env = dict(env, NAGI_SAMPLE_PORT=str(port), NAGI_HTTP_AUTHORITY=f"127.0.0.1:{port}",
                      NAGI_DEMO_AUTHORIZATION=authorization)
    with (output / "http-auth.log").open("w", encoding="utf-8") as log:
        process = subprocess.Popen([str(executable)], cwd=PROJECTS / "http-auth",
                                   env=server_env, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
        try:
            wait_for_health(port, process, b"ok")
            status, body, headers = response(port, "/me")
            assert (status, body) == (401, b"invalid credential")
            assert {name.lower(): value for name, value in headers.items()}["www-authenticate"] == "Bearer"
            assert response(port, "/me", headers={"Authorization": "Bearer incorrect"})[:2] == (401, b"invalid credential")
            assert response(port, "/me", headers={"Authorization": authorization})[:2] == (200, b"Hello, Nagi!")
            assert response(port, "/restricted")[:2] == (403, b"access denied")
        finally:
            if process.poll() is None:
                if os.name == "nt":
                    process.terminate()
                else:
                    process.send_signal(signal.SIGINT)
            try:
                code = process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
                raise RuntimeError("HTTP auth sample did not stop")
            if os.name != "nt":
                assert code == 0, f"HTTP auth shutdown failed: {code}"
    assert response_after_stop(port), "HTTP auth listener remains after shutdown"
    return "terminated" if os.name == "nt" else "graceful"


def check_supervised_service(executable, env, output):
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    server_env = dict(env, NAGI_SAMPLE_PORT=str(port), NAGI_HTTP_AUTHORITY=f"127.0.0.1:{port}")
    with (output / "supervised-service.log").open("w", encoding="utf-8") as log:
        process = subprocess.Popen(
            [str(executable)], cwd=PROJECTS / "supervised-service", env=server_env,
            stdin=subprocess.DEVNULL, stdout=log, stderr=log,
        )
        try:
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError(f"Supervised service exited early: {process.returncode}")
                try:
                    if response(port, "/counter")[:2] == (200, b"0"):
                        break
                except OSError:
                    pass
                time.sleep(0.05)
            else:
                raise RuntimeError("Supervised service did not become ready")
            assert response(port, "/counter", "5", method="POST")[:2] == (200, b"5")
            assert response(port, "/counter", "-1", method="POST")[:2] == (
                409, b"increment must be 1..1000 and total at most 1000000",
            )
            for body in ["not JSON", '"5"', "1.5"]:
                assert response(port, "/counter", body, method="POST")[:2] == (
                    400, b"send a JSON integer",
                )
            assert response(port, "/counter")[:2] == (200, b"5")
            assert response(port, "/counter", "2", method="POST")[:2] == (200, b"7")
            assert response(port, "/shutdown", method="POST")[:2] == (204, b"")
            assert response(port, "/counter")[:2] == (503, b"counter unavailable")
        finally:
            if process.poll() is None:
                if os.name == "nt":
                    process.terminate()
                else:
                    process.send_signal(signal.SIGINT)
            try:
                code = process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
                raise RuntimeError("Supervised service did not stop")
            if os.name != "nt":
                assert code == 0, f"Supervised service shutdown failed: {code}"
    assert response_after_stop(port), "Supervised service listener remains after shutdown"
    return "terminated" if os.name == "nt" else "graceful"


def check_task_results(executable, env):
    actual = run([executable], env=env, timeout=10).splitlines()
    assert actual[-2:] == ["after scope", "task-results: OK"], actual
    assert Counter(actual[:-2]) == Counter(
        ["42", "business sentinel", "sibling finished"]
    ), actual


def build_forms(compiler, name, env, output):
    # Copies keep repository sources intact; saved Low must work after the
    # original High is removed. The handwritten Low is a separate input.
    with tempfile.TemporaryDirectory(prefix="nagi-library-forms-") as temporary:
        project = Path(temporary) / name
        shutil.copytree(PROJECTS / name, project,
                        ignore=shutil.ignore_patterns("build", ".nagi", "native-target"))
        high = project / "main.nagi"
        lowered = project / "lowered"
        run([compiler, "lower", high, "--project", project, "--out", lowered], env=env)
        saved = project / "saved.low"
        saved.write_bytes((lowered / "generated.low").read_bytes())
        for mode, source in [("high", high), ("saved-low", saved),
                             ("handwritten-low", project / "main.low")]:
            if mode == "saved-low":
                high.unlink()
            directory = output / name / mode
            directory.mkdir(parents=True, exist_ok=True)
            generated = project / (mode + "-build")
            options = [source, "--project", project, "--out", generated]
            run([compiler, "check", *options], env=env)
            result = run([compiler, "build", *options], env=env, diagnostics=True)
            (directory / "build.log").write_text(result, encoding="utf-8")
            yield mode, native_executable(result), directory


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", default=str(
        Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "release" / ("nagic" + EXE)
    ))
    parser.add_argument("--only", choices=["supervised-service", "task-results"],
                        help="Run one project in High, saved Low and handwritten Low")
    args = parser.parse_args()
    compiler = Path(shutil.which(args.compiler) or args.compiler).resolve()
    target = Path(os.environ.get("NAGI_NATIVE_TARGET_DIR", ROOT / "native-target")).resolve()
    output = ROOT / "build/library-example-verification"
    output.mkdir(parents=True, exist_ok=True)
    # This is a source-checkout verification: use this checkout's runtime.
    env = dict(os.environ, NAGI_ROOT=str(ROOT), NAGI_NATIVE_TARGET_DIR=str(target))
    rows = []

    def build(name, entry):
        project = PROJECTS / name
        run([compiler, "check", "--project", project], env=env)
        result = run([compiler, "build", "--project", project], env=env, diagnostics=True)
        (output / (name + "-build.log")).write_text(result, encoding="utf-8")
        return native_executable(result)

    def verify_forms(name):
        for mode, executable, directory in build_forms(compiler, name, env, output):
            row = {"project": name, "form": mode, "check_build_run": "passed"}
            if name == "supervised-service":
                row["shutdown"] = check_supervised_service(executable, env, directory)
            else:
                check_task_results(executable, env)
            rows.append(row)
            print(f"Passed: {name} ({mode})", flush=True)

    if args.only:
        verify_forms(args.only)
        (output / (args.only + "-results.json")).write_text(json.dumps(rows, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({"forms": len(rows), "status": "passed"}), flush=True)
        return

    for name, (entry, expected) in OUTPUTS.items():
        executable = build(name, entry)
        actual = run([executable], cwd=PROJECTS / name, env=env, timeout=10)
        assert actual == expected, (name, actual)
        if name == "low-kernel":
            source = PROJECTS / name / (entry + ".nagi")
            result = run([compiler, "build", source, "--no-project"], env=env, diagnostics=True)
            assert run([native_executable(result)], env=env, timeout=10) == actual
        rows.append({"project": name, "check_build_run": "passed"})
        print(f"Passed: {name}", flush=True)

    executable = build("foundation-cli", "foundation_cli")
    outputs = []
    for engine in ["nagi", "rust"]:
        selected = dict(env, NAGI_PRICING_ENGINE=engine)
        actual = run([executable], env=selected, input="Notebook\n999\n3\n1250\n", timeout=10)
        lines = actual.splitlines()
        quote = json.loads(lines[4])
        assert (quote["subtotal_cents"], quote["discount_cents"], quote["total_cents"]) == (2997, 374, 2623)
        assert lines[-1] == "Notebook"
        largest = run([executable], env=selected, input="Maximum\n100000000\n10000\n10000\n", timeout=10)
        assert json.loads(largest.splitlines()[4])["total_cents"] == 0
        rejected = subprocess.run(
            [str(executable)], cwd=ROOT, env=selected, input="Invalid\n999\n0\n0\n",
            text=True, capture_output=True, timeout=10,
        )
        assert rejected.returncode != 0 and "quantity" in rejected.stderr
        outputs.append(actual)
    assert outputs[0] == outputs[1]
    rows.append({"project": "foundation-cli", "engines": ["nagi", "rust"], "check_build_run": "passed"})
    print("Passed: foundation-cli (both engines)", flush=True)

    executable = build("foundation-report", "foundation_report")
    reports = [json.loads(run([executable], env=dict(env, NAGI_PRICING_ENGINE=engine), timeout=10)) for engine in ["nagi", "rust"]]
    assert reports[0] == reports[1]
    report = reports[0]
    assert report["total_cents"] == 3123
    assert [(line["id"], line["quote"]["total_cents"]) for line in report["quotes"]] == [(1, 2623), (2, 500)]
    assert report["rejected"] == [{"id": 3, "reason": "quantity must be between 1 and 10000"}]
    rows.append({"project": "foundation-report", "engines": ["nagi", "rust"], "check_build_run": "passed"})
    print("Passed: foundation-report (both engines)", flush=True)

    executable = build("custom-http", "custom_http")
    invalid = subprocess.run([str(executable)], env=dict(env, NAGI_SAMPLE_PORT="0"), text=True, capture_output=True, timeout=10)
    assert invalid.returncode != 0 and "port must" in invalid.stderr
    shutdown = check_http(executable, env, output)
    rows.append({"project": "custom-http", "check_build_run": "passed", "shutdown": shutdown})
    (output / "results.json").write_text(json.dumps(rows, indent=2) + "\n", encoding="utf-8")
    print(f"Passed: custom-http ({shutdown} shutdown)", flush=True)

    executable = build("http-auth", "main")
    shutdown = check_http_auth(executable, env, output)
    rows.append({"project": "http-auth", "check_build_run": "passed", "shutdown": shutdown})
    (output / "results.json").write_text(json.dumps(rows, indent=2) + "\n", encoding="utf-8")
    print(f"Passed: http-auth ({shutdown} shutdown)", flush=True)

    verify_forms("supervised-service")
    verify_forms("task-results")
    (output / "results.json").write_text(json.dumps(rows, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"checks": len(rows), "status": "passed"}), flush=True)


if __name__ == "__main__":
    main()
