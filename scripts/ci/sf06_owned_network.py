"""Run two owned-network SF06 oracles; never fall back to an external endpoint.

Compilation happens outside this helper. Only namespace-local interfaces/routes
are created, and the already-built binary runs after UID/capability removal.
Raw child output is bounded, retained in memory, and never printed/uploaded.
"""
from __future__ import annotations

import argparse
import hashlib
import ipaddress
import json
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import subprocess
import sys
import threading

CASES = (
    "production_owned_dns_tls_grant_and_shutdown",
    "production_owned_dns_rejects_mixed_unsafe_family",
)
RAW_LIMIT = 1024 * 1024
CASE_SECONDS = 120
LIMIT_KEYS = frozenset((
    "root_anchors", "root_bytes", "config_bytes", "jobs", "queue", "job_roots",
    "dns_roots", "dns_children", "http_drivers", "waiters", "waiter_bytes", "senders",
    "whole_ms", "connect_ms", "attempts", "input_bytes", "sockets", "max_headers",
    "max_header_bytes", "max_http_buffer_bytes", "max_request_bytes", "max_response_bytes", "max_target_bytes",
))


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate field")
        result[key] = value
    return result


class Refused(Exception):
    pass


def capture(command: list[str], seconds: int = CASE_SECONDS) -> tuple[bytes, int, bool]:
    process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, start_new_session=True)
    raw = bytearray()
    oversized = False

    def drain():
        nonlocal oversized
        assert process.stdout is not None
        with process.stdout:
            while chunk := process.stdout.read(8192):
                available = max(0, RAW_LIMIT + 1 - len(raw))
                raw.extend(chunk[:available])
                oversized |= len(raw) > RAW_LIMIT or len(chunk) > available

    reader = threading.Thread(target=drain, daemon=True)
    reader.start()
    try:
        status = process.wait(timeout=seconds)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=10)
        status = -1
    reader.join(timeout=10)
    return bytes(raw), status, oversized or reader.is_alive()


def case_result(name: str, raw: bytes, status: int, oversized: bool) -> dict:
    result = {"case": name, "state": "failed"}
    if name not in CASES or status != 0 or oversized or len(raw) > RAW_LIMIT:
        return result
    try:
        text = raw.decode("utf-8", errors="strict").replace("\r\n", "\n")
    except UnicodeError:
        return result
    if "\r" in text or "\x00" in text:
        return result
    lines = text.splitlines()
    rows = [(i, line) for i, line in enumerate(lines)
            if line.startswith("test ") and not line.startswith("test result:")]
    running = [(i, line) for i, line in enumerate(lines) if line.startswith("running ")]
    summaries = [(i, line) for i, line in enumerate(lines) if line.startswith("test result:")]
    records = [(i, line) for i, line in enumerate(lines) if "sf06_production_profile" in line]
    # Match ALL candidate rows/summaries, not only the expected positive shape.
    # The exact two-test executable inventory is independently checked below.
    if (len(rows) != 1 or rows[0][1] != f"test {name} ... ok"
            or len(running) != 1 or running[0][1] != "running 1 test"
            or len(summaries) != 1 or len(records) != 1):
        return result
    summary = re.fullmatch(
        r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; "
        + str(len(CASES) - 1) + r" filtered out; finished in [0-9]{1,6}\.[0-9]{2}s", summaries[0][1])
    record = re.fullmatch(r"sf06_production_profile ([^\n]{1,8192})", records[0][1])
    if (summary is None or record is None
            or not running[0][0] < rows[0][0] < summaries[0][0]
            or not running[0][0] < records[0][0] < summaries[0][0]):
        return result
    try:
        profile = json.loads(record[1], object_pairs_hook=unique_object)
        if (set(profile) != {"schema_version", "case", "elapsed_us", "counts", "configured_limits"}
                or type(profile["schema_version"]) is not int or profile["schema_version"] != 1
                or profile["case"] != name or type(profile["elapsed_us"]) is not int
                or not 0 < profile["elapsed_us"] <= CASE_SECONDS * 1_000_000):
            return result
        expected_counts = {"dns_a": 1, "dns_aaaa": 1, "http": int(name == CASES[0]), "joins": 3}
        if (profile["counts"] != expected_counts
                or any(type(value) is not int for value in profile["counts"].values())):
            return result
        limits = profile["configured_limits"]
        if (set(limits) != LIMIT_KEYS or any(type(value) is not int or not 0 < value <= 1_000_000_000
                                            for value in limits.values())):
            return result
    except (ValueError, KeyError, TypeError, AttributeError, RecursionError):
        return result
    result.update(state="passed", profile=profile)
    # Only fixed identities and validated finite numeric fields are emitted.
    return result


def inventory_valid(raw: bytes, status: int, oversized: bool) -> bool:
    if status != 0 or oversized or len(raw) > RAW_LIMIT:
        return False
    try:
        text = raw.decode("utf-8", errors="strict").replace("\r\n", "\n")
    except UnicodeError:
        return False
    if "\r" in text or "\x00" in text:
        return False
    lines = [line for line in text.splitlines() if line]
    return (len(lines) == len(CASES) + 1
            and lines[-1] == f"{len(CASES)} tests, 0 benchmarks"
            and sorted(lines[:-1]) == sorted(f"{name}: test" for name in CASES))


def pid_isolated(host_inode: int, *, check_host_init: bool = False) -> bool:
    if os.getpid() != 1 or type(host_inode) is not int or host_inode <= 0:
        return False
    current = os.stat("/proc/self/ns/pid").st_ino
    return (current != host_inode and (not check_host_init
            or current != os.stat("/proc/1/ns/pid").st_ino))


def watch_parent() -> None:
    # Only PID1 may consume this read end. Every subprocess gets DEVNULL stdin;
    # only the outer unprivileged parent owns the pipe's write end.
    if os.getpid() != 1 or not stat.S_ISFIFO(os.fstat(0).st_mode):
        raise Refused("parent-lease")

    def eof():
        try:
            os.read(0, 1)
        finally:
            # EOF (or unexpected input/read failure) exits namespace init. Linux
            # then kills/reaps ALL namespace descendants, including setsid ones.
            os._exit(125)

    threading.Thread(target=eof, daemon=True, name="parent-lease").start()
    # Popen inherited the launch-critical mask. Native children must not inherit
    # that temporary suppression of their ordinary termination signals.
    signal.pthread_sigmask(signal.SIG_UNBLOCK, (signal.SIGINT, signal.SIGTERM))


class Cancelled(Exception):
    pass


def supervised(command: list[str]) -> int:
    process = None
    previous = {sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}

    def cancel(_signal, _frame):
        raise Cancelled()

    # Block only the launch/assignment critical section, so cancellation cannot
    # orphan a successfully launched child before this owner receives its pipe.
    blocked = signal.pthread_sigmask(signal.SIG_BLOCK, previous)
    try:
        for sig in previous:
            signal.signal(sig, cancel)
        process = subprocess.Popen(command, stdin=subprocess.PIPE, close_fds=True,
                                   start_new_session=True, stderr=subprocess.DEVNULL)
        signal.pthread_sigmask(signal.SIG_SETMASK, blocked)
        return process.wait()
    except Cancelled:
        return 1
    finally:
        # No signal may interrupt lease close + waiter reap. Closing the sole
        # write end works across sudo, setpriv exec, and detached native sessions.
        signal.pthread_sigmask(signal.SIG_BLOCK, previous)
        try:
            if process is not None:
                try:
                    if process.stdin is not None:
                        process.stdin.close()
                finally:
                    process.wait()
        finally:
            for sig, handler in previous.items():
                signal.signal(sig, handler)
            signal.pthread_sigmask(signal.SIG_SETMASK, blocked)


def run_ip(*arguments: str) -> None:
    subprocess.run(["ip", *arguments], stdin=subprocess.DEVNULL, check=True, stdout=subprocess.DEVNULL,
                   stderr=subprocess.DEVNULL, timeout=10)


def read_ip(*arguments: str):
    raw = subprocess.check_output(["ip", "-j", *arguments], stdin=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                  timeout=10)
    if len(raw) > 65536:
        raise Refused("network-inventory")
    return json.loads(raw)


def topology_valid(links, addresses, routes4, routes6) -> bool:
    names = {"lo", "owned0", "owned1"}
    if not isinstance(links, list) or {x.get("ifname") for x in links} != names:
        return False
    allowed4 = (ipaddress.ip_network("127.0.0.0/8"), ipaddress.ip_network("11.0.0.0/30"))
    allowed6 = (ipaddress.ip_network("::1/128"), ipaddress.ip_network("ff00::/8"))
    try:
        owned = set()
        for interface in addresses:
            if interface.get("ifname") not in names:
                return False
            for address in interface.get("addr_info", []):
                value = ipaddress.ip_address(address["local"])
                allowed = allowed4 if value.version == 4 else allowed6
                if not any(value in network for network in allowed):
                    return False
                if str(value) in ("11.0.0.1", "11.0.0.2"):
                    owned.add(str(value))
        if owned != {"11.0.0.1", "11.0.0.2"}:
            return False
        for version, routes, allowed in ((4, routes4, allowed4), (6, routes6, allowed6)):
            for route in routes:
                if route.get("dev") not in names or "gateway" in route:
                    return False
                destination = route.get("dst", "default")
                if destination == "default":
                    return False
                network = ipaddress.ip_network(destination, strict=False)
                if network.version != version or not any(network.subnet_of(item) for item in allowed):
                    return False
    except (KeyError, TypeError, ValueError, AttributeError):
        return False
    return True


def network_isolated(host_inode: int) -> bool:
    # The privileged entry additionally checks PID1 before any network mutation.
    # After dropping capabilities, /proc/1/ns/net may correctly be unreadable.
    return os.stat("/proc/self/ns/net").st_ino != host_inode


def check_network(host_inode: int) -> None:
    if not network_isolated(host_inode) or not topology_valid(
            read_ip("link", "show"), read_ip("addr", "show"),
            read_ip("-4", "route", "show", "table", "all"),
            read_ip("-6", "route", "show", "table", "all")):
        raise Refused("network-isolation")


def execute(args) -> int:
    if not pid_isolated(args.host_pid_inode):
        raise Refused("pid-namespace")
    # setpriv exec replaces all previous threads. Re-arm before native captures.
    watch_parent()
    if os.geteuid() != args.uid or args.uid == 0 or os.getegid() != args.gid:
        raise Refused("identity")
    status = dict(line.split(":", 1) for line in Path("/proc/self/status").read_text().splitlines() if ":" in line)
    if (any(int(status[key].strip(), 16) != 0 for key in ("CapEff", "CapPrm", "CapBnd", "CapInh", "CapAmb"))
            or status["NoNewPrivs"].strip() != "1"):
        raise Refused("capabilities")
    check_network(args.host_inode)
    if hashlib.sha256(args.binary.read_bytes()).hexdigest() != args.binary_sha256:
        raise Refused("binary-identity")
    raw, code, oversized = capture([str(args.binary), "--list", "--ignored"])
    if not inventory_valid(raw, code, oversized):
        raise Refused("inventory")
    results = []
    for name in CASES:
        raw, code, oversized = capture([str(args.binary), name, "--ignored", "--exact", "--show-output"])
        results.append(case_result(name, raw, code, oversized))
    check_network(args.host_inode)
    if hashlib.sha256(args.binary.read_bytes()).hexdigest() != args.binary_sha256:
        raise Refused("binary-identity")
    passed = all(item["state"] == "passed" for item in results)
    print("sf06_owned_network_summary " + json.dumps({
        "schema_version": 1, "status": "passed" if passed else "failed",
        "source_sha": args.source_sha, "binary_sha256": args.binary_sha256,
        "isolation": "namespace-no-egress-unprivileged", "cases": results,
    }, separators=(",", ":")))
    return 0 if passed else 1


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--inside", action="store_true")
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--host-inode", type=int)
    parser.add_argument("--host-pid-inode", type=int)
    parser.add_argument("--uid", type=int)
    parser.add_argument("--gid", type=int)
    parser.add_argument("--binary-sha256")
    args = parser.parse_args(argv)
    try:
        if sys.platform != "linux" or not re.fullmatch(r"[0-9a-f]{40}", args.source_sha):
            raise Refused("platform-or-source")
        for command in ("ip", "unshare", "setpriv"):
            if shutil.which(command) is None:
                raise Refused("prerequisite")
        args.binary = args.binary.resolve(strict=True)
        if not args.binary.is_file() or not os.access(args.binary, os.X_OK):
            raise Refused("binary")
        common = ["--binary", str(args.binary), "--source-sha", args.source_sha]
        script = str(Path(__file__).resolve())
        if args.execute:
            return execute(args)
        if args.inside:
            if (os.geteuid() != 0 or not args.uid or args.uid < 0 or args.gid is None or args.gid < 0
                    or not args.host_inode or not pid_isolated(args.host_pid_inode, check_host_init=True)
                    or not re.fullmatch(r"[0-9a-f]{64}", args.binary_sha256 or "")
                    or not network_isolated(args.host_inode)
                    or os.stat("/proc/self/ns/net").st_ino == os.stat("/proc/1/ns/net").st_ino):
                raise Refused("namespace-entry")
            watch_parent()
            if {x.get("ifname") for x in read_ip("link", "show")} != {"lo"}:
                raise Refused("initial-network")
            run_ip("link", "set", "lo", "up")
            run_ip("link", "add", "owned0", "type", "veth", "peer", "name", "owned1")
            for device, address in (("owned0", "11.0.0.1/30"), ("owned1", "11.0.0.2/30")):
                run_ip("link", "set", "dev", device, "addrgenmode", "none")
                run_ip("addr", "add", address, "dev", device)
                run_ip("link", "set", device, "up")
            check_network(args.host_inode)
            command = ["setpriv", "--reuid", str(args.uid), "--regid", str(args.gid), "--clear-groups",
                       "--bounding-set=-all", "--inh-caps=-all", "--ambient-caps=-all", "--no-new-privs",
                       sys.executable, "-I", "-B", script, "--execute", *common, "--uid", str(args.uid), "--gid", str(args.gid),
                       "--host-inode", str(args.host_inode), "--host-pid-inode", str(args.host_pid_inode),
                       "--binary-sha256", args.binary_sha256]
            os.execvp(command[0], command)
        if os.geteuid() == 0 or shutil.which("sudo") is None:
            raise Refused("unprivileged-launcher")
        expected = subprocess.check_output(["git", "rev-parse", "HEAD"], stdin=subprocess.DEVNULL,
                                           text=True, timeout=10).strip()
        if expected != args.source_sha:
            raise Refused("source-identity")
        digest = hashlib.sha256(args.binary.read_bytes()).hexdigest()
        command = ["sudo", "-n", "unshare", "--net", "--pid", "--fork", "--kill-child", "--", sys.executable, "-I", "-B", script, "--inside", *common,
                   "--uid", str(os.geteuid()), "--gid", str(os.getegid()),
                   "--host-inode", str(os.stat("/proc/self/ns/net").st_ino),
                   "--host-pid-inode", str(os.stat("/proc/self/ns/pid").st_ino), "--binary-sha256", digest]
        return supervised(command)
    except (Refused, OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError):
        print('sf06_owned_network_summary {"schema_version":1,"status":"unavailable"}')
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
