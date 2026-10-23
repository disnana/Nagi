"""Failure-only SF07 supplement. It can never replace the primary test result.

Only the existing bounded closed-field collector's trace may be published.
Raw Cargo/test output stays private and is removed before this process exits.
"""
from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys

import auth_stage_diagnostic as stages
from session_private_deadline_diagnostic import capture as capture_process

CASE = ("auth::prelogin::tests::fixed_wire_owner_tests::"
        "actual_prelogin_wire_fixed_owner_outlives_issuer_until_last_bytes_clone")
OPT_IN = "NAGI_PRELOGIN_WIRE_STAGE_DIAG"


def command() -> list[str]:
    return ["cargo", "test", "--locked", "-p", "nagi-runtime", "--lib", CASE,
            "--", "--exact", "--nocapture"]


def run(root: Path) -> int:
    # Exclusive ownership: a repeated invocation cannot overwrite another trace
    # or delete a raw file owned by a different invocation.
    try:
        root.mkdir(parents=True, exist_ok=False)
    except OSError:
        print("sf07_prelogin_stage state=unavailable reason=directory")
        return 1
    raw_file = root / "auth-stage-prelogin.raw"
    environment = dict(os.environ)
    environment[OPT_IN] = "1"
    environment["CARGO_TERM_COLOR"] = "never"
    environment["RUSTFLAGS"] = (environment.get("RUSTFLAGS", "")
                                + " --cfg nagi_auth_stage_diag").strip()
    print("sf07_prelogin_stage attempt=supplemental scheduling=perturbed", flush=True)
    try:
        if "CARGO_ENCODED_RUSTFLAGS" in environment:
            print("sf07_prelogin_stage state=unavailable reason=encoded_flags")
            return 1
        raw, code, oversized = capture_process(command(), environment)
        if oversized:
            # Preserve the existing collector's oversize refusal even if a
            # timed-out drain returned a shorter prefix. Never relax its limit.
            raw = raw[:stages.RAW_LIMIT + 1].ljust(stages.RAW_LIMIT + 1, b"\0")
        with raw_file.open("xb") as output:
            output.write(raw)
        valid = stages.capture("prelogin", root, code, cases={CASE: CASE})
        return 0 if valid and not oversized else 1
    except (OSError, ValueError, subprocess.SubprocessError):
        print("sf07_prelogin_stage state=unavailable reason=capture")
        return 1
    finally:
        try:
            raw_file.unlink(missing_ok=True)
        except OSError:
            print("sf07_prelogin_stage state=unavailable reason=cleanup")
            return 1


def main() -> int:
    if len(sys.argv) != 2:
        print("sf07_prelogin_stage state=unavailable reason=arguments")
        return 1
    return run(Path(sys.argv[1]))


if __name__ == "__main__":
    raise SystemExit(main())
