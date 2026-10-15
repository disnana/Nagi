"""Require exactly five passing call_member_ editor_assistance tests."""
import pathlib
import re
import sys

_ANSI = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
_SUMMARY = re.compile(
    r"(?m)^\s*test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; "
    r"(\d+) ignored; (\d+) measured; (\d+) filtered out; finished in [^\r\n]+\s*$"
)


def verify(text):
    matches = list(_SUMMARY.finditer(_ANSI.sub("", text)))
    if len(matches) != 1:
        raise AssertionError(f"expected exactly one Cargo test summary, found {len(matches)}")
    status, passed, failed, ignored, _measured, _filtered = matches[0].groups()
    if (status, int(passed), int(failed), int(ignored)) != ("ok", 5, 0, 0):
        raise AssertionError(
            "expected exactly 5 passed, 0 failed, 0 ignored; "
            f"got status={status}, passed={passed}, failed={failed}, ignored={ignored}"
        )
    return 5


def main(argv=None):
    args = sys.argv[1:] if argv is None else argv
    if len(args) != 1:
        raise SystemExit("usage: assert_call_member_test_summary.py CARGO_LOG")
    try:
        verify(pathlib.Path(args[0]).read_text(encoding="utf-8"))
    except (OSError, UnicodeError, AssertionError) as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    main()
