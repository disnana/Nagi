"""Require one complete, successful Go backend integration-test summary."""
from __future__ import annotations

import argparse
import re
from pathlib import Path

RUNNING = re.compile(r"^running ([0-9]+) tests?$", re.MULTILINE)
SUMMARY = re.compile(
    r"^test result: (ok|FAILED)\. ([0-9]+) passed; ([0-9]+) failed; "
    r"([0-9]+) ignored; ([0-9]+) measured; ([0-9]+) filtered out;",
    re.MULTILINE,
)


def validate(text: str, expected: int) -> None:
    running = RUNNING.findall(text)
    summaries = SUMMARY.findall(text)
    if running != [str(expected)]:
        raise ValueError(f"expected one running {expected} tests marker, got {running!r}")
    if len(summaries) != 1:
        raise ValueError(f"expected one test summary, got {len(summaries)}")
    status, passed, failed, ignored, measured, filtered = summaries[0]
    wanted = ("ok", str(expected), "0", "0", "0", "0")
    if (status, passed, failed, ignored, measured, filtered) != wanted:
        raise ValueError(
            "Go backend execution was incomplete: "
            f"{status=}, {passed=}, {failed=}, {ignored=}, {measured=}, {filtered=}"
        )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path)
    parser.add_argument("--expected", type=int, required=True)
    args = parser.parse_args()
    if args.expected <= 0:
        raise SystemExit("expected test count must be positive")
    try:
        text = args.log.read_text(encoding="utf-8")
        validate(text, args.expected)
    except (OSError, UnicodeError, ValueError) as error:
        raise SystemExit(str(error)) from None
    print(f"Verified {args.expected} Go backend integration tests.")


if __name__ == "__main__":
    main()
