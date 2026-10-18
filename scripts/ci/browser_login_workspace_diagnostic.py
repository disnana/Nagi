"""One-shot, bounded same-executable BrowserLogin workspace diagnostic."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import selectors
import signal
import subprocess
import sys
import time
import tomllib

ROOT = Path(__file__).resolve().parents[2]
_SPEC = importlib.util.spec_from_file_location(
    "sf07_checkpoint_summary", Path(__file__).with_name("sf07_checkpoint_summary.py")
)
_SUMMARY = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_SUMMARY)

TEST_TARGET = "browser_login_public_api"
TARGET_TESTS = (
    "public_login_claims_once_then_named_verifier_issue_and_appended_cookies",
    "json_value_nodes_and_container_depth_accept_exact_bounds",
)
ALL_TESTS = _SUMMARY.BROWSER_COUNTER_CASES
BUILD_COMMAND = (
    "test", "--workspace", "--no-run", "--lib", "--test", TEST_TARGET,
    "--message-format=json", "--locked",
)
MAX_BUILD_BYTES = 64 * 1024 * 1024
MAX_TEST_BYTES = _SUMMARY.MAX_BYTES
MAX_LINE_BYTES = 1024 * 1024
MAX_EXE_BYTES = 512 * 1024 * 1024
BUILD_TIMEOUT_SECONDS = 20 * 60
CONTROL_TIMEOUT_SECONDS = 60
FEATURE_NAME = re.compile(r"[A-Za-z0-9_-]{1,64}\Z")
VERSION = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?\Z")
DECIMAL = r"(?:0|[1-9][0-9]*)"
STOCK_DIAGNOSTIC = re.compile(
    r"browser_login_stock_cause case=concurrent_once "
    rf"a_status=(?P<a_status>{DECIMAL}) b_status=(?P<b_status>{DECIMAL}) "
    r"a_auth_cookie=(?P<a_auth_cookie>true|false) "
    r"b_auth_cookie=(?P<b_auth_cookie>true|false) "
    r"a_deletion=(?P<a_deletion>true|false) b_deletion=(?P<b_deletion>true|false) "
    rf"verifier_calls=(?P<verifier_calls>{DECIMAL}) "
    rf"handler_calls=(?P<handler_calls>{DECIMAL}) "
    r"claim_committed=(?P<claim_committed>true|false)\Z"
)
JSON_DIAGNOSTIC = re.compile(
    r"browser_login_json_bound_diag phase=exact_bounds "
    rf"status=(?P<status>{DECIMAL}) body_bytes=(?P<body_bytes>{DECIMAL}) "
    rf"expected_body_bytes=(?P<expected_body_bytes>{DECIMAL}) "
    r"body_matches=(?P<body_matches>true|false) "
    rf"verifier_calls=(?P<verifier_calls>{DECIMAL}) "
    rf"handler_calls=(?P<handler_calls>{DECIMAL}) "
    r"claim_committed=(?P<claim_committed>true|false)\Z"
)
DIAGNOSTIC_CASES = {
    "concurrent_once": TARGET_TESTS[0],
    "exact_bounds": TARGET_TESTS[1],
}
DIAGNOSTIC_FAMILY_MARKERS = ("browser_login_stock", "browser_login_json_bound")
DIAGNOSTIC_RANGES = {
    "a_status": 999, "b_status": 999, "status": 999,
    "body_bytes": 8193, "expected_body_bytes": 8193,
    "verifier_calls": 1023, "handler_calls": 1023,
}


def _stop_group(process):
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    try:
        process.wait(timeout=2)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait()


def run_bounded(command, cwd, max_bytes, timeout_seconds=None):
    """Capture merged process output in memory only, with a strict byte ceiling."""
    try:
        process = subprocess.Popen(
            list(command), cwd=str(cwd), stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, bufsize=0,
            start_new_session=True,
        )
    except OSError:
        return None, b"", "spawn_failed"
    selector = selectors.DefaultSelector()
    output = bytearray()
    state = None
    deadline = None if timeout_seconds is None else time.monotonic() + timeout_seconds
    try:
        os.set_blocking(process.stdout.fileno(), False)
        selector.register(process.stdout, selectors.EVENT_READ)
        while True:
            if deadline is not None and time.monotonic() >= deadline:
                state = "timeout"
                _stop_group(process)
                break
            wait = 0.25 if deadline is None else max(0.0, min(0.25, deadline - time.monotonic()))
            events = selector.select(wait) if selector.get_map() else ()
            for key, _ in events:
                try:
                    chunk = os.read(key.fd, 65536)
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                if len(output) + len(chunk) > max_bytes:
                    state = "output_limit"
                    _stop_group(process)
                    break
                output.extend(chunk)
            if state is not None:
                break
            if not selector.get_map() and process.poll() is not None:
                break
            if not selector.get_map() and wait:
                time.sleep(wait)
        if state is not None and process.poll() is None:
            _stop_group(process)
        exit_code = process.wait()
        return exit_code, bytes(output), state
    finally:
        selector.close()
        if process.stdout is not None:
            process.stdout.close()


def _toml(path):
    with Path(path).open("rb") as source:
        return tomllib.load(source)


def _versioned_package_id(package_id):
    if not isinstance(package_id, str) or not package_id or len(package_id) > 4096:
        return None
    suffix = package_id.rsplit("#", 1)[-1]
    version = suffix.rsplit("@", 1)[-1]
    return version if VERSION.fullmatch(version) else None


def _features(raw):
    if not isinstance(raw, list) or len(raw) > 128:
        return None
    if any(not isinstance(name, str) or not FEATURE_NAME.fullmatch(name) for name in raw):
        return None
    if len(set(raw)) != len(raw):
        return None
    return tuple(sorted(raw))


def _runtime_identity(root):
    root_doc = _toml(root / "Cargo.toml")
    runtime_doc = _toml(root / "runtime/Cargo.toml")
    package = runtime_doc.get("package", {})
    if package.get("name") != "nagi-runtime":
        return None
    version = package.get("version")
    if isinstance(version, dict) and version.get("workspace") is True:
        version = root_doc.get("workspace", {}).get("package", {}).get("version")
    if not isinstance(version, str) or not VERSION.fullmatch(version):
        return None
    return "nagi-runtime@" + version


def parse_build(raw, exit_code, root):
    """Project only the expected integration artifact and resolved rusqlite row."""
    if exit_code != 0 or len(raw) > MAX_BUILD_BYTES:
        return None, "build_failed"
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeError:
        return None, "invalid"
    target_manifest = (root / "runtime/Cargo.toml").resolve()
    target_artifacts = []
    sqlite_artifacts = []
    finished = []
    for line in text.splitlines():
        if len(line.encode("utf-8")) > MAX_LINE_BYTES:
            return None, "invalid"
        if not line.startswith("{"):
            continue
        try:
            message = json.loads(line)
        except (ValueError, TypeError):
            return None, "invalid"
        if not isinstance(message, dict):
            return None, "invalid"
        if message.get("reason") == "build-finished":
            finished.append(message.get("success"))
            continue
        if message.get("reason") != "compiler-artifact":
            continue
        target = message.get("target", {})
        if not isinstance(target, dict):
            continue
        name = target.get("name")
        kinds = target.get("kind", [])
        manifest = message.get("manifest_path")
        package_id = message.get("package_id")
        if name == TEST_TARGET and isinstance(kinds, list) and "test" in kinds:
            target_artifacts.append((message, target, manifest, package_id))
        if name == "rusqlite" and isinstance(kinds, list) and "lib" in kinds:
            features = _features(message.get("features"))
            version = _versioned_package_id(package_id)
            if features is None or version is None or not isinstance(manifest, str):
                return None, "invalid"
            try:
                sqlite_manifest = Path(manifest).resolve()
                sqlite_package = _toml(sqlite_manifest).get("package", {})
            except (OSError, tomllib.TOMLDecodeError):
                return None, "invalid"
            if sqlite_package.get("name") != "rusqlite" or sqlite_package.get("version") != version:
                return None, "invalid"
            sqlite_artifacts.append({
                "id_sha256": hashlib.sha256(package_id.encode("utf-8")).hexdigest(),
                "version": version,
                "features": features,
            })
    if finished != [True] or len(target_artifacts) != 1 or not sqlite_artifacts:
        return None, "invalid"
    message, target, manifest, package_id = target_artifacts[0]
    if (not isinstance(manifest, str) or Path(manifest).resolve() != target_manifest
            or target.get("test") is not True):
        return None, "invalid"
    executable = message.get("executable")
    if not isinstance(executable, str) or not executable:
        return None, "invalid"
    exe_path = Path(executable)
    try:
        resolved_exe = exe_path.resolve(strict=True)
        resolved_exe.relative_to((root / "target").resolve())
        if exe_path.is_symlink() or not resolved_exe.is_file():
            return None, "invalid"
        exe_bytes = resolved_exe.stat().st_size
    except (OSError, ValueError):
        return None, "invalid"
    if exe_bytes <= 0 or exe_bytes > MAX_EXE_BYTES:
        return None, "invalid"
    target_version = _versioned_package_id(package_id)
    runtime_label = _runtime_identity(root)
    if target_version is None or runtime_label != "nagi-runtime@" + target_version:
        return None, "invalid"
    sqlite_rows = {
        (row["id_sha256"], row["version"], row["features"])
        for row in sqlite_artifacts
    }
    if len(sqlite_rows) != 1:
        return None, "invalid"
    sqlite_id, sqlite_version, sqlite_features = next(iter(sqlite_rows))
    if not {"bundled", "hooks", "limits"}.issubset(sqlite_features):
        return None, "invalid"
    return {
        "executable": resolved_exe,
        "executable_bytes": exe_bytes,
        "package": runtime_label,
        "package_id_sha256": hashlib.sha256(package_id.encode("utf-8")).hexdigest(),
        "rusqlite_version": sqlite_version,
        "rusqlite_id_sha256": sqlite_id,
        "rusqlite_features": sqlite_features,
    }, "passed"


def parse_run(raw, expected):
    if len(raw) > MAX_TEST_BYTES:
        return None, "invalid"
    raw = raw.replace(b"\r\n", b"\n")
    if b"\r" in raw:
        return None, "invalid"
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeError:
        return None, "invalid"
    count = len(expected)
    filtered = len(ALL_TESTS) - count
    running = re.findall(r"(?m)^running ([0-9]{1,2}) tests?$", text)
    rows = re.findall(r"(?m)^test ([A-Za-z_][A-Za-z0-9_]*) \.\.\. (ok|FAILED|ignored)$", text)
    test_lines = [line for line in text.splitlines()
                  if line.startswith("test ") and not line.startswith("test result:")]
    summaries = re.findall(r"(?m)^test result: ([^\n]+)$", text)
    if any(re.match(r"^error(?:\[E[0-9]+\]|:)", line)
           for line in text.splitlines()):
        return None, "invalid"
    if (running != [str(count)] or len(rows) != count or len(test_lines) != count
            or {name for name, _ in rows} != set(expected)
            or len({name for name, _ in rows}) != count
            or any(status == "ignored" for _, status in rows)
            or len(summaries) != 1):
        return None, "invalid"
    failures = sum(status == "FAILED" for _, status in rows)
    if failures:
        pattern = (rf"FAILED\. {count-failures} passed; {failures} failed; 0 ignored; "
                   rf"0 measured; {filtered} filtered out; finished in [0-9.]+s")
        state = "test_failed"
    else:
        pattern = (rf"ok\. {count} passed; 0 failed; 0 ignored; 0 measured; "
                   rf"{filtered} filtered out; finished in [0-9.]+s")
        state = "passed"
    if not re.fullmatch(pattern, summaries[0]):
        return None, "invalid"
    by_name = dict(rows)
    return tuple((name, "failed" if by_name[name] == "FAILED" else "passed")
                 for name in expected), state


def project_diagnostics(raw, cases):
    raw = raw.replace(b"\r\n", b"\n")
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeError:
        return "unavailable", ()
    if _SUMMARY.BROWSER_COUNTER_SECRET_TAINT.search(text):
        return "unavailable", ()
    lines = [line for line in text.splitlines()
             if any(marker in line for marker in DIAGNOSTIC_FAMILY_MARKERS)]
    if not lines:
        return "absent", ()
    case_states = dict(cases)
    values = []
    seen = set()
    for line in lines:
        stock = STOCK_DIAGNOSTIC.fullmatch(line)
        exact = JSON_DIAGNOSTIC.fullmatch(line)
        if bool(stock) == bool(exact):
            return "unavailable", ()
        match = stock or exact
        family = "concurrent_once" if stock else "exact_bounds"
        test_name = DIAGNOSTIC_CASES[family]
        if test_name not in case_states or case_states[test_name] != "failed" or family in seen:
            return "unavailable", ()
        seen.add(family)
        sample = {"case": family}
        for name, raw_value in match.groupdict().items():
            if name in DIAGNOSTIC_RANGES:
                if len(raw_value) > len(str(DIAGNOSTIC_RANGES[name])):
                    return "unavailable", ()
                value = int(raw_value)
                if value > DIAGNOSTIC_RANGES[name]:
                    return "unavailable", ()
            else:
                value = raw_value == "true"
            sample[name] = value
        values.append(sample)
    return "present", tuple(values)


def _hash_file(path):
    path = Path(path)
    if path.is_symlink() or not path.is_file():
        return None
    try:
        size = path.stat().st_size
        if size <= 0 or size > MAX_EXE_BYTES:
            return None
        digest = hashlib.sha256()
        with path.open("rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(chunk)
        return digest.hexdigest()
    except OSError:
        return None


def _case_line(name, state):
    return "sf07_browser_case name=" + name + " state=" + state


def _diagnostic_line(sample):
    pieces = ["sf07_workspace_failure_diagnostic", "case=" + sample["case"]]
    for name, value in sample.items():
        if name == "case":
            continue
        rendered = ("true" if value else "false") if isinstance(value, bool) else str(value)
        pieces.append(name + "=" + rendered)
    return " ".join(pieces)


def _build(root):
    command = [os.environ.get("CARGO", "cargo"), *BUILD_COMMAND]
    exit_code, raw, capture_state = run_bounded(
        command, root, MAX_BUILD_BYTES, BUILD_TIMEOUT_SECONDS
    )
    if capture_state:
        print("sf07_workspace_build state=" + capture_state)
        return None
    info, state = parse_build(raw, exit_code, root)
    if state != "passed":
        suffix = "none" if exit_code is None else str(exit_code)
        print("sf07_workspace_build state=" + state + " cargo_exit=" + suffix)
        return None
    exe_hash = _hash_file(info["executable"])
    if exe_hash is None:
        print("sf07_workspace_build state=invalid")
        return None
    print("sf07_workspace_build state=passed package=" + info["package"]
          + " target=" + TEST_TARGET
          + " package_id_sha256=" + info["package_id_sha256"]
          + " executable_bytes=" + str(info["executable_bytes"])
          + " executable_sha256=" + exe_hash)
    print("sf07_workspace_rusqlite package=rusqlite@" + info["rusqlite_version"]
          + " package_id_sha256=" + info["rusqlite_id_sha256"]
          + " feature_count=" + str(len(info["rusqlite_features"]))
          + " features=" + ",".join(info["rusqlite_features"])
          + " feature_union=required")
    return info["executable"], exe_hash


def _run_profile(executable, baseline_hash, root, profile, expected, args):
    before_hash = _hash_file(executable)
    if before_hash is None:
        print("sf07_workspace_run profile=" + profile + " state=unavailable process_exit=none executable_unchanged=false cwd=runtime")
        return False
    command = [str(executable), *args]
    exit_code, raw, capture_state = run_bounded(
        command, root / "runtime", MAX_TEST_BYTES, None
    )
    after_hash = _hash_file(executable)
    unchanged = before_hash == baseline_hash == after_hash
    if capture_state:
        print("sf07_workspace_run profile=" + profile + " state=" + capture_state
              + " process_exit=" + ("none" if exit_code is None else str(exit_code))
              + " executable_unchanged=" + str(unchanged).lower() + " cwd=runtime")
        return False
    cases, state = parse_run(raw, expected)
    if cases is not None:
        if state == "passed" and exit_code != 0:
            state = "invalid"
            cases = None
        elif state == "test_failed" and (exit_code is None or exit_code == 0):
            state = "invalid"
            cases = None
    if not unchanged:
        state = "invalid"
        cases = None
    print("sf07_workspace_run profile=" + profile + " state=" + state
          + " process_exit=" + ("none" if exit_code is None else str(exit_code))
          + " executable_unchanged=" + str(unchanged).lower()
          + " executable_sha256=" + (after_hash or "unavailable") + " cwd=runtime")
    if cases is None:
        print("sf07_workspace_cases state=unavailable profile=" + profile)
        print("sf07_workspace_diagnostics state=unavailable profile=" + profile)
        return False
    print("sf07_workspace_cases state=complete profile=" + profile + " count=" + str(len(cases)))
    for name, case_state in cases:
        print(_case_line(name, case_state))
    diag_state, diagnostics = project_diagnostics(raw, cases)
    print("sf07_workspace_diagnostics state=" + diag_state + " profile=" + profile
          + " count=" + str(len(diagnostics)))
    for sample in diagnostics:
        print(_diagnostic_line(sample))
    return state == "passed"


def main():
    if not ROOT.is_dir():
        print("sf07_workspace_diagnostic state=invalid")
        return 1
    if any(name in os.environ for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUST_TEST_THREADS")):
        print("sf07_workspace_diagnostic state=unsupported_environment")
        return 1
    built = _build(ROOT)
    if built is None:
        return 1
    executable, baseline_hash = built
    list_code, list_raw, list_capture = run_bounded(
        [str(executable), "--list"], ROOT / "runtime", MAX_TEST_BYTES,
        CONTROL_TIMEOUT_SECONDS,
    )
    list_state = (_SUMMARY.classify_browser_counter_list(list_raw)
                  if list_capture is None and list_code == 0 else "invalid")
    print("sf07_workspace_inventory state=" + list_state
          + " process_exit=" + ("none" if list_code is None else str(list_code))
          + " executable_unchanged=" + str(_hash_file(executable) == baseline_hash).lower())
    if list_state != "passed" or _hash_file(executable) != baseline_hash:
        return 1
    profiles = (
        ("single-concurrent", (TARGET_TESTS[0],), (TARGET_TESTS[0], "--exact")),
        ("single-json-bounds", (TARGET_TESTS[1],), (TARGET_TESTS[1], "--exact")),
        ("parallel-ten", ALL_TESTS, ()),
    )
    passed = True
    for profile, expected, args in profiles:
        if not _run_profile(executable, baseline_hash, ROOT, profile, expected, args):
            passed = False
    print("sf07_workspace_diagnostic state=" + ("passed" if passed else "test_failed"))
    return 0 if passed else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception:
        print("sf07_workspace_diagnostic state=internal_error")
        raise SystemExit(1)
