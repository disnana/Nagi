#!/usr/bin/env python3
"""Bounded CLI check/lower timing over unchanged Phase 3 characterization inputs.

No Cargo, native build, runtime throughput, memory, or Future measurement.
Use the SAME checkout/fixture path and --work-directory for before and after.
Each label gets immutable raw observations/snapshots; work paths are reusable.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import time

NAMES = ("http-inspection", "http-borrow-mappers", "actor-data", "data-derives")
EXPECTED_BEFORE = "465cb2187be6566d54bac450d7fbbf0ae5ea382a9edef939280a6d935bf17359"


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def observed(command: list[str], cwd: Path) -> str:
    result = subprocess.run(command, cwd=cwd, capture_output=True, timeout=10, check=True)
    return result.stdout.decode("utf-8", errors="replace").strip()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True, help="new immutable result directory")
    parser.add_argument("--work-directory", type=Path, required=True, help="reuse the exact same output paths after aggregation")
    parser.add_argument("--label", choices=("before", "after"), required=True)
    parser.add_argument("--samples", type=int, default=7)
    parser.add_argument("--warmups", type=int, default=1)
    args = parser.parse_args()
    if not (1 <= args.samples <= 15 and 0 <= args.warmups <= 3):
        parser.error("bounded counts: samples 1..15, warmups 0..3")
    compiler = args.compiler.resolve(strict=True)
    repository = args.repository.resolve(strict=True)
    directory = args.directory.resolve()
    work = args.work_directory.resolve()
    if directory.exists():
        parser.error(f"result directory must be new: {directory}")
    directory.mkdir(parents=True)
    work.mkdir(parents=True, exist_ok=True)
    compiler_hash = digest(compiler.read_bytes())
    if args.label == "before" and compiler_hash != EXPECTED_BEFORE:
        parser.error("saved baseline SHA256 differs; no measurement performed")
    env = os.environ.copy()
    env["NAGI_ROOT"] = str(repository)
    fixtures = repository / "compiler/tests/fixtures/resource-contract"
    inputs = []
    for name in NAMES:
        path = (fixtures / f"{name}.nagi").resolve(strict=True)
        data = path.read_bytes()
        inputs.append({"name": name, "path": str(path), "bytes": len(data), "sha256": digest(data)})
        saved = directory / "inputs" / path.name
        saved.parent.mkdir(exist_ok=True)
        saved.write_bytes(data)
    metadata = {
        "label": args.label, "compiler": str(compiler), "compiler_sha256": compiler_hash,
        "repository": str(repository), "head": observed(["git", "rev-parse", "HEAD"], repository),
        "tree": observed(["git", "rev-parse", "HEAD^{tree}"], repository),
        "git_status": observed(["git", "status", "--short"], repository),
        "platform": platform.platform(), "machine": platform.machine(), "python": platform.python_version(),
        "cpu_count": os.cpu_count(), "rustc": observed(["rustc", "--version", "--verbose"], repository),
        "compiler_version": observed([str(compiler), "version"], repository),
        "environment": {key: env.get(key) for key in ("NAGI_ROOT", "RUSTUP_HOME", "CARGO_HOME", "CARGO_TARGET_DIR", "CARGO_NET_OFFLINE", "CARGO_PROFILE_DEV_DEBUG", "CARGO_PROFILE_TEST_DEBUG", "CARGO_INCREMENTAL")},
        "warmups_per_case": args.warmups, "samples_per_case": args.samples,
        "work_directory": str(work), "inputs": inputs, "script_sha256": digest(Path(__file__).read_bytes()),
        "scope": "wall clock around subprocess.run: includes process startup, CLI/frontend and CLI filesystem writes; excludes Python validation/hash/log/snapshot writes",
        "limits": "shared host, warm repetitions; no native Cargo, runtime throughput, allocation/RSS/CPU, Future size, or cold-cache measurement; saved executable build flags not independently inferred",
        "deadline_seconds": 180, "per_process_timeout_seconds": 30,
    }
    write_json(directory / "environment.json", metadata)
    rows = []
    errors = []
    snapshots: dict[tuple[str, str], bytes] = {}
    cases = [(item, mode) for item in inputs for mode in ("check", "lower")]
    sequence = [(item, mode, "warmup", n) for n in range(args.warmups) for item, mode in cases]
    for n in range(args.samples):
        # Deterministic rotating order; no concurrent measurements.
        ordered = cases[n % len(cases):] + cases[:n % len(cases)]
        sequence.extend((item, mode, "sample", n) for item, mode in ordered)
    deadline = time.monotonic() + 180
    with (directory / "timings.jsonl").open("x", encoding="utf-8") as raw:
        for ordinal, (item, mode, kind, sample) in enumerate(sequence):
            output = work / item["name"] / mode
            command = [str(compiler), mode, item["path"], "--no-project", "--out", str(output)]
            row = {"ordinal": ordinal, "fixture": item["name"], "mode": mode, "kind": kind, "sample": sample, "command": command, "cwd": str(repository)}
            start = time.perf_counter_ns()
            try:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise TimeoutError("overall measurement deadline exceeded")
                result = subprocess.run(command, cwd=repository, env=env, capture_output=True, timeout=min(30, remaining))
                row.update(elapsed_ms=(time.perf_counter_ns() - start) / 1_000_000, returncode=result.returncode,
                           stdout=result.stdout.decode("utf-8", errors="replace"), stderr=result.stderr.decode("utf-8", errors="replace"))
                if result.returncode != 0:
                    raise RuntimeError(f"CLI returned {result.returncode}")
                path = output / "generated.low"
                generated = path.read_bytes()
                row.update(low_bytes=len(generated), low_sha256=digest(generated), generated_low=str(path))
                key = (item["name"], mode)
                if key in snapshots and snapshots[key] != generated:
                    raise RuntimeError("un-normalized Low changed between identical repeated invocations")
                if (directory / "inputs" / (item["name"] + ".nagi")).read_bytes() != Path(item["path"]).read_bytes():
                    raise RuntimeError("input changed during measurements")
                snapshots[key] = generated
                saved = directory / "outputs" / item["name"] / mode / "generated.low"
                saved.parent.mkdir(parents=True, exist_ok=True)
                if not saved.exists():
                    saved.write_bytes(generated)
            except Exception as error:
                row.setdefault("elapsed_ms", (time.perf_counter_ns() - start) / 1_000_000)
                row["failure"] = f"{type(error).__name__}: {error}"
                if isinstance(error, subprocess.TimeoutExpired):
                    row.update(stdout=(error.stdout or b"").decode("utf-8", errors="replace"), stderr=(error.stderr or b"").decode("utf-8", errors="replace"))
                errors.append(row["failure"])
            rows.append(row)
            raw.write(json.dumps(row, ensure_ascii=False) + "\n")
            raw.flush()
            if errors:
                break
    summaries = []
    for item, mode in cases:
        selected = [row["elapsed_ms"] for row in rows if row["fixture"] == item["name"] and row["mode"] == mode and row["kind"] == "sample" and "failure" not in row]
        if selected:
            summaries.append({"fixture": item["name"], "mode": mode, "samples": len(selected), "median_ms": statistics.median(selected), "min_ms": min(selected), "max_ms": max(selected), "mean_ms": statistics.mean(selected)})
    equality = []
    for item in inputs:
        check = snapshots.get((item["name"], "check"))
        lower = snapshots.get((item["name"], "lower"))
        equal = check is not None and check == lower
        equality.append({"fixture": item["name"], "check_lower_low_bytes_equal": equal, "sha256": None if check is None else digest(check)})
        if not equal:
            errors.append(f'{item["name"]}: check/lower un-normalized Low mismatch or missing output')
    result = {"label": args.label, "attempted_invocations": len(rows), "expected_invocations": len(sequence), "successful_invocations": sum("failure" not in row for row in rows), "errors": errors, "summaries": summaries, "check_lower_equality": equality}
    write_json(directory / "summary.json", result)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return int(bool(errors) or len(rows) != len(sequence))


if __name__ == "__main__":
    raise SystemExit(main())
