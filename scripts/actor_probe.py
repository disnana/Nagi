"""Generate, build and measure actual Nagi and equivalent Rust std.actor code.

Uses one release executable and the Nagi application's exact manifest/lock.
Timing and allocation runs are separate; no sockets or external load generator.
Example: python scripts/actor_probe.py --nagic target/release/nagic --cpu 2
Use --prepare-only to inspect the generated source/project before any build.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
NAGI_SOURCE = """import std.actor as actor
async def probe_factory(context: shared[unit]) -> Result[i64, Error]:
    return ok(0)
async def probe_scalar_handler(state: i64, message: i64) -> Result[actor.Turn[i64, i64, Error], Error]:
    return ok(actor.turn[i64, i64, Error](state + 1, ok(message)))
async def probe_string_handler(state: i64, message: str) -> Result[actor.Turn[i64, str, Error], Error]:
    return ok(actor.turn[i64, str, Error](state + 1, ok(message)))
async def probe_lifecycle_handler(state: i64, message: i64) -> Result[actor.Turn[i64, i64, Error], Error]:
    if message == 0:
        await sleep(100)
    if message < 0:
        return error("probe controlled crash")
    return ok(actor.turn[i64, i64, Error](state + 1, ok(state + 1)))
def probe_scalar_actor(group: view[actor.Supervisor[unit]]) -> Result[actor.Actor[i64, i64, Error], Error]:
    options = try actor.actor_options(64, 1048576, 1048576, 5000, actor.RestartPolicy.TEMPORARY)
    return actor.register[i64, i64, i64, Error](group, "nagi-scalar", probe_factory, probe_scalar_handler, options)
def probe_string_actor(group: view[actor.Supervisor[unit]]) -> Result[actor.Actor[str, str, Error], Error]:
    options = try actor.actor_options(64, 1048576, 1048576, 5000, actor.RestartPolicy.TEMPORARY)
    return actor.register[i64, str, str, Error](group, "nagi-string", probe_factory, probe_string_handler, options)
def probe_lifecycle_actor(group: view[actor.Supervisor[unit]]) -> Result[actor.Actor[i64, i64, Error], Error]:
    options = try actor.actor_options(1, 1048576, 1048576, 5000, actor.RestartPolicy.TRANSIENT)
    return actor.register[i64, i64, i64, Error](group, "nagi-lifecycle", probe_factory, probe_lifecycle_handler, options)
async def probe_scalar_call(worker: view[actor.Actor[i64, i64, Error]], message: i64, mailbox_ms: i64) -> Result[Result[i64, Error], actor.CallError]:
    return await actor.call(worker, message, mailbox_ms, 5000)
async def probe_string_call(worker: view[actor.Actor[str, str, Error]], message: str, mailbox_ms: i64) -> Result[Result[str, Error], actor.CallError]:
    return await actor.call(worker, message, mailbox_ms, 5000)
def main() -> unit:
    return assert_true(True)
"""


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def runtime_sha256():
    digest = hashlib.sha256()
    for path in [ROOT / "runtime/Cargo.toml", *sorted((ROOT / "runtime/src").rglob("*.rs"))]:
        digest.update(str(path.relative_to(ROOT)).encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def snapshot(pid):
    """Optional Linux process evidence; includes every thread in this process."""
    try:
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        return {"cpu_ticks": int(fields[11]) + int(fields[12]),
                "rss_bytes": int(fields[21]) * os.sysconf("SC_PAGE_SIZE"),
                "threads": int(fields[17])}
    except (OSError, ValueError, IndexError):
        return {}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--nagic", type=Path, default=Path(os.environ.get("NAGIC", ROOT / "target/release/nagic")))
    parser.add_argument("--phase", choices=["all", "timing", "allocation", "idle", "lifecycle"], default="all")
    parser.add_argument("--iterations", type=int, default=20_000)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--allocation-iterations", type=int, default=1000)
    parser.add_argument("--idle-ms", type=int, default=1000)
    parser.add_argument("--mailbox-ms", type=int, choices=[0, 5000], default=5000,
                        help="compare immediate ready admission with bounded admission waiting")
    parser.add_argument("--string-bytes", default="64,4096")
    parser.add_argument("--cpu", type=int, help="pin the current_thread probe to an allowed Linux CPU")
    parser.add_argument("--prepare-only", action="store_true", help="write source/project config without running nagic or Cargo")
    parser.add_argument("--skip-build", action="store_true", help="reuse the existing generated/probe binary")
    parser.add_argument("--output", type=Path, default=ROOT / "benchmarks/results/actor-probe.jsonl")
    args = parser.parse_args()
    try:
        sizes = [int(value) for value in args.string_bytes.split(",")]
    except ValueError:
        parser.error("string-bytes must be comma-separated integers")
    if min(args.iterations, args.rounds, args.allocation_iterations) < 1 or args.idle_ms < 0:
        parser.error("iterations/rounds must be positive; idle-ms must be nonnegative")
    if not sizes or any(size < 0 or size > 65_536 for size in sizes):
        parser.error("string sizes must be 0..65536")
    if args.cpu is not None and (not hasattr(os, "sched_getaffinity") or args.cpu not in os.sched_getaffinity(0)):
        parser.error("CPU affinity requires an allowed Linux CPU")
    source_dir = ROOT / "build/actor_probe_input"
    generated = ROOT / "build/actor_probe_generated"
    project = ROOT / "build/actor_probe"
    source_dir.mkdir(parents=True, exist_ok=True)
    (source_dir / "actor_generated.nagi").write_text(NAGI_SOURCE)
    # Tokio is already a runtime dependency; declaring it in this Nagi project
    # gives both sides identical direct dependencies for current_thread control.
    (source_dir / "nagi.toml").write_text('entry = "actor_generated.nagi"\n[rust.dependencies]\ntokio = { version = "1.48", features = ["rt", "sync", "time", "macros"] }\n')
    if args.prepare_only:
        print(f"prepared {source_dir}")
        return
    native = Path(os.environ.get("NAGI_NATIVE_TARGET_DIR", ROOT / "build/native-target")).resolve()
    environment = dict(os.environ, CARGO_TARGET_DIR=str(native), NAGI_NATIVE_TARGET_DIR=str(native), CARGO_NET_OFFLINE="true")
    original, replacement = 'name = "nagi-actor-generated"', 'name = "nagi-actor-probe"'
    inputs = {"runtime_source_sha256": runtime_sha256(),
              "nagi_input_sha256": sha256(source_dir / "actor_generated.nagi"),
              "probe_input_sha256": sha256(ROOT / "benchmarks/actor_probe.rs")}
    signature = project / "build-inputs.json"
    if args.skip_build and (not signature.is_file() or json.loads(signature.read_text()) != inputs):
        parser.error("runtime or probe inputs changed; rebuild without --skip-build")
    if not args.skip_build:
        subprocess.run([str(args.nagic.resolve()), "build", "--project", str(source_dir), "--out", str(generated)], cwd=ROOT, env=environment, check=True)
        (project / "src").mkdir(parents=True, exist_ok=True)
        for filename in ("Cargo.toml", "Cargo.lock"):
            text = (generated / filename).read_text()
            if text.count(original) != 1:
                parser.error(f"unexpected generated package in {filename}")
            (project / filename).write_text(text.replace(original, replacement, 1))
        source = (generated / "src/main.rs").read_text()
        entry = r"(?m)^fn main\(\)"
        if len(re.findall(entry, source)) != 1:
            parser.error("expected one generated executable entry")
        source = re.sub(entry, "fn generated_unused_main()", source, count=1)
        (project / "src/main.rs").write_text(source + "\n" + (ROOT / "benchmarks/actor_probe.rs").read_text())
        subprocess.run([os.environ.get("CARGO", "cargo"), "build", "--release", "--locked", "--offline", "--manifest-path", str(project / "Cargo.toml")], cwd=ROOT, env=environment, check=True)
        signature.write_text(json.dumps(inputs, indent=2) + "\n")
    for filename in ("Cargo.toml", "Cargo.lock"):
        text = (generated / filename).read_text()
        if text.count(original) != 1 or text.replace(original, replacement, 1) != (project / filename).read_text():
            parser.error(f"probe {filename} must match generated project except root package name")
    binary = native / "release" / ("nagi-actor-probe" + (".exe" if os.name == "nt" else ""))
    evidence = {"phase":"environment", **inputs, "platform":platform.platform(), "cpu_affinity":[args.cpu] if args.cpu is not None else sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
                "generated_manifest_sha256":sha256(generated / "Cargo.toml"), "generated_lock_sha256":sha256(generated / "Cargo.lock"),
                "probe_manifest_sha256":sha256(project / "Cargo.toml"), "probe_lock_sha256":sha256(project / "Cargo.lock"),
                "generated_source_sha256":sha256(generated / "src/main.rs"), "probe_source_sha256":sha256(project / "src/main.rs"), "binary_sha256":sha256(binary),
                "manifest_lock_difference":"root package renamed once; all dependencies and release profile copied"}
    options = {"preexec_fn":lambda: os.sched_setaffinity(0, {args.cpu})} if args.cpu is not None else {}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w") as output:
        output.write(json.dumps(evidence) + "\n")
        command = [str(binary), args.phase, str(args.iterations), str(args.rounds), str(args.allocation_iterations), str(args.idle_ms), args.string_bytes, str(args.mailbox_ms)]
        with subprocess.Popen(command, env=environment, stdout=subprocess.PIPE, text=True, **options) as process:
            before_idle = None
            for line in process.stdout:
                row = json.loads(line)
                if row["phase"] == "idle_started":
                    before_idle = (time.monotonic(), snapshot(process.pid))
                if row["phase"] == "idle_completed" and before_idle:
                    elapsed = time.monotonic() - before_idle[0]
                    after = snapshot(process.pid)
                    row.update(before=before_idle[1], after=after, observed_wall_seconds=elapsed)
                    if row.get("cpu_ticks_after") is not None and row.get("cpu_ticks_before") is not None:
                        row["process_cpu_seconds"] = (row["cpu_ticks_after"] - row["cpu_ticks_before"]) / os.sysconf("SC_CLK_TCK")
                        row["cpu_tick_resolution_seconds"] = 1 / os.sysconf("SC_CLK_TCK")
                    elif "cpu_ticks" in after and "cpu_ticks" in before_idle[1]:
                        row["process_cpu_seconds"] = (after["cpu_ticks"] - before_idle[1]["cpu_ticks"]) / os.sysconf("SC_CLK_TCK")
                output.write(json.dumps(row) + "\n")
                output.flush()
            status = process.wait()
            if status:
                raise subprocess.CalledProcessError(status, command)
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
