#!/usr/bin/env python3
"""Compare ordinary dev and opt-in units with eight real-Cargo build cases.

Requires Python 3.11+, a built compiler, and cached real runtime dependencies.
Clean means a new target with warm registry/OS caches. No existing cache is
removed; --prune-owned-targets removes only completed targets created here.
"""
from __future__ import annotations

import argparse
import copy
from dataclasses import dataclass, field
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import statistics
import subprocess
import tempfile
import time
import tomllib

from benchmark_project_builds import size
from native_artifacts import native_executable
from verify_build_receipt import verify as verify_receipt


CASES = ("clean", "nochange", "bodyedit", "public_api_edit", "multimodule_edit",
         "async_edit", "runtime_dependency_edit", "corrupt_cache")
MARKER = "NAGI_SMART_BUILD_BENCH_MARKER"
BROKEN_LOCK = b"version = [not valid TOML\n"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def reject_overrides(environment: dict[str, str]) -> None:
    exact = {"CARGO_INCREMENTAL", "CARGO_ENCODED_RUSTFLAGS", "RUSTFLAGS", "RUSTC",
             "RUSTDOC", "RUSTDOCFLAGS"}
    prefixes = ("CARGO_PROFILE_", "CARGO_TARGET_", "CARGO_BUILD_", "RUSTC_", "NAGI_")
    rejected = sorted(key for key in environment
                      if key in exact or key.startswith(prefixes))
    if rejected:
        raise ValueError("unset build/runtime overrides: " + ", ".join(rejected))


def runtime_workspace_manifest(text: str, members: tuple[str, ...] = ("runtime",)) -> str:
    """Change only workspace.members; reject any unreviewed TOML rewrite."""
    original = tomllib.loads(text)
    if members not in (("runtime",), ("runtime", "runtime-core")) or any(
            member not in original.get("workspace", {}).get("members", []) for member in members):
        raise ValueError("runtime root must explicitly contain the selected runtime workspace members")
    if "default-members" in original["workspace"]:
        raise ValueError("runtime-copy default-members requires an explicit design")
    section = re.search(r"(?ms)^\[workspace\][ \t]*\n(.*?)(?=^\[|\Z)", text)
    if section is None:
        raise ValueError("cannot locate the runtime root workspace table")
    body, count = re.subn(r"(?ms)^members[ \t]*=[ \t]*\[.*?\]", "members = " + json.dumps(list(members)),
                          section[1], count=1)
    if count != 1:
        raise ValueError("cannot rewrite runtime workspace members")
    result = text[:section.start(1)] + body + text[section.end(1):]
    expected = copy.deepcopy(original)
    expected["workspace"]["members"] = list(members)
    if tomllib.loads(result) != expected:
        raise ValueError("runtime-copy rewrite changed more than workspace.members")
    return result


def validate_measurement_paths(source: Path, *paths: Path) -> None:
    for path in paths:
        if any(path.resolve().is_relative_to((source / member).resolve())
               for member in ("runtime", "runtime-core")):
            raise ValueError("measurement paths must stay outside the original runtime source trees")


def copy_runtime(source: Path, destination: Path) -> dict:
    validate_measurement_paths(source, destination)
    core = source / "runtime-core"
    members = ("runtime", "runtime-core") if core.exists() or core.is_symlink() else ("runtime",)
    # A source copy must not retain aliases to the developer's runtime.
    for member in members:
        tree = source / member
        if (not tree.is_dir() or tree.is_symlink()
                or any(path.is_symlink() for path in tree.rglob("*"))):
            raise ValueError("runtime copy requires plain source trees without symlinks")
    destination.mkdir()
    manifest = (source / "Cargo.toml").read_text(encoding="utf-8")
    rewritten = runtime_workspace_manifest(manifest, members)
    (destination / "Cargo.toml").write_text(rewritten, encoding="utf-8")
    shutil.copyfile(source / "Cargo.lock", destination / "Cargo.lock")
    for member in members:
        shutil.copytree(source / member, destination / member,
                        ignore=shutil.ignore_patterns("target", "native-target", "__pycache__", ".git"))
    library = destination / "runtime/src/lib.rs"
    if MARKER in library.read_text(encoding="utf-8"):
        raise ValueError("runtime source already contains the benchmark marker")
    return {"source": str(source), "copy": str(destination),
            "original_workspace_manifest_sha256": digest(source / "Cargo.toml"),
            "copied_workspace_manifest_sha256": digest(destination / "Cargo.toml"),
            "original_members": tomllib.loads(manifest)["workspace"]["members"],
            "copied_members": list(members), "workspace_change": "workspace.members only",
            "seed_lock_sha256": digest(destination / "Cargo.lock"),
            "source_library_sha256": digest(library),
            "copied_runtime_bytes": sum(size(destination / member) for member in members)}


@dataclass
class Fixture:
    count: int
    values: list[int] = field(init=False)
    api_type: str = "i64"
    second: int = 5
    ready: int = 3

    def __post_init__(self) -> None:
        self.values = list(range(self.count))

    def edit(self, case: str) -> None:
        if case == "bodyedit":
            self.values[0] = 1001
        elif case == "public_api_edit":
            self.api_type = "i32"
        elif case == "multimodule_edit":
            self.values[1] = 1002
            self.second = 9
        elif case == "async_edit":
            self.ready = 11

    def sources(self) -> dict[str, str]:
        functions = "".join(f"def value_{i}() -> i64:\n    return {value}\n"
                            for i, value in enumerate(self.values))
        aggregate = ("def aggregate() -> i64:\n    total = 0\n"
                     + "".join(f"    total += value_{i}()\n" for i in range(self.count))
                     + "    return total\n")
        api = "helper.api_value()"
        if self.api_type == "i32":
            api = f"i64({api})"
        return {
            "helper.nagi": functions + aggregate + f"def api_value() -> {self.api_type}:\n    return 41\n",
            "helper2.nagi": f"def second() -> i64:\n    return {self.second}\n",
            "main.nagi": ('import "helper.nagi" as helper\nimport "helper2.nagi" as extra\n'
                          + f"async def ready() -> i64:\n    return {self.ready}\n"
                          + "async def main():\n"
                          + f"    print(helper.aggregate() + {api} + extra.second() + await ready())\n"),
        }

    def write(self, project: Path) -> None:
        for name, text in self.sources().items():
            (project / name).write_text(text, encoding="utf-8")

    def expected(self) -> str:
        return f"{sum(self.values) + 41 + self.second + self.ready}\n"


def measure(command: list[str], cwd: Path, environment: dict[str, str], log: Path) -> dict:
    """Includes descendant CPU; RSS is one child high-water mark, not a sum."""
    start = time.perf_counter()
    with log.open("wb") as output:
        child = subprocess.Popen(command, cwd=cwd, env=environment, stdout=output,
                                 stderr=subprocess.STDOUT, start_new_session=os.name == "posix")
        try:
            if hasattr(os, "wait4"):
                _, status, usage = os.wait4(child.pid, 0)
                child.returncode = os.waitstatus_to_exitcode(status)
                cpu = usage.ru_utime + usage.ru_stime
                rss = usage.ru_maxrss / (1024 if platform.system() == "Darwin" else 1)
            else:
                child.wait()
                cpu = rss = None
        except BaseException:
            if os.name == "posix":
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            else:
                child.kill()
            child.wait()
            raise
    elapsed = time.perf_counter() - start
    text = log.read_text(encoding="utf-8", errors="replace")
    statistics_lines = [line.removeprefix("nagi-build-stats: ") for line in text.splitlines()
                        if line.startswith("nagi-build-stats: ")]
    natives = [line.removeprefix("native: ") for line in text.splitlines()
               if line.startswith("native: ")]
    if len(statistics_lines) > 1 or len(natives) > 1:
        raise RuntimeError(f"ambiguous compiler output: {log}")
    return {"command": command, "wall_s": elapsed, "cpu_s": cpu,
            "peak_child_rss_kib": rss, "exit_code": child.returncode,
            "log": str(log), "log_sha256": digest(log),
            "stats": json.loads(statistics_lines[0]) if statistics_lines else None,
            "native": natives[0] if natives else None,
            "cargo_lock_parse_rejection_observed": "Cargo.lock" in text
            and bool(re.search(r"failed to parse lock file|TOML parse error", text, re.IGNORECASE)),
            "runtime_compilation_observed": bool(re.search(r"Compiling nagi-runtime(?:-core)? v", text))}


def execute(binary: Path, expected: str, project: Path, environment: dict[str, str], log: Path) -> dict:
    start = time.perf_counter()
    result = subprocess.run([str(binary)], cwd=project, env=environment,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
    log.write_bytes(result.stdout + b"\n--- stderr ---\n" + result.stderr)
    actual = result.stdout.decode("utf-8")
    if result.returncode or actual != expected or result.stderr:
        raise RuntimeError(f"native output/exit mismatch: {binary}; see {log}")
    return {"wall_s": time.perf_counter() - start, "exit_code": result.returncode,
            "stdout": actual, "log": str(log), "log_sha256": digest(log)}


def external_lock_packages(path: Path) -> list:
    packages = tomllib.loads(path.read_text(encoding="utf-8")).get("package", [])
    return sorted((p["name"], p["version"], p["source"], p.get("checksum"))
                  for p in packages if "source" in p)


def latest(out: Path) -> tuple[Path, bytes]:
    paths = list((out / ".nagi/apps").glob("*/latest.json"))
    if len(paths) != 1:
        raise RuntimeError(f"expected one published application: {out}")
    return paths[0], paths[0].read_bytes()


def reject_false_success(result: dict, latest_path: Path, original_latest: bytes) -> None:
    if result["exit_code"] == 0 or result.get("native") is not None:
        raise RuntimeError("corrupt Cargo.lock was accepted or published a native path")
    if result.get("cargo_lock_parse_rejection_observed") is not True:
        raise RuntimeError("corrupt-lock sample failed for an unconfirmed or infrastructure reason")
    if latest_path.read_bytes() != original_latest:
        raise RuntimeError("failed corrupt-cache build changed latest")


def corrupt_lock_case(out: Path, fail, recover, preserve) -> dict:
    """Always restore owned lock bytes, even if the expected rejection is broken."""
    lock = out / "Cargo.lock"
    original_lock = lock.read_bytes()
    latest_path, original_latest = latest(out)
    lock.write_bytes(BROKEN_LOCK)
    try:
        failure = fail()
        reject_false_success(failure, latest_path, original_latest)
        preserve()
    finally:
        lock.write_bytes(original_lock)
    recovery = recover()
    preserve()
    return {"corruption_scope": "owned out/Cargo.lock syntax; same mutation in both modes",
            "rejected": failure, "recovery": recovery,
            "prior_latest_preserved_on_failure": True, "lock_restored_before_recovery": True}


def capacity(paths: list[Path], minimum_bytes: int) -> dict:
    free = {str(path): shutil.disk_usage(path).free for path in paths}
    if any(value < minimum_bytes for value in free.values()):
        raise RuntimeError(f"insufficient free space; retaining evidence and owned target: {free}")
    return free


def distribution(values: list[float | int | None]) -> dict | None:
    if not values or any(value is None for value in values):
        return None
    return {"median": statistics.median(values), "min": min(values), "max": max(values)}


def metrics(samples: list[dict]) -> dict:
    result = {key: distribution([sample.get(key) for sample in samples])
              for key in ("wall_s", "cpu_s", "peak_child_rss_kib", "target_bytes", "output_bytes")}
    for key in ("cargo_fresh_artifacts", "cargo_compiled_artifacts"):
        result[key] = distribution([(sample.get("stats") or {}).get(key) for sample in samples])
    result["source_cache_hits"] = distribution([sample.get("source_cache_hits") for sample in samples])
    result["source_cache_misses"] = distribution([sample.get("source_cache_misses") for sample in samples])
    result["native_startup_wall_s"] = distribution([
        sample.get("native_execution", {}).get("wall_s") for sample in samples])
    result["count"] = len(samples)
    return result


def summaries(configurations: list[dict]) -> dict:
    result = {}
    for mode in ("ordinary", "units"):
        selected = [configuration for configuration in configurations if configuration["mode"] == mode]
        result[mode] = {}
        for case in CASES:
            samples = [configuration["samples"][case] for configuration in selected]
            if case == "corrupt_cache":
                result[mode][case] = {stage: metrics([sample[stage] for sample in samples])
                                      for stage in ("rejected", "recovery")}
            else:
                result[mode][case] = metrics(samples)
    return result


def prune_target(root: Path, configuration: dict) -> None:
    target = Path(configuration["target"])
    if (configuration.get("complete") is not True or target.is_symlink()
            or target.parent.is_symlink() or target.name != "target"
            or target.parent.parent != root or target.resolve().parent.parent != root.resolve()):
        raise RuntimeError("refusing to prune an unexpected or incomplete target")
    shutil.rmtree(target)
    configuration["owned_target_pruned"] = True


def run_configuration(report: dict, folder: Path, project: Path, runtime: Path, library_base: bytes,
                      compiler: Path, compiler_hash: str, mode: str, count: int, save,
                      minimum_free: int) -> dict:
    folder.mkdir()
    out, target = folder / "out", folder / "target"
    out.mkdir()
    if target.exists():
        raise RuntimeError("clean target already exists")
    shutil.copyfile(runtime / "Cargo.lock", out / "Cargo.lock")
    library = runtime / "runtime/src/lib.rs"
    library.write_bytes(library_base + f"\npub const {MARKER}: u64 = 0;\n".encode())
    fixture = Fixture(count)
    fixture.write(project)
    configuration = {"mode": mode, "folder": str(folder), "target": str(target),
                     "target_initially_absent": True, "samples": {}, "executables": [],
                     "initial_runtime_library_sha256": digest(library)}
    report["configurations"].append(configuration)
    environment = dict(os.environ, NAGI_ROOT=str(runtime), NAGI_NATIVE_TARGET_DIR=str(target),
                       CARGO_NET_OFFLINE="true", CARGO_TERM_COLOR="never")
    command = [str(compiler), "build", "main.nagi", "--no-project", "--out", str(out),
               "--profile", "dev", "--build-stats"]
    if mode == "units":
        command.append("--incremental=units")
    old = configuration["executables"]

    preservation_round = 0

    def preserve(label: str) -> None:
        nonlocal preservation_round
        preservation_round += 1
        for index, executable in enumerate(old):
            path = Path(executable["path"])
            if digest(path) != executable["sha256"]:
                raise RuntimeError(f"old executable bytes changed: {path}")
            execute(path, executable["expected_stdout"], project, environment,
                    folder / f"{label}-round-{preservation_round}-old-{index}.log")

    def build(label: str, success: bool = True) -> dict:
        capacity([folder, Path(tempfile.gettempdir())], minimum_free)
        if digest(compiler) != compiler_hash:
            raise RuntimeError("compiler bytes changed during the comparison")
        print(f"{folder.name}: {label}", flush=True)
        result = measure(command, project, environment, folder / f"{label}.log")
        result["expected_stdout"] = fixture.expected()
        result["source_sha256"] = {name: digest(project / name) for name in fixture.sources()}
        result["runtime_library_sha256"] = digest(library)
        configuration["samples"][label] = result
        save()
        if digest(compiler) != compiler_hash:
            raise RuntimeError("compiler bytes changed during the measured build")
        if not success:
            return result
        stats = result.get("stats")
        if (result["exit_code"] != 0 or not isinstance(stats, dict)
                or stats.get("profile") != "dev" or stats.get("checks") != "full"
                or stats.get("cargo_always_invoked") is not True
                or stats.get("native_reuse") != "cargo_only"):
            raise RuntimeError(f"build failed or weakened its check/Cargo contract: {result['log']}")
        body = stats.get("generated_body", {})
        if mode == "units" and body.get("mode") != "cargo_units":
            raise RuntimeError("primitive units fixture unexpectedly fell back")
        if mode == "ordinary" and body.get("mode") != "monolithic":
            raise RuntimeError("ordinary build unexpectedly enabled source reuse")
        source_results = ([body.get("source_cache")] +
                          [unit.get("source_cache") for unit in body.get("units", [])])
        if mode == "units" and any(value not in ("hit", "miss") for value in source_results):
            raise RuntimeError("missing or unknown source-cache observation")
        result["source_cache_hits"] = source_results.count("hit") if mode == "units" else None
        result["source_cache_misses"] = source_results.count("miss") if mode == "units" else None
        binary = native_executable(out, target)
        if result.get("native") is None or Path(result["native"]).resolve() != binary:
            raise RuntimeError("reported native path disagrees with published latest")
        if any(binary == Path(executable["path"]) for executable in old):
            raise RuntimeError("build reused a published executable path")
        verify_receipt(binary.parent / "build-receipt.json", compiler)
        result["native_execution"] = execute(binary, fixture.expected(), project, environment,
                                               folder / f"{label}-native.log")
        preserve(label)
        old.append({"path": str(binary), "sha256": digest(binary), "expected_stdout": fixture.expected()})
        result["old_executables_verified"] = len(old) - 1
        result["target_bytes"], result["output_bytes"] = size(target), size(out)
        result["external_packages"] = external_lock_packages(out / "Cargo.lock")
        if result["external_packages"] != report.setdefault("external_packages", result["external_packages"]):
            raise RuntimeError("external package resolution changed between modes/cases")
        if label == "runtime_dependency_edit":
            prior_sample = configuration["samples"]["async_edit"]
            if (result["runtime_library_sha256"] == prior_sample["runtime_library_sha256"]
                    or stats.get("cargo_compiled_artifacts", 0)
                    <= prior_sample["stats"].get("cargo_compiled_artifacts", 0)):
                raise RuntimeError("runtime source edit did not increase compiled Cargo artifacts")
            if mode == "units":
                prior = prior_sample["stats"]["generated_body"]
                unit_keys = lambda value: [(unit["id"], unit["sha256"]) for unit in value["units"]]
                if (body.get("sha256") != prior.get("sha256") or body.get("cargo_fresh") is not False
                        or unit_keys(body) != unit_keys(prior)
                        or result["source_cache_misses"] != 0):
                    raise RuntimeError("runtime edit broke source identity or reused stale native body")
        save()
        return result

    for case in CASES[:-1]:
        fixture.edit(case)
        fixture.write(project)
        if case == "runtime_dependency_edit":
            library.write_bytes(library_base + f"\npub const {MARKER}: u64 = 1;\n".encode())
        build(case)
    configuration["samples"]["corrupt_cache"] = corrupt_lock_case(
        out, lambda: build("corrupt_cache_rejected", False), lambda: build("corrupt_cache_recovery"),
        lambda: preserve("corrupt-cache-preservation"))
    configuration["final_target_bytes"], configuration["final_output_bytes"] = size(target), size(out)
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
    parser.add_argument("--functions", type=int, default=100)
    parser.add_argument("--prune-owned-targets", action="store_true")
    parser.add_argument("--min-free-gib", type=float, default=2.0)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = argument_parser()
    args = parser.parse_args(argv)
    if not 3 <= args.repeat <= 30 or not 8 <= args.functions <= 2000:
        parser.error("repeat must be 3..30 and functions 8..2000")
    if not 1 <= args.min_free_gib <= 100:
        parser.error("min-free-gib must be 1..100")
    try:
        reject_overrides(dict(os.environ))
    except ValueError as error:
        parser.error(str(error))
    if args.output.exists() or args.output.is_symlink():
        parser.error("output must be a new report path; existing files are never overwritten")
    compiler, runtime_source = args.compiler.resolve(strict=True), args.runtime_root.resolve(strict=True)
    try:
        validate_measurement_paths(runtime_source, args.work_dir, args.output)
    except ValueError as error:
        parser.error(str(error))
    compiler_hash = digest(compiler)
    args.work_dir.mkdir(parents=True, exist_ok=True)
    capacity([args.work_dir, Path(tempfile.gettempdir())], int(args.min_free_gib * 1024 ** 3))
    root = Path(tempfile.mkdtemp(prefix="nagi-smart-matrix-", dir=args.work_dir.resolve()))
    project, runtime = root / "project", root / "runtime-copy"
    project.mkdir()
    report = {"schema_version": 1, "complete": False, "compiler": str(compiler),
              "compiler_sha256": compiler_hash, "work_directory": str(root),
              "platform": platform.platform(), "machine": platform.machine(), "cpu_count": os.cpu_count(),
              "repeat": args.repeat, "functions": args.functions, "cases": list(CASES),
              "conditions": "sequential; alternating mode order by trial; same source/runtime paths; "
              "fresh per-configuration targets and identical seed lock; warm registry/OS caches; "
              "standard generated dev profile; full checks and Cargo on every build; "
              "CPU includes descendants; RSS is one child high-water mark, not a concurrent sum",
              "native_execution_scope": "output/exit and process startup wall time, not production throughput",
              "runtime_reset_policy": "same owned copy/path; original library bytes plus marker 0 "
              "before each configuration, marker 1 at runtime edit; other copied files retained",
              "prune_owned_targets": args.prune_owned_targets,
              "configurations": [], "runtime_copy": copy_runtime(runtime_source, runtime)}
    library_base = (runtime / "runtime/src/lib.rs").read_bytes()

    def save() -> None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x", encoding="utf-8") as initial_report:
        initial_report.write(json.dumps(report, indent=2) + "\n")
    try:
        report["rustc"] = subprocess.check_output(["rustc", "-Vv"], text=True).strip()
        report["cargo"] = subprocess.check_output(["cargo", "-V"], text=True).strip()
        for trial in range(args.repeat):
            modes = ("ordinary", "units") if trial % 2 == 0 else ("units", "ordinary")
            for mode in modes:
                configuration = run_configuration(
                    report, root / f"trial-{trial + 1}-{mode}", project, runtime, library_base,
                    compiler, compiler_hash, mode, args.functions, save,
                    int(args.min_free_gib * 1024 ** 3))
                configuration["trial"] = trial + 1
                if args.prune_owned_targets:
                    # Only the exact target of this successful, newly owned configuration.
                    prune_target(root, configuration)
                save()
        report["summaries"] = summaries(report["configurations"])
        report["free_space_after"] = capacity([root, Path(tempfile.gettempdir())], 0)
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
