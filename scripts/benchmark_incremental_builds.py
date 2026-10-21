#!/usr/bin/env python3
"""Compare ordinary dev builds and opt-in generated-body reuse on a real project.

Fresh targets measure cold Cargo compilation with warm registry/OS caches.
Retains uniquely owned outputs/logs; never deletes a caller's cache.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import statistics
import subprocess
import tempfile
from pathlib import Path

from benchmark_project_builds import measure, size


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", required=True, type=Path)
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--runtime-root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeat", type=int, default=5)
    parser.add_argument("--functions", type=int, default=1000)
    args = parser.parse_args()
    if not 1 <= args.repeat <= 30 or not 1 <= args.functions <= 5000:
        parser.error("repeat must be 1..30 and functions 1..5000")
    args.work_dir.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="nagi-incremental-", dir=args.work_dir.resolve()))
    runtime = args.runtime_root.resolve()
    project = root / "project"
    project.mkdir()
    (project / "nagi.toml").write_text("entry='main.nagi'\n", encoding="utf-8")
    body = "".join(f"def value_{i}() -> i64:\n    return {i}\n" for i in range(args.functions))
    main_source = 'import "helper.nagi"\ndef main():\n' + "".join(f"    print(value_{i}())\n" for i in range(args.functions))
    report = {"schema_version": 1, "complete": False, "platform": platform.platform(),
              "machine": platform.machine(), "cpu_count": os.cpu_count(), "functions": args.functions,
              "work_directory": str(root), "runtime_root": str(runtime),
              "rustc": subprocess.check_output(["rustc", "-Vv"], text=True).strip(),
              "cargo": subprocess.check_output(["cargo", "-V"], text=True).strip(),
              "lock_sha256": hashlib.sha256((runtime / "Cargo.lock").read_bytes()).hexdigest(),
              "conditions": "sequential; same source paths; fresh target and seeded lock per configuration; warm registry/OS caches; unmodified Cargo dev profile; real runtime; RSS is largest child high-water mark, not concurrent sum",
              "configurations": {}}

    def save() -> None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    # Profile/rustflags overrides obscure the comparison. Do not silently
    # normalize an unknown environment and then claim identical conditions.
    overrides = [key for key in os.environ if key.startswith("CARGO_PROFILE_")
                 or key in {"CARGO_INCREMENTAL", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"}]
    if overrides:
        parser.error("unset Cargo profile/compiler overrides before comparison: " + ", ".join(sorted(overrides)))
    for label, compiler, extra in [("ordinary-dev", args.baseline.resolve(), []),
                                   ("incremental-dev", args.compiler.resolve(), ["--incremental"])]:
        folder = root / label
        folder.mkdir()
        out = folder / "out"
        out.mkdir()
        shutil.copyfile(runtime / "Cargo.lock", out / "Cargo.lock")
        env = dict(os.environ, NAGI_ROOT=str(runtime), NAGI_NATIVE_TARGET_DIR=str(folder / "target"), CARGO_NET_OFFLINE="true")
        (project / "main.nagi").write_text(main_source, encoding="utf-8")
        (project / "helper.nagi").write_text(body, encoding="utf-8")
        configuration = {"compiler_sha256": hashlib.sha256(compiler.read_bytes()).hexdigest(), "samples": {}}
        report["configurations"][label] = configuration

        def run(name: str, expected_first: int = 0, *, observations: bool = True) -> dict:
            print(f"{label}: {name}", flush=True)
            result = measure([str(compiler), "build", "--project", str(project), "--out", str(out), "--profile", "dev", *(["--build-stats"] if observations else []), *extra], project, env, folder / f"{name}.log")
            ran = subprocess.run([result["native"]], cwd=project, env=env, capture_output=True, text=True, timeout=30)
            expected = [str(expected_first)] + [str(i) for i in range(1, args.functions)]
            if ran.returncode != 0 or ran.stdout.splitlines() != expected:
                raise RuntimeError(f"native output mismatch: {folder / name}")
            configuration["samples"][name] = result
            save()
            return result

        run("clean")
        for index in range(args.repeat):
            run(f"warm-{index}")
        for index in range(args.repeat):
            run(f"warm-plain-{index}", observations=False)
        (project / "helper.nagi").write_text("# comment changes only source positions\n" + body, encoding="utf-8")
        run("comment")
        (project / "helper.nagi").write_text(body.replace("return 0\n", "return 42\n", 1), encoding="utf-8")
        run("function-body", 42)
        (project / "helper.nagi").write_text(body.replace("value_0() -> i64", "value_0() -> i32"), encoding="utf-8")
        run("public-api")
        configuration["warm_median_s"] = statistics.median(configuration["samples"][f"warm-{i}"]["elapsed_s"] for i in range(args.repeat))
        configuration["warm_plain_median_s"] = statistics.median(configuration["samples"][f"warm-plain-{i}"]["elapsed_s"] for i in range(args.repeat))
        configuration["target_bytes"] = size(folder / "target")
        configuration["output_bytes"] = size(out)
        save()
    report["complete"] = True
    save()
    print(args.output, flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
