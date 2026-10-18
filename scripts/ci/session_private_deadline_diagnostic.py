"""Finite private Session phase witness; never prints native output or errors."""
from __future__ import annotations

from collections import Counter
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import threading

_NATIVE_SPEC = importlib.util.spec_from_file_location(
    "session_native_finish_diagnostic", Path(__file__).with_name("session_native_finish_diagnostic.py"))
native_finish = importlib.util.module_from_spec(_NATIVE_SPEC)
_NATIVE_SPEC.loader.exec_module(native_finish)

PREFIX = "session_private_deadline_diag"
RAW_LIMIT = 256 * 1024
MARKER_LIMIT = 16
JSON_LIMIT = 16384
PRIVATE = "auth::session::http_bridge::tests::"
CALIBRATION = "http_server::private_deadline_calibration::"
# Tuple of exact source-owned name and required calibration terminal/event pairs.
# None means a functional test, whose existing native assertions remain the gate.
CASES = {
    "foreign_grant": (PRIVATE + "session_authorizer_receives_real_metadata_and_foreign_grant_is_refused", None),
    "head_finalizer": (PRIVATE + "session_head_mapper_panic_timeout_and_304_use_managed_finalizer", None),
    "prep_timer": (CALIBRATION + "preparation_timer", ((1, 1),)),
    "prep_late": (CALIBRATION + "preparation_late_ready", ((2, 2),)),
    "handler_timer": (CALIBRATION + "handler_timer", ((3, 1),)),
    "handler_late": (CALIBRATION + "handler_late_ready", ((4, 2),)),
    "positive_200": (CALIBRATION + "positive_200", ()),
    "positive_400": (CALIBRATION + "positive_400", ()),
    "positive_401": (CALIBRATION + "positive_401", ()),
    "positive_500": (CALIBRATION + "positive_500", ()),
    "body_408": (CALIBRATION + "body_408_is_separate", ()),
    "concurrent": (CALIBRATION + "concurrent_request_owners", ((1, 1), (3, 1))),
}
FIELDS = ("phase", "event", "db", "polls", "begins", "commands", "commits", "budget_ms",
          "entry_remaining_ms", "phase_remaining_ms", "exit_remaining_ms", "terminal")
MARKER = re.compile(re.escape(PREFIX) + r" status=504 " + " ".join(
    re.escape(field) + r"=(0|[1-9][0-9]{0,4})" for field in FIELDS) + r"\Z")
SUMMARY = re.compile(
    r"test result: (ok|FAILED)\. ([01]) passed; ([01]) failed; 0 ignored; 0 measured; "
    r"[0-9]+ filtered out; finished in [0-9]+\.[0-9]+s\Z")


def marker(line: str) -> dict | None:
    match = MARKER.fullmatch(line)
    if match is None:
        return None
    values = dict(zip(FIELDS, map(int, match.groups())))
    terminal = values["terminal"]
    if terminal not in (1, 2, 3, 4):
        return None
    if values["phase"] != (9 if terminal <= 2 else 6):
        return None
    if values["event"] not in (0, 1, 2) or values["db"] > 14:
        return None
    if any(values[key] > 1023 for key in ("polls", "begins", "commands", "commits")):
        return None
    if any(values[key] > 60000 for key in FIELDS[7:11]):
        return None
    if not (values["exit_remaining_ms"] <= values["phase_remaining_ms"]
            <= values["entry_remaining_ms"] <= values["budget_ms"]):
        return None
    if values["budget_ms"] == 0:
        return None
    return {"status": 504, **values}


def collect(case: str, raw: bytes, returncode: int, *, oversized: bool = False) -> dict:
    # Unknown identity is never echoed. Callers cannot enlarge the native suite.
    if case not in CASES:
        return {"case": "unknown", "state": "invalid", "reason": 1, "facts": [], "native_finish": []}
    result = {"case": case, "state": "invalid", "reason": 2, "facts": [], "native_finish": []}
    if oversized or len(raw) > RAW_LIMIT:
        return result
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeError:
        return result
    if "\x00" in text:
        return result
    lines = text.splitlines()
    name, expected = CASES[case]
    rows = [line for line in lines if line.startswith("test ") and " ... " in line]
    summaries = [SUMMARY.fullmatch(line) for line in lines if line.startswith("test result:")]
    if lines.count("running 1 test") != 1 or len(rows) != 1 or len(summaries) != 1:
        result["reason"] = 3
        return result
    summary = summaries[0]
    if summary is None:
        result["reason"] = 3
        return result
    outcome, passed, failed = summary.groups()
    good = outcome == "ok" and passed == "1" and failed == "0" and returncode == 0
    bad = outcome == "FAILED" and passed == "0" and failed == "1" and returncode > 0
    if not (good or bad) or rows != [f"test {name} ... {'ok' if good else 'FAILED'}"]:
        result["reason"] = 3
        return result
    facts = []
    native_facts = []
    seen = set()
    headings = [index for index, line in enumerate(lines)
                if line.startswith("---- ") and line.endswith(" stdout ----")]
    block_start = headings[0] if len(headings) == 1 and lines[headings[0]] == f"---- {name} stdout ----" else -1
    block_end = next((index for index in range(block_start + 1, len(lines))
                      if lines[index] in ("successes:", "failures:")
                      or lines[index].startswith("test result:")), len(lines)) if block_start >= 0 else -1
    for index, line in enumerate(lines):
        # Prefix embedded in an arbitrary tail, alternate shape, or truncation
        # is invalid rather than silently ignored or rendered.
        if native_finish.PREFIX in line:
            value = native_finish.parse(line)
            legacy = marker(lines[index - 1]) if index > 0 else None
            if (not block_start < index < block_end or value is None
                    or not native_finish.paired(value, legacy, 3)
                    or line in seen or len(native_facts) >= MARKER_LIMIT):
                result["reason"] = 4
                return result
            native_facts.append(value)
            seen.add(line)
            continue
        if PREFIX not in line:
            continue
        if not block_start < index < block_end:
            result["reason"] = 4
            return result
        value = marker(line)
        if value is None or line in seen or len(facts) >= MARKER_LIMIT:
            result["reason"] = 4
            return result
        facts.append(value)
        seen.add(line)
    if bad and not facts:
        result.update(state="unavailable", reason=5)
        return result
    if expected is not None:
        actual = Counter((item["terminal"], item["event"]) for item in facts)
        if actual != Counter(expected) or any(item["db"] != 0 or any(
                item[key] != 0 for key in ("begins", "commands", "commits")) for item in facts):
            result["reason"] = 6
            return result
    result.update(state="passed" if good else "tests_failed", reason=0, facts=facts, native_finish=native_facts)
    if len(json.dumps(result, separators=(",", ":")).encode()) > JSON_LIMIT:
        return {"case": case, "state": "invalid", "reason": 2, "facts": [], "native_finish": []}
    return result


def source_inventory(root: Path) -> bool:
    try:
        private = (root / "runtime/src/auth/session/http_bridge_tests.rs").read_text()
        calibration = (root / "runtime/src/http_server/private_deadline_calibration.rs").read_text()
        for name, _ in CASES.values():
            source = private if name.startswith(PRIVATE) else calibration
            suffix = name.rsplit("::", 1)[1]
            if len(re.findall(r"(?m)^async fn " + re.escape(suffix) + r"\(\) \{", source)) != 1:
                return False
        registered = set(re.findall(r"(?m)^async fn (\w+)\(\) \{", calibration))
        return registered == {name.removeprefix(CALIBRATION) for name, _ in CASES.values()
                              if name.startswith(CALIBRATION)}
    except (OSError, UnicodeError):
        return False


def capture(command: list[str], env: dict) -> tuple[bytes, int, bool]:
    # Drain stdout+stderr continuously, retaining at most RAW_LIMIT+1 bytes.
    # Raw captures remain memory-only and are never uploaded or printed.
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=env,
                               start_new_session=os.name != "nt")
    retained = bytearray()
    oversized = False

    def drain():
        nonlocal oversized
        assert process.stdout is not None
        with process.stdout:
            while chunk := process.stdout.read(8192):
                available = max(0, RAW_LIMIT + 1 - len(retained))
                retained.extend(chunk[:available])
                oversized |= len(retained) > RAW_LIMIT or len(chunk) > available

    reader = threading.Thread(target=drain, daemon=True)
    reader.start()
    try:
        code = process.wait(timeout=600)
    except subprocess.TimeoutExpired:
        if os.name == "nt":
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
        else:
            os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=10)
        code = -1
    reader.join(timeout=10)
    if reader.is_alive():
        oversized = True
    return bytes(retained), code, oversized


def native_command(name: str) -> list[str]:
    return ["cargo", "test", "--locked", "-p", "nagi-runtime", "--lib", name,
            "--", "--exact", "--show-output"]


def _prep_timer_pair_control_passes(raw: bytes, returncode: int, *, oversized: bool = False) -> bool:
    """Prove the swapped-handler mutant emitted one exact paired witness.

    This is a counterfactual check only: it repairs the two terminal fields in
    memory and asks the unchanged collector whether the corrected evidence
    passes. It never changes what the native test or collector accepts.
    """
    if oversized or len(raw) > RAW_LIMIT or returncode != 0:
        return False
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeError:
        return False
    if "\x00" in text or b"\r" in raw.replace(b"\r\n", b""):
        return False
    private_prefix = PREFIX.encode("ascii")
    native_prefix = native_finish.PREFIX.encode("ascii")
    if raw.count(private_prefix) != 1 or raw.count(native_prefix) != 1:
        return False
    # A private terminal and its native_finish projection are emitted as two
    # adjacent lines from the same Closed snapshot. Reject any extra terminal
    # observation before considering a repair.
    if raw.count(b" terminal=3") != 2:
        return False
    lines = text.splitlines()
    private_prefix_text = private_prefix.decode("ascii")
    native_prefix_text = native_prefix.decode("ascii")
    private_positions = [i for i, line in enumerate(lines) if private_prefix_text in line]
    native_positions = [i for i, line in enumerate(lines) if native_prefix_text in line]
    if len(private_positions) != 1 or len(native_positions) != 1:
        return False
    private_index, native_index = private_positions[0], native_positions[0]
    if native_index != private_index + 1:
        return False
    # The source mutation changes only the enum label from
    # PreparationTimer (1) to HandlerTimer (3). Keep the preparation phase and
    # validate the legacy record after the one intended enum-field repair.
    legacy_line = lines[private_index]
    if legacy_line.count(" terminal=3") != 1:
        return False
    legacy = marker(legacy_line.replace(" terminal=3", " terminal=1", 1))
    native = native_finish.parse(lines[native_index])
    if legacy is None or native is None or (legacy["terminal"], legacy["event"]) != (1, 1):
        return False
    # The native record must still bind to the emitted wrong enum value while
    # sharing the exact request counters validated in the repaired legacy row.
    paired_legacy = dict(legacy, terminal=3)
    if not native_finish.paired(native, paired_legacy, 3):
        return False

    # Preserve LF/CRLF bytes and touch only the two validated terminal fields.
    raw_lines = raw.splitlines(keepends=True)
    if len(raw_lines) != len(lines):
        return False
    for index in (private_index, native_index):
        if raw_lines[index].count(b" terminal=3") != 1:
            return False
        raw_lines[index] = raw_lines[index].replace(b" terminal=3", b" terminal=1", 1)
    repaired = b"".join(raw_lines)
    return collect("prep_timer", repaired, returncode)["state"] == "passed"


def wiring_controls(root: Path, env: dict) -> bool:
    """CI-only owned-checkout mutations; restore exact bytes even on failure."""
    path = root / "runtime/src/http_server.rs"
    original = path.read_bytes()
    digest = hashlib.sha256(original).hexdigest()
    anchor = (b"observer.emit_private(crate::auth::session_deadline_observer::"
              b"PrivateTerminal::PreparationTimer);")
    # This frozen digest is generated from the immutable reviewed candidate.
    expected_digest = "e4af47720616a8f6818a47d2cb35408004cf35f3be1f4e6f71567e6318efca31"
    canonical = original.replace(b"\r\n", b"\n")
    if (b"\r" in canonical or hashlib.sha256(canonical).hexdigest() != expected_digest
            or original.count(anchor) != 1):
        print('session_private_deadline_control {"state":"invalid","reason":1}')
        return False
    variants = {
        "prep_emit_removed": b"let _ = observer;",
        "prep_handler_swapped": anchor.replace(b"PreparationTimer", b"HandlerTimer"),
    }
    passed = True
    try:
        for control, replacement in variants.items():
            path.write_bytes(original.replace(anchor, replacement))
            raw, code, oversized = capture(native_command(CASES["prep_timer"][0]), env)
            result = collect("prep_timer", raw, code, oversized=oversized)
            if control == "prep_emit_removed":
                valid_red = code == 0 and result["reason"] == 6
                # The passing exact native test must have no private witness.
                valid_red &= PREFIX.encode() not in raw
            else:
                valid_red = _prep_timer_pair_control_passes(raw, code, oversized=oversized)
            passed &= valid_red
            print("session_private_deadline_control " + json.dumps({
                "control": control, "state": "expected_red" if valid_red else "invalid"},
                separators=(",", ":")))
    except (OSError, ValueError, subprocess.SubprocessError):
        passed = False
        print('session_private_deadline_control {"state":"invalid","reason":2}')
    finally:
        # Restoration failure propagates and cannot turn into a passing check.
        path.write_bytes(original)
        restored = hashlib.sha256(path.read_bytes()).hexdigest() == digest
        passed &= restored
        print("session_private_deadline_control " + json.dumps({
            "control": "source_restored", "state": "passed" if restored else "invalid"},
            separators=(",", ":")))
    return passed


def main() -> int:
    if sys.argv[1:] != ["--run"]:
        print('session_private_deadline_summary {"state":"invalid","reason":9}')
        return 1
    root = Path.cwd()
    if not source_inventory(root):
        print('session_private_deadline_summary {"state":"invalid","reason":7}')
        return 1
    env = dict(os.environ)
    env.pop("ICU4X_DATA_DIR", None)
    env["CARGO_TERM_COLOR"] = "never"
    env["RUSTFLAGS"] = (env.get("RUSTFLAGS", "") + " --cfg nagi_auth_stage_diag").strip()
    failed = False
    calibration_passed = True
    for case, (name, _) in CASES.items():
        try:
            raw, code, oversized = capture(native_command(name), env)
            result = collect(case, raw, code, oversized=oversized)
        except (OSError, ValueError, subprocess.SubprocessError):
            result = {"case": case, "state": "unavailable", "reason":8, "facts": [], "native_finish": []}
        failed |= result["state"] != "passed"
        if name.startswith(CALIBRATION):
            calibration_passed &= result["state"] == "passed"
        # Only fixed identities and validated bounded numeric facts reach logs.
        print("session_private_deadline_summary " + json.dumps(result, separators=(",", ":")))
    if calibration_passed:
        try:
            failed |= not wiring_controls(root, env)
        except OSError:
            print('session_private_deadline_control {"state":"invalid","reason":4}')
            failed = True
    else:
        print('session_private_deadline_control {"state":"unavailable","reason":3}')
        failed = True
    return int(failed)


if __name__ == "__main__":
    raise SystemExit(main())
