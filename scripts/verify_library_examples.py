"""Build and exercise the seven library projects, including a real HTTP server."""
from __future__ import annotations

import argparse
import http.client
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
PROJECTS = ROOT / "test-nagi-code/library-examples"
EXE = ".exe" if os.name == "nt" else ""
OUTPUTS = {
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


def run(args, *, cwd=ROOT, env=None, input=None, timeout=180):
    result = subprocess.run(
        [str(arg) for arg in args], cwd=cwd, env=env, input=input,
        text=True, capture_output=True, timeout=timeout,
    )
    if result.returncode:
        raise RuntimeError(f"Command failed: {args}\n{result.stdout}{result.stderr}")
    return result.stdout


def response(port, path="/health", body=None):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
    try:
        connection.request("GET", path, body=body)
        result = connection.getresponse()
        return result.status, result.read(), dict(result.getheaders())
    finally:
        connection.close()


def wait_for_health(port, process):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"HTTP sample exited early: {process.returncode}")
        try:
            if response(port)[:2] == (200, b"ok\n"):
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", default=str(
        Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "release" / ("nagic" + EXE)
    ))
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
        result = run([compiler, "build", "--project", project], env=env)
        (output / (name + "-build.log")).write_text(result, encoding="utf-8")
        return target / "release" / ("nagi-" + entry.replace("_", "-") + EXE)

    for name, (entry, expected) in OUTPUTS.items():
        executable = build(name, entry)
        actual = run([executable], cwd=PROJECTS / name, env=env, timeout=10)
        assert actual == expected, (name, actual)
        if name == "low-kernel":
            source = PROJECTS / name / (entry + ".nagi")
            run([compiler, "build", source, "--no-project"], env=env)
            assert run([executable], env=env, timeout=10) == actual
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
    print(json.dumps({"projects": len(rows), "status": "passed"}), flush=True)


if __name__ == "__main__":
    main()
