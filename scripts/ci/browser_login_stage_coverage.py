"""Require fixed native BrowserLogin stage oracles from the existing broad run."""
from __future__ import annotations

import json
import pathlib
import re
import sys

RAW_LIMIT = 256 * 1024
BASE = "auth::prelogin::tests::expiry_review_tests::"
REQUIRED = (
    BASE + "consumed_missing_row_returns_403_while_original_request_gate_is_live",
    BASE + "consumed_missing_row_restoration_queue_wait_obeys_real_gate_expiry_504",
    BASE + "http_claim_tightened_gate_wakes_during_real_postcommit_maintenance_wait",
    "http_server::browser_login::stage_tests::live_claimed_200_runs_verifier_and_handler_and_returns_200",
    "http_server::browser_login::stage_tests::consumed_prepare_err_1000_returns_401_without_cookie",
    "http_server::browser_login::stage_tests::consumed_prepare_panic_1000_returns_500_without_cookie",
    "http_server::browser_login::stage_tests::consumed_prepare_pending_200_returns_504_and_drops_once",
    "http_server::browser_login::stage_tests::consumed_prepare_non_yielding_200_late_ready_returns_504",
    "http_server::browser_login::stage_tests::committed_issue_live_200_delivers_auth_cookie_once",
    "http_server::browser_login::stage_tests::committed_issue_handler_err_1000_returns_500_without_cookie",
    "http_server::browser_login::stage_tests::committed_issue_handler_panic_1000_returns_500_without_cookie",
    "http_server::browser_login::stage_tests::committed_issue_handler_pending_200_returns_504_and_drops_once",
    "auth::session::http_bridge::delivery_tests::real_commit_survives_dropped_intent_mapper_and_panic_without_cookie",
    "auth::session::http_bridge::delivery_tests::real_commit_then_controlled_handler_timeout_drops_future_without_cookie",
    "auth::session::http_bridge::delivery_tests::expiry_after_real_commit_retires_applied_material_without_cookie",
    "http_server::tests::security_tests::authorizer_timeout_shares_budget_and_invalidates_intermediate_scope",
    "http_server::tests::security_tests::late_verifier_failure_uses_the_exhausted_security_budget",
)
SUMMARY = re.compile(
    r"^test result: ok\. ([1-9][0-9]*) passed; 0 failed; [0-9]+ ignored; "
    r"[0-9]+ measured; [0-9]+ filtered out; finished in [0-9.]+s$",
    re.M,
)


def coverage(raw: bytes | None, cargo_exit: int) -> tuple[dict, int]:
    # Reconstruct only source-whitelisted names and closed statuses. Never copy
    # an unmatched row, panic, SQL, response material, path or error detail.
    states = {name: "unavailable" for name in REQUIRED}
    record = {"schema": "nagi-browser-native-stage-coverage-v1", "status": "failed",
              "required_count": len(REQUIRED), "cases": states}
    if raw is None or len(raw) > RAW_LIMIT or cargo_exit != 0:
        return record, 1
    try:
        text = raw.decode("utf-8", errors="strict").replace("\r\n", "\n")
    except UnicodeDecodeError:
        return record, 1
    summaries = SUMMARY.findall(text)
    for name in REQUIRED:
        rows = re.findall(r"^test " + re.escape(name) + r" \.\.\. (ok|FAILED|ignored)$", text, re.M)
        states[name] = rows[0] if len(rows) == 1 else "missing" if not rows else "duplicate"
    valid = (len(summaries) == 1 and int(summaries[0]) >= len(REQUIRED)
             and all(state == "ok" for state in states.values()))
    if valid:
        record["status"] = "passed"
    return record, 0 if valid else 1


def main(argv: list[str]) -> int:
    if len(argv) != 4:
        return 2
    try:
        cargo_exit = int(argv[2])
        with pathlib.Path(argv[1]).open("rb") as stream:
            raw = stream.read(RAW_LIMIT + 1)
    except (OSError, ValueError):
        raw, cargo_exit = None, 1
    record, status = coverage(raw, cargo_exit)
    # Output identity is caller-owned; content has no caller-supplied strings.
    pathlib.Path(argv[3]).write_text(json.dumps(record, sort_keys=True) + "\n", encoding="ascii")
    print("browser_native_stage_coverage=" + record["status"])
    return status


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
