"""Strict sanitizer for opt-in Session and BrowserLogin public-test failures."""
from __future__ import annotations

import json
import pathlib
import re
import sys

import auth_stage_diagnostic as auth_diag

RAW_LIMIT = 256 * 1024
OUTPUT_LIMIT = 16 * 1024
MARKER_LIMIT = 16

SESSION_CASES = frozenset({
    "controlled_same_file_writer_reports_native_token_identity_boundary",
    "checked_profiles_reject_unsealed_or_incompatible_policy_sources",
    "tokens_are_authenticated_reusable_and_preserve_bounded_ledger",
    "guarded_rotation_and_logout_invalidate_old_generation_tokens",
    "body_checkpoint_rejects_generation_changed_after_actual_authorization",
    "finite_origin_and_header_refusals_preserve_existing_token_and_capacity",
    "one_reusable_token_admits_two_distinct_actual_requests",
    "guarded_partial_body_preparation_uses_original_security_deadline",
    "failed_guarded_handler_never_finalizes_committed_rotation_cookie",
    "mismatched_startup_origin_refuses_token_and_guarded_routes_before_bind",
    "pending_guarded_authorizer_expires_at_bound_session_and_recovers_permit",
})
BROWSER_CASES = frozenset({
    "public_login_claims_once_then_named_verifier_issue_and_appended_cookies",
    "input_and_strict_json_rejections_do_not_consume_or_call_verifier",
    "committed_claim_stays_consumed_on_verifier_error_panic_and_deadline",
    "issue_commit_then_handler_err_panic_or_timeout_never_delivers_auth_cookie",
    "wire_body_cap_is_independent_of_db_quota_and_checked_before_claim",
    "browser_profile_refuses_non_post_and_wrong_startup_origin",
    "body_own_deadline_408_security_earlier_or_equal_504_before_claim",
    "shutdown_cancels_actual_request_and_committed_token_cannot_return",
    "closed_native_pool_refuses_before_verifier_and_session_capacity_never_redelivers",
    "json_value_nodes_and_container_depth_accept_exact_bounds",
})
SUITES = {"session": SESSION_CASES, "browser": BROWSER_CASES}

IDENT = r"[A-Za-z_][A-Za-z0-9_]*"
ROW = re.compile(rf"^test ({IDENT}) \.\.\. (ok|FAILED|ignored)$")
RUNNING = re.compile(r"^running (0|[1-9][0-9]{0,3}) tests?$", re.M)
SUMMARY = re.compile(
    r"^test result: (ok|FAILED)\. (0|[1-9][0-9]{0,3}) passed; "
    r"(0|[1-9][0-9]{0,3}) failed; (0|[1-9][0-9]{0,3}) ignored; "
    r"[0-9]+ measured; [0-9]+ filtered out; finished in [0-9.]+s$",
    re.M,
)
STDOUT_HEADER = re.compile(rf"---- ({IDENT}) stdout ----\Z")
DEC = r"(?:0|[1-9][0-9]{0,4})"
STATUS = r"(?:[1-5][0-9]{2})"
BOOL = r"(?:true|false)"
BROWSER_PHASE_PREFIX = "browser_login_phase_diag"
SESSION_PREFIX = "session_deadline_diag"
BOOTSTRAP_DEADLINE_PREFIX = "prelogin_bootstrap_deadline_diag"
BOOTSTRAP_PREFIX = "browser_login_bootstrap_diag"
PAIR_PREFIX = "browser_login_stock_cause"
JSON_BOUND_PREFIX = "browser_login_json_bound_diag"
CLAIM_MARKER_TEST = "committed_claim_stays_consumed_on_verifier_error_panic_and_deadline"
CLAIM_VARIANT_HINT = "browser_login_public_api_diag case=committed_claim_"
CLAIM_COUNT = r"(?:0|[1-9][0-9]{0,3})"
CLAIM_INITIAL_MARKER = re.compile(
    rf"browser_login_public_api_diag case=committed_claim_initial phase=initial_response "
    rf"iteration=([0-4]) expected_status=(401|500|504) status=({STATUS}) "
    rf"auth_cookie=({BOOL}) deletion=({BOOL}) verifier_calls=({CLAIM_COUNT}) "
    rf"handler_calls=({CLAIM_COUNT}) claim_committed=({BOOL})\Z"
)
CLAIM_RETRY_MARKER = re.compile(
    rf"browser_login_public_api_diag case=committed_claim_retry phase=retry_response "
    rf"iteration=([0-4]) status=({STATUS}) body_bytes=({CLAIM_COUNT}) "
    rf"set_cookie_count=({CLAIM_COUNT}) auth_cookie=({BOOL}) deletion=({BOOL}) "
    rf"verifier_calls=({CLAIM_COUNT}) handler_calls=({CLAIM_COUNT}) claim_committed=({BOOL})\Z"
)
CLAIM_REPLAY_MARKER = re.compile(
    rf"browser_login_public_api_diag case=committed_claim_replay_proof "
    rf"iteration=([0-4]) row_absent=({BOOL}) verifier_unchanged=({BOOL}) "
    rf"handler_unchanged=({BOOL})\Z"
)
SESSION_MARKER = re.compile(
    r"session_deadline_diag status=504 phase=(0|[1-9][0-9]?) "
    r"event=(0|[1-9][0-9]?) db=(0|[1-9][0-9]?) exit=(0|[1-9][0-9]?) "
    r"polls=(0|[1-9][0-9]{0,3}) begins=(0|[1-9][0-9]{0,3}) "
    r"commands=(0|[1-9][0-9]{0,3}) commits=(0|[1-9][0-9]{0,3}) "
    r"budget_ms=(0|[1-9][0-9]{0,4}) entry_remaining_ms=(0|[1-9][0-9]{0,4}) "
    r"phase_remaining_ms=(0|[1-9][0-9]{0,4}) exit_remaining_ms=(0|[1-9][0-9]{0,4})\Z"
)
BOOTSTRAP_DEADLINE_MARKER = re.compile(
    SESSION_MARKER.pattern.replace(SESSION_PREFIX, BOOTSTRAP_DEADLINE_PREFIX).removesuffix(r"\Z")
    + r" security_budget_ms=(0|[1-9][0-9]{0,4}) selected_end_kind=([01])\Z"
)
BOOTSTRAP_MARKER = re.compile(
    rf"browser_login_bootstrap_diag check=material status=({STATUS}) "
    r"class=(request|refusal|internal|unavailable|deadline|other) "
    r"set_cookie_count=(0|[1-9][0-9]?) body_bytes=(0|[1-9][0-9]{0,3}) "
    rf"auth_cookie=({BOOL}) deletion=({BOOL})\Z"
)
PAIR_MARKER = re.compile(
    rf"browser_login_stock_cause case=concurrent_once a_status=({STATUS}) b_status=({STATUS}) "
    rf"a_auth_cookie=({BOOL}) b_auth_cookie=({BOOL}) a_deletion=({BOOL}) b_deletion=({BOOL}) "
    r"verifier_calls=(0|[1-9][0-9]{0,3}) handler_calls=(0|[1-9][0-9]{0,3}) "
    rf"claim_committed=({BOOL})\Z"
)
JSON_BOUND_MARKER = re.compile(
    rf"browser_login_json_bound_diag phase=exact_bounds status=({STATUS}) "
    r"body_bytes=(0|[1-9][0-9]{0,3}) expected_body_bytes=(0|[1-9][0-9]{0,3}) "
    rf"body_matches=({BOOL}) verifier_calls=(0|[1-9][0-9]{{0,3}}) "
    rf"handler_calls=(0|[1-9][0-9]{{0,3}}) claim_committed=({BOOL})\Z"
)
AUTH_PUBLIC_LINE = re.compile(
    rf"session_reply=test=({IDENT}) check=(cookie_count|token_response|token_cookie) "
    r"status=([1-5][0-9]{2}) class=(success|invalid_request|invalid_or_expired|denied|internal|unavailable|deadline|other) "
    r"set_cookie_count=(0|[1-9][0-9]{0,4}) body_bytes=(0|[1-9][0-9]{0,4})\Z"
)
AUTH_PUBLIC_UNAVAILABLE = re.compile(rf"session_reply=test=({IDENT}) unavailable\Z")
AUTH_PUBLIC_GLOBAL_UNAVAILABLE = re.compile(r"session_reply=unavailable\Z")
AUTH_WIRE_LINE = re.compile(
    rf"browser_wire=test=({IDENT}) case=wire_body_cap phase=exact_body_response "
    r"iteration=([01]) status=([1-5][0-9]{2}) body_bytes=(0|[1-9][0-9]{0,3}) "
    r"expected_body_bytes=(192) body_matches=(true|false) "
    r"set_cookie_count=(0|[1-9][0-9]{0,4}) auth_cookie=(true|false) deletion=(true|false)\Z"
)
AUTH_WIRE_UNAVAILABLE = re.compile(rf"browser_wire=test=({IDENT}) unavailable\Z")
HTTP_IO_LINE = re.compile(rf"http_test_io=phase=({auth_diag.IO_PHASE}) kind=({auth_diag.IO_KIND})\Z")
KNOWN_MARKER_PREFIXES = (
    BROWSER_PHASE_PREFIX,
    SESSION_PREFIX,
    BOOTSTRAP_DEADLINE_PREFIX,
    BOOTSTRAP_PREFIX,
    PAIR_PREFIX,
    JSON_BOUND_PREFIX,
    auth_diag.SAFE_REPLY_PREFIX,
    auth_diag.WIRE_BODY_PREFIX,
    auth_diag.TEST_IO_MARKER_PREFIX,
    auth_diag.HTTP_STALLED_PREFIX,
)


def _status_class(status: int) -> str:
    if 200 <= status <= 299:
        return "success"
    return {
        400: "invalid_request",
        401: "invalid_or_expired",
        403: "denied",
        500: "internal",
        503: "unavailable",
        504: "deadline",
    }.get(status, "other")


# Exact existing browser_login_observer.rs Display order and private enum ranges.
BROWSER_PHASE_FIELDS = (
    ("request", 18446744073709551615), ("status", 504),
    ("phase", 16), ("event", 2), ("before_gate", 4), ("poll_gate", 4),
    ("saw_pending", 1), ("pending_phase", 16), ("pending_gate", 4),
    ("saw_live_pending", 1), ("live_pending_phase", 16), ("gate_refused", 1),
    ("row_missing", 1), ("rollback_ack", 1), ("restore_commit_ack", 1),
    ("exit", 9), ("exit_gate", 4), ("consumed", 1),
)
BROWSER_PHASE_MARKER = re.compile(BROWSER_PHASE_PREFIX + " " + " ".join(
    rf"{name}=(0|[1-9][0-9]{{0,19}})" for name, _ in BROWSER_PHASE_FIELDS
) + r"\Z")


def parse_browser_phase_marker(line: str) -> dict | None:
    match = BROWSER_PHASE_MARKER.fullmatch(line)
    if match is None:
        return None
    values = {name: int(value) for (name, _), value in zip(BROWSER_PHASE_FIELDS, match.groups())}
    if values["status"] != 504 or any(values[name] > limit for name, limit in BROWSER_PHASE_FIELDS):
        return None
    return values


def parse_session_marker(line: str) -> dict | None:
    match = SESSION_MARKER.fullmatch(line)
    if match is None:
        return None
    keys = (
        "phase", "event", "db", "exit", "polls", "begins", "commands", "commits",
        "budget_ms", "entry_remaining_ms", "phase_remaining_ms", "exit_remaining_ms",
    )
    values = dict(zip(keys, map(int, match.groups())))
    if (values["phase"] > 4 or values["event"] > 2 or values["db"] > 14 or
            values["exit"] > 4 or max(values[k] for k in ("polls", "begins", "commands", "commits")) > 1023 or
            max(values[k] for k in ("budget_ms", "entry_remaining_ms", "phase_remaining_ms", "exit_remaining_ms")) > 60000 or
            values["commits"] > values["begins"] or
            not (values["exit_remaining_ms"] <= values["phase_remaining_ms"] <=
                 values["entry_remaining_ms"] <= values["budget_ms"]) or
            (values["exit"] in (1, 2) and values["exit_remaining_ms"] != 0)):
        return None
    return {"status": 504, **values}


def parse_bootstrap_marker(line: str) -> dict | None:
    match = BOOTSTRAP_MARKER.fullmatch(line)
    if match is None:
        return None
    status, classification, cookie_text, body_text, auth_cookie, deletion = match.groups()
    status, cookie_count, body_bytes = map(int, (status, cookie_text, body_text))
    expected = {
        400: "request", 401: "refusal", 403: "refusal", 500: "internal",
        503: "unavailable", 504: "deadline",
    }.get(status, "other")
    if status == 200 or classification != expected or cookie_count > 65 or body_bytes > 8193:
        return None
    return {
        "check": "material", "status": status, "class": classification,
        "set_cookie_count": cookie_count, "body_bytes": body_bytes,
        "auth_cookie": auth_cookie == "true", "deletion": deletion == "true",
    }


def parse_bootstrap_deadline_marker(line: str) -> dict | None:
    match = BOOTSTRAP_DEADLINE_MARKER.fullmatch(line)
    if match is None:
        return None
    keys = (
        "phase", "event", "db", "exit", "polls", "begins", "commands", "commits",
        "budget_ms", "entry_remaining_ms", "phase_remaining_ms", "exit_remaining_ms",
        "security_budget_ms", "selected_end_kind",
    )
    values = dict(zip(keys, map(int, match.groups())))
    if (not 5 <= values["phase"] <= 8 or values["event"] > 2 or values["db"] > 14 or
            values["exit"] > 4 or
            max(values[k] for k in ("polls", "begins", "commands", "commits")) > 1023 or
            max(values[k] for k in ("budget_ms", "entry_remaining_ms", "phase_remaining_ms",
                                  "exit_remaining_ms", "security_budget_ms")) > 60000 or
            values["commits"] > values["begins"] or
            not (values["exit_remaining_ms"] <= values["phase_remaining_ms"] <=
                 values["entry_remaining_ms"] <= values["budget_ms"] <= values["security_budget_ms"]) or
            (values["exit"] in (1, 2) and values["exit_remaining_ms"] != 0) or
            (values["selected_end_kind"] == 0 and values["budget_ms"] != values["security_budget_ms"])):
        return None
    # Kind 1 can have equal rounded/clamped ms values; it compares native Instants.
    # This is a request snapshot in a failed test, not proof of that assertion's cause.
    return {"status": 504, **values}


def parse_pair_marker(line: str) -> dict | None:
    match = PAIR_MARKER.fullmatch(line)
    if match is None:
        return None
    (a_status, b_status, a_auth, b_auth, a_delete, b_delete,
     verifier, handler, committed) = match.groups()
    a_status, b_status, verifier, handler = map(int, (a_status, b_status, verifier, handler))
    if {a_status, b_status} == {200, 403} or max(verifier, handler) > 1023:
        return None
    return {
        "case": "concurrent_once", "a_status": a_status, "b_status": b_status,
        "a_auth_cookie": a_auth == "true", "b_auth_cookie": b_auth == "true",
        "a_deletion": a_delete == "true", "b_deletion": b_delete == "true",
        "verifier_calls": verifier, "handler_calls": handler,
        "claim_committed": committed == "true",
    }


def parse_json_bound_marker(line: str) -> dict | None:
    match = JSON_BOUND_MARKER.fullmatch(line)
    if match is None:
        return None
    status, body_text, expected_text, matches, verifier_text, handler_text, committed = match.groups()
    status, body_bytes, expected_bytes, verifier, handler = map(
        int, (status, body_text, expected_text, verifier_text, handler_text)
    )
    if (body_bytes > 8193 or expected_bytes not in (9, 31) or
            verifier > 1023 or handler > 1023 or
            (matches == "true" and body_bytes != expected_bytes)):
        return None
    return {
        "phase": "exact_bounds", "status": status, "body_bytes": body_bytes,
        "expected_body_bytes": expected_bytes, "body_matches": matches == "true",
        "verifier_calls": verifier, "handler_calls": handler,
        "claim_committed": committed == "true",
    }


def parse_claim_marker(line: str) -> tuple[str, dict] | None:
    """Parse only the three fixed committed-claim witnesses from the Rust test."""
    match = CLAIM_INITIAL_MARKER.fullmatch(line)
    if match is not None:
        (iteration_text, expected_text, status_text, auth_cookie, deletion,
         verifier_text, handler_text, committed) = match.groups()
        iteration, expected, status, verifier, handler = map(
            int, (iteration_text, expected_text, status_text, verifier_text, handler_text)
        )
        expected_by_iteration = (401, 500, 504, 504, 401)
        if (expected != expected_by_iteration[iteration] or verifier > 1023 or handler > 1023 or
                (status == expected and auth_cookie == "false" and deletion == "true")):
            return None
        return "claim_initial", {
            "case": "committed_claim_initial", "phase": "initial_response",
            "iteration": iteration, "expected_status": expected, "status": status,
            "auth_cookie": auth_cookie == "true", "deletion": deletion == "true",
            "verifier_calls": verifier, "handler_calls": handler,
            "claim_committed": committed == "true",
        }

    match = CLAIM_RETRY_MARKER.fullmatch(line)
    if match is not None:
        (iteration_text, status_text, body_text, cookie_text, auth_cookie,
         deletion, verifier_text, handler_text, committed) = match.groups()
        iteration, status, body_bytes, cookie_count, verifier, handler = map(
            int, (iteration_text, status_text, body_text, cookie_text, verifier_text, handler_text)
        )
        # request() caps the complete wire reply at 8192 bytes before constructing Reply.
        if (body_bytes > 8192 or cookie_count > 8192 or verifier > 1023 or handler > 1023 or
                (status == 403 and auth_cookie == "false" and deletion == "false")):
            return None
        return "claim_retry", {
            "case": "committed_claim_retry", "phase": "retry_response",
            "iteration": iteration, "status": status, "body_bytes": body_bytes,
            "set_cookie_count": cookie_count, "auth_cookie": auth_cookie == "true",
            "deletion": deletion == "true", "verifier_calls": verifier,
            "handler_calls": handler, "claim_committed": committed == "true",
        }

    match = CLAIM_REPLAY_MARKER.fullmatch(line)
    if match is not None:
        iteration_text, row_absent, verifier_unchanged, handler_unchanged = match.groups()
        return "claim_replay_proof", {
            "case": "committed_claim_replay_proof", "iteration": int(iteration_text),
            "row_absent": row_absent == "true",
            "verifier_unchanged": verifier_unchanged == "true",
            "handler_unchanged": handler_unchanged == "true",
        }
    return None


def _read_bounded(path: pathlib.Path) -> tuple[bytes | None, str]:
    try:
        with path.open("rb") as stream:
            raw = stream.read(RAW_LIMIT + 1)
    except OSError:
        return None, "missing"
    if len(raw) > RAW_LIMIT:
        return None, "oversize"
    try:
        raw.decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        return None, "invalid_utf8"
    return raw, "valid"


def _suite_structure(name: str, raw: bytes | None, input_status: str, exit_code: int) -> tuple[dict, str | None]:
    expected = SUITES[name]
    record = {
        "cargo_exit": exit_code if isinstance(exit_code, int) and 0 <= exit_code <= 255 else None,
        "input_status": input_status,
        "inventory_status": "unavailable",
        "running_count": None,
        "passed_count": None,
        "failed_count": None,
        "ignored_count": None,
        "failed_cases": [],
    }
    if raw is not None and len(raw) > RAW_LIMIT:
        input_status = "oversize"
        raw = None
        record["input_status"] = input_status
    if input_status != "valid":
        raw = None
    if raw is None or record["cargo_exit"] is None:
        record["inventory_status"] = "input_unavailable" if raw is None else "invalid_exit_code"
        return record, None
    try:
        text = raw.decode("utf-8", errors="strict").replace("\r\n", "\n")
    except UnicodeDecodeError:
        record["input_status"] = "invalid_utf8"
        record["inventory_status"] = "input_unavailable"
        return record, None
    running = RUNNING.findall(text)
    summaries = SUMMARY.findall(text)
    rows = []
    malformed_test_row = False
    for line in text.splitlines():
        match = ROW.fullmatch(line)
        if match:
            rows.append(match.groups())
        elif line.startswith("test ") and " ... " in line:
            malformed_test_row = True
    stdout_names = [m.group(1) for line in text.splitlines() if (m := STDOUT_HEADER.fullmatch(line))]
    if len(running) != 1:
        record["inventory_status"] = (
            "no_test_execution" if not running and not rows and not summaries and record["cargo_exit"] != 0
            else "zero_tests" if running == ["0"] else "invalid_running_count"
        )
        return record, text
    running_count = int(running[0])
    record["running_count"] = running_count
    if running_count == 0:
        record["inventory_status"] = "zero_tests"
        return record, text
    if len(summaries) != 1 or malformed_test_row:
        record["inventory_status"] = "incomplete"
        return record, text
    summary_kind, passed_text, failed_text, ignored_text = summaries[0]
    passed, failed, ignored = map(int, (passed_text, failed_text, ignored_text))
    record.update({"passed_count": passed, "failed_count": failed, "ignored_count": ignored})
    names = [name for name, _ in rows]
    row_statuses = [status for _, status in rows]
    # Test output is public JSON only for the fixed source inventory. Unknown
    # runner/user-controlled test identifiers must never enter sanitized facts.
    failed_cases = sorted(name for name, status in rows if status == "FAILED" and name in expected)
    record["failed_cases"] = failed_cases
    if (running_count != len(expected) or len(rows) != running_count or
            len(set(names)) != len(names) or set(names) != expected or
            any(header not in expected for header in stdout_names) or
            len(set(stdout_names)) != len(stdout_names) or
            passed != row_statuses.count("ok") or failed != row_statuses.count("FAILED") or
            ignored != row_statuses.count("ignored") or passed + failed + ignored != running_count or
            ((summary_kind == "ok") != (failed == 0))):
        record["inventory_status"] = "inventory_invalid"
        return record, text
    if ignored:
        record["inventory_status"] = "ignored_tests"
        return record, text
    if failed:
        record["inventory_status"] = "failed" if record["cargo_exit"] != 0 else "invalid_zero_exit_failure"
        return record, text
    if record["cargo_exit"] != 0:
        record["inventory_status"] = "nonzero_after_passing_tests"
        return record, text
    record["inventory_status"] = "passed"
    return record, text


def _auth_facts(text: str, exit_code: int, cases: dict[str, str]) -> tuple[list[dict], bool]:
    # These three closed BrowserLogin witnesses are parsed here, then removed from
    # the older parser input whose public marker grammar intentionally covers only wire_body_cap.
    auth_text = "\n".join(line for line in text.splitlines() if CLAIM_VARIANT_HINT not in line)
    lines, invalid = auth_diag.public_test_diagnostics(auth_text, exit_code, cases)
    facts = []
    for line in lines:
        if match := AUTH_PUBLIC_LINE.fullmatch(line):
            test, check, status, classification, cookies, body = match.groups()
            if classification != _status_class(int(status)):
                invalid = True
                continue
            facts.append({
                "kind": "session_reply", "test": test, "check": check,
                "status": int(status), "class": classification,
                "set_cookie_count": int(cookies), "body_bytes": int(body),
            })
        elif match := AUTH_PUBLIC_UNAVAILABLE.fullmatch(line):
            facts.append({"kind": "session_reply", "test": match.group(1), "availability": "unavailable"})
        elif AUTH_PUBLIC_GLOBAL_UNAVAILABLE.fullmatch(line):
            facts.append({"kind": "session_reply", "availability": "unavailable"})
        elif match := AUTH_WIRE_LINE.fullmatch(line):
            test, iteration, status, body, expected, matches, cookies, auth_cookie, deletion = match.groups()
            facts.append({
                "kind": "browser_wire", "test": test, "case": "wire_body_cap",
                "phase": "exact_body_response", "iteration": int(iteration),
                "status": int(status), "body_bytes": int(body),
                "expected_body_bytes": int(expected), "body_matches": matches == "true",
                "set_cookie_count": int(cookies), "auth_cookie": auth_cookie == "true",
                "deletion": deletion == "true",
            })
        elif match := AUTH_WIRE_UNAVAILABLE.fullmatch(line):
            facts.append({"kind": "browser_wire", "test": match.group(1), "availability": "unavailable"})
        else:
            invalid = True
    return facts, invalid


def _test_output_blocks(text: str) -> tuple[dict[str, str], bool]:
    blocks: dict[str, list[str]] = {}
    current: str | None = None
    invalid = False
    for line in text.splitlines():
        header = STDOUT_HEADER.fullmatch(line)
        if header:
            current = header.group(1)
            if current in blocks:
                invalid = True
            blocks.setdefault(current, [])
            continue
        if line == "failures:" or line.startswith("test result:"):
            current = None
            continue
        if current is not None:
            blocks[current].append(line)
    return {name: "\n".join(lines) for name, lines in blocks.items()}, invalid


def _marker_facts(suite_name: str, text: str, exit_code: int, failed_cases: set[str],
                  cases: dict[str, str]) -> tuple[dict, int, bool]:
    facts = {
        "session_deadline": [], "bootstrap": [], "pair": [], "json_bound": [],
        "bootstrap_deadline": [], "browser_phase": [],
        "claim_initial": [], "claim_retry": [], "claim_replay_proof": [],
        "public": [], "http_test_io": [],
    }
    invalid = False
    blocks, bad_blocks = _test_output_blocks(text)
    invalid |= bad_blocks
    for block_name in blocks:
        if block_name not in cases or block_name not in failed_cases:
            invalid = True
    marker_count = 0
    seen_exact: set[str] = set()
    seen_case_kinds: set[tuple[str, str]] = set()
    http_processed: set[str] = set()
    claim_order: list[tuple[str, dict]] = []
    output_blocks = blocks
    for test_name, block in output_blocks.items():
        # A malformed inventory or an unknown stdout header may contain arbitrary
        # identifiers. Mark the record invalid above, but do not parse or persist it.
        if test_name not in cases or test_name not in failed_cases:
            continue
        for line in block.splitlines():
            mentions = [prefix for prefix in KNOWN_MARKER_PREFIXES if prefix in line]
            if not mentions:
                continue
            marker_count += len(mentions)
            if marker_count > MARKER_LIMIT or len(mentions) != 1 or exit_code == 0 or test_name not in failed_cases:
                invalid = True
                continue
            prefix = mentions[0]
            exact_key = f"{test_name}\n{line}"
            if exact_key in seen_exact and prefix != BOOTSTRAP_DEADLINE_PREFIX:
                invalid = True
                continue
            seen_exact.add(exact_key)
            key = None
            parsed = None
            if prefix == BROWSER_PHASE_PREFIX:
                if suite_name != "browser":
                    invalid = True
                    continue
                parsed = parse_browser_phase_marker(line)
                key = ("browser_phase", test_name)
                if parsed is not None and any(item["request"] == parsed["request"]
                                              for item in facts["browser_phase"]):
                    invalid = True
                    continue
            elif prefix == SESSION_PREFIX:
                if suite_name != "session":
                    invalid = True
                    continue
                parsed = parse_session_marker(line)
                key = ("session_deadline", test_name)
                if parsed is not None:
                    parsed["test"] = test_name
            elif prefix == BOOTSTRAP_DEADLINE_PREFIX:
                if suite_name != "browser":
                    invalid = True
                    continue
                parsed = parse_bootstrap_deadline_marker(line)
                key = ("bootstrap_deadline", test_name)
            elif prefix == BOOTSTRAP_PREFIX:
                if suite_name != "browser":
                    invalid = True
                    continue
                parsed = parse_bootstrap_marker(line)
                key = ("bootstrap", test_name)
            elif prefix == PAIR_PREFIX:
                if suite_name != "browser" or test_name != "public_login_claims_once_then_named_verifier_issue_and_appended_cookies":
                    invalid = True
                    continue
                parsed = parse_pair_marker(line)
                key = ("pair", test_name)
            elif prefix == JSON_BOUND_PREFIX:
                if suite_name != "browser" or test_name != "json_value_nodes_and_container_depth_accept_exact_bounds":
                    invalid = True
                    continue
                parsed = parse_json_bound_marker(line)
                key = ("json_bound", test_name)
            elif prefix in (auth_diag.SAFE_REPLY_PREFIX, auth_diag.WIRE_BODY_PREFIX):
                if prefix == auth_diag.WIRE_BODY_PREFIX and CLAIM_VARIANT_HINT in line:
                    if suite_name != "browser" or test_name != CLAIM_MARKER_TEST:
                        invalid = True
                        continue
                    claim = parse_claim_marker(line)
                    if claim is None:
                        invalid = True
                        continue
                    fact_name, parsed = claim
                    key = (fact_name, test_name)
                    if key in seen_case_kinds:
                        invalid = True
                        continue
                    seen_case_kinds.add(key)
                    parsed["test"] = test_name
                    facts[fact_name].append(parsed)
                    claim_order.append((fact_name, parsed))
                    continue
                # Existing canonical public-test parser validates and converts these below.
                continue
            elif prefix == auth_diag.TEST_IO_MARKER_PREFIX:
                # Validate once per failed test with the existing closed HTTP-I/O vocabulary.
                if test_name in http_processed:
                    continue
                http_processed.add(test_name)
                normalized = auth_diag.http_io_failures(block)
                if any(item.endswith("=unavailable") or item.endswith("_loss=observed") for item in normalized):
                    invalid = True
                    continue
                parsed_items = []
                for item in normalized:
                    match = HTTP_IO_LINE.fullmatch(item)
                    if match:
                        parsed_items.append({"test": test_name, "phase": match.group(1), "kind": match.group(2)})
                raw_count = sum(auth_diag.TEST_IO_MARKER_PREFIX in candidate for candidate in block.splitlines())
                if raw_count != len(parsed_items):
                    invalid = True
                    continue
                facts["http_test_io"].extend(parsed_items)
                continue
            else:
                # Stalled-response markers belong to another fixed suite, not these APIs.
                invalid = True
                continue
            if parsed is None or key is None or (key[0] not in ("session_deadline", "bootstrap_deadline", "browser_phase") and key in seen_case_kinds):
                invalid = True
                continue
            parsed["test"] = test_name
            if key[0] not in ("session_deadline", "bootstrap_deadline", "browser_phase"):
                seen_case_kinds.add(key)
            facts[key[0]].append(parsed)

    claim_facts = facts["claim_initial"] + facts["claim_retry"] + facts["claim_replay_proof"]
    if claim_facts:
        iterations = {item["iteration"] for item in claim_facts}
        if len(iterations) != 1 or (facts["claim_initial"] and
                                    (facts["claim_retry"] or facts["claim_replay_proof"])):
            invalid = True
        replay = facts["claim_replay_proof"]
        retry = facts["claim_retry"]
        if replay and not retry and all(
                replay[0][field] for field in ("row_absent", "verifier_unchanged", "handler_unchanged")):
            invalid = True
        if retry and replay:
            order = [kind for kind, _ in claim_order]
            if order.index("claim_retry") > order.index("claim_replay_proof"):
                invalid = True

    if marker_count > MARKER_LIMIT:
        invalid = True
    public, public_invalid = _auth_facts(text, exit_code, cases)
    facts["public"].extend(public)
    invalid |= public_invalid
    # Any marker outside a failing, allowlisted test's captured stdout is unbound.
    global_marker_count = sum(
        sum(prefix in line for prefix in KNOWN_MARKER_PREFIXES)
        for line in text.splitlines()
    )
    if global_marker_count != marker_count:
        invalid = True
    if marker_count > MARKER_LIMIT:
        invalid = True
    return facts, marker_count, invalid


def diagnose(session_raw: bytes | None, session_input_status: str, session_exit: int,
             browser_raw: bytes | None, browser_input_status: str, browser_exit: int) -> tuple[dict, int]:
    record = {
        "schema": "nagi-session-deadline-public-diagnostic-v1",
        "source_inventory": {"session_tests": len(SESSION_CASES), "browser_tests": len(BROWSER_CASES)},
        "status": "diagnostic_invalid",
        "marker_count": 0,
        "suites": {},
    }
    all_passed = True
    any_test_failure = False
    any_invalid = False
    for name, raw, input_status, exit_code in (
        ("session", session_raw, session_input_status, session_exit),
        ("browser", browser_raw, browser_input_status, browser_exit),
    ):
        summary, text = _suite_structure(name, raw, input_status, exit_code)
        cases = {case: case for case in SUITES[name]}
        failed = set(summary["failed_cases"])
        if text is None:
            text = ""
        facts, marker_count, marker_invalid = _marker_facts(name, text, exit_code, failed, cases)
        summary.update(facts)
        summary["diagnostic_status"] = "invalid" if marker_invalid else "valid"
        record["marker_count"] += marker_count
        if summary["inventory_status"] != "passed":
            all_passed = False
            if summary["inventory_status"] == "failed":
                any_test_failure = True
            elif summary["inventory_status"] not in ("no_test_execution",):
                any_invalid = True
        if marker_invalid:
            any_invalid = True
        record["suites"][name] = summary
    if record["marker_count"] > MARKER_LIMIT:
        any_invalid = True
    if any_invalid:
        record["status"] = "diagnostic_invalid"
    elif any_test_failure:
        record["status"] = "tests_failed"
    elif all_passed:
        record["status"] = "passed"
    else:
        record["status"] = "tests_not_run_or_incomplete"
    return record, 0 if record["status"] == "passed" else 1


def _write_output(path: pathlib.Path, record: dict) -> bool:
    encoded = (json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")
    if len(encoded) > OUTPUT_LIMIT:
        encoded = b'{"schema":"nagi-session-deadline-public-diagnostic-v1","status":"output_limit_exceeded"}\n'
        ok = False
    else:
        ok = True
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        temporary = path.with_name(path.name + ".tmp")
        temporary.write_bytes(encoded)
        temporary.replace(path)
    except OSError:
        return False
    return ok


def main(argv: list[str]) -> int:
    if len(argv) != 5:
        return 2
    root = pathlib.Path(argv[1])
    try:
        session_exit, browser_exit = int(argv[2]), int(argv[3])
    except ValueError:
        return 2
    session_raw, session_status = _read_bounded(root / "session-deadline-session.raw")
    browser_raw, browser_status = _read_bounded(root / "session-deadline-browser.raw")
    record, status = diagnose(session_raw, session_status, session_exit, browser_raw, browser_status, browser_exit)
    if not _write_output(pathlib.Path(argv[4]), record):
        return 1
    # Reconstruct only parsed numeric facts + source-whitelisted test identity.
    # Never cat raw stdout/stderr or print an invalid sanitizer record.
    if record["status"] == "tests_failed":
        for fact in record["suites"]["browser"]["browser_phase"]:
            print("browser_login_phase=" + json.dumps(fact, sort_keys=True, separators=(",", ":")))
    return status


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
