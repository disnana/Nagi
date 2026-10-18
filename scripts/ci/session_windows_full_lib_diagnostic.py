"""Compare the R6 focused profiles with one full same-executable libtest run."""
from __future__ import annotations

import contextlib
import argparse
import hashlib
import importlib.util
import io
import os
import pathlib
import re
import shutil
import sys

R6_MODULE_SHA256 = "2d0ba175c6814bb69d3ec2aad557b57b74127447b766699321932a58e838415c"
LIST_LIMIT = 512 * 1024
FULL_OUTPUT_LIMIT = 8 * 1024 * 1024
FULL_STDERR_LIMIT = 2 * 8192
FULL_TIMEOUT = 600
_HASH = re.compile(r"[0-9a-f]{64}\Z")


def _load_r6(root):
    path = root / "scripts/ci/session_windows_same_exe_diagnostic.py"
    try:
        with path.open("rb") as stream:
            raw = stream.read(64 * 1024 + 1)
        if len(raw) > 64 * 1024:
            return None
        raw.decode("utf-8", errors="strict")
        canonical = raw.replace(b"\r\n", b"\n")
        if b"\r" in canonical or hashlib.sha256(canonical).hexdigest() != R6_MODULE_SHA256:
            return None
        spec = importlib.util.spec_from_file_location("nagi_r6_same_exe_diagnostic", path)
        if spec is None or spec.loader is None:
            return None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module
    except (OSError, UnicodeError, ImportError, ValueError):
        return None


def _full_inventory(raw, parser, required_names):
    if len(raw) > LIST_LIMIT or not raw.endswith(b"\n"):
        return None
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeError:
        return None
    normalized = text.replace("\r\n", "\n")
    if "\r" in normalized:
        return None
    lines = normalized.splitlines()
    if len(lines) < 3 or lines[-2] != "":
        return None
    footer = parser.CASE_FOOTER.fullmatch(lines[-1])
    if footer is None:
        return None
    names = []
    for line in lines[:-2]:
        entry = parser.CASE_ENTRY.fullmatch(line)
        if entry is None:
            return None
        name = entry.group(1)
        if (len(name) > parser.CASE_NAME_LIMIT or name in names
                or len(names) >= parser.CASE_LIMIT):
            return None
        names.append(name)
    if (not names or len(names) != int(footer.group(1))
            or not set(required_names).issubset(names)):
        return None
    return names


def _full_stage_stderr(raw, parser):
    """Accept exactly one bounded, loss-free Guard block for each known slot."""
    if len(raw) > FULL_STDERR_LIMIT or not raw.endswith(b"\n"):
        return None
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeError:
        return None
    normalized = text.replace("\r\n", "\n")
    if "\r" in normalized:
        return None
    lines = normalized.splitlines()
    slots = {}
    index = 0
    while index < len(lines):
        header = lines[index]
        if header == "auth-stage slot=1 (test-only; scheduling perturbed)":
            slot = 1
        elif header == "auth-stage slot=2 (test-only; scheduling perturbed)":
            slot = 2
        else:
            return None
        if slot in slots:
            return None
        block_bytes = len((header + "\n").encode("utf-8"))
        index += 1
        records = 0
        while index < len(lines) and lines[index] != "lost=false":
            line = lines[index]
            if (line == "lost=true" or line.startswith("auth-stage slot=")
                    or not parser.RECORD.fullmatch(line)):
                return None
            block_bytes += len((line + "\n").encode("utf-8"))
            records += 1
            if records > 64 or block_bytes > 8192:
                return None
            index += 1
        if index >= len(lines):
            return None
        block_bytes += len(b"lost=false\n")
        if block_bytes > 8192:
            return None
        slots[slot] = records
        index += 1
    return slots if set(slots) == {1, 2} else None


def _profile(raw, expected, expected_filtered, base, limit):
    if len(raw) > limit:
        return None
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeError:
        return None
    normalized = text.replace("\r\n", "\n")
    if "\r" in normalized:
        return None
    ignored_reason = re.compile(r"^test ([A-Za-z_][A-Za-z0-9_:]*) \.\.\. ignored, .{1,256}$")
    rows = []
    for line in normalized.splitlines(keepends=True):
        ending = "\n" if line.endswith("\n") else ""
        content = line[:-1] if ending else line
        match = ignored_reason.fullmatch(content)
        if match:
            line = f"test {match.group(1)} ... ignored{ending}"
        rows.append(line)
    normalized_raw = "".join(rows).encode("utf-8")
    result = base._test_profile(normalized_raw, expected)
    if result is None:
        return None
    try:
        lines = normalized_raw.decode("utf-8", errors="strict").splitlines()
    except UnicodeError:
        return None
    summaries = [base.SUMMARY.fullmatch(line) for line in lines]
    summaries = [summary for summary in summaries if summary is not None]
    if len(summaries) != 1 or int(summaries[0].group(6)) != expected_filtered:
        return None
    return result


def _focused_line_is_closed(line, base):
    if re.fullmatch(r"build=pass executable_sha256=[0-9a-f]{64} cfg=nagi_auth_stage_diag\+nagi_browser_login_diag", line):
        return True
    if re.fullmatch(r"build=fail exit=-?[0-9]{1,4} bounded=(?:pass|fail) cfg=nagi_auth_stage_diag\+nagi_browser_login_diag", line):
        return True
    if re.fullmatch(r"build=fail exit=-?[0-9]{1,4} artifact=invalid cfg=nagi_auth_stage_diag\+nagi_browser_login_diag", line):
        return True
    if re.fullmatch(r"inventory=(?:pass tests=5|fail)", line):
        return True
    if re.fullmatch(
        r"profile=(?:single|parallel) state=(?:pass|fail) exit=-?[0-9]{1,4} selected=(?:1|5)", line
    ):
        return True
    if re.fullmatch(
        r"profile=(?:single|parallel) state=invalid exit=-?[0-9]{1,4} hash=(?:same|changed)"
        r" stdout_valid=(?:true|false) stderr_valid=(?:true|false) bounded=(?:true|false)", line
    ):
        return True
    if re.fullmatch(r"profile=(?:single|parallel) state=unavailable exit=-?[0-9]{1,4}", line):
        return True
    if line.startswith("case="):
        rest = line[len("case="):]
        name, separator, state = rest.partition(" state=")
        if not separator or name not in base.CASE_NAMES:
            return False
        if state in ("pass", "ignored source=unavailable"):
            return True
        match = re.fullmatch(r"fail source=(tests|observer_tests) line=([0-9]{1,3}) column=([0-9]{1,4})", state)
        if match:
            source = base.CASE_SOURCE[name]
            return (source == match.group(1) + ".rs"
                    and 1 <= int(match.group(2)) <= base.SOURCE_LINE_COUNTS[source]
                    and 1 <= int(match.group(3)) <= 9999)
        return state == "fail source=unavailable"
    if re.fullmatch(r"result=(?:pass|fail)", line):
        return True
    if line == "result=unavailable stage=environment":
        return True
    if line == "result=unavailable stage=parser parser_match=false":
        return True
    if line.startswith("result=unavailable stage=preflight"):
        return bool(re.fullmatch(
            r"result=unavailable stage=preflight cargo_found=(?:true|false) root_exists=(?:true|false)"
            r" runtime_exists=(?:true|false) source_tests_match=(?:true|false)"
            r" source_observer_match=(?:true|false)", line))
    return False


def _emit_focused(raw, base):
    try:
        lines = raw.getvalue().splitlines()
    except (AttributeError, UnicodeError):
        return False
    if not lines or any(not _focused_line_is_closed(line, base) for line in lines):
        return False
    for line in lines:
        print("focused_" + line)
    return True


def _emit_full_evidence(profile, names, full_code, bounded, guards, hash_same, base):
    """Preserve validated stdout evidence even when the separate Guard stream fails."""
    failed = [name for name in names if profile["states"][name] == "FAILED"]
    ignored = [name for name in names if profile["states"][name] == "ignored"]
    print(f"full=observed exit={full_code} tests={len(names)} passed={profile['passed']}"
          f" failed={profile['failed']} ignored={profile['ignored']} filtered=0"
          f" stdout_valid=true stderr_valid={str(guards is not None).lower()}"
          f" bounded={str(bounded).lower()} hash={'same' if hash_same else 'changed'}"
          + (f" slot1_records={guards[1]} slot2_records={guards[2]}" if guards is not None else ""))
    print("ignored_policy=observed_not_counted_as_passed")
    for name in failed:
        print(f"failed_case={name}")
    for name in ignored:
        print(f"ignored_case={name}")

    known_failed = tuple(name for name in base.CASE_NAMES if name in failed)
    known_panic_lines = []
    for line in profile["panics"]:
        match = base.PANIC.fullmatch(line)
        if match and match.group(1) in known_failed:
            known_panic_lines.append(line)
    locations, bad_location = base._panic_locations(known_panic_lines, known_failed)
    for name in base.CASE_NAMES:
        state = profile["states"][name]
        if state == "FAILED":
            loc = locations[name]
            if loc is None or bad_location:
                print(f"known_case={name} state=fail source=unavailable")
            else:
                print(f"known_case={name} state=fail source={loc[0]} line={loc[1]} column={loc[2]}")
        else:
            print(f"known_case={name} state={'ignored' if state == 'ignored' else 'pass'}")
    success = (full_code == 0 and not failed and guards is not None and hash_same
               and bounded and not bad_location)
    return success, ignored


def _build_calls(base, root, cargo, original_capture):
    calls = []

    def record(command, cwd, env, limit, *args, **kwargs):
        result = original_capture(command, cwd, env, limit, *args, **kwargs)
        calls.append((list(command), pathlib.Path(cwd), dict(env), limit, result))
        return result

    saved_capture = base.capture
    focused_output = io.StringIO()
    try:
        base.capture = record
        with contextlib.redirect_stdout(focused_output):
            focused_code = base.main(["--root", str(root), "--cargo", cargo])
    finally:
        base.capture = saved_capture
    return focused_code, focused_output, calls


def _valid_r6_calls(calls, root, cargo, base):
    if len(calls) != 4:
        return False
    exe = calls[1][0][0]
    expected = (
        [cargo, "test", "--locked", "-p", "nagi-runtime", "--lib", "--no-run", "--message-format=json"],
        [exe, *base.CASE_NAMES, "--exact", "--list"],
        [exe, base.CASE_NAMES[0], "--exact", "--show-output"],
        [exe, *base.CASE_NAMES, "--exact", "--show-output"],
    )
    if tuple(call[0] for call in calls) != expected:
        return False
    if calls[0][1] != root or calls[0][2].get("RUSTFLAGS") != base.RUSTFLAGS:
        return False
    runtime = root / "runtime"
    env = calls[0][2]
    for command, cwd, call_env, _limit, _result in calls[1:]:
        if cwd != runtime or call_env != env:
            return False
    return (env.get("CARGO_TERM_COLOR") == "never"
            and "CARGO_ENCODED_RUSTFLAGS" not in env and "RUST_TEST_THREADS" not in env)


def main(argv=None):
    try:
        return _main(argv)
    except Exception:
        print("result=unavailable stage=internal")
        return 2


def _main(argv=None):
    cli = argparse.ArgumentParser(add_help=False)
    cli.add_argument("--root", required=True)
    cli.add_argument("--cargo")
    args = cli.parse_args(argv)
    root = pathlib.Path(args.root).resolve()
    cargo = args.cargo or shutil.which("cargo")
    if not cargo or not root.is_dir() or not (root / "runtime").is_dir():
        print("result=unavailable stage=preflight")
        return 2
    base = _load_r6(root)
    if base is None or not base._validate_source(root):
        print("result=unavailable stage=source")
        return 2
    if "CARGO_ENCODED_RUSTFLAGS" in os.environ or "RUST_TEST_THREADS" in os.environ:
        print("result=unavailable stage=environment")
        return 2
    closed = base._parser(root)
    if closed is None:
        print("result=unavailable stage=parser")
        return 2

    focused_code, focused_output, calls = _build_calls(base, root, cargo, base.capture)
    focus_schema_valid = _emit_focused(focused_output, base)
    if not focus_schema_valid:
        print("focused=invalid")
    if not _valid_r6_calls(calls, root, cargo, base):
        print("result=fail stage=focused_protocol")
        return 1

    build_out, _build_err, build_code, build_big, build_timeout = calls[0][4]
    exe = base._cargo_artifact(build_out, root) if build_code == 0 and not build_big and not build_timeout else None
    observed_hash = base._bound_executable_sha256(exe, root) if exe else None
    build_hashes = [match.group(1) for line in focused_output.getvalue().splitlines()
                   if (match := re.fullmatch(
                       r"build=pass executable_sha256=([0-9a-f]{64}) cfg=nagi_auth_stage_diag\+nagi_browser_login_diag",
                       line))]
    if (len(build_hashes) != 1 or observed_hash != build_hashes[0]
            or exe != pathlib.Path(calls[1][0][0])):
        print("result=fail stage=artifact")
        return 1
    exe_hash = build_hashes[0]
    if not focus_schema_valid or focused_code not in (0, 1):
        focused_code = 1
    print(f"same_executable_sha256={exe_hash}")

    env = calls[1][2]
    runtime = root / "runtime"
    if base._bound_executable_sha256(exe, root) != exe_hash:
        print("full_inventory=unavailable reason=executable_changed")
        print("result=fail")
        return 1
    listed = base.capture([str(exe), "--list"], runtime, env, LIST_LIMIT)
    listed_ok = (listed[2] == 0 and not listed[1] and not listed[3] and not listed[4]
                 and base._bound_executable_sha256(exe, root) == exe_hash)
    names = _full_inventory(listed[0], closed, base.CASE_NAMES) if listed_ok else None
    if names is None:
        print("full_inventory=fail")
        print("result=fail")
        return 1
    print(f"full_inventory=pass tests={len(names)}")

    before = base._bound_executable_sha256(exe, root)
    if before != exe_hash:
        print("full=unavailable reason=executable_changed")
        print("result=fail")
        return 1
    full_out, full_err, full_code, oversized, timed_out = base.capture(
        [str(exe), "--show-output"], runtime, env, FULL_OUTPUT_LIMIT, timeout=FULL_TIMEOUT
    )
    after = base._bound_executable_sha256(exe, root)
    bounded = not oversized and not timed_out
    profile = _profile(full_out, names, 0, base, FULL_OUTPUT_LIMIT) if bounded else None
    guards = _full_stage_stderr(full_err, closed) if bounded else None
    if profile is None:
        print("full=invalid"
              + f" exit={full_code} bounded={str(bounded).lower()}"
              + f" stdout_valid={str(profile is not None).lower()}"
              + f" guards_valid={str(guards is not None).lower()}"
              + f" hash={'same' if after == exe_hash else 'changed'}")
        print("result=fail")
        return 1
    success, ignored = _emit_full_evidence(
        profile, names, full_code, bounded, guards, after == exe_hash, base
    )
    if focused_code != 0:
        success = False
    if success and ignored:
        print("result=pass_with_ignored")
    else:
        print(f"result={'pass' if success else 'fail'}")
    return 0 if success else 1


if __name__ == "__main__":
    raise SystemExit(main())
