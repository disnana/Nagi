#!/usr/bin/env python3
"""Run the public Nagi Actor workload in a hard Docker CPU/memory cgroup."""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import selectors
import secrets
import shutil
import subprocess
import sys
import time

import public_supervisor_probe as probe


DOCKER = ["docker", "--host=unix:///var/run/docker.sock"]
WALL_SECONDS = 30
MEMORY_BYTES = 1024**3
PIDS_LIMIT = 512


def docker_output(*args: str) -> str:
    return subprocess.check_output([*DOCKER, *args], text=True).strip()


def normalized_cgroup_directory(value: str, root: Path = Path("/sys/fs/cgroup")) -> Path:
    # A nested cgroup namespace may expose the outer Docker path as
    # /../docker/<id>.  Map only leading namespace-parent markers back onto
    # this host's cgroup mount; reject traversal anywhere else.
    parts = value.lstrip("/").split("/")
    while parts and parts[0] == "..":
        parts.pop(0)
    if not parts or any(part in ("", ".", "..") for part in parts):
        raise RuntimeError("unsafe container cgroup v2 path")
    return root.joinpath(*parts)


def cgroup_directory(pid: int) -> Path:
    lines = (Path("/proc") / str(pid) / "cgroup").read_text().splitlines()
    unified = [line.split("::", 1)[1] for line in lines if line.startswith("0::")]
    if len(unified) != 1:
        raise RuntimeError("container process has no unique cgroup v2 path")
    path = normalized_cgroup_directory(unified[0])
    if not path.is_dir():
        raise RuntimeError("container cgroup disappeared while the process was live")
    return path


def key_values(path: Path) -> dict[str, int]:
    values: dict[str, int] = {}
    for line in path.read_text().splitlines():
        key, value = line.split()
        values[key] = int(value)
    return values


def cgroup_snapshot(path: Path) -> dict[str, object]:
    cpu = key_values(path / "cpu.stat")
    memory_events = key_values(path / "memory.events")
    return {
        "memory_current_bytes": int((path / "memory.current").read_text()),
        "memory_peak_bytes": int((path / "memory.peak").read_text()),
        "pids_current": int((path / "pids.current").read_text()),
        "cpu_usage_usec": cpu["usage_usec"],
        "cpu_user_usec": cpu["user_usec"],
        "cpu_system_usec": cpu["system_usec"],
        "memory_events": memory_events,
        "memory_max": (path / "memory.max").read_text().strip(),
        "memory_swap_max": (path / "memory.swap.max").read_text().strip(),
        "cpu_max": (path / "cpu.max").read_text().strip(),
        "cpuset_cpus_effective": (path / "cpuset.cpus.effective").read_text().strip(),
        "pids_max": (path / "pids.max").read_text().strip(),
    }


def expand_cpu_set(value: str) -> list[int]:
    result: list[int] = []
    for part in value.split(","):
        if "-" in part:
            first, last = (int(item) for item in part.split("-", 1))
            if first > last:
                raise RuntimeError("reversed cgroup CPU range")
            result.extend(range(first, last + 1))
        else:
            result.append(int(part))
    if not result or len(set(result)) != len(result):
        raise RuntimeError("invalid cgroup CPU set")
    return result


def validate_cgroup_limits(snapshot: dict[str, object], cpus: int,
                           cpu_ids: list[int]) -> None:
    if snapshot["memory_max"] != str(MEMORY_BYTES):
        raise RuntimeError("kernel memory.max contract mismatch")
    if snapshot["memory_swap_max"] != "0":
        raise RuntimeError("kernel memory.swap.max contract mismatch")
    quota, period = (int(value) for value in str(snapshot["cpu_max"]).split())
    if quota != cpus * period or period <= 0:
        raise RuntimeError("kernel cpu.max contract mismatch")
    if expand_cpu_set(str(snapshot["cpuset_cpus_effective"])) != cpu_ids:
        raise RuntimeError("kernel cpuset contract mismatch")
    if snapshot["pids_max"] != str(PIDS_LIMIT):
        raise RuntimeError("kernel pids.max contract mismatch")


def inspect_container(container: str) -> dict[str, object]:
    values = json.loads(docker_output("inspect", container))
    if len(values) != 1:
        raise RuntimeError("docker inspect did not return exactly one container")
    return values[0]


def validate_host_config(config: dict[str, object], cpus: int, cpu_set: str) -> None:
    expected = {
        "Memory": MEMORY_BYTES,
        "MemorySwap": MEMORY_BYTES,
        "NanoCpus": cpus * 1_000_000_000,
        "CpusetCpus": cpu_set,
        "PidsLimit": PIDS_LIMIT,
        "ReadonlyRootfs": True,
        "NetworkMode": "none",
    }
    actual = {key: config.get(key) for key in expected}
    if actual != expected:
        raise RuntimeError(f"docker resource contract mismatch: {actual!r}")
    if config.get("CapDrop") != ["ALL"]:
        raise RuntimeError("docker capability drop contract mismatch")
    if config.get("SecurityOpt") != ["no-new-privileges"]:
        raise RuntimeError("docker security option contract mismatch")


def cleanup_container(result: dict[str, object], container: str) -> None:
    try:
        cleanup = subprocess.run([*DOCKER, "rm", "-f", container], capture_output=True,
                                 text=True, timeout=5, check=False)
        if cleanup.returncode:
            detail = (cleanup.stderr or cleanup.stdout).strip()[:4096]
            raise RuntimeError(f"docker rm failed ({cleanup.returncode}): {detail}")
        residual = subprocess.run(
            [*DOCKER, "ps", "-a", "--no-trunc", "--filter", f"id={container}",
             "--format", "{{.ID}}"], capture_output=True, text=True, timeout=5, check=False)
        if residual.returncode:
            detail = (residual.stderr or residual.stdout).strip()[:4096]
            raise RuntimeError(f"docker cleanup verification failed ({residual.returncode}): {detail}")
        residual_ids = residual.stdout.split()
        if residual_ids:
            raise RuntimeError(f"container remains after cleanup: {residual_ids!r}")
        result["cleanup"] = {"rm_returncode": 0, "residual_container_ids": []}
    except Exception as problem:
        message = f"{type(problem).__name__}: {problem}"
        if "error" in result:
            result["cleanup_error"] = message
        else:
            result["error"] = message
        result["status"] = "failed"


def wait_for_container(cidfile: Path, process: subprocess.Popen[bytes]) -> tuple[str, int, Path, dict[str, object]]:
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if cidfile.exists() and cidfile.read_text().strip():
            container = cidfile.read_text().strip()
            inspected = inspect_container(container)
            pid = int(inspected["State"]["Pid"])
            if pid > 0:
                return container, pid, cgroup_directory(pid), inspected
        if process.poll() is not None:
            raise RuntimeError(f"docker run exited before startup: {process.returncode}")
        time.sleep(.01)
    raise RuntimeError("docker container startup deadline")


def run_case(image: str, expected_image_id: str, output: Path, clients: int,
             iterations: int, repeat: int, cpu_ids: list[int]) -> dict[str, object]:
    probe.capacity(output)
    cpus = len(cpu_ids)
    label = f"cpus-{cpus}-actors-{clients + 1}-round-{repeat}"
    name = f"nagi-actor-cgroup-{os.getpid()}-{secrets.token_hex(5)}"
    cidfile = output / f"{label}.cid"
    stderr_path = output / f"{label}.stderr"
    raw_path = output / f"{label}.jsonl.gz"
    cpu_set = ",".join(str(value) for value in cpu_ids)
    result: dict[str, object] = {
        "active_clients": clients,
        "actors": clients + 1,
        "iterations_per_client": iterations,
        "round": repeat,
        "cpu_quota_count": cpus,
        "cpuset_cpus": cpu_ids,
        "status": "failed",
        "checkpoints": [],
        "phases": [],
    }
    command = [
        *DOCKER, "run", "-i", "--name", name, "--cidfile", str(cidfile),
        "--network=none", "--read-only", "--cap-drop=ALL",
        "--security-opt=no-new-privileges", f"--memory={MEMORY_BYTES}",
        f"--memory-swap={MEMORY_BYTES}", f"--cpus={cpus}",
        f"--cpuset-cpus={cpu_set}", f"--pids-limit={PIDS_LIMIT}",
        "--ulimit=core=0", "-e", f"NAGI_THREADS={cpus}",
        "-e", f"NAGI_PROBE_CLIENTS={clients}",
        "-e", f"NAGI_PROBE_ITERATIONS={iterations}", image,
    ]
    process: subprocess.Popen[bytes] | None = None
    container: str | None = None
    selector: selectors.BaseSelector | None = None
    started = time.monotonic()
    try:
        with stderr_path.open("wb") as stderr, gzip.open(raw_path, "wt", encoding="utf-8") as raw:
            process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                       stderr=stderr, start_new_session=True)
            assert process.stdout is not None and process.stdin is not None
            container, pid, cgroup, inspected = wait_for_container(cidfile, process)
            validate_host_config(inspected["HostConfig"], cpus, cpu_set)
            if inspected["Image"] != expected_image_id:
                raise RuntimeError("container image changed during measurement")
            result["container_id"] = container
            result["container_pid"] = pid
            result["docker_image_id"] = inspected["Image"]
            result["host_config"] = {
                key: inspected["HostConfig"][key]
                for key in ("Memory", "MemorySwap", "NanoCpus", "CpusetCpus", "PidsLimit",
                            "ReadonlyRootfs", "NetworkMode", "CapDrop", "SecurityOpt")
            }
            selector = selectors.DefaultSelector()
            selector.register(process.stdout, selectors.EVENT_READ)
            buffer = b""
            eof = False
            while not eof:
                if time.monotonic() - started > WALL_SECONDS:
                    raise RuntimeError("wall guard")
                probe.capacity(output)
                for key, _ in selector.select(.02):
                    chunk = os.read(key.fd, 65536)
                    if not chunk:
                        eof = True
                        break
                    buffer += chunk
                    if len(buffer) > probe.MAX_LINE:
                        raise RuntimeError("bounded JSON line exceeded")
                    while b"\n" in buffer:
                        line, buffer = buffer.split(b"\n", 1)
                        raw.write(line.decode("utf-8") + "\n")
                        value = json.loads(line)
                        if "checkpoint" in value:
                            expected = len(result["checkpoints"])
                            if value["checkpoint"] != expected or expected > 3:
                                raise ValueError("checkpoint order")
                            snapshot = cgroup_snapshot(cgroup)
                            validate_cgroup_limits(snapshot, cpus, cpu_ids)
                            snapshot.update(checkpoint=expected, alive_tasks=value["alive_tasks"])
                            result["checkpoints"].append(snapshot)
                            process.stdin.write(b"continue\n")
                            process.stdin.flush()
                        else:
                            if result["phases"] or value.get("phase") != 0:
                                raise ValueError("phase order")
                            phase = probe.summarize_phase(value, iterations, True, clients)
                            if value.get("recovery") != [1, 1, 2, 0]:
                                raise ValueError("missing public failure and recovery evidence")
                            phase.update(public_failure_count=1, public_restart_count=1,
                                         recovered_generation=2, recovered_state=0)
                            result["phases"].append(phase)
                            if phase["failures"]:
                                raise RuntimeError("benchmark call did not complete successfully")
            selector.close()
            selector = None
            code = process.wait(timeout=max(.1, WALL_SECONDS - (time.monotonic() - started)))
            if code or buffer or len(result["checkpoints"]) != 4 or len(result["phases"]) != 1:
                raise RuntimeError(f"incomplete container run, exit={code}")
            final = inspect_container(container)
            if final["State"]["ExitCode"] != 0 or final["State"]["OOMKilled"]:
                raise RuntimeError(f"container failed: {final['State']!r}")
            before, _, _, released = result["checkpoints"]
            result["released_task_delta"] = released["alive_tasks"] - before["alive_tasks"]
            result["final_memory_current_delta_bytes"] = (
                released["memory_current_bytes"] - before["memory_current_bytes"])
            if result["released_task_delta"]:
                raise RuntimeError("Tokio task observation did not return to baseline")
            if any(checkpoint["memory_events"].get("oom", 0) or
                   checkpoint["memory_events"].get("oom_kill", 0)
                   for checkpoint in result["checkpoints"]):
                raise RuntimeError("container observed an OOM event")
            result["status"] = "passed"
    except Exception as problem:
        result["error"] = f"{type(problem).__name__}: {problem}"
    finally:
        if selector is not None:
            selector.close()
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=1)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        if container is None and cidfile.exists():
            container = cidfile.read_text().strip() or None
        if container is not None:
            cleanup_container(result, container)
        cidfile.unlink(missing_ok=True)
    (output / f"{label}.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--clients", type=int, nargs="+", default=[4, 16, 64, 128])
    parser.add_argument("--cpus", type=int, nargs="+", default=[1, 2])
    parser.add_argument("--iterations", type=int, default=5000)
    parser.add_argument("--rounds", type=int, default=3)
    args = parser.parse_args()
    if platform.system() != "Linux" or platform.architecture()[0] != "64bit":
        parser.error("Linux64 only")
    if (not args.clients or len(args.clients) > 4 or len(set(args.clients)) != len(args.clients)
            or any(not 1 <= value <= 128 for value in args.clients)
            or not 1 <= args.iterations <= 5000 or not 1 <= args.rounds <= 3):
        parser.error("clients 1..128 (max4 stages), iterations1..5000, rounds1..3")
    available = sorted(os.sched_getaffinity(0))
    if (not args.cpus or len(set(args.cpus)) != len(args.cpus)
            or any(value not in (1, 2) or value > len(available) for value in args.cpus)):
        parser.error("available hard CPU counts1 and/or2")
    probe.capacity(args.output.parent)
    args.output.mkdir()
    image_id = docker_output("image", "inspect", args.image, "--format", "{{.Id}}")
    result: dict[str, object] = {
        "schema": 1,
        "status": "failed",
        "image": args.image,
        "image_id": image_id,
        "docker_version": docker_output("version", "--format", "{{.Server.Version}}"),
        "platform": platform.platform(),
        "host_cpu_affinity": available,
        "limits": {
            "wall_seconds": WALL_SECONDS,
            "memory_bytes": MEMORY_BYTES,
            "memory_swap_bytes": MEMORY_BYTES,
            "pids": PIDS_LIMIT,
            "minimum_free_bytes": probe.MIN_FREE,
            "hard_cgroup_enforced": True,
        },
        "cases": [],
        "disk_free_before": probe.capacity(args.output),
    }
    try:
        for cpus in args.cpus:
            for clients in args.clients:
                for repeat in range(1, args.rounds + 1):
                    case = run_case(args.image, image_id, args.output, clients, args.iterations,
                                    repeat, available[:cpus])
                    result["cases"].append(case)
                    print(f"cpus={cpus} clients={clients} round={repeat}: {case['status']}", flush=True)
                    if case["status"] != "passed":
                        raise RuntimeError(case["error"])
                    if case["phases"][0]["successes"] != clients * args.iterations:
                        raise RuntimeError("completed useful-work count mismatch")
        result["status"] = "passed"
    except Exception as problem:
        result["error"] = f"{type(problem).__name__}: {problem}"
    result["disk_free_after"] = shutil.disk_usage(args.output).free
    result["output_sha256"] = {
        path.name: hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(args.output.glob("*.jsonl.gz"))
    }
    (args.output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
