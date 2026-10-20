#!/usr/bin/env python3
"""Opt-in, exact-inventory comparison for Cargo libtest and cargo-nextest."""
from __future__ import annotations

import argparse
import json
import os
import platform
import re
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Iterable

PACKAGE = "nagic"
NEXTEST_VERSION = "0.9.148"
TARGETS = ("enum_parser", "builtin_type_contracts")
PAIR_COUNT = 4
SKIPPED_SUITE_STATUSES = {"skipped", "skipped-default-filter"}
LIBTEST_CASE = re.compile(r"^(?P<name>.+): (?P<kind>test|benchmark)$")
LIBTEST_SUMMARY = re.compile(r"^(?P<tests>\d+) tests?, (?P<benches>\d+) benchmarks?$")


class EvaluationError(ValueError):
    """The command output cannot prove a complete, equivalent test inventory."""


class CommandFailure(RuntimeError):
    def __init__(self, label: str, returncode: int | None, elapsed: float, output: str):
        super().__init__(f"{label} exited with {returncode}")
        self.label = label
        self.returncode = returncode
        self.elapsed = elapsed
        self.output = bounded_tail(output)


def bounded_tail(output: str, max_lines: int = 40, max_chars: int = 12_000) -> str:
    """Keep failure diagnostics small; successful test output is never stored."""
    tail = "\n".join(output.splitlines()[-max_lines:])
    if len(tail) > max_chars:
        tail = tail[-max_chars:]
    return tail


def parse_libtest_listing(output: str) -> set[str]:
    """Parse a complete or ignored-only `--list` output from one libtest binary."""
    cases: set[str] = set()
    summaries: list[tuple[int, int]] = []
    for raw in output.splitlines():
        line = raw.strip()
        if not line:
            continue
        summary = LIBTEST_SUMMARY.fullmatch(line)
        if summary:
            summaries.append((int(summary.group("tests")), int(summary.group("benches"))))
            continue
        case = LIBTEST_CASE.fullmatch(line)
        if case is None:
            raise EvaluationError(f"unrecognized libtest --list line: {line[:160]!r}")
        if case.group("kind") != "test":
            raise EvaluationError(f"unexpected benchmark in selected test target: {case.group('name')}")
        name = case.group("name")
        if name in cases:
            raise EvaluationError(f"duplicate libtest test name: {name}")
        cases.add(name)

    if len(summaries) != 1:
        raise EvaluationError(f"expected one libtest summary, found {len(summaries)}")
    listed_count, benchmark_count = summaries[0]
    if listed_count != len(cases):
        raise EvaluationError(f"libtest summary says {listed_count} tests but listed {len(cases)}")
    if benchmark_count != 0:
        raise EvaluationError(f"selected test target reported {benchmark_count} benchmarks")
    return cases


def combine_libtest_inventories(all_tests: set[str], ignored_subset: set[str]) -> dict[str, bool]:
    """Mark ignored cases from the ignored-only subset of libtest's complete list."""
    outside_full_list = ignored_subset - all_tests
    if outside_full_list:
        raise EvaluationError(
            f"libtest ignored list contains tests absent from the complete list: {sorted(outside_full_list)[:8]}"
        )
    result = {name: name in ignored_subset for name in all_tests}
    if not result:
        raise EvaluationError("selected libtest target has an empty inventory")
    if not any(not state for state in result.values()):
        raise EvaluationError("selected targets contain no runnable non-ignored tests")
    return result


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise EvaluationError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def parse_nextest_inventory(output: str, targets: Iterable[str] = TARGETS) -> dict[str, dict[str, bool]]:
    """Read nextest 0.9.148 JSON and return all cases, including ignored cases."""
    expected_targets = tuple(targets)
    try:
        payload = json.loads(output, object_pairs_hook=_reject_duplicate_json_keys)
    except (json.JSONDecodeError, EvaluationError) as error:
        raise EvaluationError(f"invalid nextest list JSON: {error}") from error
    if not isinstance(payload, dict):
        raise EvaluationError("nextest list root must be an object")
    suites = payload.get("rust-suites")
    if not isinstance(suites, dict):
        raise EvaluationError("nextest list has no rust-suites object")

    test_count = payload.get("test-count")
    if type(test_count) is not int or test_count < 0:
        raise EvaluationError("nextest list test-count must be a nonnegative integer")
    observed_case_count = 0
    selected: dict[str, dict[str, bool]] = {}

    for suite_id, suite in suites.items():
        if not isinstance(suite, dict):
            raise EvaluationError(f"nextest suite {suite_id!r} is not an object")
        status = suite.get("status")
        binary_name = suite.get("binary-name")
        package_name = suite.get("package-name")
        testcases = suite.get("testcases")
        if not isinstance(testcases, dict):
            raise EvaluationError(f"nextest suite {suite_id!r} has no testcases object")
        observed_case_count += len(testcases)

        if status in SKIPPED_SUITE_STATUSES:
            if testcases:
                raise EvaluationError(f"skipped suite {suite_id!r} unexpectedly has testcases")
            continue
        if status != "listed":
            raise EvaluationError(f"unknown nextest suite status for {suite_id!r}: {status!r}")
        if not isinstance(binary_name, str) or not isinstance(package_name, str):
            raise EvaluationError(f"listed suite {suite_id!r} lacks package/binary names")
        if binary_name not in expected_targets:
            raise EvaluationError(f"unexpected listed test binary: {package_name}::{binary_name}")
        if package_name != PACKAGE:
            raise EvaluationError(f"unexpected package for {binary_name}: {package_name}")
        if binary_name in selected:
            raise EvaluationError(f"duplicate listed test binary: {binary_name}")

        cases: dict[str, bool] = {}
        for name, case in testcases.items():
            if not isinstance(name, str) or not isinstance(case, dict):
                raise EvaluationError(f"invalid testcase entry in {binary_name}")
            ignored = case.get("ignored")
            if type(ignored) is not bool:
                raise EvaluationError(f"testcase {binary_name}::{name} has no boolean ignored state")
            filter_match = case.get("filter-match")
            if not isinstance(filter_match, dict):
                raise EvaluationError(f"testcase {binary_name}::{name} has no filter-match state")
            match_status = filter_match.get("status")
            if match_status == "matches":
                pass
            elif match_status == "mismatch" and ignored and filter_match.get("reason") == "ignored":
                pass
            else:
                raise EvaluationError(
                    f"unexpected nextest filter state for {binary_name}::{name}: {filter_match!r}"
                )
            cases[name] = ignored
        if not cases:
            raise EvaluationError(f"listed nextest target {binary_name} has no testcases")
        selected[binary_name] = cases

    if test_count != observed_case_count:
        raise EvaluationError(
            f"nextest test-count says {test_count}, but rust-suites contain {observed_case_count} cases"
        )
    if set(selected) != set(expected_targets):
        raise EvaluationError(
            f"nextest listed targets differ: expected {sorted(expected_targets)}, observed {sorted(selected)}"
        )
    if not any(not ignored for cases in selected.values() for ignored in cases.values()):
        raise EvaluationError("selected nextest targets contain no runnable non-ignored tests")
    return selected


def compare_inventories(
    libtest: dict[str, dict[str, bool]], nextest: dict[str, dict[str, bool]], targets: Iterable[str] = TARGETS
) -> None:
    expected = set(targets)
    if set(libtest) != expected or set(nextest) != expected:
        raise EvaluationError("one runner did not produce exactly the selected target set")
    differences = []
    for target in sorted(expected):
        baseline = libtest[target]
        candidate = nextest[target]
        if baseline == candidate:
            continue
        missing = sorted(set(baseline) - set(candidate))
        extra = sorted(set(candidate) - set(baseline))
        ignore_mismatch = sorted(
            name for name in set(baseline).intersection(candidate) if baseline[name] != candidate[name]
        )
        differences.append(
            f"{target}: missing={missing[:5]} extra={extra[:5]} ignored-state={ignore_mismatch[:5]}"
        )
    if differences:
        raise EvaluationError("libtest/nextest inventories differ: " + "; ".join(differences))


def target_selectors(targets: Iterable[str] = TARGETS) -> list[str]:
    args: list[str] = []
    for target in targets:
        args.extend(("--test", target))
    return args


def cargo_build_command() -> list[str]:
    return ["cargo", "test", "--locked", "-p", PACKAGE, *target_selectors(), "--no-run"]


def libtest_list_command(target: str, *, ignored: bool = False) -> list[str]:
    command = ["cargo", "test", "--locked", "-p", PACKAGE, "--test", target, "--", "--list"]
    if ignored:
        command.append("--ignored")
    return command


def nextest_list_command() -> list[str]:
    return [
        "cargo", "nextest", "list", "--locked", "--message-format=json", "-p", PACKAGE,
        *target_selectors(),
    ]


def cargo_test_run_command() -> list[str]:
    return ["cargo", "test", "--locked", "-p", PACKAGE, *target_selectors(), "--no-fail-fast"]


def nextest_run_command() -> list[str]:
    return [
        "cargo", "nextest", "run", "--locked", "-p", PACKAGE, *target_selectors(),
        "--no-fail-fast", "--retries", "0", "--no-tests=fail",
    ]


def warm_order_pairs(first: str, pairs: int = PAIR_COUNT) -> list[tuple[str, str]]:
    if first not in {"libtest", "nextest"}:
        raise EvaluationError("first runner must be libtest or nextest")
    if pairs < 2 or pairs % 2 != 0:
        raise EvaluationError("pair count must be an even number of at least two")
    other = "nextest" if first == "libtest" else "libtest"
    return [(first, other) if index % 2 == 0 else (other, first) for index in range(pairs)]


def ensure_no_repo_nextest_config(cwd: Path) -> None:
    candidates = (cwd / ".config" / "nextest.toml", cwd / "nextest.toml")
    found = [str(path.relative_to(cwd)) for path in candidates if path.exists()]
    if found:
        raise EvaluationError(
            "repository nextest config would alter this default-profile comparison: " + ", ".join(found)
        )


def require_nextest_version(output: str) -> str:
    version = output.strip()
    if not re.fullmatch(r"cargo-nextest " + re.escape(NEXTEST_VERSION) + r"(?: \([^\r\n]+\))?", version):
        raise EvaluationError("installed nextest does not match the reviewed version")
    return version


def _write_report(path: Path, report: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(path)


def _clean_subprocess_env() -> dict[str, str]:
    env = os.environ.copy()
    env["CARGO_TERM_COLOR"] = "never"
    for name in tuple(env):
        if name.startswith("NEXTEST_"):
            env.pop(name, None)
    return env


def _run_timed(command: list[str], label: str, report: dict[str, Any], output_path: Path, cwd: Path) -> str:
    record: dict[str, Any] = {"label": label, "command": command, "elapsed_seconds": None, "returncode": None}
    report["commands"].append(record)
    _write_report(output_path, report)
    start = time.monotonic()
    try:
        result = subprocess.run(
            command, cwd=cwd, env=_clean_subprocess_env(), text=True,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
        )
    except OSError as error:
        record["elapsed_seconds"] = round(time.monotonic() - start, 6)
        record["error"] = str(error)
        _write_report(output_path, report)
        raise CommandFailure(label, None, record["elapsed_seconds"], str(error)) from error
    elapsed = round(time.monotonic() - start, 6)
    record["elapsed_seconds"] = elapsed
    record["returncode"] = result.returncode
    _write_report(output_path, report)
    if result.returncode != 0:
        raise CommandFailure(label, result.returncode, elapsed, result.stdout + result.stderr)
    return result.stdout


def _serializable_inventory(inventory: dict[str, dict[str, bool]]) -> dict[str, list[dict[str, Any]]]:
    return {
        target: [{"name": name, "ignored": ignored} for name, ignored in sorted(cases.items())]
        for target, cases in sorted(inventory.items())
    }


def run_benchmark(output_path: Path, first: str, cwd: Path | None = None) -> int:
    root = cwd or Path.cwd()
    report: dict[str, Any] = {
        "schema_version": 1,
        "status": "running",
        "platform": {"system": platform.system(), "release": platform.release(), "machine": platform.machine()},
        "selected_package": PACKAGE,
        "selected_targets": list(TARGETS),
        "build_mode": {"locked": True, "default_features": True, "doctests": False},
        "setup": {
            "python": "3.12",
            "cargo_nextest": "0.9.148",
            "action_step_durations": "not included in this JSON; inspect individual step durations in the workflow UI",
        },
        "cache": {"cargo_cache_action": False, "note": "no workflow cache restore/save steps are configured"},
        "measurement": {
            "test_threads": "tool defaults; no global serial override",
            "nextest_fail_fast": False,
            "nextest_retries": 0,
            "nextest_no_tests": "fail",
            "warm_pairs": PAIR_COUNT,
            "first_runner": first,
            "target_build_seconds": None,
            "inventory_seconds": {},
            "inventory_equal": None,
            "libtest_inventory": None,
            "nextest_inventory": None,
            "test_runs": [],
        },
        "commands": [],
    }
    _write_report(output_path, report)
    try:
        ensure_no_repo_nextest_config(root)
        report["tool_versions"] = {
            "nextest": require_nextest_version(_run_timed(
                ["cargo", "nextest", "--version"], "nextest version", report, output_path, root)),
            "cargo": _run_timed(["cargo", "--version"], "Cargo version", report, output_path, root).strip(),
            "rustc": _run_timed(["rustc", "--version"], "rustc version", report, output_path, root).strip(),
        }
        _write_report(output_path, report)
        _run_timed(cargo_build_command(), "build selected test binaries (--no-run)", report, output_path, root)
        report["measurement"]["target_build_seconds"] = report["commands"][-1]["elapsed_seconds"]
        report["measurement"]["target_build_note"] = "includes any Cargo dependency resolution/download and selected-target compilation"
        _write_report(output_path, report)

        baseline: dict[str, dict[str, bool]] = {}
        for target in TARGETS:
            normal_output = _run_timed(
                libtest_list_command(target), f"libtest inventory {target} all tests", report, output_path, root
            )
            ignored_output = _run_timed(
                libtest_list_command(target, ignored=True), f"libtest inventory {target} ignored", report, output_path, root
            )
            all_tests = parse_libtest_listing(normal_output)
            ignored_subset = parse_libtest_listing(ignored_output)
            baseline[target] = combine_libtest_inventories(all_tests, ignored_subset)
            report["measurement"]["inventory_seconds"][f"libtest:{target}"] = round(
                report["commands"][-1]["elapsed_seconds"] + report["commands"][-2]["elapsed_seconds"], 6
            )
            _write_report(output_path, report)

        nextest_output = _run_timed(nextest_list_command(), "nextest JSON inventory", report, output_path, root)
        candidate = parse_nextest_inventory(nextest_output)
        report["measurement"]["inventory_seconds"]["nextest"] = report["commands"][-1]["elapsed_seconds"]
        report["measurement"]["libtest_inventory"] = _serializable_inventory(baseline)
        report["measurement"]["nextest_inventory"] = _serializable_inventory(candidate)
        try:
            compare_inventories(baseline, candidate)
        except EvaluationError:
            report["measurement"]["inventory_equal"] = False
            _write_report(output_path, report)
            raise
        report["measurement"]["inventory_equal"] = True
        _write_report(output_path, report)

        for pair_index, order in enumerate(warm_order_pairs(first), start=1):
            for runner in order:
                command = cargo_test_run_command() if runner == "libtest" else nextest_run_command()
                label = f"warm pair {pair_index} {runner}"
                _run_timed(command, label, report, output_path, root)
                report["measurement"]["test_runs"].append(
                    {"pair": pair_index, "runner": runner, "order": list(order),
                     "elapsed_seconds": report["commands"][-1]["elapsed_seconds"], "returncode": 0}
                )
                _write_report(output_path, report)
        report["status"] = "complete"
        _write_report(output_path, report)
        return 0
    except CommandFailure as error:
        report["status"] = "failed"
        report["failure"] = {"step": error.label, "returncode": error.returncode, "elapsed_seconds": error.elapsed}
        _write_report(output_path, report)
        if error.output:
            print(error.output, file=sys.stderr)
        print(str(error), file=sys.stderr)
        return 1
    except Exception as error:
        report["status"] = "failed"
        report["failure"] = {"reason": str(error)}
        _write_report(output_path, report)
        print(f"nextest evaluation stopped: {error}", file=sys.stderr)
        return 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    benchmark = subparsers.add_parser("benchmark", help="compare inventory and measure balanced warm runs")
    benchmark.add_argument("--first", choices=("libtest", "nextest"), required=True)
    benchmark.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    if args.command == "benchmark":
        return run_benchmark(args.output, args.first)
    parser.error("unknown command")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
