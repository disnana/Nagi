#!/usr/bin/env python3
"""Capture bounded SF05 checks with raw output and source/cache provenance."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]


def sources() -> dict[str, str]:
    listed = subprocess.check_output(["git", "ls-files", "-co", "--exclude-standard", "-z"], cwd=ROOT).decode().split("\0")
    paths = {ROOT / name for name in listed if name and not name.startswith("benchmarks/results/") and "__pycache__" not in Path(name).parts}
    return {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(paths) if path.is_file()}



def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("destination", type=Path, help="new artifact directory; existing directories are rejected")
    parser.add_argument("--stage", choices=("red", "compiler", "sql-lib", "runtime", "native", "engine-free", "scoped", "full", "quality"), required=True)
    args = parser.parse_args()
    destination = args.destination.resolve()
    destination.mkdir(parents=True, exist_ok=False)
    commands = {
        "red": [("compiler-red", ["cargo", "test", "--locked", "-p", "nagic", "--test", "security_sf05", "--", "--nocapture"], 4)],
        "compiler": [("security", ["cargo", "test", "--locked", "-p", "nagic", "--test", "security_sf05"], 12), ("registry", ["cargo", "test", "--locked", "-p", "nagic", "--test", "resource_contract_characterization"], 4), ("sql", ["cargo", "test", "--locked", "-p", "nagic", "--test", "sql_check"], 14)],
        "sql-lib": [("sql-lib", ["cargo", "test", "--locked", "-p", "nagic", "--lib", "sql_check::"], 26)],
        "runtime": [("runtime", ["cargo", "test", "--locked", "-p", "nagi-runtime", "--lib", "sqlite::"], 79), ("public", ["cargo", "test", "--locked", "-p", "nagi-runtime", "--test", "sqlite_public_api"], 1)],
        "native": [("native", ["cargo", "test", "--locked", "-p", "nagic", "--test", "sqlite_public", "--test", "security_sf05_native"], 9)],
        "engine-free": [("security", ["cargo", "test", "--locked", "-p", "nagic", "--no-default-features", "--test", "security_sf05"], 12), ("registry", ["cargo", "test", "--locked", "-p", "nagic", "--no-default-features", "--test", "resource_contract_characterization"], 4), ("sql-disabled", ["cargo", "test", "--locked", "-p", "nagic", "--no-default-features", "--test", "sql_check"], 1), ("restore-default-compiler", ["cargo", "build", "--locked", "-p", "nagic"], 0)],
        "scoped": [("constructor-delta", ["cargo", "test", "--locked", "-p", "nagic", "--test", "security_sf05"], 12), ("constructor-engine-free", ["cargo", "test", "--locked", "-p", "nagic", "--no-default-features", "--test", "security_sf05"], 12), ("restore-default-compiler", ["cargo", "build", "--locked", "-p", "nagic"], 0), ("fmt", ["cargo", "fmt", "--all", "--", "--check"], 0)],
        "full": [("workspace", ["cargo", "test", "--locked"], 1)],
        "quality": [("fmt", ["cargo", "fmt", "--all", "--", "--check"], 0), ("clippy", ["cargo", "clippy", "--locked", "--all-targets", "--", "-D", "warnings"], 0), ("fuzz", ["cargo", "run", "--locked", "-p", "nagic", "--example", "fuzz-smoke"], 0)],
    }
    provenance = {
        "stage": args.stage,
        "source_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "source_sha256": sources(),
        "working_tree": subprocess.check_output(["git", "status", "--short"], cwd=ROOT, text=True),
        "rustc": subprocess.check_output(["rustc", "-Vv"], text=True),
        "cargo": subprocess.check_output(["cargo", "-V"], text=True).strip(),
        "cache": {key: os.environ.get(key) for key in ("CARGO_TARGET_DIR", "NAGI_NATIVE_TARGET_DIR", "CARGO_NET_OFFLINE", "CARGO_INCREMENTAL", "CARGO_BUILD_JOBS", "CARGO_PROFILE_DEV_DEBUG", "CARGO_PROFILE_TEST_DEBUG")},
        "clean_build_claim": False,
        "commands": [],
    }
    (destination / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
    failed = False
    for label, command, minimum in commands[args.stage]:
        started = time.monotonic()
        print(f"Running {label}: {' '.join(command)}", flush=True)
        log = destination / f"{label}.log"
        with log.open("wb") as output:
            completed = subprocess.run(command, cwd=ROOT, stdout=output, stderr=subprocess.STDOUT)
        text = log.read_text(errors="replace")
        results = [tuple(map(int, match)) for match in re.findall(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;", text)]
        ran = sum(passed + failures for passed, failures, _ignored in results)
        purpose_ran = ran >= minimum
        expected_exit = 101 if args.stage == "red" else 0
        purpose_failures = text.count("legacy SQL accepted in")
        accepted = completed.returncode == expected_exit and purpose_ran and (args.stage != "red" or purpose_failures >= minimum)
        record = {"label": label, "command": command, "exit": completed.returncode, "expected_exit": expected_exit, "seconds": time.monotonic() - started, "minimum_executed_tests": minimum, "observed_test_blocks": results, "executed_tests": ran, "accepted": accepted, "raw_log_sha256": hashlib.sha256(log.read_bytes()).hexdigest()}
        provenance["commands"].append(record)
        (destination / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
        print(f"{label}: exit={completed.returncode}, executed={ran}, accepted={accepted}", flush=True)
        failed |= not accepted
    return int(failed)


if __name__ == "__main__":
    raise SystemExit(main())
