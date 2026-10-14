import json
import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts" / "ci"))
import browser_login_phase_diagnostic as diagnostic


PASS_SUMMARY = (
    "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
    "9 filtered out; finished in 0.25s"
)
FAIL_SUMMARY = (
    "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; "
    "9 filtered out; finished in 0.25s"
)


def observer_line(request=17, **overrides):
    fields = {
        "request": request,
        "status": 504,
        "phase": 15,
        "event": 1,
        "before_gate": 1,
        "poll_gate": 2,
        "saw_pending": 1,
        "pending_phase": 15,
        "pending_gate": 2,
        "saw_live_pending": 1,
        "live_pending_phase": 15,
        "gate_refused": 0,
        "row_missing": 0,
        "rollback_ack": 0,
        "restore_commit_ack": 0,
        "exit": 3,
        "exit_gate": 2,
        "consumed": 1,
    }
    fields.update(overrides)
    return "browser_login_phase_diag " + " ".join(
        f"{name}={value}" for name, value in fields.items()
    )


def run_text(body, cargo_exit=0):
    return diagnostic.sanitize(body.encode("utf-8"), cargo_exit)


PUBLIC_RECORD_TYPES = {
    "committed_claim_initial": {
        "case": str,
        "phase": str,
        "iteration": int,
        "expected_status": int,
        "status": int,
        "auth_cookie": bool,
        "deletion": bool,
        "verifier_calls": int,
        "handler_calls": int,
        "claim_committed": bool,
    },
    "committed_claim_retry": {
        "case": str,
        "phase": str,
        "iteration": int,
        "status": int,
        "body_bytes": int,
        "set_cookie_count": int,
        "auth_cookie": bool,
        "deletion": bool,
        "verifier_calls": int,
        "handler_calls": int,
        "claim_committed": bool,
    },
    "committed_claim_replay_proof": {
        "case": str,
        "iteration": int,
        "row_absent": bool,
        "verifier_unchanged": bool,
        "handler_unchanged": bool,
    },
}


class BrowserLoginPhaseDiagnosticTests(unittest.TestCase):
    def assert_public_records_closed(self, records):
        for record in records:
            expected_types = PUBLIC_RECORD_TYPES[record["case"]]
            self.assertEqual(set(record), set(expected_types))
            for name, expected_type in expected_types.items():
                self.assertIs(type(record[name]), expected_type, (record["case"], name))

    def test_positive_records_are_bounded_and_private_output_is_dropped(self):
        raw = "\n".join(
            [
                "test " + diagnostic.CASE + " ... ok",
                "panic: owned verifier panic token=never-publish",
                "cookie=private-cookie body=private-body path=/private/location",
                "SQL SELECT secret_column FROM private_table error=private-error",
                observer_line(request=21),
                observer_line(request=22, phase=16, event=2, before_gate=1, poll_gate=1, exit=8, exit_gate=1),
                PASS_SUMMARY,
            ]
        )
        document, result = run_text(raw)
        self.assertEqual(result, 0)
        self.assertTrue(document["capture_valid"])
        self.assertEqual(document["case"], diagnostic.CASE)
        self.assertEqual(
            set(document),
            {"schema", "case", "command", "cargo_exit", "capture_valid", "test_summary", "records", "public_api_records"},
        )
        self.assertEqual(document["schema"], "nagi-browser-login-phase-diagnostic-v2")
        self.assertEqual(document["command"], diagnostic.COMMAND)
        self.assertIs(type(document["capture_valid"]), bool)
        self.assertIs(type(document["cargo_exit"]), int)
        self.assertEqual(document["test_summary"], "passed")
        self.assertEqual(document["cargo_exit"], 0)
        self.assertEqual(document["public_api_records"], [])
        self.assertEqual([row["request"] for row in document["records"]], [21, 22])
        self.assertTrue(all(set(row) == set(diagnostic.FIELD_NAMES) for row in document["records"]))
        self.assertTrue(
            all(type(value) is int for row in document["records"] for value in row.values())
        )
        serialized = json.dumps(document)
        for secret in (
            "owned verifier panic",
            "never-publish",
            "private-cookie",
            "private-body",
            "/private/location",
            "private_table",
            "private-error",
            "browser_login_public_api_diag",
        ):
            self.assertNotIn(secret, serialized)

    def test_failing_exact_test_remains_a_failure_and_keeps_only_valid_records(self):
        raw = "\n".join(
            [
                "test " + diagnostic.CASE + " ... FAILED",
                observer_line(request=7, phase=6, event=1, row_missing=1, rollback_ack=1),
                "browser_login_public_api_diag case=committed_claim_replay_proof iteration=2 row_absent=false verifier_unchanged=true handler_unchanged=true",
                "panic at runtime/src/private.rs:123: arbitrary failure text",
                FAIL_SUMMARY,
            ]
        )
        document, result = run_text(raw, cargo_exit=101)
        self.assertEqual(result, 1)
        self.assertTrue(document["capture_valid"])
        self.assertEqual(document["cargo_exit"], 101)
        self.assertEqual(document["test_summary"], "failed")
        self.assertEqual(len(document["records"]), 1)
        self.assertEqual(
            document["public_api_records"],
            [{
                "case": "committed_claim_replay_proof",
                "iteration": 2,
                "row_absent": False,
                "verifier_unchanged": True,
                "handler_unchanged": True,
            }],
        )
        self.assert_public_records_closed(document["public_api_records"])
        self.assertNotIn("browser_login_public_api_diag", json.dumps(document))
        self.assertNotIn("private.rs", json.dumps(document))
        self.assertNotIn("arbitrary failure text", json.dumps(document))

    def test_unknown_observer_field_invalidates_and_discards_all_records(self):
        line = observer_line(request=8) + " future_field=private-value"
        raw = "\n".join(
            ["test " + diagnostic.CASE + " ... ok", line, observer_line(request=9), PASS_SUMMARY]
        )
        document, result = run_text(raw)
        self.assertEqual(result, 1)
        self.assertFalse(document["capture_valid"])
        self.assertEqual(document["records"], [])
        self.assertEqual(document["public_api_records"], [])
        self.assertNotIn("future_field", json.dumps(document))
        self.assertNotIn("private-value", json.dumps(document))

    def test_duplicate_field_and_incomplete_record_are_rejected(self):
        complete = observer_line(request=8)
        duplicate_field = complete.replace("event=1", "event=1 event=2")
        incomplete = complete.removesuffix(" consumed=1")
        for line in (duplicate_field, incomplete):
            raw = "\n".join(
                [
                    "test " + diagnostic.CASE + " ... ok",
                    line,
                    observer_line(request=9),
                    PASS_SUMMARY,
                ]
            )
            document, result = run_text(raw)
            self.assertEqual(result, 1)
            self.assertFalse(document["capture_valid"])
            self.assertEqual(document["records"], [])
            self.assertEqual(document["public_api_records"], [])

    def test_duplicate_request_record_is_rejected(self):
        raw = "\n".join(
            [
                "test " + diagnostic.CASE + " ... ok",
                observer_line(request=8),
                observer_line(request=8, phase=16, event=2),
                PASS_SUMMARY,
            ]
        )
        document, result = run_text(raw)
        self.assertEqual(result, 1)
        self.assertFalse(document["capture_valid"])
        self.assertEqual(document["records"], [])
        self.assertEqual(document["public_api_records"], [])

    def test_malformed_record_and_wrong_case_identity_are_rejected(self):
        for raw in (
            "\n".join(
                [
                    "test " + diagnostic.CASE + " ... ok",
                    "browser_login_phase_diag request=3 token=never-publish",
                    observer_line(request=4),
                    PASS_SUMMARY,
                ]
            ),
            "\n".join(
                [
                    "test another_public_test ... ok",
                    observer_line(request=4),
                    PASS_SUMMARY,
                ]
            ),
        ):
            document, result = run_text(raw)
            self.assertEqual(result, 1)
            self.assertFalse(document["capture_valid"])
            self.assertEqual(document["records"], [])
            self.assertEqual(document["public_api_records"], [])
            self.assertNotIn("never-publish", json.dumps(document))

    def test_unknown_enum_value_and_unselected_test_summary_are_rejected(self):
        for raw in (
            "\n".join(
                [
                    "test " + diagnostic.CASE + " ... ok",
                    observer_line(request=10, phase=17),
                    observer_line(request=11),
                    PASS_SUMMARY,
                ]
            ),
            "\n".join(
                [
                    "test " + diagnostic.CASE + " ... ok",
                    "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.00s",
                ]
            ),
        ):
            document, result = run_text(raw)
            self.assertEqual(result, 1)
            self.assertFalse(document["capture_valid"])
            self.assertEqual(document["records"], [])
            self.assertEqual(document["public_api_records"], [])

    def test_success_cannot_hide_a_nonzero_cargo_exit(self):
        raw = "\n".join(
            [
                "test " + diagnostic.CASE + " ... ok",
                observer_line(request=12),
                observer_line(request=13),
                PASS_SUMMARY,
            ]
        )
        document, result = run_text(raw, cargo_exit=101)
        self.assertEqual(result, 1)
        self.assertFalse(document["capture_valid"])
        self.assertEqual(document["cargo_exit"], 101)
        self.assertEqual(document["records"], [])
        self.assertEqual(document["public_api_records"], [])

    def test_failed_compile_without_test_summary_is_explicitly_unavailable(self):
        raw = "compiler error: arbitrary SDK path /private/build/secret"
        document, result = run_text(raw, cargo_exit=101)
        self.assertEqual(result, 1)
        self.assertTrue(document["capture_valid"])
        self.assertEqual(document["test_summary"], "unavailable")
        self.assertEqual(document["records"], [])
        self.assertEqual(document["public_api_records"], [])
        self.assertNotIn("SDK", json.dumps(document))
        self.assertNotIn("/private/build/secret", json.dumps(document))

    def test_oversized_raw_is_rejected_without_copying_any_content(self):
        raw = b"secret=" + b"x" * diagnostic.RAW_LIMIT
        document, result = diagnostic.sanitize(raw, 101)
        self.assertEqual(result, 1)
        self.assertFalse(document["capture_valid"])
        self.assertEqual(document["records"], [])
        self.assertEqual(document["public_api_records"], [])
        self.assertNotIn("secret", json.dumps(document))



    def test_source_closed_initial_marker_is_retained_as_failed_numeric_record(self):
        marker = (
            "browser_login_public_api_diag case=committed_claim_initial phase=initial_response "
            "iteration=2 expected_status=504 status=500 auth_cookie=false deletion=true "
            "verifier_calls=1 handler_calls=0 claim_committed=true"
        )
        raw = "\n".join(
            [
                "test " + diagnostic.CASE + " ... FAILED",
                marker,
                "panic at private/path.rs:44: do-not-publish token=private-value",
                FAIL_SUMMARY,
            ]
        )
        document, result = run_text(raw, cargo_exit=101)
        self.assertEqual(result, 1)
        self.assertTrue(document["capture_valid"])
        self.assertEqual(document["cargo_exit"], 101)
        self.assertEqual(document["test_summary"], "failed")
        self.assertEqual(document["records"], [])
        self.assertEqual(
            document["public_api_records"],
            [{
                "case": "committed_claim_initial",
                "phase": "initial_response",
                "iteration": 2,
                "expected_status": 504,
                "status": 500,
                "auth_cookie": False,
                "deletion": True,
                "verifier_calls": 1,
                "handler_calls": 0,
                "claim_committed": True,
            }],
        )
        self.assert_public_records_closed(document["public_api_records"])
        serialized = json.dumps(document)
        for secret in ("browser_login_public_api_diag", "private/path.rs", "private-value", "do-not-publish"):
            self.assertNotIn(secret, serialized)

    def test_source_closed_retry_and_replay_markers_are_retained(self):
        retry = (
            "browser_login_public_api_diag case=committed_claim_retry phase=retry_response "
            "iteration=1 status=504 body_bytes=0 set_cookie_count=0 auth_cookie=false "
            "deletion=false verifier_calls=1 handler_calls=0 claim_committed=true"
        )
        replay = (
            "browser_login_public_api_diag case=committed_claim_replay_proof iteration=1 "
            "row_absent=true verifier_unchanged=true handler_unchanged=true"
        )
        raw = "\n".join(
            ["test " + diagnostic.CASE + " ... FAILED", retry, replay, FAIL_SUMMARY]
        )
        document, result = run_text(raw, cargo_exit=101)
        self.assertEqual(result, 1)
        self.assertTrue(document["capture_valid"])
        self.assertEqual(document["cargo_exit"], 101)
        self.assertEqual(document["records"], [])
        self.assertEqual(
            document["public_api_records"],
            [
                {
                    "case": "committed_claim_retry",
                    "phase": "retry_response",
                    "iteration": 1,
                    "status": 504,
                    "body_bytes": 0,
                    "set_cookie_count": 0,
                    "auth_cookie": False,
                    "deletion": False,
                    "verifier_calls": 1,
                    "handler_calls": 0,
                    "claim_committed": True,
                },
                {
                    "case": "committed_claim_replay_proof",
                    "iteration": 1,
                    "row_absent": True,
                    "verifier_unchanged": True,
                    "handler_unchanged": True,
                },
            ],
        )
        self.assert_public_records_closed(document["public_api_records"])
        self.assertNotIn("browser_login_public_api_diag", json.dumps(document))

    def test_public_marker_unknown_duplicate_incomplete_and_out_of_bounds_are_rejected(self):
        valid = (
            "browser_login_public_api_diag case=committed_claim_initial phase=initial_response "
            "iteration=2 expected_status=504 status=500 auth_cookie=false deletion=true "
            "verifier_calls=1 handler_calls=0 claim_committed=true"
        )
        malformed = (
            valid + " private_field=do-not-publish",
            valid.replace("case=committed_claim_initial", "case=unregistered_case"),
            "browser_login_public_api_diag case=wire_body_cap phase=exact_body_response iteration=0 status=500 body_bytes=192 expected_body_bytes=192 body_matches=false set_cookie_count=0 auth_cookie=false deletion=false",
            valid.replace("status=500", "status=500 status=500"),
            valid.removesuffix(" claim_committed=true"),
            valid.replace("iteration=2", "iteration=5"),
            valid.replace("verifier_calls=1", "verifier_calls=5"),
        )
        duplicate = (valid, valid)
        for public_lines in (*((line,) for line in malformed), duplicate):
            raw = "\n".join(
                ["test " + diagnostic.CASE + " ... FAILED", *public_lines, FAIL_SUMMARY]
            )
            document, result = run_text(raw, cargo_exit=101)
            self.assertEqual(result, 1)
            self.assertFalse(document["capture_valid"])
            self.assertEqual(document["records"], [])
            self.assertEqual(document["public_api_records"], [])
            self.assertNotIn("do-not-publish", json.dumps(document))
            self.assertNotIn("unregistered_case", json.dumps(document))

    def test_public_markers_cannot_be_reported_as_a_passing_test(self):
        marker = (
            "browser_login_public_api_diag case=committed_claim_initial phase=initial_response "
            "iteration=0 expected_status=504 status=500 auth_cookie=false deletion=true "
            "verifier_calls=1 handler_calls=0 claim_committed=true"
        )
        raw = "\n".join(
            [
                "test " + diagnostic.CASE + " ... ok",
                observer_line(request=31),
                observer_line(request=32),
                marker,
                PASS_SUMMARY,
            ]
        )
        document, result = run_text(raw, cargo_exit=0)
        self.assertEqual(result, 1)
        self.assertFalse(document["capture_valid"])
        self.assertEqual(document["cargo_exit"], 0)
        self.assertEqual(document["records"], [])
        self.assertEqual(document["public_api_records"], [])

if __name__ == "__main__":
    unittest.main()
