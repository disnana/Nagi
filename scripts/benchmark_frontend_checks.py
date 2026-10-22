#!/usr/bin/env python3
"""Compare complete Nagi checks, without Rust compilation or reusable check results.

Retains inputs, logs and raw samples in a newly owned directory. Compiler order
alternates per sample; both compilers see identical physical source/output paths.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import statistics
import subprocess
import tempfile
import time
from pathlib import Path


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def measure(command: list[str], cwd: Path, log: Path) -> dict:
    start = time.perf_counter()
    with log.open("wb") as output:
        child = subprocess.Popen(command, cwd=cwd, stdout=output, stderr=subprocess.STDOUT)
        if hasattr(os, "wait4"):
            _, status, usage = os.wait4(child.pid, 0)
            child.returncode = os.waitstatus_to_exitcode(status)
            cpu = usage.ru_utime + usage.ru_stime
            rss = usage.ru_maxrss / (1024 if platform.system() == "Darwin" else 1)
        else:
            child.wait()
            cpu = rss = None
    return {"wall_s": time.perf_counter() - start, "cpu_s": cpu,
            "peak_child_rss_kib": rss, "exit_code": child.returncode,
            "log_sha256": digest(log)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeat", type=int, default=7)
    args = parser.parse_args()
    if not 3 <= args.repeat <= 30:
        parser.error("repeat must be 3..30")
    compilers = {"baseline": args.baseline.resolve(), "current": args.compiler.resolve()}
    args.work_dir.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="nagi-frontend-bench-", dir=args.work_dir.resolve()))
    report = {"schema_version": 1, "complete": False, "platform": platform.platform(),
              "compiler_sha256": {name: digest(path) for name, path in compilers.items()},
              "conditions": "sequential alternating order; warm OS caches; complete check each time; identical source/output paths; no Rust compilation; RSS is child high-water mark",
              "work_directory": str(root), "cases": {}}

    def save() -> None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    for case, modules, functions, chain in [("small", 1, 20, False),
                                             ("modules", 16, 100, False),
                                             ("deep-imports", 32, 50, True)]:
        folder = root / case
        folder.mkdir()
        for index in range(modules):
            imports = f'import "part{index + 1}.nagi" as next_part\n' if chain and index + 1 < modules else ""
            source = imports + "".join(
                f"def value_{number}(n: i64) -> i64:\n    return n + {number}\n"
                for number in range(functions))
            (folder / f"part{index}.nagi").write_text(source, encoding="utf-8")
        imports = "".join(f'import "part{index}.nagi" as part{index}\n'
                          for index in range(1 if chain else modules))
        (folder / "main.nagi").write_text(imports + "def main():\n    print(part0.value_0(7))\n", encoding="utf-8")
        data = {"source_bytes": sum(p.stat().st_size for p in folder.glob("*.nagi")),
                "modules": modules, "functions_per_module": functions}
        report["cases"][case] = data
        for source in ["main.nagi", "saved.low"]:
            if source == "saved.low":
                # This is a physical saved-Low input, independently checked by
                # each compiler, not a reuse of the previous High check.
                (folder / source).write_bytes((folder / "out/generated.low").read_bytes())
            samples = {name: [] for name in compilers}
            data[source] = {"samples": samples}
            for sample in range(args.repeat):
                order = list(compilers) if sample % 2 == 0 else list(reversed(compilers))
                for name in order:
                    result = measure([str(compilers[name]), "check", source, "--no-project", "--out", "out"],
                                     folder, folder / f"{source}-{name}-{sample}.log")
                    samples[name].append(result)
                    save()
                    if result["exit_code"]:
                        raise RuntimeError(f"check failed: {folder}, {name}, {source}")
                before, after = (samples[name][-1] for name in compilers)
                if before["log_sha256"] != after["log_sha256"]:
                    raise RuntimeError(f"diagnostics differ: {folder}, {source}")
            # Compare generated Low with separate lower invocations, outside
            # the timed interval.
            emitted = {}
            for name, compiler in compilers.items():
                subprocess.run([str(compiler), "lower", source, "--no-project", "--out", "out"],
                               cwd=folder, check=True, capture_output=True)
                emitted[name] = digest(folder / "out/generated.low")
            if len(set(emitted.values())) != 1:
                raise RuntimeError(f"generated Low differs: {folder}, {source}")
            data[source] = {"samples": samples, "generated_low_sha256": emitted,
                            "summary": {name: {"median_s": statistics.median(s["wall_s"] for s in values),
                                               "min_s": min(s["wall_s"] for s in values),
                                               "max_s": max(s["wall_s"] for s in values)}
                                        for name, values in samples.items()}}
            save()
    report["complete"] = True
    save()
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
