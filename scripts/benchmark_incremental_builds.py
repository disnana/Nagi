#!/usr/bin/env python3
"""Compare compiler build modes on print-each and aggregate Nagi fixtures.

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
    parser.add_argument("--baseline-mode", choices=("ordinary", "body", "units"), default="ordinary")
    parser.add_argument("--current-mode", choices=("body", "units"), default="body")
    parser.add_argument("--fixture", choices=("print-each", "aggregate"), default="print-each")
    parser.add_argument("--runtime-root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeat", type=int, default=5)
    parser.add_argument("--edit-repeat", type=int, default=1,
                        help="number of distinct body/API edit samples (positive; default: 1)")
    parser.add_argument("--functions", type=int, default=1000)
    args = parser.parse_args()
    if not 1 <= args.repeat <= 30 or not 1 <= args.functions <= 5000:
        parser.error("repeat must be 1..30 and functions 1..5000")
    if args.edit_repeat < 1:
        parser.error("edit-repeat must be positive")
    args.work_dir.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="nagi-incremental-", dir=args.work_dir.resolve()))
    runtime = args.runtime_root.resolve()
    project = root / "project"
    project.mkdir()
    (project / "nagi.toml").write_text("entry='main.nagi'\n", encoding="utf-8")
    value_functions = "".join(f"def value_{i}() -> i64:\n    return {i}\n" for i in range(args.functions))
    if args.fixture == "print-each":
        main_source = 'import "helper.nagi"\ndef main():\n' + "".join(f"    print(value_{i}())\n" for i in range(args.functions))
    else:
        main_source = 'import "helper.nagi"\ndef main():\n    print(aggregate())\n'

    def helper_source(functions: str, *, widen_indices: set[int] | None = None) -> str:
        if args.fixture == "print-each":
            return functions
        widened = widen_indices or set()
        additions = []
        for index in range(args.functions):
            call = f"value_{index}()"
            if index in widened:
                call = f"i64({call})"
            additions.append(f"    total += {call}\n")
        aggregate = "def aggregate() -> i64:\n    total = 0\n" + "".join(additions) + "    return total\n"
        return functions + aggregate

    report = {"schema_version": 1, "complete": False, "platform": platform.platform(),
              "machine": platform.machine(), "cpu_count": os.cpu_count(), "functions": args.functions,
              "fixture": args.fixture,
              "native_expected_output": _native_expected_output_description(args.fixture),
              "work_directory": str(root), "runtime_root": str(runtime),
              "rustc": subprocess.check_output(["rustc", "-Vv"], text=True).strip(),
              "cargo": subprocess.check_output(["cargo", "-V"], text=True).strip(),
              "lock_sha256": hashlib.sha256((runtime / "Cargo.lock").read_bytes()).hexdigest(),
              "conditions": "sequential; same source paths; fresh target and seeded lock per configuration; warm registry/OS caches; unmodified Cargo dev profile; real runtime; RSS is largest child high-water mark, not concurrent sum",
              "modes": {
                  "baseline": {"mode": args.baseline_mode, "reuse_scope": _reuse_scope(args.baseline_mode)},
                  "current": {"mode": args.current_mode, "reuse_scope": _reuse_scope(args.current_mode)},
              },
              "configurations": {}}
    if args.edit_repeat > 1:
        report["edit_repeat"] = args.edit_repeat

    def save() -> None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    # Profile/rustflags overrides obscure the comparison. Do not silently
    # normalize an unknown environment and then claim identical conditions.
    overrides = [key for key in os.environ if key.startswith("CARGO_PROFILE_")
                 or key in {"CARGO_INCREMENTAL", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"}]
    if overrides:
        parser.error("unset Cargo profile/compiler overrides before comparison: " + ", ".join(sorted(overrides)))
    modes = {
        "ordinary": [],
        "body": ["--incremental"],
        "units": ["--incremental=units"],
    }
    configurations = [
        ("baseline", _configuration_label("baseline", args.baseline_mode), args.baseline.resolve(), modes[args.baseline_mode]),
        ("current", _configuration_label("current", args.current_mode), args.compiler.resolve(), modes[args.current_mode]),
    ]
    for role, label, compiler, extra in configurations:
        folder = root / label
        folder.mkdir()
        out = folder / "out"
        out.mkdir()
        shutil.copyfile(runtime / "Cargo.lock", out / "Cargo.lock")
        env = dict(os.environ, NAGI_ROOT=str(runtime), NAGI_NATIVE_TARGET_DIR=str(folder / "target"), CARGO_NET_OFFLINE="true")
        (project / "main.nagi").write_text(main_source, encoding="utf-8")
        (project / "helper.nagi").write_text(helper_source(value_functions), encoding="utf-8")
        mode = args.baseline_mode if role == "baseline" else args.current_mode
        configuration = {
            "role": role,
            "mode": mode,
            "reuse_scope": _reuse_scope(mode),
            "compiler_sha256": hashlib.sha256(compiler.read_bytes()).hexdigest(),
            "samples": {},
        }
        report["configurations"][label] = configuration

        def run(name: str, expected_first: int = 0, *, expected_values: list[int] | None = None,
                observations: bool = True) -> dict:
            print(f"{label}: {name}", flush=True)
            result = measure([str(compiler), "build", "--project", str(project), "--out", str(out), "--profile", "dev", *(["--build-stats"] if observations else []), *extra], project, env, folder / f"{name}.log")
            ran = subprocess.run([result["native"]], cwd=project, env=env, capture_output=True, text=True, timeout=30)
            if expected_values is not None:
                expected = _expected_native_lines(args.fixture, expected_values)
            elif args.fixture == "print-each":
                expected = [str(expected_first)] + [str(i) for i in range(1, args.functions)]
            else:
                expected = [str(sum(range(args.functions)) + expected_first)]
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
        (project / "helper.nagi").write_text("# comment changes only source positions\n" + helper_source(value_functions), encoding="utf-8")
        run("comment")
        current_values = list(range(args.functions))
        body_source = value_functions
        body_sample_names = []
        body_edits = []
        for edit_index in range(args.edit_repeat):
            target = edit_index % args.functions
            old_value = current_values[target]
            new_value = 42 + edit_index
            old_definition = f"def value_{target}() -> i64:\n    return {old_value}\n"
            new_definition = f"def value_{target}() -> i64:\n    return {new_value}\n"
            if old_definition not in body_source:
                raise RuntimeError(f"body edit source mismatch for value_{target}")
            body_source = body_source.replace(old_definition, new_definition, 1)
            current_values[target] = new_value
            (project / "helper.nagi").write_text(helper_source(body_source), encoding="utf-8")
            sample_name = "function-body" if args.edit_repeat == 1 else f"function-body-{edit_index + 1}"
            if args.edit_repeat == 1:
                run(sample_name, 42)
            else:
                run(sample_name, expected_values=current_values)
            body_sample_names.append(sample_name)
            body_edits.append({"sample": sample_name, "function": f"value_{target}",
                               "from": old_value, "to": new_value})

        api_sample_names = []
        api_edits = []
        if args.edit_repeat == 1:
            # Preserve the original single API edit input and sample name.
            api_source = value_functions.replace("value_0() -> i64", "value_0() -> i32", 1)
            (project / "helper.nagi").write_text(helper_source(api_source, widen_indices={0}), encoding="utf-8")
            run("public-api")
            api_sample_names.append("public-api")
            api_edits.append({"sample": "public-api", "function": "value_0",
                              "from_return_type": "i64", "to_return_type": "i32"})
        else:
            api_source = body_source
            api_types = ["i64"] * args.functions
            for edit_index in range(args.edit_repeat):
                target = edit_index % args.functions
                old_type = api_types[target]
                new_type = "i32" if old_type == "i64" else "i64"
                old_definition = f"def value_{target}() -> {old_type}:\n    return {current_values[target]}\n"
                new_definition = f"def value_{target}() -> {new_type}:\n    return {current_values[target]}\n"
                if old_definition not in api_source:
                    raise RuntimeError(f"API edit source mismatch for value_{target}")
                api_source = api_source.replace(old_definition, new_definition, 1)
                api_types[target] = new_type
                widened_indices = {index for index, return_type in enumerate(api_types) if return_type == "i32"}
                (project / "helper.nagi").write_text(helper_source(api_source, widen_indices=widened_indices), encoding="utf-8")
                sample_name = f"public-api-{edit_index + 1}"
                run(sample_name, expected_values=current_values)
                api_sample_names.append(sample_name)
                api_edits.append({"sample": sample_name, "function": f"value_{target}",
                                  "from_return_type": old_type, "to_return_type": new_type})
        if args.edit_repeat > 1:
            configuration["edit_summaries"] = {
                "function_body": {**_edit_summary(configuration["samples"], body_sample_names),
                                  "edits": body_edits},
                "public_api": {**_edit_summary(configuration["samples"], api_sample_names),
                               "edits": api_edits},
            }
        configuration["warm_median_s"] = statistics.median(configuration["samples"][f"warm-{i}"]["elapsed_s"] for i in range(args.repeat))
        configuration["warm_plain_median_s"] = statistics.median(configuration["samples"][f"warm-plain-{i}"]["elapsed_s"] for i in range(args.repeat))
        configuration["target_bytes"] = size(folder / "target")
        configuration["output_bytes"] = size(out)
        save()
    report["complete"] = True
    save()
    print(args.output, flush=True)
    return 0


def _reuse_scope(mode: str) -> str:
    return {
        "ordinary": "none",
        "body": "generated function bodies",
        "units": "Nagi function units (per-sample fresh/source-hit stats)",
    }[mode]


def _native_expected_output_description(fixture: str) -> str:
    return {
        "print-each": "one line per function; each line equals the current value_i() result",
        "aggregate": "one line: sum of the current value_i() results",
    }[fixture]


def _expected_native_lines(fixture: str, values: list[int]) -> list[str]:
    if fixture == "print-each":
        return [str(value) for value in values]
    return [str(sum(values))]


def _edit_summary(samples: dict, sample_names: list[str]) -> dict:
    selected = [samples[name] for name in sample_names]
    rss_values = [sample["max_child_rss_kib"] for sample in selected if sample.get("max_child_rss_kib") is not None]
    return {
        "sample_names": sample_names,
        "count": len(selected),
        "elapsed_median_s": statistics.median(sample["elapsed_s"] for sample in selected),
        "max_child_rss_median_kib": statistics.median(rss_values) if rss_values else None,
    }


def _configuration_label(role: str, mode: str) -> str:
    # Preserve the established default JSON keys while keeping custom mode
    # combinations distinct (including comparisons that use the same mode).
    if role == "baseline" and mode == "ordinary":
        return "ordinary-dev"
    if role == "current" and mode == "body":
        return "incremental-dev"
    return f"{role}-{mode}-dev"


if __name__ == "__main__":
    raise SystemExit(main())
