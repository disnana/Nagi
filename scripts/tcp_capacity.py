"""Hold local HTTP connections with two independent clients up to the FD limit.

Successful initial responses, verified held connections, and TCP handshakes are
reported separately. The test does not raise the OS file-descriptor limits.
"""
from __future__ import annotations

import argparse
import asyncio
import hashlib
import json
import os
import resource
import select
import subprocess
import sys
import time
from pathlib import Path

from http_capacity import ROOT, proc, recovery, save, server_for, stop, tcp_states

HEALTH = b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n"


async def client(count: int, source_ip: str) -> None:
    connections = []
    errors = {}
    opened = 0
    replied = 0
    gate = asyncio.Semaphore(128)

    async def exchange(reader, writer):
        writer.write(HEALTH)
        await writer.drain()
        header = await asyncio.wait_for(reader.readuntil(b"\r\n\r\n"), 5)
        assert header.split(b"\r\n", 1)[0] == b"HTTP/1.1 200 OK", header
        size = int(next(line.split(b":", 1)[1] for line in header.split(b"\r\n")
                        if line.lower().startswith(b"content-length:")))
        assert await reader.readexactly(size) == b"ok"

    async def one():
        nonlocal opened, replied
        writer = None
        async with gate:
            try:
                reader, writer = await asyncio.wait_for(
                    asyncio.open_connection("127.0.0.1", 8080, local_addr=(source_ip, 0)), 5)
                opened += 1
                await exchange(reader, writer)
                replied += 1
                connections.append((reader, writer))
            except Exception as error:
                reason = type(error).__name__ + ": " + str(error)
                errors[reason] = errors.get(reason, 0) + 1
                if writer is not None:
                    writer.close()

    start = time.monotonic()
    await asyncio.gather(*(one() for _ in range(count)))
    print(json.dumps({"requested": count, "tcp_opened": opened, "initial_http_ok": replied,
                      "errors": errors, "seconds_to_open_and_reply": time.monotonic() - start,
                      "source_ip": source_ip, "client": proc(os.getpid())}), flush=True)
    try:
        while True:
            command = sys.stdin.readline().strip()
            if command != "verify":
                break
            verified = 0
            failures = 0

            async def again(reader, writer):
                nonlocal verified, failures
                async with gate:
                    try:
                        await exchange(reader, writer)
                        verified += 1
                    except Exception:
                        failures += 1

            await asyncio.gather(*(again(reader, writer) for reader, writer in connections))
            print(json.dumps({"verified_http_ok": verified, "verification_errors": failures,
                              "client": proc(os.getpid())}), flush=True)
    finally:
        for _, writer in connections:
            writer.close()
        await asyncio.gather(*(writer.wait_closed() for _, writer in connections), return_exceptions=True)


def messages(children: list[subprocess.Popen], server: subprocess.Popen, timeout: int) -> list[dict]:
    pending = {child.stdout.fileno(): (index, child.stdout) for index, child in enumerate(children)}
    rows = [None] * len(children)
    deadline = time.monotonic() + timeout
    while pending and time.monotonic() < deadline:
        if server.poll() is not None:
            raise RuntimeError("Server exited during the connection test")
        if proc(server.pid).get("rss_kib", 0) > 2 * 1024 * 1024:
            raise RuntimeError("Server exceeded the 2 GiB measurement ceiling")
        ready, _, _ = select.select(list(pending), [], [], 1)
        for fd in ready:
            index, stream = pending.pop(fd)
            line = stream.readline()
            if not line:
                raise RuntimeError("Client exited without a measurement")
            rows[index] = json.loads(line)
    if pending:
        raise RuntimeError("Connection clients exceeded their time budget")
    return rows


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "native-target/release/nagi-crud")
    parser.add_argument("--out", type=Path, default=ROOT / "build/tcp-capacity")
    parser.add_argument("--counts", default="1000,10000,15000,16000,16400,18000")
    parser.add_argument("--server-cpus", default="0")
    parser.add_argument("--client-cpus", default="2,3")
    parser.add_argument("--client-count", type=int)
    parser.add_argument("--source-ip", default="127.0.0.2")
    args = parser.parse_args()
    if args.client_count is not None:
        if not 1 <= args.client_count <= 10000 or not args.source_ip.startswith("127."):
            parser.error("Each client must use 1..10000 connections and a loopback source address")
        asyncio.run(client(args.client_count, args.source_ip))
        return
    args.binary, args.out = args.binary.resolve(), args.out.resolve()
    counts = [int(value) for value in args.counts.split(",")]
    if any(value < 2 or value > 20000 for value in counts):
        parser.error("Connection counts must be 2..20000")
    if (args.out / "environment.json").exists():
        parser.error("Choose a new output directory; results are never overwritten")
    affinity = os.sched_getaffinity(0)
    server_cpus = {int(value) for value in args.server_cpus.split(",")}
    client_cpus = [int(value) for value in args.client_cpus.split(",")]
    if len(client_cpus) != 2 or server_cpus & set(client_cpus) or not (server_cpus | set(client_cpus)) <= affinity:
        parser.error("Choose disjoint server CPUs and two available client CPUs")
    args.out.mkdir(parents=True, exist_ok=True)
    save(args.out / "environment.json", {
        "date_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        "harness_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "http_helpers_sha256": hashlib.sha256((ROOT / "scripts/http_capacity.py").read_bytes()).hexdigest(),
        "cpu_quota": Path("/sys/fs/cgroup/cpu.max").read_text().strip(),
        "memory_limit": Path("/sys/fs/cgroup/memory.max").read_text().strip(),
        "fd_limits": resource.getrlimit(resource.RLIMIT_NOFILE),
        "server_cpus": args.server_cpus, "client_cpus": args.client_cpus, "server_workers": 1,
        "client_count": 2, "client_open_concurrency_each": 128,
        "source_address_policy": "Two new 127/8 source addresses per case, avoiding port reuse between cases",
    })
    (args.out / "tcp_harness.py").write_bytes(Path(__file__).read_bytes())
    (args.out / "http_helpers.py").write_bytes((ROOT / "scripts/http_capacity.py").read_bytes())
    results = []
    for index, count in enumerate(counts):
        folder = args.out / str(count)
        folder.mkdir()
        children = []
        logs = []
        with server_for(args, folder) as server:
            before = proc(server.pid)
            try:
                for number, size in enumerate([count // 2, count - count // 2]):
                    log = (folder / f"client-{number}.stderr").open("w")
                    logs.append(log)
                    children.append(subprocess.Popen(
                        ["taskset", "-c", str(client_cpus[number]), sys.executable, str(Path(__file__).resolve()),
                         "--client-count", str(size), "--source-ip", f"127.0.0.{2 + index * 2 + number}"],
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=log, text=True))
                initial = messages(children, server, 150)
                held = proc(server.pid)
                for child in children:
                    child.stdin.write("verify\n")
                    child.stdin.flush()
                verification = messages(children, server, 60)
                row = {"requested": count, "tcp_opened": sum(item["tcp_opened"] for item in initial),
                       "initial_http_ok": sum(item["initial_http_ok"] for item in initial),
                       "verified_held_http_ok": sum(item["verified_http_ok"] for item in verification),
                       "initial_clients": initial, "verification_clients": verification,
                       "server_before": before, "server_held": held,
                       "tcp_states_held": tcp_states(server.pid)}
            finally:
                for child in children:
                    try:
                        child.stdin.write("close\n")
                        child.stdin.flush()
                        child.wait(timeout=10)
                    except (BrokenPipeError, subprocess.TimeoutExpired):
                        stop(child)
                for log in logs:
                    log.close()
            row["recovery"] = recovery(server, 5)
            row["server_after"] = proc(server.pid)
        results.append(row)
        save(args.out / "connections.json", results)
        print(json.dumps({"phase": "connections", "requested": count,
                          "http_ok": row["initial_http_ok"], "verified_held": row["verified_held_http_ok"],
                          "server_fds": held["fds"], "server_rss_mib": round(held["rss_kib"] / 1024, 2),
                          "fds_after": row["server_after"]["fds"]}), flush=True)


if __name__ == "__main__":
    main()
