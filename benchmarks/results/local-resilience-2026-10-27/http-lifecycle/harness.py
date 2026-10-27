"""Observe idle, incomplete, and disconnected HTTP clients on loopback only."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import socket
import struct
import time
from pathlib import Path

from http_capacity import (
    ROOT,
    cgroup,
    proc,
    recovery,
    request,
    resource_abort_reason,
    save,
    server_for,
    tcp_states,
)
from native_artifacts import native_executable

HEALTH = b"GET /health HTTP/1.1\r\nHost: localhost:8080\r\n\r\n"
PARTIAL_BODY = (
    b"POST /echo HTTP/1.1\r\nHost: localhost:8080\r\n"
    b"Content-Type: application/json\r\nContent-Length: 4116\r\n\r\n{\"name\":\""
)
MAX_RESPONSE_HEADER_BYTES = 8192
MAX_RESPONSE_BODY_BYTES = 1024 * 1024
RESPONSE_DEADLINE_SECONDS = 5


def read_http_response(connection, deadline_seconds: float = RESPONSE_DEADLINE_SECONDS) -> tuple[bytes, bytes]:
    deadline = time.monotonic() + deadline_seconds

    def receive(size: int) -> bytes:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise RuntimeError("HTTP response exceeded its total deadline")
        connection.settimeout(remaining)
        return connection.recv(size)

    response = b""
    while b"\r\n\r\n" not in response:
        chunk = receive(1024)
        if not chunk:
            raise RuntimeError("connection closed before the HTTP header completed")
        response += chunk
        if len(response) > MAX_RESPONSE_HEADER_BYTES:
            raise RuntimeError(f"HTTP response header exceeded {MAX_RESPONSE_HEADER_BYTES} bytes")
    header, body = response.split(b"\r\n\r\n", 1)
    status = header.split(b"\r\n", 1)[0]
    try:
        size = int(next(line.split(b":", 1)[1].strip() for line in header.split(b"\r\n")
                        if line.lower().startswith(b"content-length:")))
    except (StopIteration, ValueError) as error:
        raise RuntimeError("HTTP response needs a valid Content-Length") from error
    if size < 0 or size > MAX_RESPONSE_BODY_BYTES:
        raise RuntimeError(f"HTTP response body exceeds {MAX_RESPONSE_BODY_BYTES} bytes")
    while len(body) < size:
        chunk = receive(min(1024, size - len(body)))
        if not chunk:
            raise RuntimeError("connection closed before the HTTP body completed")
        body += chunk
    return status, body[:size]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--out", type=Path, default=ROOT / "build/connection-lifecycle")
    parser.add_argument("--seconds", type=int, default=120)
    parser.add_argument("--count", type=int, default=100)
    parser.add_argument("--server-cpus", default="0")
    parser.add_argument("--memory-ceiling", type=int, default=12 * 1024 ** 3)
    args = parser.parse_args()
    args.binary = args.binary or native_executable(ROOT / "build/crud", fallback_name="nagi-crud")
    if not 1 <= args.count <= 200 or args.seconds < 1 or args.memory_ceiling < 1:
        parser.error("Use 1..200 connections per group and a positive observation period")
    if not {int(cpu) for cpu in args.server_cpus.split(",")} <= os.sched_getaffinity(0):
        parser.error("Server CPUs must be available")
    args.binary, args.out = args.binary.resolve(), args.out.resolve()
    if args.out.exists():
        parser.error("Choose a new output directory")
    args.out.mkdir(parents=True)
    save(args.out / "environment.json", {
        "date_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "source_commit": __import__("subprocess").check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        "harness_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "http_helpers_sha256": hashlib.sha256((ROOT / "scripts/http_capacity.py").read_bytes()).hexdigest(),
        "connections_per_group": args.count, "observation_seconds": args.seconds,
        "server_cpus": args.server_cpus, "server_workers": 1,
        "server_rss_ceiling_bytes": 2 * 1024 ** 3,
        "cgroup_memory_ceiling_bytes": args.memory_ceiling,
    })
    (args.out / "harness.py").write_bytes(Path(__file__).read_bytes())
    groups = {name: [] for name in ("no_request", "partial_header", "partial_body", "idle_keepalive")}
    with server_for(args, args.out) as server:
        def guard(phase: str) -> None:
            reason = resource_abort_reason(proc(server.pid), cgroup(), args.memory_ceiling)
            if reason is not None:
                raise RuntimeError(f"{phase} aborted: {reason}")

        guard("lifecycle startup")
        baseline = proc(server.pid)
        try:
            for name, sockets in groups.items():
                for _ in range(args.count):
                    guard("slow-client creation")
                    connection = socket.create_connection(("127.0.0.1", 8080), timeout=5)
                    sockets.append(connection)
                    if name == "partial_header":
                        connection.sendall(b"GET /health HTTP/1.1\r\nHost:")
                    elif name == "partial_body":
                        connection.sendall(PARTIAL_BODY)
                    elif name == "idle_keepalive":
                        connection.sendall(HEALTH)
                        status, body = read_http_response(connection)
                        if status != b"HTTP/1.1 200 OK" or body != b"ok":
                            raise RuntimeError("unexpected health response")
                    connection.setblocking(False)
            start = time.monotonic()
            observations = []
            normal_probes = []
            pending = {name: set(range(len(sockets))) for name, sockets in groups.items()}
            replies = {name: {} for name in groups}
            while time.monotonic() - start < args.seconds:
                guard("slow-client observation")
                for name, sockets in groups.items():
                    for index in list(pending[name]):
                        try:
                            reply = sockets[index].recv(8192)
                            if reply:
                                status = reply.split(b"\r\n", 1)[0].decode(errors="replace")
                                replies[name][status] = replies[name].get(status, 0) + 1
                            else:
                                pending[name].remove(index)
                        except BlockingIOError:
                            pass
                        except ConnectionError:
                            pending[name].remove(index)
                try:
                    status, body, latency = request("GET", "/small")
                    normal_probe = {
                        "status": status,
                        "body_sha256": hashlib.sha256(body).hexdigest(),
                        "latency_ms": latency,
                    }
                except Exception as error:
                    normal_probe = {"error": type(error).__name__ + ": " + str(error)}
                normal_probes.append(normal_probe)
                row = {"elapsed_s": time.monotonic() - start, "server": proc(server.pid),
                       "tcp_states": tcp_states(server.pid),
                       "client_sockets_without_eof": {name: len(indices) for name, indices in pending.items()},
                       "response_first_lines": replies, "normal_probe": normal_probe}
                # Freeze nested values before the next sample changes them.
                observations.append(json.loads(json.dumps(row)))
                if len(observations) % 15 == 0:
                    print(json.dumps(row), flush=True)
                time.sleep(1)
        finally:
            for sockets in groups.values():
                for connection in sockets:
                    connection.close()
        after_close = recovery(server, 5, args.memory_ceiling)
        # Exercise normal FIN and abrupt RST without altering the server.
        abrupt_before = proc(server.pid)
        for _ in range(200):
            guard("abrupt-disconnect creation")
            with socket.create_connection(("127.0.0.1", 8080), timeout=5) as connection:
                connection.sendall(b"GET /health HTTP/1.1\r\nHost:")
                connection.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
        after_reset = recovery(server, 5, args.memory_ceiling)
    if any(probe.get("status") != 200 for probe in normal_probes):
        raise RuntimeError("normal /small traffic failed while slow clients were connected")
    save(args.out / "result.json", {"baseline": baseline, "observations": observations,
                                    "normal_probes": normal_probes,
                                    "after_client_close": after_close,
                                    "abrupt_disconnect_count": 200, "abrupt_before": abrupt_before,
                                    "after_abrupt_disconnect": after_reset})
    print(json.dumps({"phase": "lifecycle_complete", "seconds": args.seconds}), flush=True)


if __name__ == "__main__":
    main()
