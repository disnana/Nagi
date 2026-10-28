"""Run a finite loopback-only overload and recovery check for the CRUD sample.

This is an application load-shedding check, not an Internet DDoS claim.  It
keeps the server and client on 127/8, caps client workers/connections, enforces
request, wall-time, server-RSS, and cgroup-memory budgets, and verifies that an
unrelated route remains usable while one route rejects excess traffic.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import statistics
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from http_capacity import (
    LOCAL_OPENER,
    URL,
    cgroup,
    load,
    proc,
    resource_abort_reason,
    save,
    server_for,
    tcp_states,
)
from native_artifacts import native_executable

ROOT = Path(__file__).resolve().parents[1]


def probe(path: str) -> dict:
    started = time.monotonic()
    try:
        with LOCAL_OPENER.open(URL + path, timeout=2) as response:
            body = response.read()
            return {
                "status": response.status,
                "body_sha256": hashlib.sha256(body).hexdigest(),
                "latency_ms": (time.monotonic() - started) * 1000,
            }
    except urllib.error.HTTPError as error:
        body = error.read()
        return {
            "status": error.code,
            "body_sha256": hashlib.sha256(body).hexdigest(),
            "latency_ms": (time.monotonic() - started) * 1000,
        }
    except Exception as error:
        return {
            "error": type(error).__name__ + ": " + str(error),
            "latency_ms": (time.monotonic() - started) * 1000,
        }


def percentile(values: list[float], fraction: float) -> float:
    if not values:
        raise ValueError("percentile needs at least one observation")
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, int((len(ordered) - 1) * fraction))]


def fixed_request_budget(args: argparse.Namespace) -> int:
    # server_for makes at most 100 readiness attempts, then three functional
    # preflight requests before yielding. Connection-refused attempts are kept
    # in the budget even when they never reach the application.
    server_preflight_upper_bound = 103
    # Supported rates divide Vegeta's one-second nanosecond pacing base. One
    # extra dispatch per phase is retained as a conservative endpoint bound.
    normal = args.normal_rate * args.normal_seconds + 1
    overload = args.overload_rate * args.overload_seconds + 1
    return normal + overload + server_preflight_upper_bound


def validate_exact_pacing_rate(rate: int) -> None:
    if rate > 1_000_000_000 or 1_000_000_000 % rate != 0:
        raise ValueError("rate must be an exact divisor of 1,000,000,000 requests/s")


def validate_request_ceiling(metrics: dict, rate: int, seconds: int) -> None:
    ceiling = rate * seconds + 1
    if metrics["requests"] > ceiling:
        raise RuntimeError(f"Vegeta exceeded the request ceiling: {metrics['requests']} > {ceiling}")


def validate_overload_metrics(metrics: dict) -> None:
    statuses = metrics["status_codes"]
    requests = metrics["requests"]
    if sum(statuses.values()) != requests:
        raise RuntimeError("overload status counts do not match the request count")
    unexpected = set(statuses) - {"200", "429"}
    if unexpected:
        raise RuntimeError("overload produced unexpected statuses: " + ", ".join(sorted(unexpected)))
    if int(statuses.get("200", 0)) == 0 or int(statuses.get("429", 0)) == 0:
        raise RuntimeError("overload must contain both accepted and HTTP 429 responses")
    unexpected_errors = set(metrics.get("errors", [])) - {"429 Too Many Requests"}
    if unexpected_errors:
        raise RuntimeError("overload produced unexpected transport/application errors")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--vegeta", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=ROOT / "build/http-resilience")
    parser.add_argument("--normal-rate", type=int, default=100)
    parser.add_argument("--normal-seconds", type=int, default=3)
    parser.add_argument("--overload-rate", type=int, default=10_000)
    parser.add_argument("--overload-seconds", type=int, default=3)
    parser.add_argument("--recovery-seconds", type=int, default=5)
    parser.add_argument("--probe-interval", type=float, default=.1)
    parser.add_argument("--request-budget", type=int, default=50_000)
    parser.add_argument("--max-workers", type=int, default=512)
    parser.add_argument("--max-connections", type=int, default=256)
    parser.add_argument("--server-cpus", default="0")
    parser.add_argument("--client-cpus", default="2,3")
    parser.add_argument("--memory-ceiling", type=int, default=12 * 1024 ** 3)
    args = parser.parse_args()
    if min(
        args.normal_rate,
        args.normal_seconds,
        args.overload_rate,
        args.overload_seconds,
        args.recovery_seconds,
        args.probe_interval,
        args.request_budget,
        args.max_workers,
        args.max_connections,
    ) <= 0:
        parser.error("rates, durations, budgets, workers, connections, and probe interval must be positive")
    try:
        validate_exact_pacing_rate(args.normal_rate)
        validate_exact_pacing_rate(args.overload_rate)
    except ValueError as error:
        parser.error(str(error))
    fixed_requests = fixed_request_budget(args)
    probe_budget = args.request_budget - fixed_requests
    if probe_budget < 2:
        parser.error(
            f"fixed request upper bound {fixed_requests} leaves fewer than two probes in "
            f"--request-budget {args.request_budget}"
        )
    affinity = os.sched_getaffinity(0)
    server_cpus = {int(value) for value in args.server_cpus.split(",")}
    client_cpus = {int(value) for value in args.client_cpus.split(",")}
    if server_cpus & client_cpus or not (server_cpus | client_cpus) <= affinity:
        parser.error("Use disjoint server/client CPUs within the available affinity")
    args.binary = (args.binary or native_executable(ROOT / "build/crud", fallback_name="nagi-crud")).resolve()
    args.vegeta = args.vegeta.resolve()
    args.out = args.out.resolve()
    args.local_address = "127.0.20.1"
    if args.out.exists():
        parser.error("Choose a new output directory; existing evidence is never overwritten")
    args.out.mkdir(parents=True)
    save(args.out / "environment.json", {
        "date_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "source_commit": __import__("subprocess").check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip(),
        "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        "vegeta_sha256": hashlib.sha256(args.vegeta.read_bytes()).hexdigest(),
        "harness_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "http_helper_sha256": hashlib.sha256((ROOT / "scripts/http_capacity.py").read_bytes()).hexdigest(),
        "request_budget": args.request_budget,
        "fixed_request_upper_bound": fixed_requests,
        "probe_request_budget": probe_budget,
        "server_rss_ceiling_bytes": 2 * 1024 ** 3,
        "cgroup_memory_ceiling_bytes": args.memory_ceiling,
        "cgroup_memory_limit": Path("/sys/fs/cgroup/memory.max").read_text().strip(),
        "cgroup_before": cgroup(),
        "server_cpus": args.server_cpus,
        "client_cpus": args.client_cpus,
        "limitations": [
            "loopback HTTP/1.1 without TLS",
            "finite local overload, not an Internet or distributed DDoS test",
            "queue length is not exposed by this public sample",
            "one process and one NAGI_THREADS worker",
        ],
    })
    (args.out / "harness.py").write_bytes(Path(__file__).read_bytes())
    (args.out / "http_helpers.py").write_bytes((ROOT / "scripts/http_capacity.py").read_bytes())

    with server_for(args, args.out) as server:
        normal = load(
            args, args.out / "normal", server, "health", args.normal_rate,
            args.normal_seconds, .25,
        )
        validate_request_ceiling(normal["metrics"], args.normal_rate, args.normal_seconds)
        if normal["metrics"]["success"] != 1 or normal["metrics"]["status_codes"] != {"200": normal["metrics"]["requests"]}:
            raise RuntimeError("normal phase did not return only HTTP 200 responses")

        probes: list[dict] = []
        args.local_address = "127.0.20.2"
        with ThreadPoolExecutor(max_workers=1) as executor:
            future = executor.submit(
                load, args, args.out / "overload", server, "health",
                args.overload_rate, args.overload_seconds, .25,
            )
            deadline = time.monotonic() + args.overload_seconds + 35
            while not future.done():
                if time.monotonic() > deadline:
                    raise RuntimeError("overload phase exceeded its wall-time budget")
                if len(probes) >= probe_budget:
                    raise RuntimeError("the independent-route probe request budget was exhausted")
                reason = resource_abort_reason(proc(server.pid), cgroup(), args.memory_ceiling)
                if reason is not None:
                    raise RuntimeError("Independent-route probe aborted: " + reason)
                probes.append(probe("/small"))
                time.sleep(args.probe_interval)
            overload = future.result()
        validate_request_ceiling(overload["metrics"], args.overload_rate, args.overload_seconds)
        if any(row.get("status") != 200 for row in probes):
            raise RuntimeError("the independent /small route failed during /health overload")
        validate_overload_metrics(overload["metrics"])

        recovery = []
        recovery_deadline = time.monotonic() + args.recovery_seconds
        while time.monotonic() < recovery_deadline:
            if len(probes) + len(recovery) >= probe_budget:
                raise RuntimeError("the recovery probe request budget was exhausted")
            reason = resource_abort_reason(proc(server.pid), cgroup(), args.memory_ceiling)
            if reason is not None:
                raise RuntimeError("Route recovery aborted: " + reason)
            row = probe("/health")
            recovery.append(row)
            if row.get("status") == 200:
                break
            time.sleep(args.probe_interval)
        if not recovery or recovery[-1].get("status") != 200:
            raise RuntimeError("the overloaded route did not recover within the configured deadline")
        after = {"server": proc(server.pid), "tcp_states": tcp_states(server.pid), "cgroup": cgroup()}

    latencies = [row["latency_ms"] for row in probes]
    result = {
        "normal": normal,
        "overload": overload,
        "independent_route": {
            "requests": len(probes),
            "statuses": {str(status): sum(row.get("status") == status for row in probes)
                         for status in sorted({row.get("status") for row in probes if "status" in row})},
            "latency_ms": {
                "median": statistics.median(latencies),
                "p95": percentile(latencies, .95),
                "p99": percentile(latencies, .99),
                "max": max(latencies),
            },
            "samples": probes,
        },
        "route_recovery": recovery,
        "after_overload": after,
    }
    save(args.out / "result.json", result)
    print(json.dumps({
        "normal_200": normal["metrics"]["status_codes"].get("200", 0),
        "overload_200": overload["metrics"]["status_codes"].get("200", 0),
        "overload_429": overload["metrics"]["status_codes"].get("429", 0),
        "independent_route_200": len(probes),
        "independent_route_p99_ms": result["independent_route"]["latency_ms"]["p99"],
        "route_recovered": recovery[-1].get("status") == 200,
        "output": str(args.out),
    }), flush=True)


if __name__ == "__main__":
    main()
