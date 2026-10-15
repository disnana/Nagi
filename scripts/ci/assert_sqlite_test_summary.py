"""Require the exact platform-specific SQLite unit-test count in Cargo output."""
import pathlib
import re
import sys

_ANSI = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
_SUMMARY = re.compile(
    r"(?m)^\s*test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; "
    r"(\d+) ignored; (\d+) measured; (\d+) filtered out; finished in [^\r\n]+\s*$"
)


def expected_count(platform):
    if platform == "win32":
        return 80
    if platform in {"linux", "darwin"}:
        return 80
    raise ValueError(f"unsupported platform for SQLite test-count guard: {platform}")


def verify(text, platform):
    matches = list(_SUMMARY.finditer(_ANSI.sub("", text)))
    if len(matches) != 1:
        raise AssertionError(f"expected exactly one Cargo test summary, found {len(matches)}")
    status, passed, failed, ignored, _measured, _filtered = matches[0].groups()
    passed, failed, ignored = int(passed), int(failed), int(ignored)
    expected = expected_count(platform)
    if status != "ok" or passed != expected or failed != 0 or ignored != 0:
        raise AssertionError(
            f"expected exactly {expected} passed, 0 failed, 0 ignored; "
            f"got status={status}, passed={passed}, failed={failed}, ignored={ignored}"
        )
    return passed


def main(argv=None):
    args = sys.argv[1:] if argv is None else argv
    if len(args) != 1:
        raise SystemExit("usage: assert_sqlite_test_summary.py CARGO_LOG")
    try:
        text = pathlib.Path(args[0]).read_text(encoding="utf-8")
        verify(text, sys.platform)
    except (OSError, UnicodeError, AssertionError, ValueError) as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    main()
