#!/usr/bin/env python3
"""Measure a disposable Nagi developer project against real runtime dependencies.

No existing project/cache is deleted. Requires a built compiler, Cargo, and the
repository runtime. Clean means a new Cargo target, not cold OS/registry caches.
"""
from __future__ import annotations

import argparse
import hashlib
import shutil
import json
import os
import platform
import statistics
import subprocess
import tempfile
import time
from pathlib import Path


def measure(command: list[str], cwd: Path, env: dict[str, str], log: Path) -> dict:
    start = time.perf_counter()
    with log.open("wb") as output:
        child = subprocess.Popen(command, cwd=cwd, env=env, stdout=output, stderr=subprocess.STDOUT)
        if hasattr(os, "wait4"):
            _, status, usage = os.wait4(child.pid, 0)
            child.returncode = os.waitstatus_to_exitcode(status)
            rss = usage.ru_maxrss if platform.system() != "Darwin" else usage.ru_maxrss / 1024
        else:
            child.wait()
            rss = None
    elapsed = time.perf_counter() - start
    text = log.read_text(encoding="utf-8", errors="replace")
    if child.returncode:
        raise RuntimeError(f"command failed ({child.returncode}); see {log}: {text[-2000:]}")
    result = {"elapsed_s": elapsed, "max_child_rss_kib": rss}
    stats = next((line.removeprefix("nagi-build-stats: ") for line in text.splitlines()
                  if line.startswith("nagi-build-stats: ")), None)
    if stats is not None:
        result["stats"] = json.loads(stats)
        # Exact OS units remain in the retained log; avoid repeating hundreds
        # of path bytes in each numeric measurement sample.
        result["stats"].pop("receipt_os", None)
    native = next((line.removeprefix("native: ") for line in text.splitlines() if line.startswith("native: ")), None)
    if native:
        result["native"] = native
    return result


def size(directory: Path) -> int:
    return sum(path.stat().st_size for path in directory.rglob("*") if path.is_file())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--runtime-root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeat", type=int, default=3)
    args = parser.parse_args()
    if not 1 <= args.repeat <= 30:
        parser.error("--repeat must be between 1 and 30")
    args.work_dir.mkdir(parents=True, exist_ok=True)
    # Retain this uniquely owned directory and logs for review, never remove a
    # caller's path or reuse another run's target directory as a clean sample.
    root = Path(tempfile.mkdtemp(prefix="nagi-build-bench-", dir=args.work_dir.resolve()))
    configurations = []
    if args.baseline:
        configurations.append(("baseline-release", args.baseline.resolve(), "release", False))
    configurations += [("current-release", args.compiler.resolve(), "release", True),
                       ("current-dev", args.compiler.resolve(), "dev", True)]
    report = {"schema_version": 1, "complete": False, "platform": platform.platform(), "machine": platform.machine(),
              "cpu_count": os.cpu_count(), "work_directory": str(root),
              "rustc": subprocess.check_output(["rustc", "-Vv"], text=True).strip(),
              "cargo": subprocess.check_output(["cargo", "-V"], text=True).strip(),
              "seed_lock_sha256": hashlib.sha256((args.runtime_root.resolve() / "Cargo.lock").read_bytes()).hexdigest(),
              "cargo_offline": os.environ.get("CARGO_NET_OFFLINE") == "true",
              "conditions": "sequential; fresh Cargo target per configuration; warm registry and OS caches; same source paths; normal project uses same seeded Cargo.lock; CPU probe resolves its own offline lockfile; real Nagi runtime; RSS is largest child-process high-water mark, not concurrent process sum",
              "ci_total_s": None, "app_tests_s": None, "configurations": {}}
    def save() -> None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    for label, compiler, profile, enhanced in configurations:
        project = root / "project"
        project.mkdir(exist_ok=True)
        configuration = root / label
        configuration.mkdir()
        output_directory = configuration / "build"
        output_directory.mkdir()
        # Seed every configuration with the same reviewed runtime dependency graph.
        shutil.copyfile(args.runtime_root.resolve() / "Cargo.lock", output_directory / "Cargo.lock")
        (project / "nagi.toml").write_text("entry='main.nagi'\n", encoding="utf-8")
        original = 'from "helper.nagi" import value\ndef main():\n    print(value() + 1)\n'
        helper = "def value() -> i64:\n    return 41\n"
        (project / "main.nagi").write_text(original)
        (project / "helper.nagi").write_text(helper)
        env = dict(os.environ, NAGI_ROOT=str(args.runtime_root.resolve()),
                   NAGI_NATIVE_TARGET_DIR=str(configuration / "target"))
        extra = ["--profile", profile, "--build-stats"] if enhanced else []
        samples = {}
        def run(name: str, verb: str = "build") -> dict:
            print(f"{label}: {name}", flush=True)
            value = measure([str(compiler), verb, "--out", str(output_directory), *extra] if verb == "build" else [str(compiler), verb, "--out", str(output_directory)],
                            project, env, configuration / f"{name}.log")
            samples[name] = value
            report["configurations"].setdefault(label, {})["samples"] = samples
            save()
            return value
        run("clean")
        for i in range(args.repeat):
            run(f"warm-{i}")
            run(f"check-{i}", "check")
        (project / "main.nagi").write_text(original + "    print(0)\n")
        run("one-file")
        (project / "helper.nagi").write_text(helper.replace("41", "42"))
        run("function-body")
        (project / "helper.nagi").write_text(helper.replace("41", "42").replace("i64", "i32"))
        final = run("public-api")
        binary = Path(final["native"])
        native_runs = [measure([str(binary)], project, env, configuration / f"native-{i}.log") for i in range(args.repeat)]
        (project / "cpu.nagi").write_text(
            '@rust("native::iterations")\nextern def iterations() -> i64\n'
            'def main():\n    n = iterations()\n    i = 0\n    checksum = 0\n'
            '    while i < n:\n        checksum = (checksum + i) % 1000000007\n'
            '        i = i + 1\n    print(checksum)\n', encoding="utf-8")
        (project / "cpu-native.rs").write_text(
            'pub fn iterations() -> i64 { std::hint::black_box(5_000_000) }\n', encoding="utf-8")
        cpu_build = measure([str(compiler), "build", "cpu.nagi", "--no-project", "--rust", "cpu-native.rs",
                             "--out", str(configuration / "cpu-build"), *extra], project, env, configuration / "cpu-build.log")
        cpu_runs = []
        for i in range(args.repeat):
            log = configuration / f"cpu-{i}.log"
            cpu_runs.append(measure([cpu_build["native"]], project, env, log))
            expected = str((5_000_000 * 4_999_999 // 2) % 1_000_000_007)
            if log.read_text().strip() != expected:
                raise RuntimeError(f"native checksum mismatch: {log}")
        unobserved = []
        for i in range(args.repeat):
            profile_args = ["--profile", profile] if enhanced else []
            unobserved.append(measure([str(compiler), "build", "--out", str(output_directory),
                                       *profile_args], project, env, configuration / f"without-stats-{i}.log"))
        report["configurations"][label] = {
            "samples": samples, "native_startup_samples": native_runs,
            "cpu_samples": cpu_runs, "cpu_median_s": statistics.median(v["elapsed_s"] for v in cpu_runs),
            "without_stats_samples": unobserved,
            "warm_median_s": statistics.median(samples[f"warm-{i}"]["elapsed_s"] for i in range(args.repeat)),
            "check_median_s": statistics.median(samples[f"check-{i}"]["elapsed_s"] for i in range(args.repeat)),
            "target_bytes": size(configuration / "target"), "output_bytes": size(output_directory),
            "compiler_sha256": hashlib.sha256(compiler.read_bytes()).hexdigest(),
            "generated_rust_sha256": hashlib.sha256((binary.parent / "src/main.rs").read_bytes()).hexdigest(),
            "native_bytes": binary.stat().st_size,
        }
        save()
    report["complete"] = True
    save()
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
