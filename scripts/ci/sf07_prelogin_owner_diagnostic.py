"""Failure-only SF07 supplement. It can never replace the primary test result.

Only the existing bounded closed-field collector's trace may be published.
Raw Cargo/test output stays private and is removed before publication is allowed.
"""
from __future__ import annotations

import os
from pathlib import Path
import stat
import sys

import auth_stage_diagnostic as stages
from session_private_deadline_diagnostic import capture as capture_process

CASE = ("auth::prelogin::tests::fixed_wire_owner_tests::"
        "actual_prelogin_wire_fixed_owner_outlives_issuer_until_last_bytes_clone")
OPT_IN = "NAGI_PRELOGIN_WIRE_STAGE_DIAG"


def owned_root(root: Path, identity: tuple[int, int]) -> bool:
    info = root.lstat()
    return stat.S_ISDIR(info.st_mode) and (info.st_dev, info.st_ino) == identity


def command() -> list[str]:
    return ["cargo", "test", "--locked", "-p", "nagi-runtime", "--lib", CASE,
            "--", "--exact", "--nocapture"]


def run(root: Path) -> int:
    # Exclusive ownership: a repeated invocation cannot overwrite another trace
    # or delete a raw file owned by a different invocation.
    try:
        root.mkdir(parents=True, exist_ok=False)
        info = root.lstat()
        identity = (info.st_dev, info.st_ino)
    except OSError:
        print("sf07_prelogin_stage state=unavailable reason=directory")
        return 1
    raw_file = root / "auth-stage-prelogin.raw"
    trace_file = root / "auth-stage-prelogin.trace"
    environment = dict(os.environ)
    # Only this owner may authorize publication. Cargo/test descendants must
    # never inherit the runner's output-file capability.
    environment.pop("GITHUB_OUTPUT", None)
    environment[OPT_IN] = "1"
    environment["CARGO_TERM_COLOR"] = "never"
    environment["RUSTFLAGS"] = (environment.get("RUSTFLAGS", "")
                                + " --cfg nagi_auth_stage_diag").strip()
    print("sf07_prelogin_stage attempt=supplemental scheduling=perturbed", flush=True)
    normalized = False
    result = 1
    raw_identity = None
    try:
        if "CARGO_ENCODED_RUSTFLAGS" in environment:
            print("sf07_prelogin_stage state=unavailable reason=encoded_flags")
            return 1
        raw, code, oversized = capture_process(command(), environment)
        if not owned_root(root, identity):
            raise ValueError("changed owner")
        try:
            trace_file.lstat()
        except FileNotFoundError:
            pass
        else:
            raise ValueError("preexisting trace")
        if oversized:
            # Preserve the existing collector's oversize refusal even if a
            # timed-out drain returned a shorter prefix. Never relax its limit.
            raw = raw[:stages.RAW_LIMIT + 1].ljust(stages.RAW_LIMIT + 1, b"\0")
        with raw_file.open("xb") as output:
            info = os.fstat(output.fileno())
            raw_identity = (info.st_dev, info.st_ino)
            output.write(raw)
        valid = stages.capture("prelogin", root, code, cases={CASE: CASE})
        trace_info = trace_file.lstat()
        if not stat.S_ISREG(trace_info.st_mode) or trace_info.st_nlink != 1:
            raise ValueError("nonregular trace")
        # False means a safely normalized failing diagnostic, not a collector
        # exception or a failure to create its sanitized file.
        normalized = True
        result = 0 if valid and not oversized else 1
    except Exception:
        normalized = False
        print("sf07_prelogin_stage state=unavailable reason=capture")
    finally:
        try:
            if not owned_root(root, identity):
                raise ValueError("changed owner")
            if raw_identity is not None:
                info = raw_file.lstat()
                if (not stat.S_ISREG(info.st_mode)
                        or (info.st_dev, info.st_ino) != raw_identity):
                    raise ValueError("changed raw")
                raw_file.unlink()
        except (OSError, ValueError):
            print("sf07_prelogin_stage state=unavailable reason=cleanup")
            normalized = False
            result = 1
    if normalized:
        try:
            if not owned_root(root, identity) or trace_file.lstat() != trace_info:
                raise ValueError("changed trace")
            output_path = os.environ.get("GITHUB_OUTPUT")
            if output_path:
                with Path(output_path).open("a", encoding="ascii") as output:
                    output.write("sanitized=true\n")
        except (OSError, ValueError):
            print("sf07_prelogin_stage state=unavailable reason=publication")
            return 1
    return result


def main() -> int:
    if len(sys.argv) != 2:
        print("sf07_prelogin_stage state=unavailable reason=arguments")
        return 1
    return run(Path(sys.argv[1]))


if __name__ == "__main__":
    raise SystemExit(main())
