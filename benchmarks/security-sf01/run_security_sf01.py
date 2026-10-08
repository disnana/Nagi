#!/usr/bin/env python3
"""Build and compare SF01 policy fixtures with a bounded sequential loopback run."""
from __future__ import annotations

import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import platform
import resource
import signal
import socket
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = Path(__file__).resolve().parent / "fixtures"
DEFAULT_RESULTS = ROOT / "benchmarks/results/security-sf01-2026-10-08"
EXPECTED_BODY = b'{"id":1,"title":"authorized fixture"}'
EXPECTED_CREDENTIAL = "Bearer sf01-fixture"
NORMAL_REQUESTS = 27
MAX_REQUESTS = 64


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def command_output(command: list[str], env: dict[str, str]) -> str:
    result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=30)
    if result.returncode:
        raise RuntimeError(f"{command!r} failed ({result.returncode}):\n{result.stdout}{result.stderr}")
    return (result.stdout + result.stderr).strip()


def fixture_project(mode: str, results: Path) -> Path:
    project = results / mode / "project"
    project.mkdir(parents=True, exist_ok=True)
    source = FIXTURES / mode
    (project / "main.nagi").write_bytes((source / "main.nagi").read_bytes())
    (project / "nagi.toml").write_bytes((source / "nagi.toml").read_bytes())
    native = (FIXTURES / "common/native_common.rs").read_text(encoding="utf-8")
    native += "\n\n" + (source / "authorize.rs").read_text(encoding="utf-8")
    (project / "native.rs").write_text(native, encoding="utf-8")
    return project


def build_mode(mode: str, compiler: Path, results: Path, env: dict[str, str]) -> dict:
    output = results / mode
    project = fixture_project(mode, results)
    check = subprocess.run(
        [str(compiler), "check", "--project", str(project)],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
        timeout=90,
    )
    (output / "check.log").write_text(check.stdout + check.stderr, encoding="utf-8")
    if check.returncode:
        raise RuntimeError(f"{mode} check failed ({check.returncode}):\n{check.stdout}{check.stderr}")

    build_rows = []
    executable = None
    build_dir = output / "generated-app"
    for index in range(3):
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        started = time.perf_counter()
        result = subprocess.run(
            [str(compiler), "build", "--project", str(project), "--out", str(build_dir)],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            timeout=240,
        )
        wall_seconds = time.perf_counter() - started
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        (output / f"build-{index}.log").write_text(
            result.stdout + result.stderr, encoding="utf-8"
        )
        if result.returncode:
            raise RuntimeError(
                f"{mode} build {index} failed ({result.returncode}):\n{result.stdout}{result.stderr}"
            )
        binaries = [
            line.removeprefix("native: ").strip()
            for line in (result.stdout + "\n" + result.stderr).splitlines()
            if line.startswith("native: ")
        ]
        if len(binaries) != 1:
            raise RuntimeError(f"could not identify {mode} executable:\n{result.stdout}{result.stderr}")
        executable = Path(binaries[0]).resolve()
        if not executable.is_file():
            raise RuntimeError(f"built executable is missing: {executable}")
        build_rows.append(
            {
                "run": index,
                "exit_status": result.returncode,
                "wall_seconds": wall_seconds,
                "child_user_seconds": after.ru_utime - before.ru_utime,
                "child_sys_seconds": after.ru_stime - before.ru_stime,
                "executable": str(executable),
                "executable_bytes": executable.stat().st_size,
                "executable_sha256": sha256_file(executable),
            }
        )
    assert executable is not None
    return {
        "mode": mode,
        "project": str(project),
        "check_exit_status": check.returncode,
        "build_rows": build_rows,
        "executable": str(executable),
        "executable_bytes": executable.stat().st_size,
        "executable_sha256": sha256_file(executable),
        "generated_main_sha256": sha256_file(project / "main.nagi"),
        "generated_native_sha256": sha256_file(project / "native.rs"),
        "cargo_lock_sha256": sha256_file(next(build_dir.rglob("Cargo.lock"))),
    }


def http_request(connection: http.client.HTTPConnection, path: str, credential: str | None):
    connection.putrequest("GET", path)
    if credential is not None:
        connection.putheader("Authorization", credential)
    connection.endheaders()
    response = connection.getresponse()
    body = response.read()
    headers = {key.lower(): value for key, value in response.getheaders()}
    return response.status, body, headers


def free_port() -> int:
    with socket.socket() as available:
        available.bind(("127.0.0.1", 0))
        return available.getsockname()[1]


def ensure_request_budget(used_requests: int, mode: str, label: str) -> None:
    if used_requests >= MAX_REQUESTS:
        raise RuntimeError(
            f"{mode} request budget exhausted before {label} send: "
            f"used={used_requests}, maximum={MAX_REQUESTS}"
        )


def validate_normal_response(result, mode: str, label: str) -> None:
    status, body, headers = result
    assert status == 200, (mode, label, status, body)
    assert body == EXPECTED_BODY, (mode, label, body)
    assert headers.get("content-type", "").startswith("application/json"), (mode, label, headers)


def run_server(
    mode: str,
    executable: Path,
    results: Path,
    env: dict[str, str],
    request_budget_before: int,
) -> dict:
    port = free_port()
    mode_env = dict(env, NAGI_SAMPLE_PORT=str(port))
    log_path = results / mode / "server.log"
    process = subprocess.Popen(
        [str(executable)],
        cwd=ROOT,
        env=mode_env,
        stdout=log_path.open("w", encoding="utf-8"),
        stderr=subprocess.STDOUT,
    )
    total_requests = 0
    latencies_ns = []
    measured_connection = None

    def send_request(connection, path: str, credential: str | None, label: str):
        nonlocal total_requests
        ensure_request_budget(request_budget_before + total_requests, mode, label)
        total_requests += 1
        return http_request(connection, path, credential)

    try:
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise RuntimeError(f"{mode} server exited before readiness; see {log_path}")
            try:
                connection = http.client.HTTPConnection("127.0.0.1", port, timeout=2)
                result = send_request(connection, "/health", None, "readiness")
                connection.close()
                if result[0] == 200 and result[1] == b"ok":
                    break
            except OSError:
                time.sleep(0.05)
        else:
            raise RuntimeError(f"{mode} server readiness timed out; see {log_path}")

        measured_connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
        warmup = send_request(
            measured_connection, "/documents/1", EXPECTED_CREDENTIAL, "warmup"
        )
        validate_normal_response(warmup, mode, "warmup")
        for index in range(NORMAL_REQUESTS):
            started = time.perf_counter_ns()
            response = send_request(
                measured_connection,
                "/documents/1",
                EXPECTED_CREDENTIAL,
                f"normal-{index}",
            )
            latencies_ns.append(time.perf_counter_ns() - started)
            validate_normal_response(response, mode, f"normal-{index}")
        measured_connection.close()
        measured_connection = None

        denied_connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
        denied = send_request(
            denied_connection, "/documents/2", EXPECTED_CREDENTIAL, "wrong-target"
        )
        denied_connection.close()
        assert denied[0] == 403 and denied[1] == b"permission denied", (mode, denied)

        missing_connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
        missing = send_request(
            missing_connection, "/documents/1", None, "missing-credential"
        )
        missing_connection.close()
        assert missing[0] == 401 and missing[1] == b"invalid credential", (mode, missing)
        assert missing[2].get("www-authenticate") == "Bearer", (mode, missing[2])
    finally:
        if measured_connection is not None:
            measured_connection.close()
        if process.poll() is None:
            process.send_signal(signal.SIGINT)
        try:
            exit_status = process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
            raise RuntimeError(f"{mode} server did not stop; see {log_path}") from None

    if exit_status != 0:
        raise RuntimeError(f"{mode} server shutdown failed with {exit_status}; see {log_path}")
    log_text = log_path.read_text(encoding="utf-8")
    metric_lines = [line.removeprefix("SF01_METRICS ") for line in log_text.splitlines() if line.startswith("SF01_METRICS ")]
    if len(metric_lines) != 1:
        raise RuntimeError(f"expected one SF01_METRICS record in {log_path}")
    metrics = json.loads(metric_lines[0])
    ordered = sorted(latencies_ns)
    percentile = lambda fraction: ordered[min(len(ordered) - 1, int((len(ordered) - 1) * fraction))]
    assert total_requests <= 32, (mode, total_requests)
    return {
        "mode": mode,
        "exit_status": exit_status,
        "http_requests": total_requests,
        "normal_requests_measured": len(latencies_ns),
        "sequential_one_connection": True,
        "normal_latency_ns": {
            "samples": latencies_ns,
            "min": min(latencies_ns),
            "median": int(statistics.median(latencies_ns)),
            "p95_nearest_lower_rank": percentile(0.95),
            "max": max(latencies_ns),
        },
        "denial_checks": {
            "wrong_resource": {"status": denied[0], "body": denied[1].decode()},
            "missing_credential": {
                "status": missing[0],
                "body": missing[1].decode(),
                "www_authenticate": missing[2].get("www-authenticate"),
            },
        },
        "native_measurements": metrics,
        "server_log": str(log_path),
    }


def source_hashes() -> dict[str, str]:
    paths = [
        FIXTURES / "common/native_common.rs",
        FIXTURES / "nagi-policy/main.nagi",
        FIXTURES / "nagi-policy/nagi.toml",
        FIXTURES / "nagi-policy/authorize.rs",
        FIXTURES / "rust-policy/main.nagi",
        FIXTURES / "rust-policy/nagi.toml",
        FIXTURES / "rust-policy/authorize.rs",
        Path(__file__).resolve(),
    ]
    return {str(path.relative_to(ROOT)): sha256_file(path) for path in paths}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--compiler",
        type=Path,
        default=Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "release" / "nagic",
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_RESULTS)
    args = parser.parse_args()
    compiler = args.compiler.resolve()
    results = args.output.resolve()
    if not compiler.is_file():
        raise FileNotFoundError(compiler)
    if (results / "measurement.json").exists():
        raise RuntimeError(f"refusing to overwrite completed measurement: {results}")

    env = dict(os.environ)
    env.setdefault("CARGO_HOME", "/workspace/toolchains/cargo")
    env.setdefault("RUSTUP_HOME", "/workspace/toolchains/rustup")
    env.setdefault("CARGO_TARGET_DIR", "/workspace/nagi-sf01-target")
    env.setdefault("NAGI_NATIVE_TARGET_DIR", "/workspace/nagi-sf01-native-target")
    env["NAGI_ROOT"] = str(ROOT)
    env["NAGI_THREADS"] = "1"
    env.setdefault("CARGO_NET_OFFLINE", "true")
    env.setdefault("CARGO_BUILD_JOBS", "2")
    env.setdefault("CARGO_INCREMENTAL", "0")
    env.setdefault("CARGO_PROFILE_DEV_DEBUG", "0")
    env.setdefault("CARGO_PROFILE_TEST_DEBUG", "0")
    env.setdefault("CARGO_PROFILE_RELEASE_DEBUG", "0")
    env["PATH"] = f"/workspace/toolchains/cargo/bin:/workspace/toolchains/rustup/bin:{env.get('PATH', '')}"
    results.mkdir(parents=True, exist_ok=True)

    version = {
        "compiler": command_output([str(compiler), "--version"], env),
        "rustc": command_output(["rustc", "--version"], env),
        "cargo": command_output(["cargo", "--version"], env),
    }
    environment = {
        "platform": platform.platform(),
        "python": platform.python_version(),
        "cpu_count": os.cpu_count(),
        "versions": version,
        "compiler_path": str(compiler),
        "compiler_sha256": sha256_file(compiler),
        "environment": {
            key: env.get(key)
            for key in (
                "CARGO_HOME",
                "RUSTUP_HOME",
                "CARGO_TARGET_DIR",
                "NAGI_NATIVE_TARGET_DIR",
                "NAGI_ROOT",
                "CARGO_NET_OFFLINE",
                "CARGO_BUILD_JOBS",
                "CARGO_INCREMENTAL",
                "CARGO_PROFILE_DEV_DEBUG",
                "CARGO_PROFILE_TEST_DEBUG",
                "CARGO_PROFILE_RELEASE_DEBUG",
                "NAGI_THREADS",
            )
        },
        "build_profile": "nagic build generated executable, optimized Cargo release profile; shared dependency cache retained, no clean target",
        "cache_policy": "existing CARGO_TARGET_DIR and NAGI_NATIVE_TARGET_DIR retained; offline build; three same-source builds per variant",
        "source_sha256": source_hashes(),
    }
    (results / "environment.json").write_text(json.dumps(environment, indent=2) + "\n", encoding="utf-8")

    build_rows = [build_mode(mode, compiler, results, env) for mode in ("nagi-policy", "rust-policy")]
    (results / "build-results.json").write_text(json.dumps(build_rows, indent=2) + "\n", encoding="utf-8")

    request_budget = 0
    runtime_rows = []
    for row in build_rows:
        runtime = run_server(
            row["mode"], Path(row["executable"]), results, env, request_budget
        )
        request_budget += runtime["http_requests"]
        runtime_rows.append(runtime)
    assert request_budget <= MAX_REQUESTS, request_budget

    report = {
        "status": "passed",
        "request_budget": {"used": request_budget, "maximum": MAX_REQUESTS},
        "comparison": "generated Nagi policy versus handwritten Rust policy through the same standard authorized_policy dispatcher, verifier, AuthScope binding, Grant issuance, bounded reservation, and Grant.submit target-bound operation",
        "input": {
            "method": "GET",
            "valid_path": "/documents/1",
            "valid_authorization": EXPECTED_CREDENTIAL,
            "valid_response": EXPECTED_BODY.decode(),
            "owner_subject": 7,
            "document_target": 1,
            "body_bytes": 0,
        },
        "request_pattern": {
            "per_variant": "one explicit public /health readiness request, one valid warmup, 27 sequential valid keep-alive requests, one valid-credential wrong-target denial, one missing-credential denial",
            "concurrency": 1,
            "normal_latency_samples_per_variant": NORMAL_REQUESTS,
            "no_load_sweep_or_capacity_exhaustion": True,
        },
        "limits": {
            "scope_heap_allocation_and_refcount": "not observable through the public API; not measured",
            "scope_retention": "only authorizer-entry-to-grant-mint interval is visible and measured; dispatcher time before authorizer is not",
            "grant_retention": "grant-mint-to-handler-native-entry and grant-mint-to-submit intervals are measured; no heap/refcount probe",
            "future_sizes": "inner verifier, inner policy, inner native read operation, and user callback futures are measured; private outer dispatcher future contents are not",
            "allocation_counts": "calling-thread Future::poll allocation windows and synchronous Grant::from_authorized window, via the runtime metrics allocator; the future itself is not heap allocated by the probe",
            "processing_time": "sequential loopback HTTP/1 keep-alive round trip, instrumented build, 27 small normal requests per variant; not a production SLO or backend comparison",
            "operation": "bounded in-memory fixture read after Grant.submit; not SQLite or general application throughput",
        },
        "runtime_runs": runtime_rows,
    }
    (results / "measurement.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": "passed", "requests": request_budget, "results": str(results)}) )


if __name__ == "__main__":
    main()
