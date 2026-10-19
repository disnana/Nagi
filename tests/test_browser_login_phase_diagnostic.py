import json
import contextlib
import io
import tempfile
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
    def test_failed_case_keeps_only_bounded_whitelisted_panic_locations(self):
        for source in ("runtime/tests/browser_login_public_api.rs",
                       r"runtime\tests\browser_login_public_api.rs"):
            with self.subTest(source=source):
                raw = "\n".join([
                    f"thread '{diagnostic.CASE}' (42) panicked at {source}:123:7:",
                    "private-cookie=do-not-publish body=secret panic payload",
                    "test " + diagnostic.CASE + " ... FAILED", FAIL_SUMMARY])
                document, result = run_text(raw, cargo_exit=101)
                self.assertEqual(result, 1)
                self.assertTrue(document["capture_valid"])
                self.assertEqual(document["failure_locations"], [{"line": 123, "column": 7}])
                for private in (source, "private-cookie", "do-not-publish", "secret"):
                    self.assertNotIn(private, json.dumps(document))

    def test_panic_location_refuses_other_cases_paths_and_unbounded_numbers(self):
        valid = f"thread '{diagnostic.CASE}' panicked at runtime/tests/browser_login_public_api.rs:123:7:"
        for line in (valid.replace(diagnostic.CASE, "another_test"),
                     valid.replace("runtime/tests/", "/private/runtime/tests/"),
                     valid.replace(".rs:123:7:", ".rs:0:7:"),
                     valid.replace(".rs:123:7:", ".rs:1000000:7:"),
                     valid.replace(".rs:123:7:", ".rs:123:1000000:"),
                     valid + " secret=do-not-publish", "private prefix " + valid):
            with self.subTest(line=line):
                document, result = run_text("\n".join([
                    line, "test " + diagnostic.CASE + " ... FAILED", FAIL_SUMMARY]), 101)
                self.assertEqual(result, 1)
                self.assertEqual(document["failure_locations"], [])
                self.assertNotIn("do-not-publish", json.dumps(document))

    def test_panic_location_never_turns_failure_into_success_or_survives_invalid_capture(self):
        line = f"thread '{diagnostic.CASE}' panicked at runtime/tests/browser_login_public_api.rs:123:7:"
        for end, code, extra in ((PASS_SUMMARY, 0, "ok"),
                                 (FAIL_SUMMARY, 101, "another")):
            raw = "\n".join([line, observer_line(1), observer_line(2),
                              "test " + (diagnostic.CASE if extra == "ok" else extra)
                              + " ... " + ("ok" if code == 0 else "FAILED"), end])
            document, result = run_text(raw, code)
            self.assertEqual(result, 0 if code == 0 else 1)
            self.assertEqual(document["failure_locations"], [])
        raw = "\n".join([line] * 100 + ["test " + diagnostic.CASE + " ... FAILED", FAIL_SUMMARY])
        document, result = run_text(raw, 101)
        self.assertEqual(result, 1)
        self.assertLessEqual(len(document["failure_locations"]), 10)

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
            {"schema", "case", "command", "cargo_exit", "capture_valid", "test_summary", "observer_records_seen", "invalid_reason_counts", "records", "public_api_records", "failure_locations"},
        )
        self.assertEqual(document["schema"], "nagi-browser-login-phase-diagnostic-v4")
        self.assertEqual(set(document["invalid_reason_counts"]), set(diagnostic.INVALID_REASON_NAMES))
        self.assertTrue(all(type(n) is int and 0 <= n <= diagnostic.INVALID_REASON_LIMIT
                            for n in document["invalid_reason_counts"].values()))
        self.assertEqual(document["observer_records_seen"], 2)
        self.assertEqual(set(document["invalid_reason_counts"].values()), {0})
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

    def test_extra_allowed_deadline_record_keeps_actual_passing_test(self):
        raw = "\n".join(
            [
                "test " + diagnostic.CASE + " ... ok",
                observer_line(request=21),
                observer_line(request=22),
                observer_line(request=23),
                PASS_SUMMARY,
            ]
        )
        document, result = run_text(raw)
        self.assertEqual(result, 0)
        self.assertTrue(document["capture_valid"])
        self.assertEqual(len(document["records"]), 3)
        self.assertEqual(document["observer_records_seen"], 3)
        self.assertEqual(
            document["invalid_reason_counts"]["passing_run_observer_count_mismatch"], 0
        )
        self.assertEqual(
            sum(document["invalid_reason_counts"].values()), 0
        )
        self.assertNotIn("request=23", json.dumps(document))

    def test_passing_record_bounds_preserve_missing_and_excess_witness_rejection(self):
        # Five initial requests and at most five checked consumed-row replays.
        # Extra 504s are permitted by the exact functional test, not inferred
        # evidence that its normal business branches were exercised.
        for count in (0, 1, 2, 3, 5, 10, 11):
            with self.subTest(count=count):
                raw = "\n".join(["test " + diagnostic.CASE + " ... ok"] +
                                [observer_line(request=100+i) for i in range(count)] +
                                [PASS_SUMMARY])
                document, result = run_text(raw)
                accepted = 2 <= count <= 10
                self.assertEqual(result, 0 if accepted else 1)
                self.assertEqual(document["capture_valid"], accepted)
                self.assertEqual(len(document["records"]), count if accepted else 0)
                self.assertEqual(document["observer_records_seen"], count)
                if count < 2:
                    self.assertEqual(document["invalid_reason_counts"]["passing_run_observer_count_mismatch"], 1)
                if count == 11:
                    self.assertEqual(document["invalid_reason_counts"]["observer_record_refused"], 1)

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
        self.assertEqual(document["invalid_reason_counts"]["observer_record_refused"], 1)

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
        self.assertEqual(document["invalid_reason_counts"]["input_refused"], 1)



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

    def test_cli_emits_only_closed_counts_after_original_aggregate(self):
        raw = "\n".join(["test " + diagnostic.CASE + " ... ok",
                         observer_line(request=21), observer_line(request=22),
                         observer_line(request=23), "private-cookie-path=never-publish", PASS_SUMMARY])
        with tempfile.TemporaryDirectory(prefix="nagi-owned-cli-counts-") as owned:
            source, output = pathlib.Path(owned) / "input.raw", pathlib.Path(owned) / "output.json"
            source.write_text(raw, encoding="utf-8")
            captured = io.StringIO()
            with contextlib.redirect_stdout(captured):
                result = diagnostic.main(["collector", str(source), "0", str(output)])
            lines = captured.getvalue().splitlines()
            self.assertEqual(result, 0)
            self.assertEqual(lines[0], "browser_login_phase_capture=valid test_summary=passed cargo_exit=0")
            self.assertEqual(len(lines), 2)
            self.assertTrue(lines[1].startswith("browser_login_phase_counts observer_records_seen=3 "))
            self.assertIn("passing_run_observer_count_mismatch=0", lines[1])
            self.assertNotIn("private", captured.getvalue())
            self.assertNotIn("never-publish", captured.getvalue())
            self.assertEqual(len(json.loads(output.read_text())["records"]), 3)
            self.assertEqual(json.loads(output.read_text())["public_api_records"], [])

    def test_stdout_counts_rejects_malformed_values_and_bounds_without_echo(self):
        import copy
        document = diagnostic.empty_document(0)
        document["observer_records_seen"] = diagnostic.MAX_RECORDS + 1
        document["invalid_reason_counts"] = {name: diagnostic.INVALID_REASON_LIMIT
                                              for name in diagnostic.INVALID_REASON_NAMES}
        line = diagnostic.closed_count_line(document)
        self.assertTrue(line.startswith("browser_login_phase_counts observer_records_seen=11 "))
        self.assertLess(len(line), 512)
        for value in (True, -1, 12, "private-cookie", 10 ** 100):
            poisoned = copy.deepcopy(document); poisoned["observer_records_seen"] = value
            self.assertEqual(diagnostic.closed_count_line(poisoned), "browser_login_phase_counts unavailable")
        for value in (True, -1, 17, "private-cookie", 10 ** 100):
            poisoned = copy.deepcopy(document); poisoned["invalid_reason_counts"][diagnostic.INVALID_REASON_NAMES[0]] = value
            self.assertEqual(diagnostic.closed_count_line(poisoned), "browser_login_phase_counts unavailable")
        for mutate in (lambda d: d["invalid_reason_counts"].update({"private-cookie": 1}),
                       lambda d: d.update({"private-cookie": 1}),
                       lambda d: d.pop("records"),
                       lambda d: d.update({"invalid_reason_counts": None})):
            poisoned = copy.deepcopy(document); mutate(poisoned)
            self.assertEqual(diagnostic.closed_count_line(poisoned), "browser_login_phase_counts unavailable")

if __name__ == "__main__":
    unittest.main()
