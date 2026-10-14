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


class BrowserLoginPhaseDiagnosticTests(unittest.TestCase):
    def test_positive_records_are_bounded_and_private_output_is_dropped(self):
        raw = "\n".join(
            [
                "test " + diagnostic.CASE + " ... ok",
                "panic: owned verifier panic token=never-publish",
                "cookie=private-cookie body=private-body path=/private/location",
                "SQL SELECT secret_column FROM private_table error=private-error",
                "browser_login_public_api_diag case=committed_claim_initial phase=initial_response iteration=3 expected_status=504 status=504 auth_cookie=false deletion=true verifier_calls=1 handler_calls=0 claim_committed=true",
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
            {"schema", "case", "command", "cargo_exit", "capture_valid", "test_summary", "records"},
        )
        self.assertEqual(document["schema"], "nagi-browser-login-phase-diagnostic-v1")
        self.assertEqual(document["command"], diagnostic.COMMAND)
        self.assertIs(type(document["capture_valid"]), bool)
        self.assertIs(type(document["cargo_exit"]), int)
        self.assertEqual(document["test_summary"], "passed")
        self.assertEqual(document["cargo_exit"], 0)
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

    def test_failed_compile_without_test_summary_is_explicitly_unavailable(self):
        raw = "compiler error: arbitrary SDK path /private/build/secret"
        document, result = run_text(raw, cargo_exit=101)
        self.assertEqual(result, 1)
        self.assertTrue(document["capture_valid"])
        self.assertEqual(document["test_summary"], "unavailable")
        self.assertEqual(document["records"], [])
        self.assertNotIn("SDK", json.dumps(document))
        self.assertNotIn("/private/build/secret", json.dumps(document))

    def test_oversized_raw_is_rejected_without_copying_any_content(self):
        raw = b"secret=" + b"x" * diagnostic.RAW_LIMIT
        document, result = diagnostic.sanitize(raw, 101)
        self.assertEqual(result, 1)
        self.assertFalse(document["capture_valid"])
        self.assertEqual(document["records"], [])
        self.assertNotIn("secret", json.dumps(document))


if __name__ == "__main__":
    unittest.main()
