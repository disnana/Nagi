"""Compare --show-output with one bounded same-executable --nocapture run."""
from __future__ import annotations

import argparse
import contextlib
import hashlib
import importlib.util
import io
import pathlib
import re
import shutil
import sys
import tempfile

R3_SHA256 = "cbec58ae6d3610d711c069866a9e2cf55047fd436334e57b37ba32bca1ce627f"
R3_RELATIVE = pathlib.Path("scripts/ci/session_windows_full_lib_diagnostic.py")
LOAD_LIMIT = 64 * 1024
_BUILD = ["test", "--locked", "-p", "nagi-runtime", "--lib", "--no-run", "--message-format=json"]
_HASH_LINE = re.compile(r"same_executable_sha256=([0-9a-f]{64})\Z")
_RESULT_LINE = re.compile(r"result=(pass|pass_with_ignored|fail)\Z")
_FAILED_CASE = re.compile(r"failed_case=([A-Za-z_][A-Za-z0-9_:]*)\Z")


class _BinaryStdout:
    """Capture the reused parser's binary-safe, closed diagnostic output."""

    def __init__(self):
        self.buffer = io.BytesIO()

    def write(self, value):
        return len(value)

    def flush(self):
        return None


def _load_r3(root):
    path = root / R3_RELATIVE
    try:
        with path.open("rb") as stream:
            raw = stream.read(LOAD_LIMIT + 1)
        if len(raw) > LOAD_LIMIT:
            return None
        raw.decode("utf-8", errors="strict")
        canonical = raw.replace(b"\r\n", b"\n")
        if b"\r" in canonical or hashlib.sha256(canonical).hexdigest() != R3_SHA256:
            return None
        spec = importlib.util.spec_from_file_location("nagi_r3_full_lib_diagnostic", path)
        if spec is None or spec.loader is None:
            return None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module
    except (OSError, UnicodeError, ImportError, ValueError):
        return None


def _same_runtime_cwd(cwd, root):
    try:
        runtime = (root.resolve(strict=True) / "runtime").resolve(strict=True)
        return pathlib.Path(cwd).resolve(strict=True) == runtime
    except (OSError, RuntimeError):
        return False


def _run_show(r3, root, cargo):
    calls = []
    loaded = {}
    original_loader = r3._load_r6

    def load_and_record(source_root):
        base = original_loader(source_root)
        if base is None or "base" in loaded:
            return None
        original_capture = base.capture

        def record(command, cwd, env, limit, *args, **kwargs):
            result = original_capture(command, cwd, env, limit, *args, **kwargs)
            calls.append((tuple(str(item) for item in command), pathlib.Path(cwd), dict(env),
                          limit, result))
            return result

        loaded.update(base=base, capture=original_capture)
        base.capture = record
        return base

    r3._load_r6 = load_and_record
    output = io.StringIO()
    argv = ["--root", str(root)]
    if cargo:
        argv.extend(("--cargo", cargo))
    try:
        with contextlib.redirect_stdout(output):
            code = r3.main(argv)
    finally:
        r3._load_r6 = original_loader
        if loaded:
            loaded["base"].capture = loaded["capture"]
    return code, output.getvalue(), calls, loaded.get("base"), loaded.get("capture")


def _show_state(code, output):
    results = [match.group(1) for line in output.splitlines()
               if (match := _RESULT_LINE.fullmatch(line))]
    if len(results) != 1:
        return "unavailable"
    if code == 0 and results[0] in ("pass", "pass_with_ignored"):
        return results[0]
    return "fail"


def _recorded_profile(calls, root, r3, base, output):
    if len(calls) != 6:
        return None
    build_command = calls[0][0]
    if len(build_command) != len(_BUILD) + 1 or list(build_command[1:]) != _BUILD:
        return None
    exe_text = calls[1][0][0]
    if not exe_text or any(call[0][0] != exe_text for call in calls[1:]):
        return None
    if calls[4][0] != (exe_text, "--list") or calls[5][0] != (exe_text, "--show-output"):
        return None
    runtime = root.resolve() / "runtime"
    env = calls[5][2]
    if any(not _same_runtime_cwd(call[1], root) or call[2] != env for call in calls[1:]):
        return None
    if (env.get("RUSTFLAGS") != base.RUSTFLAGS or env.get("CARGO_TERM_COLOR") != "never"
            or "CARGO_ENCODED_RUSTFLAGS" in env or "RUST_TEST_THREADS" in env):
        return None
    if calls[0][1].resolve() != root.resolve():
        return None
    build_out, _build_err, build_code, build_big, build_timeout = calls[0][4]
    if build_code != 0 or build_big or build_timeout:
        return None
    exe = base._cargo_artifact(build_out, root)
    if exe is None or pathlib.Path(exe).resolve(strict=True) != pathlib.Path(exe_text).resolve(strict=True):
        return None
    hashes = [_HASH_LINE.fullmatch(line).group(1) for line in output.splitlines()
              if _HASH_LINE.fullmatch(line)]
    exe_hash = base._bound_executable_sha256(exe, root)
    if len(hashes) != 1 or exe_hash is None or hashes[0] != exe_hash:
        return None
    listed_out, listed_err, listed_code, listed_big, listed_timeout = calls[4][4]
    if listed_code != 0 or listed_err or listed_big or listed_timeout:
        return None
    parser = base._parser(root)
    if parser is None:
        return None
    names = r3._full_inventory(listed_out, parser, base.CASE_NAMES)
    if names is None:
        return None
    show_out, _show_err, show_code, show_big, show_timeout = calls[5][4]
    if show_code != 0 or show_big or show_timeout or not show_out:
        return None
    if base._bound_executable_sha256(exe, root) != exe_hash:
        return None
    return exe, exe_hash, env, runtime, names, parser


def _summary_profile(raw, summary_pattern, expected_tests, exit_code):
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeError:
        return None
    normalized = text.replace("\r\n", "\n")
    if "\r" in normalized:
        return None
    matches = [summary_pattern.fullmatch(line) for line in normalized.splitlines()]
    matches = [match for match in matches if match is not None]
    if len(matches) != 1:
        return None
    match = matches[0]
    outcome = match.group(1)
    passed, failed, ignored, measured, filtered = (int(match.group(index)) for index in range(2, 7))
    totals_valid = (passed + failed + ignored == expected_tests
                    and measured == 0 and filtered == 0)
    exit_valid = ((exit_code == 0 and outcome == "ok." and failed == 0)
                  or (exit_code != 0 and outcome == "FAILED." and failed > 0))
    return {
        "valid": totals_valid and exit_valid,
        "tests": expected_tests,
        "passed": passed,
        "failed": failed,
        "ignored": ignored,
        "measured": measured,
        "filtered": filtered,
    }


def _project_broad(parser, stdout, stderr, exit_code, oversized, timed_out, cases,
                   summary_pattern):
    if oversized or timed_out or not isinstance(stdout, bytes) or not isinstance(stderr, bytes):
        return False, False, (), None, False
    raw = stdout + stderr
    if len(raw) > parser.RAW_LIMIT:
        return False, False, (), None, False
    summary = _summary_profile(raw, summary_pattern, len(cases), exit_code)
    try:
        with tempfile.TemporaryDirectory(prefix="nagi-session-nocapture-") as temporary:
            root = pathlib.Path(temporary)
            (root / "auth-stage-broad.raw").write_bytes(raw)
            sink = _BinaryStdout()
            with contextlib.redirect_stdout(sink):
                valid = parser.capture("broad", root, exit_code, cases=cases)
            projected = sink.buffer.getvalue()
            with (root / "auth-stage-broad.trace").open("rb") as stream:
                trace = stream.read(parser.OUTPUT_LIMIT["broad"] + 1)
            if (len(projected) > parser.OUTPUT_LIMIT["broad"]
                    or projected != trace or not projected.endswith(b"\n")):
                return False, True, (), summary, False
            lines = projected.decode("ascii", errors="strict").splitlines()
            if any("\r" in line for line in lines):
                return False, True, (), summary, bool(valid)
            # The pinned legacy broad parser requires a record in each Guard block;
            # zero-record blocks stay unavailable even though the R3 parser accepts them.
            parser_valid = bool(valid)
            overall_valid = parser_valid and summary is not None and summary["valid"]
            return overall_valid, True, tuple(lines), summary, parser_valid
    except (OSError, UnicodeError, ValueError, TypeError):
        return False, True, (), summary, False


def _failed_inventory_names(lines, base):
    found = set()
    for line in lines:
        match = _FAILED_CASE.fullmatch(line)
        if match is not None and match.group(1) in base.CASE_NAMES:
            found.add(match.group(1))
    return tuple(name for name in base.CASE_NAMES if name in found)


def _known_failure_locations(base, failed_names, stdout, stderr):
    """Project positions only for failures already named by the closed parser."""
    expected = tuple(name for name in base.CASE_NAMES if name in set(failed_names))
    if not expected:
        return {}
    try:
        text = (stdout + stderr).decode("utf-8", errors="strict")
    except UnicodeError:
        return {name: None for name in expected}
    normalized = text.replace("\r\n", "\n")
    if "\r" in normalized:
        return {name: None for name in expected}
    panic_lines = []
    for line in normalized.splitlines():
        match = base.PANIC.fullmatch(line)
        if match is not None and match.group(1) in expected:
            panic_lines.append(line)
    locations, malformed = base._panic_locations(panic_lines, expected)
    if malformed:
        return {name: None for name in expected}
    return locations


def _known_failure_lines(base, diagnostics, stdout, stderr):
    failed_names = _failed_inventory_names(diagnostics, base)
    locations = _known_failure_locations(base, failed_names, stdout, stderr)
    lines = []
    for name in failed_names:
        location = locations.get(name)
        if location is None:
            lines.append(f"nocapture_known_case={name} state=fail source=unavailable")
        else:
            lines.append(f"nocapture_known_case={name} state=fail source={location[0]}"
                         f" line={location[1]} column={location[2]}")
    return tuple(lines)


def _main(argv=None):
    cli = argparse.ArgumentParser(add_help=False)
    cli.add_argument("--root", required=True)
    cli.add_argument("--cargo")
    args = cli.parse_args(argv)
    root = pathlib.Path(args.root).resolve()
    if not root.is_dir() or not (root / "runtime").is_dir():
        print("result=unavailable stage=preflight")
        return 2
    r3 = _load_r3(root)
    if r3 is None:
        print("result=unavailable stage=source")
        return 2

    show_code, show_output, calls, base, _capture = _run_show(r3, root, args.cargo)
    show_state = _show_state(show_code, show_output)
    print(f"show_profile={show_state}")
    if show_state not in ("pass", "pass_with_ignored") or base is None:
        print("comparison=fail stage=show_output")
        return 1

    profile = _recorded_profile(calls, root, r3, base, show_output)
    if profile is None:
        print("comparison=fail stage=show_protocol")
        return 1
    exe, exe_hash, env, runtime, names, parser = profile
    print(f"same_executable_sha256={exe_hash}")
    if base._bound_executable_sha256(exe, root) != exe_hash:
        print("nocapture=unavailable reason=executable_changed")
        print("comparison=fail")
        return 1

    # Keep the exact full-libtest executable, environment, and canonical runtime cwd.
    stdout, stderr, native_exit, oversized, timed_out = base.capture(
        [str(exe), "--nocapture"], runtime, env, parser.RAW_LIMIT,
        timeout=r3.FULL_TIMEOUT,
    )
    same_hash = base._bound_executable_sha256(exe, root) == exe_hash
    cases = {name: name for name in names}
    valid, bounded, diagnostics, summary, parser_valid = _project_broad(
        parser, stdout, stderr, native_exit, oversized, timed_out, cases, base.SUMMARY
    )
    summary_valid = summary is not None and summary["valid"]
    state = "pass" if native_exit == 0 and bounded and valid and same_hash else "fail"
    print(f"nocapture_profile={state} exit={native_exit} bounded={str(bounded).lower()}"
          f" parser={'pass' if parser_valid else 'fail'}"
          f" summary={'pass' if summary_valid else 'fail'}"
          f" hash={'same' if same_hash else 'changed'}")
    if summary is None:
        print("nocapture_counts=unavailable")
    else:
        print("nocapture_counts=tests={tests} passed={passed} failed={failed} ignored={ignored}"
              " measured={measured} filtered={filtered}".format(**summary))
    for line in diagnostics:
        print("nocapture_" + line)
    for line in _known_failure_lines(base, diagnostics, stdout, stderr):
        print(line)
    if state != "pass":
        print("comparison=fail")
        return 1
    print(f"comparison={show_state}")
    return 0


def main(argv=None):
    try:
        return _main(argv)
    except Exception:
        print("result=unavailable stage=internal")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
