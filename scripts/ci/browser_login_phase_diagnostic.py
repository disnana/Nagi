"""Sanitize one exact BrowserLogin diagnostic test run into closed JSON fields."""
from __future__ import annotations

import json
import pathlib
import re
import sys

CASE = "committed_claim_stays_consumed_on_verifier_error_panic_and_deadline"
COMMAND = (
    "cargo test --locked -p nagi-runtime --test browser_login_public_api "
    f"{CASE} -- --exact --nocapture"
)
RAW_LIMIT = 1024 * 1024
MAX_RECORDS = 10
U64_MAX = (1 << 64) - 1

# Field order is part of the private observer wire contract. Numeric values are
# checked against the closed enum dictionaries in the observer packet.
RECORD = re.compile(
    r"browser_login_phase_diag "
    r"request=(0|[1-9][0-9]{0,19}) "
    r"status=(504) "
    r"phase=(0|[1-9]|1[0-6]) "
    r"event=([0-2]) "
    r"before_gate=([0-4]) "
    r"poll_gate=([0-4]) "
    r"saw_pending=([01]) "
    r"pending_phase=(0|[1-9]|1[0-6]) "
    r"pending_gate=([0-4]) "
    r"saw_live_pending=([01]) "
    r"live_pending_phase=(0|[1-9]|1[0-6]) "
    r"gate_refused=([01]) "
    r"row_missing=([01]) "
    r"rollback_ack=([01]) "
    r"restore_commit_ack=([01]) "
    r"exit=(0|[1-9]) "
    r"exit_gate=([0-4]) "
    r"consumed=([01])\Z",
    re.ASCII,
)
CASE_ROW = re.compile(rf"test {re.escape(CASE)} \.\.\. (ok|FAILED)\Z", re.ASCII)
CASE_ROW_LIKE = re.compile(r"test [A-Za-z_][A-Za-z0-9_:]* \.\.\. (?:ok|FAILED|ignored)\Z", re.ASCII)
SUMMARY = {
    "passed": re.compile(
        r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; "
        r"9 filtered out; finished in [0-9]+(?:\.[0-9]+)?s\Z",
        re.ASCII,
    ),
    "failed": re.compile(
        r"test result: FAILED\. 0 passed; 1 failed; 0 ignored; 0 measured; "
        r"9 filtered out; finished in [0-9]+(?:\.[0-9]+)?s\Z",
        re.ASCII,
    ),
}
SUMMARY_PREFIX = "test result:"
FIELD_NAMES = (
    "request",
    "status",
    "phase",
    "event",
    "before_gate",
    "poll_gate",
    "saw_pending",
    "pending_phase",
    "pending_gate",
    "saw_live_pending",
    "live_pending_phase",
    "gate_refused",
    "row_missing",
    "rollback_ack",
    "restore_commit_ack",
    "exit",
    "exit_gate",
    "consumed",
)

# These exact markers are emitted by the selected public test only on a
# failure branch. Their phrases and booleans are closed source enums; no
# arbitrary text, panic payload, response material, path, or token is parsed.
MAX_PUBLIC_RECORDS = 15  # three fixed marker cases for each of five iterations
PUBLIC_BOOL = r"(?:true|false)"
PUBLIC_UINT = r"(?:0|[1-9][0-9]{0,4})"
PUBLIC_MARKERS = {
    "committed_claim_initial": (
        "initial_response",
        re.compile(
            r"browser_login_public_api_diag case=committed_claim_initial phase=initial_response "
            r"iteration=(?P<iteration>[0-4]) "
            r"expected_status=(?P<expected_status>401|500|504) "
            r"status=(?P<status>[1-5][0-9]{2}) "
            rf"auth_cookie=(?P<auth_cookie>{PUBLIC_BOOL}) "
            rf"deletion=(?P<deletion>{PUBLIC_BOOL}) "
            rf"verifier_calls=(?P<verifier_calls>{PUBLIC_UINT}) "
            rf"handler_calls=(?P<handler_calls>{PUBLIC_UINT}) "
            rf"claim_committed=(?P<claim_committed>{PUBLIC_BOOL})\Z",
            re.ASCII,
        ),
    ),
    "committed_claim_retry": (
        "retry_response",
        re.compile(
            r"browser_login_public_api_diag case=committed_claim_retry phase=retry_response "
            r"iteration=(?P<iteration>[0-4]) "
            r"status=(?P<status>[1-5][0-9]{2}) "
            rf"body_bytes=(?P<body_bytes>{PUBLIC_UINT}) "
            rf"set_cookie_count=(?P<set_cookie_count>{PUBLIC_UINT}) "
            rf"auth_cookie=(?P<auth_cookie>{PUBLIC_BOOL}) "
            rf"deletion=(?P<deletion>{PUBLIC_BOOL}) "
            rf"verifier_calls=(?P<verifier_calls>{PUBLIC_UINT}) "
            rf"handler_calls=(?P<handler_calls>{PUBLIC_UINT}) "
            rf"claim_committed=(?P<claim_committed>{PUBLIC_BOOL})\Z",
            re.ASCII,
        ),
    ),
    "committed_claim_replay_proof": (
        None,
        re.compile(
            r"browser_login_public_api_diag case=committed_claim_replay_proof "
            r"iteration=(?P<iteration>[0-4]) "
            rf"row_absent=(?P<row_absent>{PUBLIC_BOOL}) "
            rf"verifier_unchanged=(?P<verifier_unchanged>{PUBLIC_BOOL}) "
            rf"handler_unchanged=(?P<handler_unchanged>{PUBLIC_BOOL})\Z",
            re.ASCII,
        ),
    ),
}
PUBLIC_BOOL_FIELDS = {
    "auth_cookie",
    "deletion",
    "claim_committed",
    "row_absent",
    "verifier_unchanged",
    "handler_unchanged",
}

# Closed aggregate reasons explain a refused capture without retaining any raw
# line. Values saturate so a bounded input cannot create unbounded output.
INVALID_REASON_NAMES = (
    "input_refused",
    "observer_record_refused",
    "public_record_refused",
    "summary_refused",
    "case_row_refused",
    "summary_case_mismatch",
    "passing_run_summary_mismatch",
    "passing_run_observer_count_mismatch",
    "failed_run_pass_summary",
    "passing_run_public_markers",
)
INVALID_REASON_LIMIT = MAX_PUBLIC_RECORDS + 1


def note_invalid(document: dict, reason: str) -> None:
    counts = document["invalid_reason_counts"]
    counts[reason] = min(INVALID_REASON_LIMIT, counts[reason] + 1)


def parse_public_record(line: str) -> dict | None:
    for case, (phase, pattern) in PUBLIC_MARKERS.items():
        match = pattern.fullmatch(line)
        if match is None:
            continue
        raw_values = match.groupdict()
        record = {"case": case}
        if phase is not None:
            record["phase"] = phase
        for name, value in raw_values.items():
            record[name] = value == "true" if name in PUBLIC_BOOL_FIELDS else int(value)
        if not 0 <= record["iteration"] <= 4:
            return None
        if record.get("verifier_calls", 0) > 4 or record.get("handler_calls", 0) > 4:
            return None
        if record.get("body_bytes", 0) > 8192 or record.get("set_cookie_count", 0) > 16:
            return None
        return record
    return None


def empty_document(cargo_exit: int) -> dict:
    return {
        "schema": "nagi-browser-login-phase-diagnostic-v3",
        "case": CASE,
        "command": COMMAND,
        "cargo_exit": cargo_exit,
        "capture_valid": False,
        "test_summary": "unavailable",
        "observer_records_seen": 0,
        "invalid_reason_counts": {name: 0 for name in INVALID_REASON_NAMES},
        "records": [],
        "public_api_records": [],
    }


def sanitize(raw: bytes | None, cargo_exit: int) -> tuple[dict, int]:
    """Return only constants, closed numbers/booleans, and parsed records."""
    document = empty_document(cargo_exit)
    if raw is None:
        note_invalid(document, "input_refused")
        return document, 1
    if len(raw) > RAW_LIMIT:
        note_invalid(document, "input_refused")
        return document, 1

    try:
        text = raw.decode("utf-8", errors="replace")
    except Exception:
        note_invalid(document, "input_refused")
        return document, 1

    records: list[dict] = []
    public_records: list[dict] = []
    public_record_ids: set[tuple[str, int]] = set()
    request_ids: set[int] = set()
    summaries: list[str] = []
    case_rows: list[str] = []
    invalid = False
    for line in text.splitlines():
        if "browser_login_phase_diag" in line:
            document["observer_records_seen"] = min(
                MAX_RECORDS + 1, document["observer_records_seen"] + 1
            )
            match = RECORD.fullmatch(line)
            if match is None or len(records) >= MAX_RECORDS:
                note_invalid(document, "observer_record_refused")
                invalid = True
                continue
            values = [int(value) for value in match.groups()]
            if values[0] > U64_MAX or values[0] in request_ids:
                note_invalid(document, "observer_record_refused")
                invalid = True
                continue
            request_ids.add(values[0])
            records.append(dict(zip(FIELD_NAMES, values)))
            continue

        if "browser_login_public_api_diag" in line:
            public_record = parse_public_record(line)
            if public_record is None or len(public_records) >= MAX_PUBLIC_RECORDS:
                note_invalid(document, "public_record_refused")
                invalid = True
                continue
            record_id = (public_record["case"], public_record["iteration"])
            if record_id in public_record_ids:
                note_invalid(document, "public_record_refused")
                invalid = True
                continue
            public_record_ids.add(record_id)
            public_records.append(public_record)
            continue

        if line.startswith(SUMMARY_PREFIX):
            matched_status = next(
                (status for status, pattern in SUMMARY.items() if pattern.fullmatch(line)),
                None,
            )
            if matched_status is None:
                note_invalid(document, "summary_refused")
                invalid = True
            else:
                summaries.append(matched_status)
            continue

        if line.startswith("test ") and CASE_ROW_LIKE.fullmatch(line):
            match = CASE_ROW.fullmatch(line)
            if match is None:
                note_invalid(document, "case_row_refused")
                invalid = True
            else:
                case_rows.append(match.group(1))

    summary_status = summaries[0] if len(summaries) == 1 else None
    row_status = case_rows[0] if len(case_rows) == 1 else None
    if len(summaries) > 1:
        note_invalid(document, "summary_refused")
        invalid = True
    if len(case_rows) > 1:
        note_invalid(document, "case_row_refused")
        invalid = True
    if summary_status is None:
        if case_rows:
            note_invalid(document, "case_row_refused")
            invalid = True
    elif row_status != ("ok" if summary_status == "passed" else "FAILED"):
        note_invalid(document, "summary_case_mismatch")
        invalid = True

    # A passing run must show both known deadline cases. Failed runs retain any
    # valid records observed before the test stopped, up to its ten requests.
    if cargo_exit == 0:
        if summary_status != "passed":
            note_invalid(document, "passing_run_summary_mismatch")
            invalid = True
        if len(records) != 2:
            note_invalid(document, "passing_run_observer_count_mismatch")
            invalid = True
    elif summary_status == "passed":
        note_invalid(document, "failed_run_pass_summary")
        invalid = True
    if cargo_exit == 0 and public_records:
        # The public-test markers are printed only immediately before an
        # assertion failure in this exact source test.
        note_invalid(document, "passing_run_public_markers")
        invalid = True

    document["test_summary"] = summary_status or "unavailable"
    if not invalid:
        document["capture_valid"] = True
        document["records"] = records
        document["public_api_records"] = public_records
        return document, 0 if cargo_exit == 0 else 1

    # Drop all event records if any candidate record was malformed or the
    # summary/case identity did not match the exact fixed test contract.
    document["records"] = []
    document["public_api_records"] = []
    return document, 1


def closed_count_line(document: dict) -> str:
    unavailable = "browser_login_phase_counts unavailable"
    expected = {"schema", "case", "command", "cargo_exit", "capture_valid",
                "test_summary", "observer_records_seen", "invalid_reason_counts",
                "records", "public_api_records"}
    if type(document) is not dict or set(document) != expected:
        return unavailable
    seen = document["observer_records_seen"]
    counts = document["invalid_reason_counts"]
    if (type(seen) is not int or not 0 <= seen <= MAX_RECORDS + 1
            or type(counts) is not dict or set(counts) != set(INVALID_REASON_NAMES)):
        return unavailable
    if any(type(counts[name]) is not int or not 0 <= counts[name] <= INVALID_REASON_LIMIT
           for name in INVALID_REASON_NAMES):
        return unavailable
    return ("browser_login_phase_counts observer_records_seen=" + str(seen) + " "
            + " ".join(name + "=" + str(counts[name]) for name in INVALID_REASON_NAMES))


def main(argv: list[str]) -> int:
    if len(argv) != 4:
        return 2
    raw_path = pathlib.Path(argv[1])
    try:
        cargo_exit = int(argv[2], 10)
        if not 0 <= cargo_exit <= 255:
            return 2
    except ValueError:
        return 2
    output_path = pathlib.Path(argv[3])

    try:
        with raw_path.open("rb") as stream:
            raw = stream.read(RAW_LIMIT + 1)
    except OSError:
        raw = None

    document, result = sanitize(raw, cargo_exit)
    try:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(
            json.dumps(document, ensure_ascii=True, indent=2, sort_keys=True) + "\n",
            encoding="ascii",
        )
    except OSError:
        print("browser_login_phase_capture=write_failed", file=sys.stderr)
        return 1

    print(
        "browser_login_phase_capture="
        + ("valid" if document["capture_valid"] else "invalid")
        + " test_summary="
        + document["test_summary"]
        + " cargo_exit="
        + str(cargo_exit)
    )
    print(closed_count_line(document))
    return result


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
