#!/usr/bin/env python3
"""Compare the default runtime and opt-in auto selection on real Nagi apps.

Uses the existing smart-build harness for measurement, receipts and owned-cache
cleanup. Fresh targets have warm registry/OS caches. No caller cache is deleted.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile

import benchmark_smart_build_matrix as shared


CASES = ("clean", "nochange", "bodyedit", "public_api_edit", "runtime_dependency_edit", "corrupt_cache")


@dataclass
class Fixture:
    name: str
    value: int = 41
    api_changed: bool = False

    def edit(self, case: str) -> None:
        if case == "bodyedit":
            self.value += 1
        elif case == "public_api_edit":
            self.api_changed = True

    def source(self) -> str:
        if self.name == "hello":
            kind = "f64" if self.api_changed else "i64"
            literal = f"{self.value}.0" if self.api_changed else str(self.value)
            one = "1.0" if self.api_changed else "1"
            return (f"def value() -> {kind}:\n    return {literal}\n"
                    'def main():\n    print("Hello, Nagi!")\n'
                    f"    print(value() + {one})\n")
        if self.name == "point":
            field = "    z: i64\n" if self.api_changed else ""
            argument = ", z=7" if self.api_changed else ""
            return (f"class Point:\n    x: f64\n    y: f64\n{field}"
                    f"def value() -> f64:\n    return {self.value - 40}.5\n"
                    f"def main():\n    p = Point(x=value(), y=2.5{argument})\n    print(p.x + p.y)\n")
        raise ValueError(f"unknown fixture: {self.name}")

    def expected(self) -> str:
        if self.name == "hello":
            return f"Hello, Nagi!\n{self.value + 1}\n"
        if self.name == "point":
            return f"{self.value - 37}\n"
        raise ValueError(f"unknown fixture: {self.name}")


def validate_sample(sample: dict, mode: str) -> None:
    stats = sample.get("stats") or {}
    expected = {"requested": mode, "selected": "core" if mode == "auto" else "full",
                "reason": "eligible_core" if mode == "auto" else "full_requested"}
    if (sample.get("exit_code") != 0 or stats.get("profile") != "dev"
            or stats.get("checks") != "full" or stats.get("cargo_always_invoked") is not True
            or stats.get("native_reuse") != "cargo_only"
            or stats.get("generated_body", {}).get("mode") != "monolithic"
            or any(type(stats.get(key)) is not int or stats[key] < 0 for key in
                   ("cargo_fresh_artifacts", "cargo_compiled_artifacts"))
            or stats.get("runtime") != expected):
        raise RuntimeError(f"failed build, fallback, or changed checking contract: {sample.get('log')}")


def summaries(configurations: list[dict]) -> dict:
    result = {}
    for fixture in ("hello", "point"):
        result[fixture] = {}
        for mode in ("full", "auto"):
            selected = [c for c in configurations if c["fixture"] == fixture and c["mode"] == mode]
            result[fixture][mode] = {}
            for case in CASES:
                samples = [c["samples"][case] for c in selected]
                result[fixture][mode][case] = (
                    {stage: shared.metrics([s[stage] for s in samples]) for stage in ("rejected", "recovery")}
                    if case == "corrupt_cache" else shared.metrics(samples))
    return result


def record_dependencies(report: dict, mode: str, packages: list) -> None:
    # Separate packages legitimately produce different lockfile closures. Every
    # retained registry package must still match the same seed's version/hash.
    current = {tuple(p) for p in packages}
    if not current.issubset({tuple(p) for p in report["seed_external_packages"]}):
        raise RuntimeError("dependency resolution escaped the reviewed seed lock")
    observed = report.setdefault("external_packages", {})
    if packages != observed.setdefault(mode, packages):
        raise RuntimeError("external package resolution changed within one runtime mode")
    if "full" in observed and "auto" in observed:
        if not {tuple(p) for p in observed["auto"]}.issubset({tuple(p) for p in observed["full"]}):
            raise RuntimeError("core dependencies are not a subset of the ordinary runtime")


def run_configuration(report: dict, folder: Path, project: Path, runtime: Path, library_base: bytes,
                      compiler: Path, mode: str, fixture_name: str, save, minimum: int) -> dict:
    folder.mkdir()
    out, target = folder / "out", folder / "target"
    out.mkdir()
    if target.exists():
        raise RuntimeError("clean target already exists")
    shutil.copyfile(runtime / "Cargo.lock", out / "Cargo.lock")
    library = runtime / "runtime/src/lib.rs"
    library.write_bytes(library_base + f"\npub const {shared.MARKER}: u64 = 0;\n".encode())
    fixture = Fixture(fixture_name)
    configuration = {"mode": mode, "fixture": fixture_name, "target": str(target),
                     "folder": str(folder), "target_initially_absent": True,
                     "samples": {}, "executables": []}
    report["configurations"].append(configuration)
    environment = dict(os.environ, NAGI_ROOT=str(runtime), NAGI_NATIVE_TARGET_DIR=str(target),
                       CARGO_NET_OFFLINE="true", CARGO_TERM_COLOR="never",
                       CARGO_BUILD_JOBS=str(report["jobs"]), CARGO_PROFILE_DEV_DEBUG=str(report["debug_info"]))
    command = [str(compiler), "build", "main.nagi", "--no-project", "--out", str(out),
               "--profile", "dev", "--build-stats"]
    if mode == "auto":
        command += ["--runtime-profile", "auto"]
    round_number = 0

    def preserve(label: str) -> None:
        nonlocal round_number
        round_number += 1
        for index, old in enumerate(configuration["executables"]):
            binary = Path(old["path"])
            if shared.digest(binary) != old["sha256"]:
                raise RuntimeError(f"old executable bytes changed: {binary}")
            shared.execute(binary, old["expected_stdout"], project, environment,
                           folder / f"{label}-{round_number}-old-{index}.log")

    def build(label: str, success: bool = True) -> dict:
        shared.capacity([folder, Path(tempfile.gettempdir())], minimum)
        if shared.digest(compiler) != report["compiler_sha256"]:
            raise RuntimeError("compiler bytes changed during comparison")
        print(f"{folder.name}: {label}", flush=True)
        sample = shared.measure(command, project, environment, folder / f"{label}.log")
        sample.update(source_sha256=shared.digest(project / "main.nagi"),
                      runtime_library_sha256=shared.digest(library), expected_stdout=fixture.expected())
        configuration["samples"][label] = sample
        save()
        if shared.digest(compiler) != report["compiler_sha256"]:
            raise RuntimeError("compiler bytes changed during measured build")
        if not success:
            return sample
        validate_sample(sample, mode)
        binary = shared.native_executable(out, target)
        if sample.get("native") is None or Path(sample["native"]).resolve() != binary:
            raise RuntimeError("reported native path disagrees with published latest")
        if any(binary == Path(old["path"]) for old in configuration["executables"]):
            raise RuntimeError("published executable path was reused")
        shared.verify_receipt(binary.parent / "build-receipt.json", compiler)
        sample["native_execution"] = shared.execute(binary, fixture.expected(), project, environment,
                                                    folder / f"{label}-native.log")
        preserve(label)
        configuration["executables"].append({"path": str(binary), "sha256": shared.digest(binary),
                                            "expected_stdout": fixture.expected()})
        sample["target_bytes"], sample["output_bytes"] = shared.size(target), shared.size(out)
        packages = shared.external_lock_packages(out / "Cargo.lock")
        record_dependencies(report, mode, packages)
        sample["external_package_count"] = len(packages)
        if label == "clean":
            log = Path(sample["log"]).read_text(encoding="utf-8", errors="replace")
            compiled = re.findall(r"(?m)^\s*Compiling ([A-Za-z0-9_-]+) v", log)
            sample["compiled_package_names"] = sorted(set(compiled))
            expensive = {"aws-lc-sys", "aws-lc-rs", "axum", "jsonwebtoken", "rustls", "hickory-resolver"}
            if mode == "auto" and expensive.intersection(compiled):
                raise RuntimeError("core cold build compiled full-only packages")
            if mode == "full" and "aws-lc-sys" not in compiled:
                raise RuntimeError("default full cold build did not observe the expected dependency compilation")
        if label == "runtime_dependency_edit" and sample["runtime_compilation_observed"] is not True:
            raise RuntimeError("runtime source edit did not rebuild the runtime")
        save()
        return sample

    for case in CASES[:-1]:
        fixture.edit(case)
        (project / "main.nagi").write_text(fixture.source(), encoding="utf-8")
        if case == "runtime_dependency_edit":
            library.write_bytes(library_base + f"\npub const {shared.MARKER}: u64 = 1;\n".encode())
        build(case)
    configuration["samples"]["corrupt_cache"] = shared.corrupt_lock_case(
        out, lambda: build("corrupt_cache_rejected", False), lambda: build("corrupt_cache_recovery"),
        lambda: preserve("corrupt-cache"))
    configuration["complete"] = True
    save()
    return configuration


def argument_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--runtime-root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--debug-info", type=int, choices=(0, 1, 2), default=0)
    parser.add_argument("--prune-owned-targets", action="store_true")
    parser.add_argument("--min-free-gib", type=float, default=3.0)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = argument_parser()
    args = parser.parse_args(argv)
    if not 3 <= args.repeat <= 30 or not 1 <= args.jobs <= 64 or not 1 <= args.min_free_gib <= 100:
        parser.error("repeat must be 3..30, jobs 1..64 and min-free-gib 1..100")
    try:
        shared.reject_overrides(dict(os.environ))
    except ValueError as error:
        parser.error(str(error))
    if args.output.exists() or args.output.is_symlink():
        parser.error("output must be a new report path")
    compiler, source = args.compiler.resolve(strict=True), args.runtime_root.resolve(strict=True)
    try:
        shared.validate_measurement_paths(source, args.work_dir, args.output)
    except ValueError as error:
        parser.error(str(error))
    args.work_dir.mkdir(parents=True, exist_ok=True)
    minimum = int(args.min_free_gib * 1024 ** 3)
    shared.capacity([args.work_dir, Path(tempfile.gettempdir())], minimum)
    root = Path(tempfile.mkdtemp(prefix="nagi-runtime-profiles-", dir=args.work_dir.resolve()))
    project, runtime = root / "project", root / "runtime-copy"
    project.mkdir()
    report = {"schema_version": 1, "complete": False, "compiler": str(compiler),
              "compiler_sha256": shared.digest(compiler), "work_directory": str(root),
              "platform": platform.platform(), "machine": platform.machine(), "cpu_count": os.cpu_count(),
              "repeat": args.repeat, "jobs": args.jobs, "debug_info": args.debug_info,
              "cases": list(CASES), "configurations": [],
              "conditions": "sequential, alternating full/auto order; same source/runtime paths and seed lock; "
              "fresh per-configuration Cargo targets, warm registry/OS caches; dev with explicit debug-info/jobs; "
              "full checks and Cargo every build; descendant CPU, largest-child RSS (not concurrent sum)",
              "native_execution_scope": "output/exit and process startup, not production throughput",
              "runtime_copy": shared.copy_runtime(source, runtime)}
    report["seed_external_packages"] = shared.external_lock_packages(runtime / "Cargo.lock")
    library_base = (runtime / "runtime/src/lib.rs").read_bytes()

    def save() -> None:
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x", encoding="utf-8") as initial:
        initial.write(json.dumps(report, indent=2) + "\n")
    try:
        report["rustc"] = subprocess.check_output(["rustc", "-Vv"], text=True).strip()
        report["cargo"] = subprocess.check_output(["cargo", "-V"], text=True).strip()
        for trial in range(args.repeat):
            for fixture in ("hello", "point"):
                modes = ("full", "auto") if trial % 2 == 0 else ("auto", "full")
                for mode in modes:
                    configuration = run_configuration(
                        report, root / f"trial-{trial + 1}-{fixture}-{mode}", project, runtime,
                        library_base, compiler, mode, fixture, save, minimum)
                    configuration["trial"] = trial + 1
                    if args.prune_owned_targets:
                        shared.prune_target(root, configuration)
                        for old in configuration["executables"]:
                            if shared.digest(Path(old["path"])) != old["sha256"]:
                                raise RuntimeError("owned-cache cleanup changed a published executable")
                    save()
        report["summaries"] = summaries(report["configurations"])
        report["free_space_after"] = shared.capacity([root, Path(tempfile.gettempdir())], 0)
        report["complete"] = True
        save()
    except BaseException as error:
        report["error"] = f"{type(error).__name__}: {error}"
        save()
        raise
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
