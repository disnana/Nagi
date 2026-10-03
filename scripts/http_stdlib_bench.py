"""Compare the same HTTP endpoints in Nagi, direct Rust, and the legacy server.

Linux only. Build the three release binaries before running this script. Results
include raw wrk output, CPU/RSS snapshots, affinity, and all failed requests.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import signal
import statistics
import subprocess
import time
import urllib.request

from http_bench import snapshot

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wrk", type=Path, required=True)
    parser.add_argument("--duration", type=int, default=5)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--soak", type=int, default=120)
    parser.add_argument("--output", type=Path, default=ROOT / "benchmarks/results/http-stdlib")
    parser.add_argument("--build-matching-rust", action="store_true",
                        help="build the Rust equivalent using the Nagi application's exact Cargo.lock")
    parser.add_argument("--rust-binary", type=Path,
                        help="default: a previously built matching Rust baseline in the native target")
    args = parser.parse_args()
    if args.duration < 1 or args.repeats < 1 or args.soak < 0:
        parser.error("duration/repeats must be positive; soak must be nonnegative")
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    native = Path(os.environ.get("NAGI_NATIVE_TARGET_DIR", ROOT / "build/native-target"))
    if args.build_matching_rust:
        generated = ROOT / "build/http_stdlib"
        comparison = ROOT / "build/http_stdlib_rust"
        (comparison / "src").mkdir(parents=True, exist_ok=True)
        original_name = 'name = "nagi-http-stdlib"'
        comparison_name = 'name = "nagi-http-rust-baseline"'
        for filename in ("Cargo.toml", "Cargo.lock"):
            text = (generated / filename).read_text()
            if text.count(original_name) != 1:
                raise RuntimeError(f"unexpected generated {filename}: build benchmarks/http_stdlib.nagi first")
            (comparison / filename).write_text(text.replace(original_name, comparison_name, 1))
        (comparison / "src/main.rs").write_text((ROOT / "runtime/examples/http_stdlib_baseline.rs").read_text())
        subprocess.run(["cargo", "build", "--release", "--locked", "--manifest-path",
                        str(comparison / "Cargo.toml")], env=dict(os.environ, CARGO_TARGET_DIR=str(native)), check=True)
    rust_binary = args.rust_binary or native / "release/nagi-http-rust-baseline"
    binaries = {
        "nagi": (native / "release/nagi-http-stdlib", 8086),
        "rust": (rust_binary, 8086),
        "legacy_axum": (target / "release/examples/axum_baseline", 8082),
    }
    for binary, _ in binaries.values():
        if not binary.is_file():
            parser.error(f"build the release binary first: {binary}")
    available = sorted(os.sched_getaffinity(0))
    if len(available) < 3:
        parser.error("at least three available CPUs are required")
    server_cpu, client_cpus = available[0], available[-2:]
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    environment = {
        "platform": platform.platform(),
        "available_cpus": available,
        "server_cpu": server_cpu,
        "client_cpus": client_cpus,
        "server_workers": 1,
        "client_threads": 2,
        "connections": 64,
        "duration_seconds": args.duration,
        "repeats": args.repeats,
        "clock_ticks_per_second": os.sysconf("SC_CLK_TCK"),
        "method": "local closed-loop wrk; separate CPU affinity on a shared host",
        "nagi_rust_options": "identical std.http.server defaults, no DB",
        "legacy_options": "existing Axum baseline; not identical admission/deadline policy",
        "binaries_sha256": {name: hashlib.sha256(binary.read_bytes()).hexdigest()
                            for name, (binary, _) in binaries.items()},
    }
    comparison_lock = ROOT / "build/http_stdlib_rust/Cargo.lock"
    generated_lock = ROOT / "build/http_stdlib/Cargo.lock"
    if comparison_lock.is_file() and generated_lock.is_file():
        same = (comparison_lock.read_text().replace('name = "nagi-http-rust-baseline"', 'name = "nagi-http-stdlib"')
                == generated_lock.read_text())
        environment["matching_dependency_lock"] = same
        if args.build_matching_rust and not same:
            raise RuntimeError("Rust/Nagi dependency locks differ")
    for name in ("cpu.max", "memory.max"):
        path = Path("/sys/fs/cgroup") / name
        if path.is_file():
            environment["cgroup_" + name.replace(".", "_")] = path.read_text().strip()
    model = next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines()
                  if line.startswith("model name")), "unknown")
    environment["cpu_model"] = model
    (output / "environment.json").write_text(json.dumps(environment, indent=2) + "\n")
    env = dict(os.environ, NAGI_THREADS="1", NAGI_DB=":memory:")
    cases = [
        ("plaintext", "/health", []),
        ("small_json", "/small", []),
        ("post_4k", "/echo", ["echo", "4096"]),
    ]
    rows = []

    def load(port, case, duration):
        command = [
            "taskset", "-c", ",".join(map(str, client_cpus)), str(args.wrk),
            "-t2", "-c64", f"-d{duration}s", "--latency", "-s",
            str(ROOT / "benchmarks/http.lua"), f"http://127.0.0.1:{port}{case[1]}",
            "--", *case[2],
        ]
        result = subprocess.run(command, capture_output=True, text=True, check=True)
        row = json.loads(next(line[7:] for line in result.stdout.splitlines() if line.startswith("RESULT ")))
        return row, result.stdout

    def start(name, label):
        binary, port = binaries[name]
        log = (output / f"{name}-server-{label}.log").open("w")
        process = subprocess.Popen(
            ["taskset", "-c", str(server_cpu), str(binary)],
            env=dict(env, NAGI_SAMPLE_PORT=str(port)), stdout=log, stderr=log,
        )
        try:
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError(f"{name} exited; see server log")
                try:
                    with urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=.5) as response:
                        assert response.status == 200 and response.read() == b"ok"
                    break
                except OSError:
                    time.sleep(.05)
            else:
                raise RuntimeError(f"{name} did not become ready")
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/small", timeout=2) as response:
                assert json.load(response) == {"id": 1, "name": "alice", "age": 18}
            request = urllib.request.Request(
                f"http://127.0.0.1:{port}/echo", data=b'{"name":"test","age":18}',
                headers={"Content-Type": "application/json"},
            )
            with urllib.request.urlopen(request, timeout=2) as response:
                assert json.load(response) == {"name": "test", "age": 18}
            return process, log, port
        except BaseException:
            stop(process, log)
            raise

    def stop(process, log):
        if process.poll() is None:
            process.send_signal(signal.SIGINT)
        try:
            code = process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
            raise RuntimeError("server failed to stop within 15 seconds")
        finally:
            log.close()
        if code != 0:
            raise RuntimeError(f"server shutdown failed: {code}")

    for repeat in range(args.repeats):
        order = list(binaries)
        random.Random(714 + repeat).shuffle(order)
        for name in order:
            process, log, port = start(name, repeat)
            try:
                for case in cases:
                    load(port, case, 1)
                    before = snapshot(process.pid)
                    started = time.monotonic()
                    row, raw = load(port, case, args.duration)
                    elapsed = time.monotonic() - started
                    after = snapshot(process.pid)
                    row.update(
                        implementation=name, case=case[0], repeat=repeat,
                        before=before, after=after, wall_seconds=elapsed,
                        cpu_seconds=(after["cpu_ticks"] - before["cpu_ticks"]) / os.sysconf("SC_CLK_TCK"),
                    )
                    rows.append(row)
                    (output / f"wrk-{name}-{case[0]}-{repeat}.txt").write_text(raw)
                    (output / "runs.json").write_text(json.dumps(rows, indent=2) + "\n")
                    print(f"{name} {case[0]} #{repeat}: {row['requests_s']:.0f} req/s", flush=True)
            finally:
                stop(process, log)

    summary = []
    for case in cases:
        for name in binaries:
            selected = [row for row in rows if row["case"] == case[0] and row["implementation"] == name]
            summary.append({
                "implementation": name, "case": case[0],
                **{key: statistics.median(row[key] for row in selected) for key in ["requests_s", "p50_us", "p95_us", "p99_us", "cpu_seconds"]},
                "rss_kib": statistics.median(row["after"]["rss_kib"] for row in selected),
                "errors": sum(row[key] for row in selected for key in ["connect_errors", "read_errors", "write_errors", "timeouts", "status_errors"]),
            })
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")

    if args.soak:
        process, log, port = start("nagi", "soak")
        try:
            load(port, cases[-1], 1)
            command = [
                "taskset", "-c", ",".join(map(str, client_cpus)), str(args.wrk),
                "-t2", "-c64", f"-d{args.soak}s", "--latency", "-s",
                str(ROOT / "benchmarks/http.lua"), f"http://127.0.0.1:{port}/echo", "--", "echo", "4096",
            ]
            client = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            started, samples = time.monotonic(), []
            while client.poll() is None:
                samples.append({"elapsed_seconds": time.monotonic() - started, **snapshot(process.pid)})
                time.sleep(1)
            raw, errors = client.communicate()
            if client.returncode:
                raise RuntimeError(f"wrk failed: {errors}")
            after = []
            for _ in range(5):
                after.append(snapshot(process.pid))
                time.sleep(1)
            row = json.loads(next(line[7:] for line in raw.splitlines() if line.startswith("RESULT ")))
            row.update(duration_seconds=args.soak, resource_samples=samples, after_load=after)
            (output / "soak.json").write_text(json.dumps(row, indent=2) + "\n")
            (output / "soak-wrk.txt").write_text(raw)
        finally:
            stop(process, log)
    print(json.dumps({"runs": len(rows), "soak_seconds": args.soak, "output": str(output)}))


if __name__ == "__main__":
    main()
