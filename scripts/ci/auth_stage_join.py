"""Join completed auth-stage stdout/stderr without changing strict consumers.

Capture files are not capped while Cargo runs. After it exits, the joined raw
prefix is at most RAW_LIMIT+1 bytes so existing readers still detect oversize.
This helper runs no process and never publishes raw data or exception details.
"""
from __future__ import annotations

from pathlib import Path
import stat
import sys

from auth_stage_diagnostic import RAW_LIMIT

NAMES = ("csrf", "prelogin", "broad", "cases")


def join(root: Path, name: str) -> int:
    if name not in NAMES:
        print("auth_stage_join state=unavailable reason=arguments")
        return 1
    stdout = root / f"auth-stage-{name}.raw"
    stderr = root / f"auth-stage-{name}.stderr.raw"
    try:
        if not all(stat.S_ISREG(path.lstat().st_mode) for path in (stdout, stderr)):
            raise ValueError("nonregular capture")
        if stdout.samefile(stderr):
            raise ValueError("aliased captures")
        limit = RAW_LIMIT + 1
        with stdout.open("rb") as source:
            combined = source.read(limit)
        if len(combined) < limit:
            combined += b"\n"
            with stderr.open("rb") as source:
                combined += source.read(limit - len(combined))
        with stdout.open("wb") as output:
            output.write(combined)
    except (OSError, ValueError):
        print("auth_stage_join state=unavailable reason=io")
        return 1
    print("auth_stage_join state=joined scope=" + name)
    return 0


def main() -> int:
    if len(sys.argv) != 3:
        print("auth_stage_join state=unavailable reason=arguments")
        return 1
    return join(Path(sys.argv[1]), sys.argv[2])


if __name__ == "__main__":
    raise SystemExit(main())
