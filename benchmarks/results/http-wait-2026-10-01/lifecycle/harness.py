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

from http_capacity import ROOT, proc, recovery, save, server_for, tcp_states

HEALTH = b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "native-target/release/nagi-crud")
    parser.add_argument("--out", type=Path, default=ROOT / "build/connection-lifecycle")
    parser.add_argument("--seconds", type=int, default=120)
    parser.add_argument("--count", type=int, default=100)
    parser.add_argument("--server-cpus", default="0")
    args = parser.parse_args()
    if not 1 <= args.count <= 200 or args.seconds < 1:
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
    })
    (args.out / "harness.py").write_bytes(Path(__file__).read_bytes())
    groups = {name: [] for name in ("no_request", "partial_header", "partial_body", "idle_keepalive")}
    with server_for(args, args.out) as server:
        baseline = proc(server.pid)
        try:
            for name, sockets in groups.items():
                for _ in range(args.count):
                    connection = socket.create_connection(("127.0.0.1", 8080), timeout=5)
                    sockets.append(connection)
                    if name == "partial_header":
                        connection.sendall(b"GET /health HTTP/1.1\r\nHost:")
                    elif name == "partial_body":
                        connection.sendall(b"POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 4116\r\n\r\n{\"name\":\"")
                    elif name == "idle_keepalive":
                        connection.sendall(HEALTH)
                        response = b""
                        while not response.endswith(b"\r\n\r\nok"):
                            response += connection.recv(1024)
                            if len(response) > 4096:
                                raise RuntimeError("Unexpected health response")
                    connection.setblocking(False)
            start = time.monotonic()
            observations = []
            pending = {name: set(range(len(sockets))) for name, sockets in groups.items()}
            replies = {name: {} for name in groups}
            while time.monotonic() - start < args.seconds:
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
                row = {"elapsed_s": time.monotonic() - start, "server": proc(server.pid),
                       "tcp_states": tcp_states(server.pid),
                       "client_sockets_without_eof": {name: len(indices) for name, indices in pending.items()},
                       "response_first_lines": replies}
                # Freeze nested values before the next sample changes them.
                observations.append(json.loads(json.dumps(row)))
                if len(observations) % 15 == 0:
                    print(json.dumps(row), flush=True)
                time.sleep(1)
        finally:
            for sockets in groups.values():
                for connection in sockets:
                    connection.close()
        after_close = recovery(server, 5)
        # Exercise normal FIN and abrupt RST without altering the server.
        abrupt_before = proc(server.pid)
        for _ in range(200):
            with socket.create_connection(("127.0.0.1", 8080), timeout=5) as connection:
                connection.sendall(b"GET /health HTTP/1.1\r\nHost:")
                connection.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
        after_reset = recovery(server, 5)
    save(args.out / "result.json", {"baseline": baseline, "observations": observations,
                                    "after_client_close": after_close,
                                    "abrupt_disconnect_count": 200, "abrupt_before": abrupt_before,
                                    "after_abrupt_disconnect": after_reset})
    print(json.dumps({"phase": "lifecycle_complete", "seconds": args.seconds}), flush=True)


if __name__ == "__main__":
    main()
