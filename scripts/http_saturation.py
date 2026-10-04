"""Use fixed concurrent wrk clients to supplement paced load measurements.

This closed-loop test excludes unsent arrivals from its latency measurements.
All traffic stays on loopback. Requires native wrk 4.2.0 and Linux/cgroup v2.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import resource
import subprocess
import time
from pathlib import Path

from http_capacity import ROOT, cgroup, proc, recovery, save, server_for, stop
from native_artifacts import native_executable


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--wrk", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=ROOT / "build/http-saturation")
    parser.add_argument("--duration", type=int, default=8)
    parser.add_argument("--server-cpus", default="0")
    parser.add_argument("--client-cpus", default="2,3")
    parser.add_argument("--connections", default="128,512,2048")
    args = parser.parse_args()
    args.binary = args.binary or native_executable(ROOT / "build/crud", fallback_name="nagi-crud")
    counts = [int(value) for value in args.connections.split(",")]
    if not counts or any(count < 2 or count > 4096 for count in counts) or args.duration < 1:
        parser.error("Use 2..4096 connections and a positive duration")
    server_cpus = {int(cpu) for cpu in args.server_cpus.split(",")}
    client_cpus = {int(cpu) for cpu in args.client_cpus.split(",")}
    if server_cpus & client_cpus or not (server_cpus | client_cpus) <= os.sched_getaffinity(0):
        parser.error("Use disjoint server/client CPUs within available affinity")
    args.binary, args.wrk, args.out = (path.resolve() for path in (args.binary, args.wrk, args.out))
    if args.out.exists():
        parser.error("Choose a new output directory")
    args.out.mkdir(parents=True)
    save(args.out / "environment.json", {
        "date_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        "load_generator_sha256": hashlib.sha256(args.wrk.read_bytes()).hexdigest(),
        "harness_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "http_helpers_sha256": hashlib.sha256((ROOT / "scripts/http_capacity.py").read_bytes()).hexdigest(),
        "lua_sha256": hashlib.sha256((ROOT / "benchmarks/http.lua").read_bytes()).hexdigest(),
        "server_cpus": args.server_cpus, "client_cpus": args.client_cpus,
        "server_workers": 1, "client_threads": 2, "fd_limits": resource.getrlimit(resource.RLIMIT_NOFILE),
        "cpu_quota": Path("/sys/fs/cgroup/cpu.max").read_text().strip(),
        "limitations": ["fixed-concurrency closed-loop; excludes unsent arrivals", "HTTP/1.1 without TLS on loopback"],
    })
    (args.out / "harness.py").write_bytes(Path(__file__).read_bytes())
    results = []
    for case, path, extras in [("health", "/health", []), ("json_4k", "/echo", ["echo", "4096"])]:
        for count in counts:
            for repeat in range(3):
                folder = args.out / f"{case}-{count}-{repeat}"
                folder.mkdir()
                command = ["taskset", "-c", args.client_cpus, str(args.wrk), "-t2", f"-c{count}",
                           f"-d{args.duration}s", "--timeout", "5s", "--latency", "-s", str(ROOT / "benchmarks/http.lua"),
                           "http://127.0.0.1:8080" + path, "--"] + extras
                with server_for(args, folder) as server:
                    subprocess.run([*command[:5], "-c32", "-d1s", *command[7:]],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, check=True, timeout=10)
                    before = proc(server.pid)
                    children_before = resource.getrusage(resource.RUSAGE_CHILDREN)
                    start = time.monotonic()
                    samples = []
                    with (folder / "wrk.txt").open("w") as output:
                        worker = subprocess.Popen(command, stdout=output, stderr=output)
                        try:
                            while worker.poll() is None:
                                samples.append({"elapsed_s": time.monotonic() - start,
                                                "server": proc(server.pid), "client": proc(worker.pid)})
                                if (samples[-1]["server"].get("exited") or
                                        samples[-1]["server"].get("rss_kib", 0) > 2 * 1024 * 1024 or
                                        cgroup()["memory_current"] > 12 * 1024 ** 3 or
                                        time.monotonic() - start > args.duration + 15):
                                    raise RuntimeError("Server exit or measurement resource/time ceiling")
                                time.sleep(.25)
                            if worker.returncode:
                                raise RuntimeError("wrk failed; see " + str(folder / "wrk.txt"))
                        finally:
                            stop(worker)
                    wall = time.monotonic() - start
                    after = proc(server.pid)
                    children_after = resource.getrusage(resource.RUSAGE_CHILDREN)
                    native = json.loads(next(line[7:] for line in (folder / "wrk.txt").read_text().splitlines() if line.startswith("RESULT ")))
                    row = {"case": case, "connections": count, "repeat": repeat, "duration_s": args.duration,
                           "metrics": native, "server_before": before, "server_after": after, "resources": samples,
                           "server_cpu_percent_of_one_core": (after["cpu_ticks"] - before["cpu_ticks"]) / os.sysconf("SC_CLK_TCK") / wall * 100,
                           "client_cpu_cores": (children_after.ru_utime + children_after.ru_stime - children_before.ru_utime - children_before.ru_stime) / wall,
                           "command": command, "recovery": recovery(server, 2)}
                results.append(row)
                save(args.out / "results.json", results)
                print(json.dumps({"phase": "saturation", "case": case, "connections": count, "repeat": repeat,
                                  "rps": native["requests_s"], "cpu_percent": row["server_cpu_percent_of_one_core"]}), flush=True)


if __name__ == "__main__":
    main()
