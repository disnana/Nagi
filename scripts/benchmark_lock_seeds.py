#!/usr/bin/env python3
"""Compare ordinary and bundled-seed first builds, with warm registry caches.

Every configuration owns a fresh output and Cargo target. This is an offline
comparison, not an index/download benchmark or a native throughput benchmark.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

import benchmark_smart_build_matrix as shared
import verify_application_examples as applications


def tree_hashes(root: Path) -> dict[str, str]:
    paths = sorted(root.rglob("*"))
    if root.is_symlink() or any(p.is_symlink() for p in paths):
        raise ValueError("use a plain extracted distribution without symbolic links")
    return {str(p.relative_to(root)): shared.digest(p) for p in paths if p.is_file()}


def preserved(configuration: dict, compiler: Path) -> None:
    for item in configuration["executables"]:
        binary = Path(item["path"])
        if shared.digest(binary) != item["sha256"]:
            raise RuntimeError(f"published executable changed: {binary}")
        shared.verify_receipt(binary.parent / "build-receipt.json", compiler)


def run_configuration(report: dict, folder: Path, project: Path, runtime: Path,
                      compiler: Path, case: str, mode: str, save, minimum: int) -> dict:
    folder.mkdir()
    out, target = folder / "out", folder / "target"
    out.mkdir()
    if target.exists() or (out / "Cargo.lock").exists():
        raise RuntimeError("first build must have neither a target nor an output lock")
    runtime_profile = "auto" if case == "point-auto" else "full"
    selected = "core" if case == "point-auto" else "full"
    configuration = {"case": case, "mode": mode, "folder": str(folder),
                     "target": str(target), "target_initially_absent": True,
                     "initial_out_lock_absent": True, "samples": {}, "executables": []}
    report["configurations"].append(configuration)
    environment = dict(os.environ, NAGI_ROOT=str(runtime), NAGI_NATIVE_TARGET_DIR=str(target),
                       CARGO_NET_OFFLINE="true", CARGO_TERM_COLOR="never", CARGO_INCREMENTAL="0",
                       CARGO_BUILD_JOBS=str(report["jobs"]),
                       CARGO_PROFILE_DEV_DEBUG=str(report["debug_info"]))
    def build(label: str, generated: Path = out, source_project: Path = project) -> dict:
        shared.capacity([folder, Path(tempfile.gettempdir())], minimum)
        if shared.digest(compiler) != report["compiler_sha256"]:
            raise RuntimeError("compiler changed during comparison")
        command = [str(compiler), "build", str(source_project / "main.nagi"), "--no-project",
                   "--out", str(generated), "--profile", "dev", "--runtime-profile", runtime_profile,
                   "--build-stats"]
        if mode == "seed":
            command.append("--seed-lock")
        print(f"{folder.name}: {label}", flush=True)
        sample = shared.measure(command, source_project, environment, folder / f"{label}.log")
        configuration["samples"][label] = sample
        save()
        stats = sample.get("stats") or {}
        if (sample["exit_code"] != 0 or stats.get("checks") != "full"
                or stats.get("cargo_always_invoked") is not True
                or stats.get("native_reuse") != "cargo_only"
                or stats.get("runtime", {}).get("selected") != selected):
            raise RuntimeError(f"failed build or unexpected checking/runtime selection: {sample['log']}")
        # Require evidence that the optimization ran, rather than timing a fallback.
        lock_source = ("bundled" if mode == "seed" else "cargo") if label in ("clean", "newapp") else "output"
        if (stats.get("lockfile", {}).get("source") != lock_source
                or stats.get("lockfile", {}).get("requested") is not (mode == "seed")):
            raise RuntimeError(f"unexpected lock source: {sample['log']}")
        binary = Path(sample["native"])
        shared.verify_receipt(binary.parent / "build-receipt.json", compiler)
        if case == "stock-full":
            sample["smoke"] = applications.verifier(source_project)(binary, environment, folder / f"{label}-smoke")
        else:
            sample["native_execution"] = shared.execute(binary, "4\n", source_project, environment,
                                                        folder / f"{label}-native.log")
        packages = shared.external_lock_packages(generated / "Cargo.lock")
        if mode == "seed" and not {tuple(p) for p in packages}.issubset(
                {tuple(p) for p in report["seed_external_packages"]}):
            raise RuntimeError("seeded resolution escaped the bundled registry version/checksum inventory")
        sample["external_packages"] = packages
        sample["cargo_lock_sha256"] = shared.digest(generated / "Cargo.lock")
        sample["target_bytes"], sample["output_bytes"] = shared.size(target), shared.size(generated)
        configuration["executables"].append({"path": str(binary), "sha256": shared.digest(binary)})
        preserved(configuration, compiler)
        if tree_hashes(runtime) != report["distribution_sha256"]:
            raise RuntimeError("distribution inputs changed during comparison")
        save()
        return sample

    initial = build("clean")
    warm = build("nochange")
    if initial["external_packages"] != warm["external_packages"]:
        raise RuntimeError("unchanged build changed dependency resolution")
    second_project, second_out = folder / "another-project", folder / "another-out"
    shutil.copytree(project, second_project, ignore=shutil.ignore_patterns("__pycache__"))
    second_out.mkdir()
    reused = build("newapp", second_out, second_project)
    if initial["external_packages"] != reused["external_packages"]:
        raise RuntimeError("new application changed dependency resolution within the fixed offline registry")
    configuration["new_application_shared_target"] = True
    configuration["complete"] = True
    save()
    return configuration


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--runtime-root", type=Path, required=True,
                        help="plain extracted distribution with its validated bundled seed")
    parser.add_argument("--work-dir", type=Path, required=True, help="new owned directory")
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--debug-info", type=int, choices=(0, 1, 2), default=0)
    parser.add_argument("--minimum-free-gib", type=int, default=2)
    parser.add_argument("--prune-owned-targets", action="store_true")
    args = parser.parse_args()
    if args.repeat < 3 or args.jobs < 1 or args.minimum_free_gib < 1:
        parser.error("use at least 3 repetitions, 1 job and 1 GiB free-space reserve")
    shared.reject_overrides(dict(os.environ))
    compiler, runtime, root = args.compiler.resolve(strict=True), args.runtime_root.resolve(strict=True), args.work_dir.resolve()
    if (root.exists() or root.is_relative_to(runtime) or runtime.is_relative_to(root)
            or compiler.is_relative_to(root)):
        parser.error("work-dir must be new and separate from compiler/distribution inputs")
    distribution = tree_hashes(runtime)
    root.mkdir(parents=True)
    (root / "OWNED-BENCHMARK").write_text("Nagi lock-seed benchmark targets only\n", encoding="utf-8")
    point = root / "point"
    point.mkdir()
    (point / "main.nagi").write_text("class Point:\n    x: f64\n    y: f64\ndef main():\n    p = Point(x=1.5, y=2.5)\n    print(p.x + p.y)\n", encoding="utf-8")
    stock = root / "stock"
    shutil.copytree(applications.PROJECTS / "stock-report", stock)
    report = {"compiler": str(compiler), "compiler_sha256": shared.digest(compiler),
              "runtime_root": str(runtime), "distribution_sha256": distribution,
              "harness_sha256": shared.digest(Path(__file__)),
              "measurement_helper_sha256": shared.digest(Path(shared.__file__)),
              "application_helper_sha256": shared.digest(Path(applications.__file__)),
              "distribution_release": json.loads((runtime / "release.json").read_text(encoding="utf-8")),
              "seed_external_packages": shared.external_lock_packages(runtime / "runtime-seed/Cargo.lock"),
              "platform": platform.platform(), "machine": platform.machine(),
              "cpu_count": os.cpu_count(), "jobs": args.jobs, "debug_info": args.debug_info,
              "toolchain": {name: subprocess.check_output(command, text=True).strip() for name, command in
                            (("rustc", ["rustc", "-Vv"]), ("cargo", ["cargo", "--version"]))},
              "conditions": {"profile": "dev", "offline": True, "cargo_incremental": False,
                             "registry_and_os_caches_warm": True, "fresh_target_per_configuration": True,
                             "no_online_index_download_or_native_throughput_claim": True,
                             "rss_is_child_high_water_mark_not_process_tree_sum": True},
              "fixture_sha256": {"point": tree_hashes(point), "stock": tree_hashes(stock)},
              "configurations": [], "complete": False}

    def save() -> None:
        (root / "results.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    save()
    for trial in range(args.repeat):
        for index, case in enumerate(("point-full", "point-auto", "stock-full")):
            order = ("ordinary", "seed") if (trial + index) % 2 == 0 else ("seed", "ordinary")
            for mode in order:
                configuration = run_configuration(report, root / f"{trial}-{case}-{mode}",
                                                  stock if case == "stock-full" else point,
                                                  runtime, compiler, case, mode, save,
                                                  args.minimum_free_gib * 1024**3)
                if args.prune_owned_targets:
                    shared.prune_target(root, configuration)
                    preserved(configuration, compiler)
                    save()
    report["summary"] = {case: {mode: {label: shared.metrics([
        c["samples"][label] for c in report["configurations"] if c["case"] == case and c["mode"] == mode
    ]) for label in ("clean", "nochange", "newapp")} for mode in ("ordinary", "seed")}
        for case in ("point-full", "point-auto", "stock-full")}
    report["complete"] = True
    save()


if __name__ == "__main__":
    main()
