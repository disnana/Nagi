"""One-shot Windows auth-stage comparison; only closed fields are printed."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import re
import shutil
import signal
import subprocess
import sys
import threading
import tomllib

RUSTFLAGS = "--cfg nagi_auth_stage_diag --cfg nagi_browser_login_diag"
CFG_LABEL = "nagi_auth_stage_diag+nagi_browser_login_diag"
BUILD_LIMIT = 16 * 1024 * 1024
TEST_LIMIT = 256 * 1024
CAPTURE_TIMEOUT = 600
KILL_TIMEOUT = 10
CASE_NAMES = (
    "auth::prelogin::tests::consumed_bearer_identity_can_issue_session_without_replacing_original_ceiling",
    "auth::prelogin::tests::observer_tests::observed_consumed_missing_row_actual_restoration_wait_gate_expiry",
    "auth::prelogin::tests::observer_tests::observed_consumed_missing_row_live_denial",
    "auth::prelogin::tests::one_use_claim_actual_delete_and_full_commit_precede_verifier",
    "auth::prelogin::tests::two_actual_claims_have_exactly_one_committed_witness",
)
SOURCE_BINDINGS = {
    "tests.rs": ("runtime/src/auth/prelogin/tests.rs", "7f9fea291cf9bf9e7f9fce6db7797038ef898293f6f6fab3f2eb85e51ce5da4d"),
    "observer_tests.rs": ("runtime/src/auth/prelogin/observer_tests.rs", "30238968a7745259abe2dc08ffde7b4f8e4ecdcad37d3bd122d730b56d000ce9"),
}
CASE_SOURCE = {
    CASE_NAMES[0]: "tests.rs",
    CASE_NAMES[1]: "observer_tests.rs",
    CASE_NAMES[2]: "observer_tests.rs",
    CASE_NAMES[3]: "tests.rs",
    CASE_NAMES[4]: "tests.rs",
}
SOURCE_LINE_COUNTS = {"tests.rs": 508, "observer_tests.rs": 150}
PANIC = re.compile(r"^thread '([^'\r\n]{1,256})'(?: \([0-9]{1,20}\))? panicked at (.{1,512}):([0-9]{1,6}):([0-9]{1,5}):\s*$")
RUNNING = re.compile(r"^running ([1-9][0-9]{0,3}) tests?\Z")
TEST_ROW = re.compile(r"^test ([A-Za-z_][A-Za-z0-9_:]*) \.\.\. (ok|FAILED|ignored)$")
SUMMARY = re.compile(
    r"^test result: (ok\.|FAILED\.) ([0-9]{1,4}) passed; ([0-9]{1,4}) failed; "
    r"([0-9]{1,4}) ignored; ([0-9]{1,4}) measured; ([0-9]{1,4}) filtered out; "
    r"finished in ([0-9]+(?:\.[0-9]+)?s)$"
)
MAX_BINARY_BYTES = 2 * 1024 * 1024 * 1024


def _kill_tree(pid: int, platform: str, run=subprocess.run) -> None:
    if platform == "nt":
        run(["taskkill", "/PID", str(pid), "/T", "/F"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False,
            timeout=KILL_TIMEOUT)
    else:
        os.killpg(pid, signal.SIGKILL)


def capture(command, cwd, env, limit, timeout=CAPTURE_TIMEOUT, *, platform=None,
            popen=subprocess.Popen, kill_tree=_kill_tree):
    """Drain both pipes with a strict retained-byte bound; never print raw data."""
    platform = platform or os.name
    kwargs = {"stdout": subprocess.PIPE, "stderr": subprocess.PIPE, "cwd": cwd, "env": env}
    if platform == "nt":
        kwargs["creationflags"] = getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0)
    else:
        kwargs["start_new_session"] = True
    process = popen(command, **kwargs)
    retained = {"out": bytearray(), "err": bytearray()}
    oversized = [False]
    total_retained = [0]
    retained_lock = threading.Lock()

    def drain(key, stream):
        try:
            with stream:
                while True:
                    chunk = stream.read(8192)
                    if not chunk:
                        break
                    with retained_lock:
                        room = max(0, limit + 1 - total_retained[0])
                        kept = chunk[:room]
                        retained[key].extend(kept)
                        total_retained[0] += len(kept)
                        if len(chunk) > room or total_retained[0] > limit:
                            oversized[0] = True
        except OSError:
            oversized[0] = True

    threads = [threading.Thread(target=drain, args=(key, stream), daemon=True)
               for key, stream in (("out", process.stdout), ("err", process.stderr))]
    for thread in threads:
        thread.start()
    timed_out = False
    try:
        code = process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        timed_out = True
        try:
            kill_tree(process.pid, platform)
            code = process.wait(timeout=KILL_TIMEOUT)
        except (OSError, subprocess.SubprocessError, subprocess.TimeoutExpired):
            code = -1
    for thread in threads:
        thread.join(timeout=KILL_TIMEOUT)
        if thread.is_alive():
            oversized[0] = True
    return bytes(retained["out"]), bytes(retained["err"]), code, oversized[0], timed_out


def file_sha256(path: pathlib.Path):
    try:
        if not path.is_file() or path.stat().st_size > MAX_BINARY_BYTES:
            return None
        digest = hashlib.sha256()
        with path.open("rb") as stream:
            while block := stream.read(1024 * 1024):
                digest.update(block)
        return digest.hexdigest()
    except OSError:
        return None


def _source_sha256(path: pathlib.Path):
    """Bind the Git LF source bytes, allowing a Windows CRLF checkout only."""
    try:
        with path.open("rb") as stream:
            raw = stream.read(64 * 1024 + 1)
        if len(raw) > 64 * 1024:
            return None
        raw.decode("utf-8", errors="strict")
        canonical = raw.replace(b"\r\n", b"\n")
        if b"\r" in canonical:
            return None
        return hashlib.sha256(canonical).hexdigest()
    except (OSError, UnicodeError):
        return None


def _bound_executable_sha256(path: pathlib.Path, root: pathlib.Path):
    target_root = root / "target"
    try:
        target_real = target_root.resolve(strict=True)
        resolved = path.resolve(strict=True)
        if path.is_symlink() or target_root.is_symlink() or not resolved.is_relative_to(target_real):
            return None
        parent = path
        while True:
            if parent.is_symlink():
                return None
            if parent.resolve(strict=True) == target_real:
                break
            if parent.parent == parent:
                return None
            parent = parent.parent
        return file_sha256(path)
    except (OSError, RuntimeError):
        return None


def _parser(root):
    path = root / "scripts/ci/auth_stage_diagnostic.py"
    if _source_sha256(path) != "ae07182ce903c05e9b4a924e1d404571b822a13d0f48474108a8dd58a2809ac7":
        return None
    spec = importlib.util.spec_from_file_location("nagi_auth_stage_closed_parser", path)
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _validate_source(root):
    for _key, (relative, expected) in SOURCE_BINDINGS.items():
        if _source_sha256(root / relative) != expected:
            return False
    return True


def _runtime_identity(root):
    """Resolve this workspace's package name/version from its Cargo manifests."""
    try:
        root_manifest = root / "Cargo.toml"
        package_manifest = root / "runtime" / "Cargo.toml"
        if root_manifest.stat().st_size > 64 * 1024 or package_manifest.stat().st_size > 64 * 1024:
            return None
        workspace_data = tomllib.loads(root_manifest.read_text(encoding="utf-8"))
        package_data = tomllib.loads(package_manifest.read_text(encoding="utf-8"))
        package = package_data.get("package")
        if not isinstance(package, dict):
            return None
        name = package.get("name")
        version_value = package.get("version")
        if isinstance(version_value, str):
            version = version_value
        elif isinstance(version_value, dict) and version_value.get("workspace") is True:
            workspace = workspace_data.get("workspace")
            workspace_package = workspace.get("package") if isinstance(workspace, dict) else None
            version = workspace_package.get("version") if isinstance(workspace_package, dict) else None
        else:
            return None
        if not isinstance(name, str) or not name or not isinstance(version, str) or not version:
            return None
        return name, version
    except (OSError, UnicodeError, tomllib.TOMLDecodeError, TypeError, ValueError):
        return None


def _cargo_artifact(raw, root):
    if len(raw) > BUILD_LIMIT:
        return None
    found = []
    build_finished = []
    last_reason = None
    target_root = root / "target"
    manifest = root / "runtime" / "Cargo.toml"
    try:
        identity = _runtime_identity(root)
        if identity is None:
            return None
        package_name, package_version = identity
        target_real = target_root.resolve(strict=True)
        manifest_real = manifest.resolve(strict=True)
        text = raw.decode("utf-8", errors="strict")
        for line in text.splitlines():
            if not line:
                return None
            item = json.loads(line)
            if not isinstance(item, dict):
                return None
            reason = item.get("reason")
            if not isinstance(reason, str):
                return None
            last_reason = reason
            if reason == "build-finished":
                build_finished.append(item.get("success"))
            if item.get("reason") != "compiler-artifact":
                continue
            target = item.get("target")
            profile = item.get("profile")
            package = item.get("package_id")
            manifest_path = item.get("manifest_path")
            executable = item.get("executable")
            if (isinstance(target, dict) and target.get("name") == "nagi_runtime"
                    and target.get("kind") == ["lib"] and target.get("test") is True
                    and isinstance(profile, dict) and profile.get("test") is True
                    and isinstance(package, str)
                    and package.rsplit("#", 1)[-1] == f"{package_name}@{package_version}"
                    and isinstance(manifest_path, str)
                    and os.path.normcase(str(
                        (pathlib.Path(manifest_path) if pathlib.Path(manifest_path).is_absolute()
                         else root / manifest_path).resolve(strict=True)
                    )) == os.path.normcase(str(manifest_real))
                    and isinstance(executable, str) and executable):
                path = pathlib.Path(executable)
                if not path.is_absolute():
                    path = root / path
                if _bound_executable_sha256(path, root) is None:
                    return None
                found.append(path)
    except (OSError, UnicodeError, json.JSONDecodeError, TypeError, ValueError, RuntimeError):
        return None
    return found[0] if len(found) == 1 and build_finished == [True] and last_reason == "build-finished" else None


def _inventory(raw, parser, expected=CASE_NAMES):
    if len(raw) > TEST_LIMIT or not raw.endswith(b"\n"):
        return None
    try:
        lines = raw.decode("utf-8", errors="strict").splitlines()
    except UnicodeError:
        return None
    names = []
    footer = []
    separator = False
    for line in lines:
        entry = parser.CASE_ENTRY.fullmatch(line)
        end = parser.CASE_FOOTER.fullmatch(line)
        if line == "":
            if len(names) != len(expected) or footer:
                return None
            separator = True
            continue
        if entry:
            if separator or footer:
                return None
            name = entry.group(1)
            if len(name) > parser.CASE_NAME_LIMIT or name in names or len(names) >= parser.CASE_LIMIT:
                return None
            names.append(name)
        elif end and not footer:
            footer.append(int(end.group(1)))
        else:
            return None
    if (len(names) != len(expected) or len(footer) != 1
            or footer[0] != len(names) or set(names) != set(expected)):
        return None
    return names


def _test_profile(raw, expected):
    if len(raw) > TEST_LIMIT:
        return None
    try:
        lines = raw.decode("utf-8", errors="strict").splitlines()
    except UnicodeError:
        return None
    running, rows, summaries, panic_lines = [], [], [], []
    unknown_rows = False
    for line in lines:
        match = RUNNING.fullmatch(line)
        if match:
            running.append(int(match.group(1)))
            continue
        row = TEST_ROW.fullmatch(line)
        if row:
            rows.append((row.group(1), row.group(2)))
            continue
        if line.startswith("test ") and " ... " in line:
            unknown_rows = True
        summary = SUMMARY.fullmatch(line)
        if summary:
            summaries.append(tuple(summary.groups()))
        if line.startswith("thread '") and " panicked at " in line:
            panic_lines.append(line)
    if (unknown_rows or len(running) != 1 or running[0] != len(expected)
            or len(summaries) != 1 or len(rows) != len(expected)):
        return None
    row_names = [name for name, _state in rows]
    if len(set(row_names)) != len(row_names) or set(row_names) != set(expected):
        return None
    outcome, passed, failed, ignored, measured, filtered, _duration = summaries[0]
    counts = {"ok": 0, "FAILED": 0, "ignored": 0}
    states = dict(rows)
    for state in states.values():
        counts[state] += 1
    if (int(passed) != counts["ok"] or int(failed) != counts["FAILED"]
            or int(ignored) != counts["ignored"] or int(measured) != 0
            or outcome != ("ok." if counts["FAILED"] == 0 else "FAILED.")
            or int(filtered) < 0):
        return None
    return {"states": states, "panics": panic_lines, "passed": counts["ok"],
            "failed": counts["FAILED"], "ignored": counts["ignored"]}


def _panic_locations(panic_lines, expected):
    locations = {name: None for name in expected}
    malformed = False
    for line in panic_lines:
        match = PANIC.fullmatch(line)
        if not match:
            malformed = True
            continue
        thread, path, line_raw, col_raw = match.groups()
        if thread not in locations or locations[thread] is not None:
            malformed = True
            continue
        normalized = path.replace("\\", "/")
        source = None
        for candidate in ("tests.rs", "observer_tests.rs"):
            if normalized.endswith("/src/auth/prelogin/" + candidate):
                source = candidate
        binding = CASE_SOURCE.get(thread)
        try:
            number, column = int(line_raw), int(col_raw)
        except ValueError:
            malformed = True
            continue
        if (source is None or binding is None or source != binding
                or not 1 <= number <= SOURCE_LINE_COUNTS[source] or not 1 <= column <= 9999):
            malformed = True
            continue
        locations[thread] = (source.removesuffix(".rs"), number, column)
    return locations, malformed


def _print(line):
    sys.stdout.write(line + "\n")


def main(argv=None):
    try:
        return _main(argv)
    except Exception:
        _print("result=unavailable stage=internal")
        return 2


def _main(argv=None):
    cli = argparse.ArgumentParser(add_help=False)
    cli.add_argument("--root", required=True)
    cli.add_argument("--cargo")
    args = cli.parse_args(argv)
    root = pathlib.Path(args.root).resolve()
    cargo = args.cargo or shutil.which("cargo")
    if not cargo or not root.is_dir() or not (root / "runtime").is_dir() or not _validate_source(root):
        tests, expected_tests = SOURCE_BINDINGS["tests.rs"]
        observer, expected_observer = SOURCE_BINDINGS["observer_tests.rs"]
        _print("result=unavailable stage=preflight"
               + " cargo_found=" + str(bool(cargo)).lower()
               + " root_exists=" + str(root.is_dir()).lower()
               + " runtime_exists=" + str((root / "runtime").is_dir()).lower()
               + " source_tests_match=" + str(_source_sha256(root / tests) == expected_tests).lower()
               + " source_observer_match=" + str(_source_sha256(root / observer) == expected_observer).lower())
        return 2
    if "CARGO_ENCODED_RUSTFLAGS" in os.environ or "RUST_TEST_THREADS" in os.environ:
        _print("result=unavailable stage=environment")
        return 2
    closed = _parser(root)
    if closed is None:
        _print("result=unavailable stage=parser parser_match=false")
        return 2
    env = os.environ.copy()
    env["RUSTFLAGS"] = RUSTFLAGS
    env["CARGO_TERM_COLOR"] = "never"
    build_cmd = [cargo, "test", "--locked", "-p", "nagi-runtime", "--lib", "--no-run",
                 "--message-format=json"]
    out, _err, build_code, too_big, timed_out = capture(build_cmd, root, env, BUILD_LIMIT)
    if build_code != 0 or too_big or timed_out:
        _print(f"build=fail exit={build_code} bounded={'fail' if too_big or timed_out else 'pass'} cfg={CFG_LABEL}")
        return 1
    exe = _cargo_artifact(out, root)
    exe_hash = _bound_executable_sha256(exe, root) if exe else None
    if exe_hash is None:
        _print(f"build=fail exit={build_code} artifact=invalid cfg={CFG_LABEL}")
        return 1
    _print(f"build=pass executable_sha256={exe_hash} cfg={CFG_LABEL}")

    listed = capture([str(exe), *CASE_NAMES, "--exact", "--list"],
                     root / "runtime", env, TEST_LIMIT)
    inv = _inventory(listed[0], closed) if (listed[2] == 0 and not listed[1]
                                              and not listed[3] and not listed[4]) else None
    if inv is None or _bound_executable_sha256(exe, root) != exe_hash:
        _print("inventory=fail")
        return 1
    _print(f"inventory=pass tests={len(inv)}")

    overall = 0
    for label, names in (("single", (CASE_NAMES[0],)), ("parallel", CASE_NAMES)):
        if _bound_executable_sha256(exe, root) != exe_hash:
            _print(f"profile={label} state=unavailable exit=-1")
            overall = 1
            break
        command = [str(exe), *names, "--exact", "--show-output"]
        out, _err, code, oversized, timed_out = capture(command, root / "runtime", env, TEST_LIMIT)
        after = _bound_executable_sha256(exe, root)
        result = _test_profile(out, names) if not _err and not oversized and not timed_out else None
        if result is None or after != exe_hash:
            _print(f"profile={label} state=invalid exit={code} hash={'same' if after == exe_hash else 'changed'}")
            overall = 1
            continue
        good = code == 0 and result["failed"] == 0 and result["ignored"] == 0
        _print(f"profile={label} state={'pass' if good else 'fail'} exit={code} selected={len(names)}")
        locations, bad_location = _panic_locations(result["panics"], names)
        for name in names:
            state = result["states"][name]
            if state == "ok":
                _print(f"case={name} state=pass")
            elif state == "FAILED":
                loc = locations[name]
                if loc is None:
                    _print(f"case={name} state=fail source=unavailable")
                else:
                    _print(f"case={name} state=fail source={loc[0]} line={loc[1]} column={loc[2]}")
                overall = 1
            else:
                _print(f"case={name} state=ignored source=unavailable")
                overall = 1
        if code != 0 or bad_location:
            overall = 1
    _print(f"result={'pass' if overall == 0 else 'fail'}")
    return overall


if __name__ == "__main__":
    raise SystemExit(main())
