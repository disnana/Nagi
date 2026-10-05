"""Measure generation publication overhead with real Cargo and a std-only app.

The empty local runtime deliberately excludes framework/dependency build time.
Run before/after in separate, initially empty directories; no cache is deleted.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time
import tomllib


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fixture(directory: Path) -> None:
    directory.mkdir(parents=True)
    runtime = directory / "runtime"
    (runtime / "src").mkdir(parents=True)
    (runtime / "Cargo.toml").write_text(
        "[package]\nname='nagi-runtime'\nversion='0.1.0'\nedition='2021'\n[workspace]\n",
        encoding="utf-8",
    )
    (runtime / "src/lib.rs").write_text("", encoding="utf-8")


def measure(compiler: Path, directory: Path, label: str,
            mode: str, index: int, value: int) -> dict:
    source, output, target = (directory / name for name in
                              ("main.nagi", "generated", "native-target"))
    env = os.environ.copy()
    env.update(NAGI_ROOT=str(directory), NAGI_NATIVE_TARGET_DIR=str(target),
               CARGO_NET_OFFLINE="true")
    source.write_text(f"def main():\n    print({value})\n", encoding="utf-8")
    command = [str(compiler), "build", str(source), "--no-project", "--out", str(output)]
    start = time.perf_counter_ns()
    result = subprocess.run(command, cwd=directory, env=env, text=True,
                            capture_output=True, timeout=120)
    elapsed_ms = (time.perf_counter_ns() - start) / 1_000_000
    diagnostic = directory / f"{mode}-{index}.stderr.txt"
    diagnostic.write_text(result.stderr, encoding="utf-8")
    if result.returncode:
        raise RuntimeError(f"Build failed: {diagnostic}\n{result.stdout}\n{result.stderr}")
    binaries = [line.removeprefix("native: ") for line in result.stderr.splitlines()
                if line.startswith("native: ")]
    if len(binaries) != 1:
        raise RuntimeError(f"Expected exactly one native path: {diagnostic}")
    executable = Path(binaries[0]).resolve(strict=True)
    run = subprocess.run([str(executable)], cwd=directory, text=True,
                         capture_output=True, timeout=10, check=True)
    if run.stdout.strip() != str(value):
        raise AssertionError((value, run.stdout, run.stderr))
    with (output / "Cargo.toml").open("rb") as file:
        package = tomllib.load(file)["package"]["name"]
    print(f"{label} {mode} {index}: {elapsed_ms:.3f} ms", flush=True)
    return dict(mode=mode, sample=index, value=value, elapsed_ms=elapsed_ms,
                app_id=package, executable=str(executable),
                binary_bytes=executable.stat().st_size,
                binary_sha256=sha256(executable), source_sha256=sha256(source),
                generated_rust_sha256=sha256(output / "src/main.rs"),
                diagnostic=str(diagnostic), stdout=run.stdout, command=command)


def save(compiler: Path, directory: Path, label: str, rows: list[dict]) -> None:
    data = dict(label=label, compiler=str(compiler), compiler_sha256=sha256(compiler),
                platform=platform.platform(), python=platform.python_version(),
                rustc=subprocess.check_output(["rustc", "--version", "--verbose"], text=True),
                cargo=subprocess.check_output(["cargo", "--version"], text=True),
                cpu_count=os.cpu_count(), fixture="std-only with empty local runtime",
                harness_sha256=sha256(Path(__file__)), rows=rows)
    (directory / "measurements.json").write_text(
        json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--label", required=True)
    parser.add_argument("--samples", type=int, default=7)
    parser.add_argument("--compare-compiler", type=Path)
    parser.add_argument("--compare-label", default="comparison")
    args = parser.parse_args()
    if args.samples < 1:
        raise SystemExit("--samples must be positive")
    compiler = args.compiler.resolve(strict=True)
    directory = args.directory.resolve()
    if directory.exists():
        raise SystemExit(f"Use a new benchmark directory: {directory}")
    sides = [(compiler, directory, args.label, [])]
    if args.compare_compiler:
        sides = [(compiler, directory / "primary", args.label, []),
                 (args.compare_compiler.resolve(strict=True), directory / "comparison",
                  args.compare_label, [])]
    for _, folder, _, _ in sides:
        fixture(folder)
    cases = [("cold_cache", 0, 42)]
    cases += [("warm_unchanged", i, 42) for i in range(args.samples)]
    cases += [("warm_changed", i, 43 + i) for i in range(args.samples)]
    order = []
    for case_index, case in enumerate(cases):
        # Alternate pair order to reduce time/order bias on a shared host.
        for executable, folder, label, rows in sides[::1 if case_index % 2 == 0 else -1]:
            rows.append(measure(executable, folder, label, *case))
            order.append(dict(label=label, mode=case[0], sample=case[1]))
    for executable, folder, label, rows in sides:
        save(executable, folder, label, rows)
    if args.compare_compiler:
        (directory / "measurement-order.json").write_text(
            json.dumps(order, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
