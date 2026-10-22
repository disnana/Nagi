import contextlib
import hashlib
import io
import json
import pathlib
import re
import sys
import tempfile
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts" / "ci"))
import session_deadline_diagnostic as diag


class SessionDeliveryNativeDiagnosticTests(unittest.TestCase):
    CASE = "auth::session::http_bridge::delivery_tests::applied_real_issue_commit_has_exactly_one_wire_cookie"
    MARKER = (
        "session_deadline_diag status=504 phase=6 event=1 db=4 exit=0 "
        "polls=2 begins=1 commands=0 commits=0 budget_ms=1000 entry_remaining_ms=1000 "
        "phase_remaining_ms=999 exit_remaining_ms=0"
    )

    def test_delivery_cli_closed_failed_fact(self):
        # Baseline CLI returns usage=2: this regression fails at an actual
        # expected-status assertion before reading any new parser API.
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            raw, output = root / "raw", root / "result.json"
            raw.write_bytes(cargo_log({self.CASE}, failed={self.CASE}, markers={self.CASE: [self.MARKER]}))
            console = io.StringIO()
            with contextlib.redirect_stdout(console):
                status = diag.main(["diag", "--delivery", "applied", str(raw), "101", str(output)])
            self.assertEqual(status, 1, "delivery_cli_requires_closed_native_mode")
            record = json.loads(output.read_text())
            self.assertEqual(record["status"], "tests_failed")
            fact = record["suites"]["delivery"]["session_deadline"][0]
            self.assertEqual((fact["test"], fact["phase"], fact["db"], fact["exit"]),
                             (self.CASE, 6, 4, 0))
            lines = console.getvalue().splitlines()
            self.assertEqual(lines[0], "session_delivery_verdict status=tests_failed input=valid inventory=failed diagnostic=valid markers=1")
            self.assertTrue(lines[1].startswith("session_delivery_native="))
            self.assertNotIn("PRIVATE_SENTINEL", output.read_text() + console.getvalue())

    def test_delivery_case_inventory_and_existing_normal_suite(self):
        native = source_cases("runtime/src/auth/session/delivery_wire_tests.rs")
        self.assertEqual(len(native), 11)
        self.assertEqual(len(diag.DELIVERY_CASES), 4)
        self.assertLessEqual({name.removeprefix(diag.DELIVERY_MODULE) for name in diag.DELIVERY_CASES.values()}, native)
        bridge = (ROOT / "runtime/src/auth/session/http_bridge.rs").read_text()
        self.assertIn('mod delivery_tests;', bridge)
        self.assertIn('#[path = "delivery_wire_tests.rs"]', bridge)
        for case, name in diag.DELIVERY_CASES.items():
            with self.subTest(case=case):
                passed, status = diag.diagnose_delivery(cargo_log({name}), "valid", 0, case)
                self.assertEqual((status, passed["status"], passed["marker_count"]), (0, "passed", 0))
                failed, status = diag.diagnose_delivery(
                    cargo_log({name}, failed={name}, markers={name: [self.MARKER]}), "valid", 101, case)
                self.assertEqual((status, failed["status"]), (1, "tests_failed"))
                self.assertEqual(failed["suites"]["delivery"]["failed_cases"], [name])

    def test_delivery_grammar_and_legacy_phase_guard(self):
        good = self.MARKER
        self.assertIsNotNone(diag.parse_delivery_marker(good))
        self.assertIsNone(diag.parse_session_marker(good))
        legacy = good.replace("phase=6", "phase=2").replace("exit=0", "exit=1")
        self.assertIsNotNone(diag.parse_session_marker(legacy))
        self.assertIsNone(diag.parse_delivery_marker(legacy))
        unpolled = good.replace("phase=6", "phase=0").replace("event=1", "event=0")
        unpolled = unpolled.replace("db=4", "db=0").replace("polls=2", "polls=0").replace("begins=1", "begins=0")
        self.assertIsNotNone(diag.parse_delivery_marker(unpolled))
        for bad in [good + " token=PRIVATE_SENTINEL", "prefix " + good, good + "\n",
                    good.replace("phase=6", "phase=2"), good.replace("phase=6", "phase=7"),
                    good.replace("phase=6", "phase=06"), good.replace("event=1", "event=0"),
                    good.replace("polls=2", "polls=0"), good.replace("db=4", "db=15"),
                    good.replace("polls=2", "polls=1024"), good.replace("begins=1", "begins=0").replace("commits=0", "commits=1"),
                    good.replace("budget_ms=1000", "budget_ms=60001"),
                    good.replace("phase_remaining_ms=999", "phase_remaining_ms=1001"),
                    unpolled.replace("db=0", "db=4"),
                    *[good.replace("exit=0", f"exit={exit}") for exit in range(1, 5)]]:
            with self.subTest(bad=bad):
                self.assertIsNone(diag.parse_delivery_marker(bad))

    def test_delivery_rejects_other_inventory_infra_and_unbound_facts(self):
        name = self.CASE
        valid = cargo_log({name}, failed={name}, markers={name: [self.MARKER]})
        others = [
            (cargo_log(set()), 0),
            (cargo_log({name}, statuses={name: "ignored"}), 0),
            (cargo_log(set(diag.DELIVERY_CASES.values())), 0),
            (cargo_log({diag.DELIVERY_CASES["head"]}), 0),
            (valid, 0), (valid, 124), (valid, 137),
            (cargo_log({name}) + (self.MARKER + "\n").encode(), 0),
            (valid.replace(self.MARKER.encode(), (self.MARKER + " sql=PRIVATE_SENTINEL").encode()), 101),
            (valid.replace(self.MARKER.encode(), (self.MARKER + "\n" + self.MARKER).encode()), 101),
            (valid.replace(b"phase=6", b"phase=2"), 101),
            (valid.replace(self.MARKER.encode(),
                           ("---- arbitrary::PRIVATE_SENTINEL stdout ----\n" + self.MARKER).encode()), 101),
        ]
        for raw, exit in others:
            with self.subTest(exit=exit, raw=raw[:32]):
                record, status = diag.diagnose_delivery(raw, "valid", exit, "applied")
                self.assertEqual((status, record["status"]), (1, "diagnostic_invalid"))
                self.assertNotIn("PRIVATE_SENTINEL", json.dumps(record))
        for raw, input_status in [(b"error: build PRIVATE_SENTINEL", "valid"), (None, "missing")]:
            record, status = diag.diagnose_delivery(raw, input_status, 101, "applied")
            self.assertEqual(status, 1)
            self.assertNotEqual(record["status"], "tests_failed")
            self.assertNotIn("PRIVATE_SENTINEL", json.dumps(record))

    def test_delivery_cli_never_prints_invalid_or_success_record(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            raw, output = root / "raw", root / "result.json"
            for payload, exit in [
                (cargo_log({self.CASE}), 0),
                (cargo_log({self.CASE}, failed={self.CASE}, markers={self.CASE: [self.MARKER + " cookie=PRIVATE_SENTINEL"]}), 101),
                (b"error: build PRIVATE_SENTINEL", 101),
            ]:
                raw.write_bytes(payload)
                console = io.StringIO()
                with contextlib.redirect_stdout(console):
                    status = diag.main(["diag", "--delivery", "applied", str(raw), str(exit), str(output)])
                lines = console.getvalue().splitlines()
                self.assertEqual(len(lines), 1)
                self.assertTrue(lines[0].startswith("session_delivery_verdict status="))
                self.assertNotIn("PRIVATE_SENTINEL", lines[0])
                self.assertNotIn("PRIVATE_SENTINEL", output.read_text())
                self.assertEqual(status, 0 if exit == 0 else 1)
            self.assertEqual(diag.main(["diag", "--delivery", "PRIVATE_SENTINEL", str(raw), "101", str(output)]), 2)

    def test_broad_auth_does_not_reclassify_or_leak_native_markers(self):
        lines = []
        for slot, op in [(1, "CsrfIssue"), (2, "Bootstrap")]:
            lines.extend([
                f"auth-stage slot={slot} (test-only; scheduling perturbed)",
                f"op={op} stage=Entry event=Seen elapsed_us=None utc_remaining_ms=None mono=None kind=None outcome=None",
                "lost=false",
            ])
        lines.append("test result: ok. 1 passed; 0 failed; 0 ignored;")
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            snapshots = []
            for extra in ([], [self.MARKER], [self.MARKER + " sql=PRIVATE_SENTINEL"]):
                (root / "auth-stage-broad.raw").write_text("\n".join(lines + extra) + "\n")
                console = io.BytesIO()
                with mock.patch.object(diag.auth_diag.sys, "stdout", mock.Mock(buffer=console)):
                    self.assertTrue(diag.auth_diag.capture("broad", root, 0, {}))
                snapshots.append((console.getvalue(), (root / "auth-stage-broad.trace").read_bytes()))
            self.assertEqual(snapshots[0], snapshots[1])
            self.assertEqual(snapshots[0], snapshots[2])
            self.assertNotIn(b"PRIVATE_SENTINEL", snapshots[2][0])

    def test_delivery_bounded_input_and_unknown_identity_fail_closed(self):
        unknown = "arbitrary::PRIVATE_SENTINEL"
        for raw, state, exit in [
            (b"x" * (diag.RAW_LIMIT + 1), "valid", 101),
            (b"\xffPRIVATE_SENTINEL", "valid", 101),
            (cargo_log({unknown}, failed={unknown}), "valid", 101),
            (cargo_log({self.CASE}), "valid", -1),
            (cargo_log({self.CASE}), "valid", 256),
        ]:
            record, status = diag.diagnose_delivery(raw, state, exit, "applied")
            self.assertEqual((status, record["status"]), (1, "diagnostic_invalid"))
            self.assertNotIn("PRIVATE_SENTINEL", json.dumps(record))


def source_cases(relative):
    text = (ROOT / relative).read_text(encoding="utf-8")
    return set(re.findall(r"#\[tokio::test\]\s+async fn ([A-Za-z_][A-Za-z0-9_]*)\s*\(", text))


def cargo_log(case_names, failed=(), markers=None, statuses=None):
    failed = set(failed)
    markers = markers or {}
    statuses = statuses or {}
    lines = [f"running {len(case_names)} tests"]
    passed = failed_count = ignored = 0
    for name in sorted(case_names):
        status = statuses.get(name, "FAILED" if name in failed else "ok")
        lines.append(f"test {name} ... {status}")
        passed += status == "ok"
        failed_count += status == "FAILED"
        ignored += status == "ignored"
    if failed:
        lines.append("")
        lines.append("failures:")
        for name in sorted(failed):
            lines.append(f"---- {name} stdout ----")
            lines.extend(markers.get(name, []))
            lines.append("thread 'test' panicked at /private/path: token=PRIVATE_SENTINEL")
        lines.append("failures:")
        lines.extend(f"    {name}" for name in sorted(failed))
    result = "FAILED" if failed_count else "ok"
    lines.append(
        f"test result: {result}. {passed} passed; {failed_count} failed; {ignored} ignored; "
        f"0 measured; 0 filtered out; finished in 0.01s"
    )
    return ("\n".join(lines) + "\n").encode("utf-8")


def parse_both(session_raw=None, session_exit=0, browser_raw=None, browser_exit=0):
    if session_raw is None:
        session_raw = cargo_log(diag.SESSION_CASES)
    if browser_raw is None:
        browser_raw = cargo_log(diag.BROWSER_CASES)
    return diag.diagnose(session_raw, "valid", session_exit, browser_raw, "valid", browser_exit)


class SessionDeadlineDiagnosticTests(unittest.TestCase):
    BOOTSTRAP_DEADLINE = (
        "prelogin_bootstrap_deadline_diag status=504 phase=5 event=1 db=4 exit=1 "
        "polls=2 begins=1 commands=0 commits=0 budget_ms=20 entry_remaining_ms=20 "
        "phase_remaining_ms=19 exit_remaining_ms=0 security_budget_ms=2000 selected_end_kind=1"
    )

    BROWSER_PHASE = (
        "browser_login_phase_diag request=1 status=504 phase=13 event=1 before_gate=1 poll_gate=2 "
        "saw_pending=1 pending_phase=13 pending_gate=1 saw_live_pending=1 live_pending_phase=13 "
        "gate_refused=1 row_missing=0 rollback_ack=0 restore_commit_ack=0 exit=5 exit_gate=2 consumed=1"
    )

    def test_browser_phase_closed_grammar_and_bounds(self):
        good = self.BROWSER_PHASE
        self.assertEqual(diag.parse_browser_phase_marker(good)["phase"], 13)
        self.assertIsNotNone(diag.parse_browser_phase_marker(good.replace("request=1", "request=18446744073709551615")))
        for bad in (good + " token=PRIVATE_SENTINEL", "prefix " + good, good + "\n",
                    good.replace("request=1", "request=18446744073709551616"),
                    good.replace("request=1", "request=01"), good.replace("status=504", "status=200"),
                    good.replace("phase=13 event", "phase=17 event"), good.replace("event=1", "event=3"),
                    good.replace("before_gate=1", "before_gate=5"), good.replace("poll_gate=2", "poll_gate=5"),
                    good.replace("pending_phase=13", "pending_phase=17"), good.replace("pending_gate=1", "pending_gate=5"),
                    good.replace("live_pending_phase=13", "live_pending_phase=17"), good.replace("exit=5", "exit=10"),
                    good.replace("exit_gate=2", "exit_gate=5"), good.replace("consumed=1", "consumed=true"),
                    good.replace("saw_pending=1", "saw_pending=2")):
            with self.subTest(bad=bad):
                self.assertIsNone(diag.parse_browser_phase_marker(bad))

    def test_browser_phase_multiplicity_bound_to_failed_test_and_request(self):
        case = diag.CLAIM_MARKER_TEST
        markers = [self.BROWSER_PHASE.replace("request=1", f"request={n}") for n in (1, 2, 3)]
        record, status = parse_both(browser_raw=cargo_log(diag.BROWSER_CASES, failed={case}, markers={case: markers}), browser_exit=101)
        self.assertEqual((status, record["status"]), (1, "tests_failed"))
        self.assertEqual([x["request"] for x in record["suites"]["browser"]["browser_phase"]], [1, 2, 3])
        self.assertTrue(all(x["test"] == case for x in record["suites"]["browser"]["browser_phase"]))
        for markers in ([self.BROWSER_PHASE] * 2,
                        [self.BROWSER_PHASE, self.BROWSER_PHASE.replace("phase=13 event", "phase=14 event")],
                        [self.BROWSER_PHASE + " token=PRIVATE_SENTINEL"]):
            record, status = parse_both(browser_raw=cargo_log(diag.BROWSER_CASES, failed={case}, markers={case: markers}), browser_exit=101)
            self.assertEqual((status, record["status"]), (1, "diagnostic_invalid"))
            self.assertNotIn("PRIVATE_SENTINEL", json.dumps(record))
        session_case = sorted(diag.SESSION_CASES)[0]
        record, _ = parse_both(session_raw=cargo_log(diag.SESSION_CASES, failed={session_case}, markers={session_case: [self.BROWSER_PHASE]}), session_exit=101)
        self.assertEqual(record["status"], "diagnostic_invalid")
        record, _ = parse_both(browser_raw=cargo_log(diag.BROWSER_CASES) + (self.BROWSER_PHASE + "\n").encode())
        self.assertEqual(record["status"], "diagnostic_invalid")
        too_many = [self.BROWSER_PHASE.replace("request=1", f"request={n}") for n in range(17)]
        record, _ = parse_both(browser_raw=cargo_log(diag.BROWSER_CASES, failed={case}, markers={case: too_many}), browser_exit=101)
        self.assertEqual(record["status"], "diagnostic_invalid")

    def test_browser_phase_console_is_validated_failure_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "session-deadline-session.raw").write_bytes(cargo_log(diag.SESSION_CASES))
            for valid in (True, False):
                marker = self.BROWSER_PHASE + ("" if valid else " token=PRIVATE_SENTINEL")
                (root / "session-deadline-browser.raw").write_bytes(cargo_log(diag.BROWSER_CASES, failed={diag.CLAIM_MARKER_TEST}, markers={diag.CLAIM_MARKER_TEST: [marker]}))
                output = io.StringIO()
                with contextlib.redirect_stdout(output):
                    status = diag.main(["diag", str(root), "0", "101", str(root / "result.json")])
                self.assertEqual(status, 1)
                self.assertNotIn("PRIVATE_SENTINEL", output.getvalue())
                lines = output.getvalue().splitlines()
                if valid:
                    self.assertEqual(len(lines), 2)
                    self.assertTrue(lines[0].startswith("session_deadline_verdict status=tests_failed "))
                    self.assertTrue(lines[1].startswith("browser_login_phase="))
                    parsed = json.loads(lines[1].split("=", 1)[1])
                    self.assertEqual((parsed["request"], parsed["consumed"]), (1, 1))
                else:
                    self.assertEqual(len(lines), 1)
                    self.assertTrue(lines[0].startswith("session_deadline_verdict status=diagnostic_invalid "))
            (root / "session-deadline-browser.raw").write_bytes(cargo_log(diag.BROWSER_CASES))
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                status = diag.main(["diag", str(root), "0", "0", str(root / "result.json")])
            self.assertEqual(status, 0)
            self.assertEqual(len(output.getvalue().splitlines()), 1)
            self.assertTrue(output.getvalue().startswith("session_deadline_verdict status=passed "))

    def test_bootstrap_selected_deadline_bounds_and_old_session_grammar(self):
        good = self.BOOTSTRAP_DEADLINE
        observed = diag.parse_bootstrap_deadline_marker(good)
        self.assertEqual((observed["budget_ms"], observed["security_budget_ms"],
                          observed["selected_end_kind"]), (20, 2000, 1))
        self.assertIsNone(diag.parse_session_marker(good))
        for phase in range(5, 9):
            self.assertIsNotNone(diag.parse_bootstrap_deadline_marker(good.replace("phase=5", f"phase={phase}")))
        security = good.replace("security_budget_ms=2000", "security_budget_ms=20").replace("selected_end_kind=1", "selected_end_kind=0")
        self.assertIsNotNone(diag.parse_bootstrap_deadline_marker(security))
        # Strict native Instant ordering can round to equal integer ms values.
        self.assertIsNotNone(diag.parse_bootstrap_deadline_marker(security.replace("selected_end_kind=0", "selected_end_kind=1")))
        malformed = [
            good + " token=PRIVATE_SENTINEL", "prefix " + good, good + "\n",
            good.replace("status=504", "status=200"), good.replace("phase=5", "phase=4"),
            good.replace("phase=5", "phase=9"), good.replace("event=1", "event=3"),
            good.replace("db=4", "db=15"), good.replace("db=4", "db=04"),
            good.replace("exit=1", "exit=5"), good.replace("polls=2", "polls=1024"),
            good.replace("commits=0", "commits=2"),
            good.replace("exit_remaining_ms=0", "exit_remaining_ms=1"),
            good.replace("phase_remaining_ms=19", "phase_remaining_ms=21"),
            good.replace("security_budget_ms=2000", "security_budget_ms=19"),
            good.replace("security_budget_ms=2000", "security_budget_ms=60001"),
            good.replace("selected_end_kind=1", "selected_end_kind=0"),
            good.replace("selected_end_kind=1", "selected_end_kind=2"),
            good.replace("begins=1", "begins=1 begins=1"),
        ]
        for line in malformed:
            with self.subTest(line=line):
                self.assertIsNone(diag.parse_bootstrap_deadline_marker(line))

    def test_bootstrap_deadline_multiplicity_stays_bound_to_failed_browser_test(self):
        case = "committed_claim_stays_consumed_on_verifier_error_panic_and_deadline"
        # A failed loop test can contain multiple earlier, intentional 504s.
        raw = cargo_log(diag.BROWSER_CASES, failed={case}, markers={case: [self.BOOTSTRAP_DEADLINE] * 2})
        record, status = parse_both(browser_raw=raw, browser_exit=101)
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "tests_failed")
        self.assertEqual(record["marker_count"], 2)
        facts = record["suites"]["browser"]["bootstrap_deadline"]
        self.assertEqual(len(facts), 2)
        self.assertTrue(all(fact["test"] == case for fact in facts))
        self.assertNotIn("PRIVATE_SENTINEL", json.dumps(record))
        for raw, session in [
            (cargo_log(diag.BROWSER_CASES) + (self.BOOTSTRAP_DEADLINE + "\n").encode(), False),
            (cargo_log(diag.BROWSER_CASES, failed={case}, markers={case: [self.BOOTSTRAP_DEADLINE] * 17}), False),
            (cargo_log(diag.SESSION_CASES, failed={next(iter(diag.SESSION_CASES))},
                       markers={next(iter(diag.SESSION_CASES)): [self.BOOTSTRAP_DEADLINE]}), True),
        ]:
            record, code = parse_both(session_raw=raw, session_exit=101) if session else parse_both(browser_raw=raw, browser_exit=101)
            self.assertEqual(code, 1)
            self.assertEqual(record["status"], "diagnostic_invalid")

    def test_bootstrap_deadline_unknown_test_and_arbitrary_data_are_not_published(self):
        case = "committed_claim_stays_consumed_on_verifier_error_panic_and_deadline"
        secret_name = "secret_credential_identifier"
        raw = cargo_log((set(diag.BROWSER_CASES) - {case}) | {secret_name},
                        failed={secret_name}, markers={secret_name: [self.BOOTSTRAP_DEADLINE]})
        record, code = parse_both(browser_raw=raw, browser_exit=101)
        self.assertEqual(code, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")
        self.assertEqual(record["suites"]["browser"]["bootstrap_deadline"], [])
        self.assertNotIn(secret_name, json.dumps(record))
        malformed = self.BOOTSTRAP_DEADLINE + " path=/private token=PRIVATE_SENTINEL"
        raw = cargo_log(diag.BROWSER_CASES, failed={case}, markers={case: [malformed]})
        record, code = parse_both(browser_raw=raw, browser_exit=101)
        self.assertEqual(code, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")
        self.assertEqual(record["suites"]["browser"]["bootstrap_deadline"], [])
        self.assertNotIn("PRIVATE_SENTINEL", json.dumps(record))
        self.assertNotIn("/private", json.dumps(record))

    def test_source_inventory_exactly_matches_closed_case_sets(self):
        self.assertEqual(
            source_cases("runtime/tests/session_csrf_public_api.rs"), diag.SESSION_CASES
        )
        self.assertEqual(
            source_cases("runtime/tests/browser_login_public_api.rs"), diag.BROWSER_CASES
        )
        self.assertEqual((len(diag.SESSION_CASES), len(diag.BROWSER_CASES)), (11, 10))

    def test_full_success_inventories_are_accepted_without_suppression(self):
        record, status = parse_both()
        self.assertEqual(status, 0)
        self.assertEqual(record["status"], "passed")
        self.assertEqual(record["suites"]["session"]["inventory_status"], "passed")
        self.assertEqual(record["suites"]["browser"]["inventory_status"], "passed")
        self.assertEqual(record["marker_count"], 0)
        self.assertLess(len(json.dumps(record).encode("ascii")), diag.OUTPUT_LIMIT)

    def test_session_and_browser_closed_failure_markers_and_existing_parser_facts(self):
        session_test = "pending_guarded_authorizer_expires_at_bound_session_and_recovers_permit"
        session_marker_a = (
            "session_deadline_diag status=504 phase=2 event=1 db=10 exit=1 polls=3 begins=1 "
            "commands=2 commits=1 budget_ms=20 entry_remaining_ms=19 phase_remaining_ms=18 exit_remaining_ms=0"
        )
        session_marker_b = (
            "session_deadline_diag status=504 phase=2 event=1 db=11 exit=1 polls=4 begins=1 "
            "commands=2 commits=1 budget_ms=20 entry_remaining_ms=19 phase_remaining_ms=17 exit_remaining_ms=0"
        )
        session_reply_test = "failed_guarded_handler_never_finalizes_committed_rotation_cookie"
        session_reply = (
            "safe_reply_diag check=token_response status=504 class=deadline "
            "set_cookie_count=0 body_bytes=15"
        )
        session_log = cargo_log(
            diag.SESSION_CASES,
            failed=(session_test, session_reply_test),
            markers={session_test: [session_marker_a, session_marker_b], session_reply_test: [session_reply]},
        )

        pair_test = "public_login_claims_once_then_named_verifier_issue_and_appended_cookies"
        bootstrap = (
            "browser_login_bootstrap_diag check=material status=504 class=deadline "
            "set_cookie_count=0 body_bytes=15 auth_cookie=false deletion=false"
        )
        pair = (
            "browser_login_stock_cause case=concurrent_once a_status=504 b_status=403 "
            "a_auth_cookie=false b_auth_cookie=false a_deletion=true b_deletion=false "
            "verifier_calls=1 handler_calls=0 claim_committed=true"
        )
        json_test = "json_value_nodes_and_container_depth_accept_exact_bounds"
        json_bound = (
            "browser_login_json_bound_diag phase=exact_bounds status=200 body_bytes=9 "
            "expected_body_bytes=9 body_matches=false verifier_calls=1 handler_calls=1 claim_committed=true"
        )
        wire_test = "wire_body_cap_is_independent_of_db_quota_and_checked_before_claim"
        wire = (
            "browser_login_public_api_diag case=wire_body_cap phase=exact_body_response iteration=0 "
            "status=504 body_bytes=192 expected_body_bytes=192 body_matches=true "
            "set_cookie_count=1 auth_cookie=false deletion=true"
        )
        io_test = "body_own_deadline_408_security_earlier_or_equal_504_before_claim"
        io = "HTTP test I/O failed: phase=ResponseStatus kind=ConnectionReset"
        browser_log = cargo_log(
            diag.BROWSER_CASES,
            failed=(pair_test, json_test, wire_test, io_test),
            markers={pair_test: [bootstrap, pair], json_test: [json_bound], wire_test: [wire], io_test: [io]},
        )
        record, status = parse_both(session_log, 101, browser_log, 101)
        self.assertEqual(status, 1)  # test failures stay failures after diagnostics
        self.assertEqual(record["status"], "tests_failed")
        self.assertEqual(record["marker_count"], 8)
        self.assertEqual(len(record["suites"]["session"]["session_deadline"]), 2)
        self.assertEqual(len(record["suites"]["session"]["public"]), 1)
        self.assertEqual(record["suites"]["browser"]["bootstrap"][0]["class"], "deadline")
        self.assertEqual(record["suites"]["browser"]["pair"][0]["case"], "concurrent_once")
        # A false equality fact with matching lengths is a valid failure observation.
        self.assertFalse(record["suites"]["browser"]["json_bound"][0]["body_matches"])
        self.assertEqual(record["suites"]["browser"]["json_bound"][0]["body_bytes"], 9)
        self.assertEqual(record["suites"]["browser"]["public"][0]["kind"], "browser_wire")
        self.assertEqual(record["suites"]["browser"]["http_test_io"][0]["kind"], "ConnectionReset")
        rendered = json.dumps(record)
        self.assertNotIn("PRIVATE_SENTINEL", rendered)
        self.assertNotIn("/private/path", rendered)
        self.assertNotIn("token=", rendered)
        self.assertNotIn("panicked", rendered)

    def test_session_numeric_limits_ordering_and_duplicate_controls(self):
        good = (
            "session_deadline_diag status=504 phase=1 event=1 db=4 exit=1 polls=1 begins=1 "
            "commands=0 commits=0 budget_ms=20 entry_remaining_ms=19 phase_remaining_ms=18 exit_remaining_ms=0"
        )
        self.assertIsNotNone(diag.parse_session_marker(good))
        bad = [
            good + " path=/private",
            good.replace("status=504", "status=200"),
            good.replace("phase=1", "phase=5"),
            good.replace("event=1", "event=3"),
            good.replace("db=4", "db=15"),
            good.replace("exit=1", "exit=5"),
            good.replace("polls=1", "polls=1024"),
            good.replace("commits=0", "commits=2"),
            good.replace("phase_remaining_ms=18", "phase_remaining_ms=21"),
            good.replace("exit_remaining_ms=0", "exit_remaining_ms=1"),
            good.replace("db=4", "db=04"),
            good.replace("db=4", "db=/private/path"),
            good.replace("event=1", "event=1 event=1"),
            good.replace("commits=0 ", "commits=0 secret=token "),
        ]
        for line in bad:
            with self.subTest(line=line):
                self.assertIsNone(diag.parse_session_marker(line))
        case = "pending_guarded_authorizer_expires_at_bound_session_and_recovers_permit"
        duplicate = cargo_log(diag.SESSION_CASES, failed={case}, markers={case: [good, good]})
        record, code = parse_both(duplicate, 101)
        self.assertEqual(code, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")
        self.assertEqual(record["suites"]["session"]["diagnostic_status"], "invalid")

    def test_browser_status_classes_bounds_and_failure_only_conditions(self):
        self.assertIsNotNone(diag.parse_bootstrap_marker(
            "browser_login_bootstrap_diag check=material status=504 class=deadline "
            "set_cookie_count=0 body_bytes=15 auth_cookie=false deletion=false"
        ))
        for status, classification in ((400, "request"), (401, "refusal"), (403, "refusal"),
                                       (500, "internal"), (503, "unavailable"), (504, "deadline"),
                                       (201, "other"), (302, "other")):
            line = (f"browser_login_bootstrap_diag check=material status={status} class={classification} "
                    "set_cookie_count=0 body_bytes=15 auth_cookie=false deletion=false")
            self.assertIsNotNone(diag.parse_bootstrap_marker(line))
        bad_bootstrap = [
            "browser_login_bootstrap_diag check=material status=504 class=unavailable set_cookie_count=0 body_bytes=15 auth_cookie=false deletion=false",
            "browser_login_bootstrap_diag check=material status=201 class=success set_cookie_count=0 body_bytes=15 auth_cookie=false deletion=false",
            "browser_login_bootstrap_diag check=material status=200 class=success set_cookie_count=0 body_bytes=0 auth_cookie=false deletion=false",
            "browser_login_bootstrap_diag check=material status=503 class=unavailable set_cookie_count=66 body_bytes=15 auth_cookie=false deletion=false",
            "browser_login_bootstrap_diag check=material status=504 class=deadline set_cookie_count=0 body_bytes=8194 auth_cookie=false deletion=false",
            "browser_login_bootstrap_diag check=material status=504 class=deadline set_cookie_count=0 body_bytes=15 auth_cookie=false deletion=false path=/private",
        ]
        for line in bad_bootstrap:
            with self.subTest(line=line):
                self.assertIsNone(diag.parse_bootstrap_marker(line))
        expected_pair = (
            "browser_login_stock_cause case=concurrent_once a_status=504 b_status=403 "
            "a_auth_cookie=false b_auth_cookie=false a_deletion=true b_deletion=false "
            "verifier_calls=1 handler_calls=0 claim_committed=true"
        )
        self.assertIsNotNone(diag.parse_pair_marker(expected_pair))
        self.assertIsNone(diag.parse_pair_marker(expected_pair.replace("a_status=504", "a_status=200")))
        self.assertIsNone(diag.parse_pair_marker(expected_pair + " token=private"))
        exact = (
            "browser_login_json_bound_diag phase=exact_bounds status=200 body_bytes=9 "
            "expected_body_bytes=9 body_matches=false verifier_calls=1 handler_calls=1 claim_committed=true"
        )
        self.assertIsNotNone(diag.parse_json_bound_marker(exact))
        self.assertIsNone(diag.parse_json_bound_marker(exact.replace("expected_body_bytes=9", "expected_body_bytes=8")))
        self.assertIsNone(diag.parse_json_bound_marker(exact + " /private"))

    def test_unknown_case_duplicate_rows_and_old_inventory_fail_closed(self):
        old_names = diag.SESSION_CASES - {"controlled_same_file_writer_reports_native_token_identity_boundary"}
        old_log = cargo_log(old_names)
        record, status = parse_both(old_log, 0)
        self.assertEqual(status, 1)
        self.assertEqual(record["suites"]["session"]["inventory_status"], "inventory_invalid")

        unknown = set(diag.BROWSER_CASES)
        unknown.remove("input_and_strict_json_rejections_do_not_consume_or_call_verifier")
        unknown.add("unknown_case")
        record, status = parse_both(browser_raw=cargo_log(unknown))
        self.assertEqual(status, 1)
        self.assertEqual(record["suites"]["browser"]["inventory_status"], "inventory_invalid")

        duplicate = cargo_log(diag.BROWSER_CASES).decode()
        duplicate = duplicate.replace("running 10 tests", "running 11 tests", 1)
        duplicate = duplicate.replace("test result: ok. 10 passed", "test result: ok. 11 passed", 1)
        first = sorted(diag.BROWSER_CASES)[0]
        duplicate = duplicate.replace(f"test {first} ... ok", f"test {first} ... ok\ntest {first} ... ok", 1)
        record, status = parse_both(browser_raw=duplicate.encode())
        self.assertEqual(status, 1)
        self.assertEqual(record["suites"]["browser"]["inventory_status"], "inventory_invalid")

    def test_unknown_failed_rows_and_stdout_blocks_never_enter_sanitized_output(self):
        unknown_case_sentinel = "unregistered_case_PRIVATE_SENTINEL"
        # Public synthetic test bytes, never a real credential or captured log.
        marker = (
            "session_deadline_diag status=504 phase=1 event=1 db=4 exit=1 polls=1 begins=1 "
            "commands=0 commits=0 budget_ms=20 entry_remaining_ms=19 "
            "phase_remaining_ms=18 exit_remaining_ms=0"
        )

        known = "pending_guarded_authorizer_expires_at_bound_session_and_recovers_permit"
        unknown_cases = (set(diag.SESSION_CASES) - {known}) | {unknown_case_sentinel}
        unknown_row_log = cargo_log(
            unknown_cases, failed={unknown_case_sentinel}, markers={unknown_case_sentinel: [marker]}
        )
        record, status = parse_both(session_raw=unknown_row_log, session_exit=101)
        rendered = json.dumps(record)
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")
        self.assertEqual(record["suites"]["session"]["failed_cases"], [])
        self.assertEqual(record["suites"]["session"]["session_deadline"], [])
        self.assertNotIn(unknown_case_sentinel, rendered)

        known_failure = cargo_log(
            diag.SESSION_CASES, failed={known}, markers={known: [marker]}
        ).decode()
        unknown_stdout = known_failure.replace(
            "test result: FAILED.",
            f"---- {unknown_case_sentinel} stdout ----\n{marker}\nfailures:\ntest result: FAILED.",
            1,
        ).encode()
        record, status = parse_both(session_raw=unknown_stdout, session_exit=101)
        rendered = json.dumps(record)
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")
        self.assertEqual(record["suites"]["session"]["session_deadline"][0]["test"], known)
        self.assertEqual(len(record["suites"]["session"]["session_deadline"]), 1)
        self.assertNotIn(unknown_case_sentinel, rendered)

        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "session-deadline-session.raw").write_bytes(unknown_row_log)
            (root / "session-deadline-browser.raw").write_bytes(cargo_log(diag.BROWSER_CASES))
            output = root / "diagnostic.json"
            stdout, stderr = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                code = diag.main(["session_deadline_diagnostic.py", str(root), "101", "0", str(output)])
            self.assertEqual(code, 1)
            self.assertEqual(len(stdout.getvalue().splitlines()), 1)
            self.assertIn("status=diagnostic_invalid", stdout.getvalue())
            self.assertEqual(stderr.getvalue(), "")
            cli_json = output.read_text(encoding="utf-8")
            self.assertNotIn(unknown_case_sentinel, cli_json)
            self.assertNotIn("PRIVATE_SENTINEL", cli_json)

    def test_duplicate_browser_marker_and_nonfailed_context_are_rejected(self):
        case = "public_login_claims_once_then_named_verifier_issue_and_appended_cookies"
        marker = (
            "browser_login_stock_cause case=concurrent_once a_status=504 b_status=403 "
            "a_auth_cookie=false b_auth_cookie=false a_deletion=true b_deletion=false "
            "verifier_calls=1 handler_calls=0 claim_committed=true"
        )
        duplicate = cargo_log(diag.BROWSER_CASES, failed={case}, markers={case: [marker, marker]})
        record, status = parse_both(browser_raw=duplicate, browser_exit=101)
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")
        self.assertEqual(record["suites"]["browser"]["diagnostic_status"], "invalid")

        success_marker = cargo_log(diag.BROWSER_CASES).decode() + marker + "\n"
        record, status = parse_both(browser_raw=success_marker.encode())
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")

    def test_committed_claim_markers_are_closed_source_bound_and_private(self):
        case = "committed_claim_stays_consumed_on_verifier_error_panic_and_deadline"
        initial = (
            "browser_login_public_api_diag case=committed_claim_initial phase=initial_response "
            "iteration=0 expected_status=401 status=504 auth_cookie=false deletion=true "
            "verifier_calls=1 handler_calls=0 claim_committed=true"
        )
        retry = (
            "browser_login_public_api_diag case=committed_claim_retry phase=retry_response "
            "iteration=0 status=504 body_bytes=15 set_cookie_count=0 auth_cookie=false "
            "deletion=false verifier_calls=1 handler_calls=0 claim_committed=true"
        )
        replay = (
            "browser_login_public_api_diag case=committed_claim_replay_proof iteration=0 "
            "row_absent=false verifier_unchanged=true handler_unchanged=true"
        )
        expected = {
            initial: "claim_initial",
            retry: "claim_retry",
            replay: "claim_replay_proof",
        }
        for marker, kind in expected.items():
            with self.subTest(kind=kind):
                browser_log = cargo_log(diag.BROWSER_CASES, failed={case}, markers={case: [marker]})
                record, status = parse_both(browser_raw=browser_log, browser_exit=101)
                self.assertEqual(status, 1)  # a valid marker does not suppress the test failure
                self.assertEqual(record["status"], "tests_failed")
                self.assertEqual(record["suites"]["browser"]["diagnostic_status"], "valid")
                self.assertEqual(len(record["suites"]["browser"][kind]), 1)
                fact = record["suites"]["browser"][kind][0]
                self.assertEqual(fact["test"], case)
                self.assertEqual(fact["case"], kind.replace("claim_", "committed_claim_"))
                self.assertEqual(fact["iteration"], 0)
                rendered = json.dumps(record)
                self.assertNotIn("PRIVATE_SENTINEL", rendered)
                self.assertNotIn("/private/path", rendered)

        # Retry and its independent post-claim proof can both be emitted in one iteration.
        browser_log = cargo_log(
            diag.BROWSER_CASES, failed={case}, markers={case: [retry, replay]}
        )
        record, status = parse_both(browser_raw=browser_log, browser_exit=101)
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "tests_failed")
        self.assertEqual(record["suites"]["browser"]["diagnostic_status"], "valid")

        # Source emits neither marker for successful initial/retry boundary shapes.
        impossible_success_shapes = (
            initial.replace("status=504", "status=401"),
            retry.replace("status=504", "status=403"),
        )
        for marker in impossible_success_shapes:
            with self.subTest(success_shape=marker.split(" phase=")[0]):
                record, status = parse_both(
                    browser_raw=cargo_log(diag.BROWSER_CASES, failed={case}, markers={case: [marker]}),
                    browser_exit=101,
                )
                self.assertEqual(status, 1)
                self.assertEqual(record["status"], "diagnostic_invalid")

        # A proof marker without a retry marker is emitted only if proof is incomplete.
        replay_all_true = replay.replace("row_absent=false", "row_absent=true")
        record, status = parse_both(
            browser_raw=cargo_log(diag.BROWSER_CASES, failed={case}, markers={case: [replay_all_true]}),
            browser_exit=101,
        )
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")

        # The source prints retry before awaiting independent row/counter probes and proof.
        reverse_order = cargo_log(
            diag.BROWSER_CASES, failed={case}, markers={case: [replay, retry]}
        )
        record, status = parse_both(browser_raw=reverse_order, browser_exit=101)
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")

        bad_lines = [
            initial.replace("iteration=0", "iteration=1"),  # expected status is source-fixed per iteration
            initial.replace("verifier_calls=1", "verifier_calls=1024"),
            retry.replace("body_bytes=15", "body_bytes=8193"),
            retry.replace("set_cookie_count=0", "set_cookie_count=8193"),
            replay.replace("iteration=0", "iteration=5"),
            initial + " token=PRIVATE_SENTINEL",
            initial.replace(" handler_calls=0", ""),
            "browser_login_public_api_diag case=committed_claim_unknown phase=unknown iteration=0 token=PRIVATE_SENTINEL",
        ]
        for marker in bad_lines:
            with self.subTest(marker=marker):
                browser_log = cargo_log(diag.BROWSER_CASES, failed={case}, markers={case: [marker]})
                record, status = parse_both(browser_raw=browser_log, browser_exit=101)
                self.assertEqual(status, 1)
                self.assertEqual(record["status"], "diagnostic_invalid")
                self.assertNotIn("PRIVATE_SENTINEL", json.dumps(record))

        wrong_test = "public_login_claims_once_then_named_verifier_issue_and_appended_cookies"
        browser_log = cargo_log(
            diag.BROWSER_CASES, failed={wrong_test}, markers={wrong_test: [initial]}
        )
        record, status = parse_both(browser_raw=browser_log, browser_exit=101)
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")

        session_case = next(iter(diag.SESSION_CASES))
        session_log = cargo_log(diag.SESSION_CASES, failed={session_case}, markers={session_case: [retry]})
        record, status = parse_both(session_raw=session_log, session_exit=101)
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")

        mixed_iteration = cargo_log(
            diag.BROWSER_CASES,
            failed={case},
            markers={case: [retry, replay.replace("iteration=0", "iteration=1")]},
        )
        record, status = parse_both(browser_raw=mixed_iteration, browser_exit=101)
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")

        impossible_order = cargo_log(
            diag.BROWSER_CASES, failed={case}, markers={case: [initial, retry]}
        )
        record, status = parse_both(browser_raw=impossible_order, browser_exit=101)
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")

        duplicate = cargo_log(
            diag.BROWSER_CASES, failed={case}, markers={case: [retry, retry]}
        )
        record, status = parse_both(browser_raw=duplicate, browser_exit=101)
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")

    def test_compile_failure_zero_tests_ignored_and_unexpected_success_marker(self):
        compile_raw = b"error: compiler failed; source=/private/path token=PRIVATE_SENTINEL\n"
        record, status = parse_both(compile_raw, 101)
        self.assertEqual(status, 1)
        self.assertEqual(record["suites"]["session"]["inventory_status"], "no_test_execution")
        self.assertNotIn("PRIVATE_SENTINEL", json.dumps(record))
        zero = b"running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n"
        record, status = parse_both(zero, 0)
        self.assertEqual(status, 1)
        self.assertEqual(record["suites"]["session"]["inventory_status"], "zero_tests")
        ignored = cargo_log(diag.BROWSER_CASES, statuses={next(iter(diag.BROWSER_CASES)): "ignored"})
        record, status = parse_both(browser_raw=ignored)
        self.assertEqual(status, 1)
        self.assertEqual(record["suites"]["browser"]["inventory_status"], "ignored_tests")
        unexpected = cargo_log(diag.SESSION_CASES).decode() + (
            "session_deadline_diag status=504 phase=1 event=1 db=4 exit=1 polls=1 begins=1 "
            "commands=0 commits=0 budget_ms=20 entry_remaining_ms=19 phase_remaining_ms=18 exit_remaining_ms=0\n"
        )
        record, status = parse_both(unexpected.encode(), 0)
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")

    def test_marker_limit_raw_limit_utf8_and_output_bound(self):
        case = "pending_guarded_authorizer_expires_at_bound_session_and_recovers_permit"
        many = []
        for poll in range(1, 18):
            many.append(
                "session_deadline_diag status=504 phase=1 event=1 db=4 exit=1 "
                f"polls={poll} begins=1 commands=0 commits=0 budget_ms=20 "
                "entry_remaining_ms=19 phase_remaining_ms=18 exit_remaining_ms=0"
            )
        record, status = parse_both(cargo_log(diag.SESSION_CASES, failed={case}, markers={case: many}), 101)
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")

        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            over = root / "over.raw"
            over.write_bytes(b"x" * (diag.RAW_LIMIT + 1))
            content, state = diag._read_bounded(over)
            self.assertIsNone(content)
            self.assertEqual(state, "oversize")
            bad_utf8 = root / "utf8.raw"
            bad_utf8.write_bytes(b"\xff")
            content, state = diag._read_bounded(bad_utf8)
            self.assertIsNone(content)
            self.assertEqual(state, "invalid_utf8")
            output = root / "result.json"
            sample, _ = parse_both()
            self.assertTrue(diag._write_output(output, sample))
            self.assertLessEqual(output.stat().st_size, diag.OUTPUT_LIMIT)
            with mock.patch.object(diag, "OUTPUT_LIMIT", 2):
                self.assertFalse(diag._write_output(output, sample))
                self.assertLessEqual(output.stat().st_size, 128)


    def test_browser_assertion_markers_are_closed_and_bind_to_failed_case(self):
        issue_response_marker = "browser_login_assertion_diag case=issue_commit mode=12 stage=issue_response expected_status=504 status=503 auth_cookie=unobserved deletion=unobserved"
        valid = (
            "browser_login_assertion_diag case=committed_claim mode=3 stage=initial_response expected_status=504 status=500 auth_cookie=unobserved deletion=unobserved",
            "browser_login_assertion_diag case=committed_claim mode=4 stage=initial_headers expected_status=504 status=504 cache_no_store=true vary_match=false",
            "browser_login_assertion_diag case=committed_claim mode=4 stage=initial_state expected_status=504 status=504 prelogin_rows_empty=false verifier_calls=unobserved handler_calls=unobserved",
            "browser_login_assertion_diag case=committed_claim mode=1 stage=initial_challenge expected_status=401 status=401 bearer_challenge=false",
            "browser_login_assertion_diag case=committed_claim mode=5 stage=initial_challenge expected_status=401 status=401 bearer_challenge=false",
            "browser_login_assertion_diag case=committed_claim mode=3 stage=retry_response expected_status=403 status=500 auth_cookie=unobserved deletion=unobserved",
            "browser_login_assertion_diag case=committed_claim mode=3 stage=retry_headers expected_status=403 status=403 cache_no_store=false vary_match=unobserved",
            "browser_login_assertion_diag case=committed_claim mode=3 stage=retry_headers expected_status=403 status=500 cache_no_store=false vary_match=unobserved",
            "browser_login_assertion_diag case=committed_claim mode=3 stage=replay_proof expected_status=403 status=500 row_absent=true verifier_unchanged=false handler_unchanged=true",
            issue_response_marker,
            "browser_login_assertion_diag case=issue_commit mode=12 stage=issue_headers expected_status=504 status=504 cache_no_store=true vary_match=false",
            "browser_login_assertion_diag case=issue_commit mode=12 stage=issue_rows expected_status=504 status=504 prelogin_rows_empty=true session_rows=2",
        )
        for marker in valid:
            with self.subTest(marker=marker):
                self.assertIsNotNone(diag.parse_assertion_marker(marker))
        cookie_failure = "browser_login_assertion_diag case=issue_commit mode=12 stage=issue_response expected_status=504 status=504 auth_cookie=true deletion=unobserved"
        self.assertIsNotNone(diag.parse_assertion_marker(cookie_failure))

        issue_test = "issue_commit_then_handler_err_panic_or_timeout_never_delivers_auth_cookie"
        marker = issue_response_marker
        raw = cargo_log(diag.BROWSER_CASES, failed={issue_test},
                        markers={issue_test: [self.BROWSER_PHASE, marker]})
        record, status = parse_both(browser_raw=raw, browser_exit=101)
        self.assertEqual((status, record["status"]), (1, "tests_failed"))
        facts = record["suites"]["browser"]["assertion"]
        self.assertEqual(len(facts), 1)
        self.assertEqual((facts[0]["test"], facts[0]["case"], facts[0]["mode"], facts[0]["stage"]),
                         (issue_test, "issue_commit", 12, "issue_response"))
        self.assertEqual((facts[0]["expected_status"], facts[0]["status"]), (504, 503))
        self.assertEqual(record["suites"]["browser"]["browser_phase"][0]["phase"], 13)
        self.assertNotIn("PRIVATE_SENTINEL", json.dumps(record))

        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "session-deadline-session.raw").write_bytes(cargo_log(diag.SESSION_CASES))
            (root / "session-deadline-browser.raw").write_bytes(raw)
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                code = diag.main(["diag", str(root), "0", "101", str(root / "result.json")])
            self.assertEqual(code, 1)
            self.assertIn("browser_login_assertion=", output.getvalue())
            self.assertNotIn("PRIVATE_SENTINEL", output.getvalue())
            serialized = output.getvalue().split("browser_login_assertion=", 1)[1].splitlines()[0]
            self.assertEqual(json.loads(serialized)["stage"], "issue_response")

    def test_browser_assertion_marker_rejects_bad_shapes_and_nonfailures(self):
        good = "browser_login_assertion_diag case=issue_commit mode=12 stage=issue_response expected_status=504 status=503 auth_cookie=unobserved deletion=unobserved"
        self.assertIsNotNone(diag.parse_assertion_marker(good))
        impossible_challenge = "browser_login_assertion_diag case=committed_claim mode=3 stage=initial_challenge expected_status=504 status=504 bearer_challenge=false"
        self.assertIsNone(diag.parse_assertion_marker(impossible_challenge))
        rejected = (
            good + " token=PRIVATE_SENTINEL",
            "prefix " + good,
            good.replace("case=issue_commit", "case=unknown"),
            good.replace("mode=12", "mode=13"),
            good.replace("stage=issue_response", "stage=initial_response"),
            good.replace("expected_status=504", "expected_status=500"),
            good.replace("status=503", "status=600"),
            good.replace("auth_cookie=unobserved", "auth_cookie=0"),
            good.replace("deletion=unobserved", "deletion=1"),
            "browser_login_assertion_diag case=issue_commit mode=12 stage=issue_rows expected_status=504 status=504 prelogin_rows_empty=true session_rows=1024",
            "browser_login_assertion_diag case=issue_commit mode=12 stage=issue_rows expected_status=504 status=504 prelogin_rows_empty=false session_rows=0",
            "browser_login_assertion_diag case=issue_commit mode=12 stage=issue_rows expected_status=504 status=504 prelogin_rows_empty=true session_rows=unobserved",
            "browser_login_assertion_diag case=committed_claim mode=4 stage=initial_state expected_status=504 status=504 prelogin_rows_empty=false verifier_calls=1 handler_calls=unobserved",
            "browser_login_assertion_diag case=committed_claim mode=4 stage=initial_state expected_status=504 status=504 prelogin_rows_empty=true verifier_calls=0 handler_calls=0",
            "browser_login_assertion_diag case=committed_claim mode=4 stage=initial_state expected_status=504 status=504 prelogin_rows_empty=true verifier_calls=1024 handler_calls=0",
            "browser_login_assertion_diag case=committed_claim mode=4 stage=initial_response expected_status=504 status=504 auth_cookie=false deletion=true",
            "browser_login_assertion_diag case=issue_commit mode=12 stage=issue_response expected_status=504 status=504 auth_cookie=false deletion=true",
            good + "\n",
            good + " raw/path=/private",
            good + " " + ("x" * 513),
        )
        for marker in rejected:
            with self.subTest(marker=marker[:100]):
                self.assertIsNone(diag.parse_assertion_marker(marker))
        legacy = "browser_login_assertion_diag case=issue_commit phase=issue_response status=504"
        self.assertIsNone(diag.parse_assertion_marker(legacy))
        issue_test = "issue_commit_then_handler_err_panic_or_timeout_never_delivers_auth_cookie"
        record, _ = parse_both(
            browser_raw=cargo_log(diag.BROWSER_CASES, failed={issue_test},
                                  markers={issue_test: [legacy]}),
            browser_exit=101,
        )
        self.assertEqual(record["status"], "diagnostic_invalid")

    def test_browser_assertion_markers_require_exact_failed_test_and_bounded_count(self):
        issue_test = "issue_commit_then_handler_err_panic_or_timeout_never_delivers_auth_cookie"
        claim_test = diag.CLAIM_MARKER_TEST
        issue = "browser_login_assertion_diag case=issue_commit mode=12 stage=issue_rows expected_status=504 status=504 prelogin_rows_empty=true session_rows=2"
        claim = "browser_login_assertion_diag case=committed_claim mode=4 stage=initial_state expected_status=504 status=504 prelogin_rows_empty=false verifier_calls=unobserved handler_calls=unobserved"

        record, _ = parse_both(browser_raw=cargo_log(diag.BROWSER_CASES, failed={issue_test}, markers={issue_test: [issue]}), browser_exit=101)
        self.assertEqual(len(record["suites"]["browser"]["assertion"]), 1)
        wrong_case = issue.replace("case=issue_commit", "case=committed_claim")
        record, _ = parse_both(browser_raw=cargo_log(diag.BROWSER_CASES, failed={issue_test}, markers={issue_test: [wrong_case]}), browser_exit=101)
        self.assertEqual(record["status"], "diagnostic_invalid")
        record, _ = parse_both(browser_raw=cargo_log(diag.BROWSER_CASES, failed={claim_test}, markers={claim_test: [issue]}), browser_exit=101)
        self.assertEqual(record["status"], "diagnostic_invalid")
        record, _ = parse_both(browser_raw=cargo_log(diag.BROWSER_CASES, failed={issue_test}, markers={issue_test: [issue, issue]}), browser_exit=101)
        self.assertEqual(record["status"], "diagnostic_invalid")
        record, _ = parse_both(browser_raw=cargo_log(diag.BROWSER_CASES, failed={claim_test, issue_test}, markers={claim_test: [claim, claim], issue_test: [issue]}), browser_exit=101)
        self.assertEqual(record["status"], "diagnostic_invalid")
        third_test = "input_and_strict_json_rejections_do_not_consume_or_call_verifier"
        with mock.patch.dict(diag.ASSERTION_TEST_CASES, {third_test: "committed_claim"}):
            record, _ = parse_both(
                browser_raw=cargo_log(
                    diag.BROWSER_CASES,
                    failed={claim_test, issue_test, third_test},
                    markers={claim_test: [claim], issue_test: [issue], third_test: [claim]},
                ),
                browser_exit=101,
            )
        self.assertEqual(record["status"], "diagnostic_invalid")
        zero = cargo_log(diag.BROWSER_CASES, failed={issue_test})
        record, _ = parse_both(browser_raw=zero, browser_exit=101)
        self.assertEqual(record["status"], "tests_failed")
        self.assertEqual(record["suites"]["browser"]["assertion"], [])


class TokenFirstSliceTests(unittest.TestCase):
    CASE = 'guarded_rotation_and_logout_invalidate_old_generation_tokens'
    MARKER = (
        'csrf_token_deadline_diag status=504 phase=2 event=1 db=4 exit=1 '
        'polls=2 begins=1 commands=0 commits=0 budget_ms=1000 '
        'entry_remaining_ms=999 phase_remaining_ms=998 exit_remaining_ms=0 terminal=5'
    )

    def test_token_stage_terminal_survive_failed_case_sanitization(self):
        raw = cargo_log(diag.SESSION_CASES, failed={self.CASE}, markers={self.CASE: [self.MARKER]})
        record, code = parse_both(session_raw=raw, session_exit=101)
        self.assertEqual((record['status'], code), ('tests_failed', 1))
        facts = record['suites']['session'].get('token_deadline', [])
        self.assertEqual(len(facts), 1)
        self.assertEqual((facts[0]['test'], facts[0]['phase'], facts[0]['event'], facts[0]['terminal']),
                         (self.CASE, 2, 1, 5))
        self.assertNotIn('PRIVATE_SENTINEL', json.dumps(record))

    def test_terminal_mapping_and_exact_bounded_grammar(self):
        parse = getattr(diag, 'parse_token_marker', lambda _: None)
        # Same existing stage/event/native grammar, plus one closed terminal enum.
        for terminal, exit_code in ((1, 0), (2, 2), (3, 3), (4, 4), (5, 1), (6, 0)):
            marker = self.MARKER.replace('exit=1 ', f'exit={exit_code} ').replace('terminal=5', f'terminal={terminal}')
            with self.subTest(terminal=terminal):
                fact = parse(marker)
                self.assertIsNotNone(fact)
                self.assertEqual(fact['terminal'], terminal)
        for bad in (self.MARKER + ' raw=PRIVATE_SENTINEL', self.MARKER + '\n',
                    self.MARKER.replace('terminal=5', 'terminal=0'),
                    self.MARKER.replace('terminal=5', 'terminal=7'),
                    self.MARKER.replace('terminal=5', 'terminal=05'),
                    self.MARKER.replace('terminal=5', 'terminal=999999999999'),
                    self.MARKER.replace('exit=1 ', 'exit=4 '),
                    self.MARKER.replace('phase=2', 'phase=9'),
                    self.MARKER.replace('begins=1', 'begins=1024'),
                    self.MARKER.replace('budget_ms=1000', 'budget_ms=60001'),
                    self.MARKER.replace('exit_remaining_ms=0', 'exit_remaining_ms=1')):
            with self.subTest(bad=bad[:70]):
                self.assertIsNone(parse(bad))

    def test_malformed_unbound_wrong_suite_and_duplicates_refuse(self):
        valid = cargo_log(diag.SESSION_CASES, failed={self.CASE}, markers={self.CASE: [self.MARKER]})
        bad_payloads = (
            valid.replace(self.MARKER.encode(), (self.MARKER + ' secret=PRIVATE_SENTINEL').encode()),
            valid.replace(self.MARKER.encode(), (self.MARKER + '\n' + self.MARKER).encode()),
            valid + (self.MARKER + '\n').encode(),
            cargo_log(diag.SESSION_CASES) + (self.MARKER + '\n').encode(),
        )
        for raw in bad_payloads:
            record, code = parse_both(session_raw=raw, session_exit=101)
            self.assertEqual((record['status'], code), ('diagnostic_invalid', 1))
            self.assertNotIn('PRIVATE_SENTINEL', json.dumps(record))
        browser = sorted(diag.BROWSER_CASES)[0]
        record, code = parse_both(browser_raw=cargo_log(diag.BROWSER_CASES, failed={browser}, markers={browser: [self.MARKER]}), browser_exit=101)
        self.assertEqual((record['status'], code), ('diagnostic_invalid', 1))

    def test_cli_prints_only_valid_failed_closed_token_facts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / 'session-deadline-session.raw').write_bytes(cargo_log(diag.SESSION_CASES, failed={self.CASE}, markers={self.CASE: [self.MARKER]}))
            (root / 'session-deadline-browser.raw').write_bytes(cargo_log(diag.BROWSER_CASES))
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                code = diag.main(['diag', str(root), '101', '0', str(root / 'record.json')])
            self.assertEqual(code, 1)
            lines = output.getvalue().splitlines()
            self.assertEqual(len(lines), 2)
            self.assertTrue(lines[0].startswith('session_deadline_verdict status=tests_failed '))
            line = lines[1]
            self.assertTrue(line.startswith('csrf_token_deadline='))
            self.assertLess(len(line.encode('ascii')), 512)
            self.assertNotIn('PRIVATE_SENTINEL', output.getvalue())
            fact = json.loads(line.split('=', 1)[1])
            self.assertEqual((fact['test'], fact['terminal']), (self.CASE, 5))
            for raw, exit_code in ((cargo_log(diag.SESSION_CASES), 0),
                                   (cargo_log(diag.SESSION_CASES, failed={self.CASE}, markers={self.CASE: [self.MARKER + ' raw=PRIVATE_SENTINEL']}), 101)):
                (root / 'session-deadline-session.raw').write_bytes(raw)
                output = io.StringIO()
                with contextlib.redirect_stdout(output):
                    result = diag.main(['diag', str(root), str(exit_code), '0', str(root / 'record.json')])
                self.assertEqual(result, 0 if exit_code == 0 else 1)
                self.assertEqual(len(output.getvalue().splitlines()), 1)
                self.assertTrue(output.getvalue().startswith('session_deadline_verdict status='))


class SessionVerdictStdoutTests(unittest.TestCase):
    def test_main_emits_closed_status_for_pass_missing_and_no_test_execution(self):
        expected_pass = (
            "session_deadline_verdict status=passed session_input=valid session_inventory=passed "
            "session_diagnostic=valid browser_input=valid browser_inventory=passed "
            "browser_diagnostic=valid markers=0"
        )
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / "session-deadline-session.raw").write_bytes(cargo_log(diag.SESSION_CASES))
            (root / "session-deadline-browser.raw").write_bytes(cargo_log(diag.BROWSER_CASES))
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                status = diag.main(["diag", str(root), "0", "0", str(root / "result.json")])
            self.assertEqual(status, 0)
            self.assertEqual(stdout.getvalue().splitlines(), [expected_pass])
            self.assertEqual(json.loads((root / "result.json").read_text())["status"], "passed")

        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                status = diag.main(["diag", str(root), "0", "0", str(root / "result.json")])
            self.assertEqual(status, 1)
            self.assertEqual(stdout.getvalue().splitlines(), [
                "session_deadline_verdict status=diagnostic_invalid "
                "session_input=missing session_inventory=input_unavailable session_diagnostic=valid "
                "browser_input=missing browser_inventory=input_unavailable browser_diagnostic=valid markers=0"
            ])

        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / "session-deadline-session.raw").write_bytes(b"error: compile failed at /private/path PRIVATE_SENTINEL\n")
            (root / "session-deadline-browser.raw").write_bytes(cargo_log(diag.BROWSER_CASES))
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                status = diag.main(["diag", str(root), "101", "0", str(root / "result.json")])
            self.assertEqual(status, 1)
            line = stdout.getvalue().strip()
            self.assertIn("status=tests_not_run_or_incomplete", line)
            self.assertIn("session_inventory=no_test_execution", line)
            self.assertNotIn("PRIVATE_SENTINEL", line)
            self.assertNotIn("/private/path", line)

    def test_delivery_mode_emits_its_own_status_line_and_preserves_json(self):
        case = diag.DELIVERY_CASES["applied"]
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            raw, output = root / "input.raw", root / "record.json"
            raw.write_bytes(cargo_log({case}))
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                status = diag.main(["diag", "--delivery", "applied", str(raw), "0", str(output)])
            self.assertEqual(status, 0)
            self.assertEqual(stdout.getvalue().splitlines(), [
                "session_delivery_verdict status=passed input=valid inventory=passed diagnostic=valid markers=0"
            ])
            self.assertEqual(json.loads(output.read_text())["status"], "passed")

    def test_verdict_projection_rejects_wrong_schema_types_unknown_keys_and_oversize(self):
        import copy
        record, _ = diag.diagnose(cargo_log(diag.SESSION_CASES), "valid", 0,
                                  cargo_log(diag.BROWSER_CASES), "valid", 0)
        valid_line = diag.closed_verdict_line(record)
        self.assertTrue(valid_line.startswith("session_deadline_verdict status=passed "))
        self.assertLessEqual(len(valid_line.encode("ascii")), diag.VERDICT_LINE_LIMIT)
        mutations = (
            lambda value: value.update({"private-cause": "PRIVATE_SENTINEL"}),
            lambda value: value.update({"schema": "unknown-schema"}),
            lambda value: value.update({"status": "private-failure"}),
            lambda value: value.update({"marker_count": True}),
            lambda value: value.update({"marker_count": diag.RAW_LIMIT + 1}),
            lambda value: value["source_inventory"].update({"session_tests": True}),
            lambda value: value["suites"]["session"].update({"inventory_status": "private-test"}),
            lambda value: value["suites"]["browser"].update({"diagnostic_status": None}),
            lambda value: value["suites"]["session"].update({"input_status": "private-path"}),
            lambda value: value["suites"]["session"].update({"failed_cases": ["PRIVATE_SENTINEL"]}),
            lambda value: value["suites"]["session"].update({"assertion": [{}] * 6000}),
        )
        for mutate in mutations:
            poisoned = copy.deepcopy(record)
            mutate(poisoned)
            line = diag.closed_verdict_line(poisoned)
            self.assertEqual(line, "session_deadline_verdict unavailable")
            self.assertLessEqual(len(line.encode("ascii")), diag.VERDICT_LINE_LIMIT)
            self.assertNotIn("PRIVATE_SENTINEL", line)

        delivery, _ = diag.diagnose_delivery(cargo_log({diag.DELIVERY_CASES["applied"]}), "valid", 0, "applied")
        delivery_line = diag.closed_verdict_line(delivery, delivery=True)
        self.assertTrue(delivery_line.startswith("session_delivery_verdict status=passed "))
        self.assertLessEqual(len(delivery_line.encode("ascii")), diag.DELIVERY_VERDICT_LINE_LIMIT)

        case = diag.CLAIM_MARKER_TEST
        marker = SessionDeadlineDiagnosticTests.BROWSER_PHASE
        over_limit, _ = diag.diagnose(
            cargo_log(diag.SESSION_CASES), "valid", 0,
            cargo_log(diag.BROWSER_CASES, failed={case}, markers={case: [marker] * (diag.MARKER_LIMIT + 1)}),
            "valid", 101,
        )
        over_limit_line = diag.closed_verdict_line(over_limit)
        self.assertTrue(over_limit_line.startswith("session_deadline_verdict status=diagnostic_invalid "))
        self.assertTrue(over_limit_line.endswith("markers=over_limit"))


class SessionNativeCallSupplementTests(unittest.TestCase):
    CASE = diag.CSRF_REUSABLE_CASE
    UNSEEN = diag.NATIVE_CALL_UNSEEN_US

    @classmethod
    def marker(cls, **changes):
        values = {
            "status": 504, "origin": 0, "terminal": 0, "diagnostic_valid": 1,
            "native_call_state": 2, "generation": 1, "call_enter_us": 123,
            "call_return_us": cls.UNSEEN, "native_state": 1, "commit_result": 0,
            "settled_us": cls.UNSEEN, "begins": 1, "commits": 0,
            "selected_deadline_passed": 1,
        }
        values.update(changes)
        return (f"{diag.NATIVE_CALL_PREFIX} status={values['status']} " + " ".join(
            f"{name}={values[name]}" for name in diag.NATIVE_CALL_FIELDS[1:]))

    def failed_record(self, raw, cargo_exit=101):
        browser = cargo_log(diag.BROWSER_CASES)
        return diag.diagnose(raw, "valid", cargo_exit, browser, "valid", 0)

    def test_closed_parser_accepts_all_visible_states_and_emits_fixed_facts(self):
        markers = [
            self.marker(origin=1, terminal=5, native_call_state=1, call_enter_us=self.UNSEEN,
                        call_return_us=self.UNSEEN, native_state=4),
            self.marker(),
            self.marker(origin=1, terminal=6, native_call_state=3, call_return_us=456),
            self.marker(native_call_state=4, call_return_us=456, native_state=2,
                        commit_result=1, commits=1),
            self.marker(origin=3, terminal=2, native_call_state=4, call_return_us=456,
                        native_state=7, commit_result=2, settled_us=789, commits=1),
            self.marker(diagnostic_valid=0, native_call_state=0, generation=0,
                        call_enter_us=self.UNSEEN, call_return_us=self.UNSEEN,
                        native_state=0, commit_result=0, settled_us=self.UNSEEN,
                        begins=0, commits=0, selected_deadline_passed=0),
        ]
        for line in markers:
            with self.subTest(line=line):
                parsed = diag.parse_native_call_marker(line)
                self.assertIsNotNone(parsed)
                self.assertTrue(diag._native_call_fact_valid(parsed))
                self.assertEqual(set(parsed), set(diag.NATIVE_CALL_FIELDS))

    def test_checkout_returned_settlement_cannot_precede_observed_native_return(self):
        impossible = self.marker(native_call_state=4, call_enter_us=10, call_return_us=20,
                                 native_state=7, commit_result=1, settled_us=1, commits=1)
        self.assertIsNone(diag.parse_native_call_marker(impossible))
        equal = self.marker(native_call_state=4, call_enter_us=10, call_return_us=20,
                            native_state=7, commit_result=1, settled_us=20, commits=1)
        self.assertIsNotNone(diag.parse_native_call_marker(equal))

    def test_closed_parser_refuses_suffix_types_ranges_and_state_conflicts(self):
        good = self.marker()
        bad = [
            good + " secret=PRIVATE_SENTINEL", "prefix " + good, good + "\n",
            good.replace("status=504", "status=200"),
            good.replace("origin=0 terminal=0", "origin=0 terminal=1"),
            good.replace("origin=0 terminal=0", "origin=1 terminal=7"),
            good.replace("native_call_state=2", "native_call_state=02"),
            good.replace("generation=1", "generation=1023"),
            good.replace("call_return_us=60000001", "call_return_us=true"),
            good.replace("call_return_us=60000001", "call_return_us=60000002"),
            good.replace("call_return_us=60000001", "call_return_us=122"),
            good.replace("native_state=1", "native_state=2"),
            good.replace("settled_us=60000001", "settled_us=1"),
            good.replace("selected_deadline_passed=1", "selected_deadline_passed=true"),
            good + " " + ("x" * diag.NATIVE_CALL_LINE_LIMIT),
        ]
        for line in bad:
            with self.subTest(line=line[:100]):
                self.assertIsNone(diag.parse_native_call_marker(line))

        unavailable = self.marker(diagnostic_valid=0, native_call_state=0, generation=0,
                                  call_enter_us=self.UNSEEN, call_return_us=self.UNSEEN,
                                  native_state=0, commit_result=0, settled_us=self.UNSEEN,
                                  begins=0, commits=0, selected_deadline_passed=0)
        for corrupt in [unavailable.replace("selected_deadline_passed=0", "selected_deadline_passed=1"),
                        unavailable.replace("call_enter_us=60000001", "call_enter_us=0")]:
            self.assertIsNone(diag.parse_native_call_marker(corrupt))

    def test_projection_is_failed_case_scoped_and_preserves_duplicate_multiset(self):
        marker = self.marker()
        raw = cargo_log(diag.SESSION_CASES, failed={self.CASE}, markers={self.CASE: [marker, marker]})
        record, status = self.failed_record(raw)
        self.assertEqual((record["status"], status), ("tests_failed", 1))
        lines = diag.session_native_call_lines(raw, 101, record)
        self.assertEqual(len(lines), 2)
        facts = [json.loads(line.split("=", 1)[1]) for line in lines]
        self.assertEqual(facts[0], facts[1])
        self.assertEqual((facts[0]["suite"], facts[0]["test"], facts[0]["native_call_state"]),
                         ("session", self.CASE, 2))
        self.assertEqual(set(facts[0]), {"suite", "test", *diag.NATIVE_CALL_FIELDS})
        self.assertTrue(all(len(line.encode("ascii")) <= 4096 for line in lines))
        self.assertNotIn("PRIVATE_SENTINEL", "\n".join(lines))

        other = next(iter(diag.SESSION_CASES - {self.CASE}))
        unbound = cargo_log(diag.SESSION_CASES, failed={other}, markers={other: [marker]})
        unbound_record, _ = self.failed_record(unbound)
        self.assertEqual(diag.session_native_call_lines(unbound, 101, unbound_record),
                         ["session_native_call=unavailable"])
        self.assertEqual(diag.session_native_call_lines(raw, 101, None),
                         ["session_native_call=unavailable"])
        self.assertEqual(diag.session_native_call_lines(raw, 0, record),
                         ["session_native_call=unavailable"])
        without_marker = cargo_log(diag.SESSION_CASES, failed={self.CASE})
        no_marker_record, _ = self.failed_record(without_marker)
        self.assertEqual(diag.session_native_call_lines(without_marker, 101, no_marker_record),
                         ["session_native_call=unavailable"])

    def test_malformed_inventory_source_and_sensitive_suffix_never_escape(self):
        marker = self.marker()
        cases = [
            cargo_log(diag.SESSION_CASES, failed={self.CASE},
                      markers={self.CASE: [marker + " cookie=PRIVATE_SENTINEL"]}),
            cargo_log(diag.SESSION_CASES, failed={self.CASE},
                      markers={self.CASE: [marker + "\n" + marker + " path=/private/PRIVATE_SENTINEL"]}),
            cargo_log(diag.SESSION_CASES, failed={self.CASE}, markers={self.CASE: [marker]}) +
                (f"---- {self.CASE} stdout ----\n{marker}\n".encode()),
        ]
        for raw in cases:
            with self.subTest(raw_length=len(raw)):
                record, _ = self.failed_record(raw)
                lines = diag.session_native_call_lines(raw, 101, record)
                self.assertEqual(lines, ["session_native_call=unavailable"])
                self.assertNotIn("PRIVATE_SENTINEL", "\n".join(lines))

        valid = cargo_log(diag.SESSION_CASES, failed={self.CASE}, markers={self.CASE: [marker]})
        record, _ = self.failed_record(valid)
        with mock.patch.object(diag, "_csrf_source_current", return_value=False):
            self.assertEqual(diag.session_native_call_lines(valid, 101, record),
                             ["session_native_call=unavailable"])
        self.assertEqual(diag.session_native_call_lines(valid + b"x" * diag.RAW_LIMIT, 101, record),
                         ["session_native_call=unavailable"])
        self.assertEqual(diag.session_native_call_lines(b"\xffPRIVATE_SENTINEL", 101, record),
                         ["session_native_call=unavailable"])

    def test_cli_keeps_v2_bytes_and_failure_exit_while_printing_bounded_facts(self):
        raw = cargo_log(diag.SESSION_CASES, failed={self.CASE}, markers={self.CASE: [self.marker()]})
        expected, expected_status = self.failed_record(raw)
        expected_bytes = (json.dumps(expected, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")
        with tempfile.TemporaryDirectory() as folder:
            root = pathlib.Path(folder)
            (root / "session-deadline-session.raw").write_bytes(raw)
            (root / "session-deadline-browser.raw").write_bytes(cargo_log(diag.BROWSER_CASES))
            output = root / "record.json"
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                status = diag.main(["diag", str(root), "101", "0", str(output)])
            self.assertEqual((status, expected_status), (1, 1))
            self.assertEqual(output.read_bytes(), expected_bytes)
            lines = stdout.getvalue().splitlines()
            self.assertEqual(lines[0], diag.closed_verdict_line(expected))
            self.assertEqual(lines[-1].split("=", 1)[0], "session_native_call")
            fact = json.loads(lines[-1].split("=", 1)[1])
            self.assertEqual(fact["test"], self.CASE)
            self.assertNotIn("PRIVATE_SENTINEL", stdout.getvalue())

    def test_projection_caps_fact_count_and_emits_only_a_fixed_unavailable_line(self):
        raw = cargo_log(diag.SESSION_CASES, failed={self.CASE},
                        markers={self.CASE: [self.marker()] * (diag.NATIVE_CALL_FACT_LIMIT + 1)})
        record, _ = self.failed_record(raw)
        self.assertEqual(diag.session_native_call_lines(raw, 101, record),
                         ["session_native_call=unavailable"])


class BrowserNativeCallSupplementTests(unittest.TestCase):
    CASE = "public_login_claims_once_then_named_verifier_issue_and_appended_cookies"

    def browser_raw(self, markers=None, failed=None):
        return cargo_log(diag.BROWSER_CASES, failed=failed or {self.CASE},
                         markers=markers or {self.CASE: [SessionNativeCallSupplementTests.marker(origin=2)]})

    def test_browser_cli_preserves_v2_bytes_and_failure_exit_with_bound_numeric_fact(self):
        raw = self.browser_raw()
        expected, verdict = parse_both(browser_raw=raw, browser_exit=101)
        self.assertEqual((expected['status'], verdict), ('tests_failed', 1))
        with tempfile.TemporaryDirectory() as folder:
            root = pathlib.Path(folder)
            (root/'session-deadline-session.raw').write_bytes(cargo_log(diag.SESSION_CASES))
            (root/'session-deadline-browser.raw').write_bytes(raw)
            output = root/'record.json'; stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                code = diag.main(['diag', str(root), '0', '101', str(output)])
            self.assertEqual(code, 1)
            self.assertEqual(output.read_bytes(), (json.dumps(expected, sort_keys=True, separators=(',', ':'))+'\n').encode('ascii'))
            lines = [line for line in stdout.getvalue().splitlines() if line.startswith('session_native_call=')]
            self.assertEqual(len(lines), 1, 'browser native-call evidence must survive the real CLI')
            fact = json.loads(lines[0].split('=', 1)[1])
            self.assertEqual((fact['suite'], fact['test'], fact['origin']), ('browser', self.CASE, 2))
            self.assertEqual(set(fact), {'suite', 'test', *diag.NATIVE_CALL_FIELDS})
            self.assertLessEqual(len(lines[0].encode('ascii')), 4096)

    def test_browser_projection_keeps_failed_case_identity_and_duplicate_multiset(self):
        other = 'committed_claim_stays_consumed_on_verifier_error_panic_and_deadline'
        marker = SessionNativeCallSupplementTests.marker(origin=2)
        raw = self.browser_raw({self.CASE: [marker, marker], other: [marker]}, {self.CASE, other})
        record, _ = parse_both(browser_raw=raw, browser_exit=101)
        lines = diag.session_native_call_lines(raw, 101, record, suite_name='browser')
        facts = [json.loads(line.split('=', 1)[1]) for line in lines]
        self.assertCountEqual([fact['test'] for fact in facts], [self.CASE, self.CASE, other])
        duplicates = [fact for fact in facts if fact['test'] == self.CASE]
        self.assertEqual(duplicates[0], duplicates[1])

    def test_browser_source_unknown_case_unbound_marker_and_sensitive_suffix_refuse(self):
        marker = SessionNativeCallSupplementTests.marker(origin=2)
        raw = self.browser_raw(); record, _ = parse_both(browser_raw=raw, browser_exit=101)
        with mock.patch.object(diag, '_browser_native_call_source_current', return_value=False):
            self.assertEqual(diag.session_native_call_lines(raw, 101, record, suite_name='browser'), ['session_native_call=unavailable'])
        for candidate in [raw + (marker+'\n').encode(),
                          raw.replace(marker.encode(), (marker+' secret=PRIVATE_SENTINEL').encode()),
                          raw.replace(self.CASE.encode(), b'unknown_case_sentinel'),
                          self.browser_raw({self.CASE: [marker]*(diag.NATIVE_CALL_FACT_LIMIT+1)})]:
            result, _ = parse_both(browser_raw=candidate, browser_exit=101)
            lines = diag.session_native_call_lines(candidate, 101, result, suite_name='browser')
            self.assertEqual(lines, ['session_native_call=unavailable'])
            self.assertNotIn('PRIVATE_SENTINEL', '\n'.join(lines))
        self.assertEqual(diag.session_native_call_lines(raw, 101, record, suite_name='unknown_case_sentinel'), ['session_native_call=unavailable'])


class ReusableCsrfFailureSupplementTests(unittest.TestCase):
    CASE = 'one_reusable_token_admits_two_distinct_actual_requests'
    PARALLEL = ('csrf_public_assertion_diag phase=ParallelReplies status_one=200 status_two=504 '
                'checked_calls=None calls_at_failure=1 checked_rows=None')
    GUARDED = ('csrf_guarded_deadline_diag status=504 phase=12 event=1 db=4 exit=1 '
               'polls=2 begins=1 commands=0 commits=0 budget_ms=1000 entry_remaining_ms=999 '
               'phase_remaining_ms=980 exit_remaining_ms=0 selected_deadline_passed=1 terminal=5')
    LOCATION = ("thread 'one_reusable_token_admits_two_distinct_actual_requests' panicked at "
                'runtime/tests/session_csrf_public_api.rs:936:5:')

    def sample(self, markers=None, exact=False, failed=True):
        raw = cargo_log({self.CASE} if exact else diag.SESSION_CASES,
                        failed={self.CASE} if failed else set(),
                        markers={self.CASE: markers or [self.LOCATION, self.PARALLEL, self.GUARDED]})
        raw = raw.replace(b"thread 'test' panicked at /private/path: token=PRIVATE_SENTINEL\n", b'')
        record = None if exact else parse_both(session_raw=raw, session_exit=101 if failed else 0)[0]
        return raw, record

    def test_current_unknown_failure_has_closed_source_location_and_unassigned_deadline_fact(self):
        raw, record = self.sample()
        observed = diag.csrf_reusable_observation(raw, 101, record=record)
        self.assertEqual(observed['location'], {'file': 'session_csrf_public_api.rs', 'line': 936, 'column': 5})
        self.assertEqual(observed['assertion']['status_two'], 504)
        self.assertIsNone(observed['assertion']['checked_calls'])
        self.assertEqual(len(observed['guarded']), 1)
        self.assertNotIn('request', observed['guarded'][0])
        rendered = diag.csrf_reusable_line(observed)
        self.assertTrue(rendered.startswith('csrf_reusable_failure={'))
        self.assertLessEqual(len(rendered.encode('ascii')), 4096)
        self.assertNotIn('/workspace', rendered)

    def test_readiness_source_pin_and_exact_line_bound(self):
        source = pathlib.Path(diag.__file__).resolve().parents[2] / 'runtime/tests/session_csrf_public_api.rs'
        canonical = source.read_bytes().replace(b'\r\n', b'\n')
        self.assertEqual(hashlib.sha256(canonical).hexdigest(), diag.CSRF_REUSABLE_SOURCE)
        self.assertEqual(len(canonical.splitlines()), 1249)
        raw, record = self.sample()
        observed = diag.csrf_reusable_observation(raw, 101, record=record)
        for line, valid in [(1249, True), (1250, False)]:
            candidate = dict(observed, location={'file': 'session_csrf_public_api.rs', 'line': line, 'column': 1})
            self.assertEqual(diag.csrf_reusable_line(candidate) != 'csrf_reusable_failure unavailable', valid)

    def test_two_identical_guarded_observations_are_a_multiset_not_request_correlation(self):
        marker = self.PARALLEL.replace('status_one=200', 'status_one=504')
        raw, record = self.sample([self.LOCATION, marker, self.GUARDED, self.GUARDED])
        observed = diag.csrf_reusable_observation(raw, 101, record=record)
        self.assertEqual(len(observed['guarded']), 2)
        self.assertEqual(observed['guarded'][0], observed['guarded'][1])
        raw, record = self.sample([self.LOCATION, self.PARALLEL, self.GUARDED, self.GUARDED])
        self.assertIsNone(diag.csrf_reusable_observation(raw, 101, record=record))

    def test_rust_thread_id_suffix_is_bounded_and_never_persisted(self):
        raw, record = self.sample([self.LOCATION, self.PARALLEL, self.GUARDED])
        baseline = diag.csrf_reusable_line(diag.csrf_reusable_observation(raw, 101, record=record))
        for identifier in ['7', '4294967295', '18446744073709551615']:
            location = self.LOCATION.replace("' panicked", f"' ({identifier}) panicked")
            raw, record = self.sample([location, self.PARALLEL, self.GUARDED])
            observed = diag.csrf_reusable_observation(raw, 101, record=record)
            self.assertEqual(observed['location']['line'], 936)
            self.assertEqual(diag.csrf_reusable_line(observed), baseline)
        for identifier in ['0', '07', '1' * 21, 'PRIVATE_SENTINEL']:
            raw, record = self.sample([self.LOCATION.replace("' panicked", f"' ({identifier}) panicked")])
            self.assertIsNone(diag.csrf_reusable_observation(raw, 101, record=record))

    def test_setup_and_shutdown_panics_do_not_fabricate_response_statuses(self):
        for line in [224, 543, 1200]:
            with self.subTest(line=line):
                raw, record = self.sample([self.LOCATION.replace(':936:', f':{line}:')])
                observed = diag.csrf_reusable_observation(raw, 101, record=record)
                self.assertEqual(observed['location']['line'], line)
                self.assertIsNone(observed['assertion'])
                self.assertEqual(observed['guarded'], [])

    def test_duplicate_malformed_wrong_location_and_sensitive_suffix_are_refused(self):
        bad_lists = [
            [self.LOCATION, self.LOCATION, self.PARALLEL],
            [self.LOCATION, self.PARALLEL, self.PARALLEL],
            [self.LOCATION.replace(':936:', ':1250:'), self.PARALLEL],
            [self.LOCATION.replace('session_csrf_public_api.rs', 'foreign.rs'), self.PARALLEL],
            [self.LOCATION, self.PARALLEL + ' token=PRIVATE_SENTINEL'],
            [self.LOCATION, self.PARALLEL.replace('checked_calls=None', 'checked_calls=Some(2)')],
            [self.LOCATION, self.GUARDED.replace('terminal=5', 'terminal=99')],
            [self.LOCATION, self.GUARDED.replace('budget_ms=1000', 'budget_ms=0600')],
            [self.LOCATION, self.GUARDED] * 3,
        ]
        for markers in bad_lists:
            with self.subTest(markers=markers):
                raw, record = self.sample(markers)
                observed = diag.csrf_reusable_observation(raw, 101, record=record)
                self.assertIsNone(observed)
                self.assertNotIn('PRIVATE_SENTINEL', diag.csrf_reusable_line(observed))

    def test_source_change_invalid_records_nonfailure_and_incomplete_inputs_are_refused(self):
        raw, record = self.sample()
        with mock.patch.object(diag, '_csrf_source_current', return_value=False):
            self.assertIsNone(diag.csrf_reusable_observation(raw, 101, record=record))
        for altered in [None, {}, {'status': 'tests_failed'}]:
            self.assertIsNone(diag.csrf_reusable_observation(raw, 101, record=altered))
        self.assertIsNone(diag.csrf_reusable_observation(raw, 0, record=record))
        self.assertIsNone(diag.csrf_reusable_observation(raw + b'\xff', 101, record=record))
        self.assertIsNone(diag.csrf_reusable_observation(b'x' * (diag.RAW_LIMIT + 1), 101, record=record))
        observed = diag.csrf_reusable_observation(raw, 101, record=record)
        observed['injected'] = 'PRIVATE_SENTINEL'
        self.assertEqual(diag.csrf_reusable_line(observed), 'csrf_reusable_failure unavailable')

    def test_exact_inventory_is_separate_and_normal_v2_remains_unchanged(self):
        raw, _ = self.sample(exact=True)
        self.assertIsNone(diag.csrf_reusable_observation(raw, 101, record=parse_both(session_raw=raw, session_exit=101)[0]))
        observed = diag.csrf_reusable_observation(raw, 101, execution='exact')
        self.assertEqual(observed['execution'], 'exact')
        self.assertEqual(observed['status'], 'observed')
        passed, _ = self.sample(exact=True, failed=False)
        accepted = diag.csrf_reusable_observation(passed, 0, execution='exact')
        self.assertEqual(accepted['status'], 'tests_passed')
        self.assertIsNone(accepted['location'])
        self.assertIsNone(diag.csrf_reusable_observation(raw, 0, execution='exact'))

    def test_cli_preserves_v2_json_and_failed_exit_while_adding_one_bounded_line(self):
        raw, baseline = self.sample()
        with tempfile.TemporaryDirectory() as folder:
            root = pathlib.Path(folder)
            (root / 'session-deadline-session.raw').write_bytes(raw)
            (root / 'session-deadline-browser.raw').write_bytes(cargo_log(diag.BROWSER_CASES))
            output = root / 'result.json'
            console = io.StringIO()
            with contextlib.redirect_stdout(console):
                status = diag.main(['diag', str(root), '101', '0', str(output)])
            self.assertEqual(status, 1)
            self.assertEqual(json.loads(output.read_text()), baseline)
            lines = console.getvalue().splitlines()
            self.assertEqual(len(lines), 3)
            self.assertTrue(lines[1].startswith('csrf_reusable_failure={'))
            self.assertEqual(lines[2], 'session_native_call=unavailable')
            self.assertNotIn('PRIVATE_SENTINEL', console.getvalue())

    def test_exact_cli_requires_one_actual_case_and_preserves_cargo_failure(self):
        with tempfile.TemporaryDirectory() as folder:
            source = pathlib.Path(folder) / 'raw'
            for failed, expected in [(False, 0), (True, 1)]:
                raw, _ = self.sample(exact=True, failed=failed)
                source.write_bytes(raw)
                console = io.StringIO()
                with contextlib.redirect_stdout(console):
                    status = diag.main(['diag', '--csrf-reusable-only', str(source), '101' if failed else '0'])
                self.assertEqual(status, expected)
                self.assertEqual(len(console.getvalue().splitlines()), 1)
                self.assertNotIn('PRIVATE_SENTINEL', console.getvalue())
            source.write_bytes(cargo_log(set()))
            console = io.StringIO()
            with contextlib.redirect_stdout(console):
                status = diag.main(['diag', '--csrf-reusable-only', str(source), '0'])
            self.assertEqual(status, 1)
            self.assertEqual(console.getvalue(), 'csrf_reusable_failure unavailable\n')

    def test_token_failure_and_closed_shape_corruptions(self):
        token = 'csrf_public_assertion_diag phase=TokenReply status=503 body_bytes=12 set_cookie_count=0'
        raw, record = self.sample([self.LOCATION.replace(':936:', ':519:'), token])
        observed = diag.csrf_reusable_observation(raw, 101, record=record)
        self.assertEqual(observed['assertion']['phase'], 'TokenReply')
        self.assertEqual(observed['assertion']['status'], 503)
        raw_with_guarded, record_with_guarded = self.sample([
            self.LOCATION.replace(':936:', ':519:'), token, self.GUARDED])
        self.assertIsNone(diag.csrf_reusable_observation(raw_with_guarded, 101, record=record_with_guarded))
        changed = dict(observed, guarded=[dict(zip(diag._CSRF_GUARDED_KEYS,
            map(int, diag._CSRF_GUARDED.fullmatch(self.GUARDED).groups())))])
        self.assertEqual(diag.csrf_reusable_line(changed), 'csrf_reusable_failure unavailable')
        for key, value in [('schema_version', True), ('guarded', [None]), ('location', {}),
                           ('execution', 'foreign'), ('source_sha256', 'PRIVATE_SENTINEL')]:
            changed = dict(observed, **{key: value})
            self.assertEqual(diag.csrf_reusable_line(changed), 'csrf_reusable_failure unavailable')
        for marker in [token.replace('status=503 body_bytes=12', 'status=200 body_bytes=60'),
                       token.replace('set_cookie_count=0', 'set_cookie_count=99999'),
                       token + ' token=PRIVATE_SENTINEL']:
            raw, record = self.sample([self.LOCATION, marker])
            self.assertIsNone(diag.csrf_reusable_observation(raw, 101, record=record))


class BrowserFailureCounterSupplementTests(unittest.TestCase):
    CASE = "input_and_strict_json_rejections_do_not_consume_or_call_verifier"
    WIRE_CASE = "wire_body_cap_is_independent_of_db_quota_and_checked_before_claim"
    FAMILY = "sf03_browser_failure_counter"
    PREFIX = "browser_failure_counter="
    UNAVAILABLE = [PREFIX + "unavailable"]

    @staticmethod
    def marker():
        # Reuse the established sanitizer's fixtures; no second grammar.
        from test_sf07_checkpoint_summary import browser_strict_counter_line
        return (browser_strict_counter_line().replace("status=403", "status=504")
                .replace("verifier_calls=0", "verifier_calls=1")
                .replace("handler_calls=0", "handler_calls=1")
                .replace("claim_committed=false", "claim_committed=true"))

    def raw(self, markers=None, failed=None):
        raw = cargo_log(diag.BROWSER_CASES, failed=failed or {self.CASE},
                        markers=markers if markers is not None else {self.CASE: [self.marker()]})
        # The generic primary-parser fixture deliberately contains a secret
        # sentinel. An ordinary assertion is needed for a valid strict sidecar.
        return raw.replace(b"thread 'test' panicked at /private/path: token=PRIVATE_SENTINEL",
                           b"assertion failed: good.status == 200")

    @staticmethod
    def wire_markers(iteration=0):
        from test_sf07_checkpoint_summary import browser_wire_counter_line
        # The existing public parser also requires the wire witness emitted
        # immediately before the counter. Keep that earlier contract intact.
        public = (f"browser_login_public_api_diag case=wire_body_cap phase=exact_body_response iteration={iteration} "
                  "status=504 body_bytes=15 expected_body_bytes=192 body_matches=false "
                  "set_cookie_count=1 auth_cookie=false deletion=true")
        counter = (browser_wire_counter_line(iteration).replace("status=200", "status=504")
                   .replace(" body_bytes=192", " body_bytes=15")
                   .replace("body_matches=true", "body_matches=false")
                   .replace("set_cookie_count=2", "set_cookie_count=1")
                   .replace("auth_cookie=true", "auth_cookie=false"))
        return [public, counter]

    def test_cli_preserves_v2_bytes_exit_and_marker_count_while_retaining_existing_counter(self):
        raw = self.raw()
        record, expected_status = parse_both(browser_raw=raw, browser_exit=101)
        expected_bytes = (json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")
        self.assertEqual((record["status"], expected_status, record["marker_count"]), ("tests_failed", 1, 0))
        with tempfile.TemporaryDirectory() as folder:
            root = pathlib.Path(folder)
            (root / "session-deadline-session.raw").write_bytes(cargo_log(diag.SESSION_CASES))
            (root / "session-deadline-browser.raw").write_bytes(raw)
            output = root / "result.json"
            console = io.StringIO()
            with contextlib.redirect_stdout(console):
                status = diag.main(["diag", str(root), "0", "101", str(output)])
            self.assertEqual(status, 1)
            self.assertEqual(output.read_bytes(), expected_bytes)
            lines = [line for line in console.getvalue().splitlines() if line.startswith(self.PREFIX)]
            self.assertEqual(len(lines), 1, "the existing failed BrowserLogin counter must survive raw deletion")
            fact = json.loads(lines[0].removeprefix(self.PREFIX))
            self.assertEqual((fact["test"], fact["case"], fact["status"]), (self.CASE, "strict_json_valid", 504))
            self.assertEqual((fact["verifier_calls"], fact["handler_calls"], fact["issue_committed"]), (1, 1, False))

    def test_both_families_crlf_and_existing_phase_facts_keep_separate_counts(self):
        for iteration in (0, 1):
            raw = self.raw(markers={
                self.CASE: [SessionDeadlineDiagnosticTests.BROWSER_PHASE, self.marker()],
                self.WIRE_CASE: self.wire_markers(iteration),
            }, failed={self.CASE, self.WIRE_CASE})
            for payload in [raw, raw.replace(b"\n", b"\r\n")]:
                record, status = parse_both(browser_raw=payload, browser_exit=101)
                before = json.dumps(record, sort_keys=True)
                lines = diag.browser_failure_counter_lines(payload, 101, record)
                self.assertEqual(len(lines), 2)
                samples = [json.loads(line.removeprefix(self.PREFIX)) for line in lines]
                self.assertEqual([sample["test"] for sample in samples], [self.CASE, self.WIRE_CASE])
                self.assertEqual([sample.get("iteration") for sample in samples], [None, iteration])
                self.assertEqual((status, record["marker_count"], len(record["suites"]["browser"]["browser_phase"])), (1, 2, 1))
                self.assertEqual(json.dumps(record, sort_keys=True), before)
                self.assertTrue(all(len(line.encode("ascii")) <= 2048 for line in lines))

    def test_owner_binding_adds_protection_beyond_the_existing_numeric_parser(self):
        import sf07_checkpoint_summary as checkpoints
        public, counter = self.wire_markers()
        raw = self.raw(markers={self.CASE: [counter], self.WIRE_CASE: [public, self.marker()]},
                       failed={self.CASE, self.WIRE_CASE})
        record, _ = parse_both(browser_raw=raw, browser_exit=101)
        self.assertEqual(record["status"], "tests_failed")
        projection = checkpoints._browser_counter_profile_projection(raw, "browser-counter-parallel")
        self.assertEqual((projection["state"], projection["counter_state"]), ("test_failed", "present"))
        self.assertEqual(diag.browser_failure_counter_lines(raw, 101, record), self.UNAVAILABLE)

    def test_each_failed_owner_emits_only_one_counter_before_the_assertion_exits(self):
        import sf07_checkpoint_summary as checkpoints
        public, first = self.wire_markers(0)
        second = self.wire_markers(1)[1]
        raw = self.raw(markers={self.WIRE_CASE: [public, first, second]}, failed={self.WIRE_CASE})
        record, _ = parse_both(browser_raw=raw, browser_exit=101)
        self.assertEqual(record["status"], "tests_failed")
        projection = checkpoints._browser_counter_profile_projection(raw, "browser-counter-parallel")
        self.assertEqual((projection["state"], projection["counter_state"]), ("test_failed", "present"))
        self.assertEqual(diag.browser_failure_counter_lines(raw, 101, record), self.UNAVAILABLE)

    def test_absence_and_success_do_not_add_output_to_existing_cli_contracts(self):
        for raw, exit_code in [(cargo_log(diag.BROWSER_CASES), 0), (self.raw(markers={}), 101)]:
            record, _ = parse_both(browser_raw=raw, browser_exit=exit_code)
            self.assertEqual(diag.browser_failure_counter_lines(raw, exit_code, record), [])

    def test_all_occurrences_must_belong_to_the_exact_failed_owner(self):
        from test_sf07_checkpoint_summary import browser_wire_counter_line
        marker = self.marker()
        good = self.raw()
        foreign = next(name for name in diag.BROWSER_CASES if name not in {self.CASE, self.WIRE_CASE})
        candidates = [
            self.raw(markers={self.CASE: [marker, marker]}),
            (marker + "\n").encode() + self.raw(markers={}),
            self.raw(markers={}) + (marker + "\n").encode(),
            self.raw(markers={foreign: [marker]}, failed={foreign}),
            self.raw(markers={self.CASE: [browser_wire_counter_line()], self.WIRE_CASE: [marker]},
                     failed={self.CASE, self.WIRE_CASE}),
            good.replace(("---- " + self.CASE + " stdout ----").encode(),
                         ("---- " + self.CASE + " stdout ----\n---- " + self.CASE + " stdout ----").encode()),
            good.replace(("---- " + self.CASE + " stdout ----").encode(),
                         ("---- " + foreign + " stdout ----").encode()),
        ]
        for raw in candidates:
            with self.subTest(raw_hash=hashlib.sha256(raw).hexdigest()):
                record, _ = parse_both(browser_raw=raw, browser_exit=101)
                self.assertEqual(diag.browser_failure_counter_lines(raw, 101, record), self.UNAVAILABLE)

    def test_existing_secret_rejection_and_marker_grammar_never_render_private_text(self):
        marker = self.marker()
        malformed = [
            marker + " token=PRIVATE_SENTINEL",
            marker + " sql=PRIVATE_SENTINEL",
            "PRIVATE_SENTINEL " + marker,
            marker.replace("status=504", "status=0504"),
            marker.replace("verifier_calls=1", "verifier_calls=1024"),
            marker.replace("handler_calls=1", "handler_calls=-1"),
            marker.replace("handler_calls=1", "handler_calls=1.0"),
            marker.replace("handler_calls=1", "handler_calls=１"),
            marker.replace("issue_committed=false", "issue_committed=False"),
            marker.replace("case=strict_json_valid", "case=PRIVATE_SENTINEL"),
            marker.replace("counters_v1", "counter_future_v2"),
        ]
        for bad in malformed:
            raw = self.raw(markers={self.CASE: [bad]})
            record, _ = parse_both(browser_raw=raw, browser_exit=101)
            lines = diag.browser_failure_counter_lines(raw, 101, record)
            self.assertEqual(lines, self.UNAVAILABLE)
            self.assertNotIn("PRIVATE_SENTINEL", "\n".join(lines))
        for private in ["secret PRIVATE_SENTINEL", "Cookie: PRIVATE_SENTINEL", "password=PRIVATE_SENTINEL"]:
            raw = self.raw() + (private + "\n").encode()
            record, _ = parse_both(browser_raw=raw, browser_exit=101)
            self.assertEqual(diag.browser_failure_counter_lines(raw, 101, record), self.UNAVAILABLE)
        # Unknown non-marker output never enters the reconstructed numeric fact.
        raw = self.raw() + b"SQL or path PRIVATE_SENTINEL\n"
        record, _ = parse_both(browser_raw=raw, browser_exit=101)
        lines = diag.browser_failure_counter_lines(raw, 101, record)
        self.assertEqual(len(lines), 1)
        self.assertNotIn("PRIVATE_SENTINEL", "\n".join(lines))

    def test_existing_64k_bound_utf8_and_line_endings_are_not_relaxed(self):
        raw = self.raw()
        record, _ = parse_both(browser_raw=raw, browser_exit=101)
        bounded = raw + b"x" * (65536 - len(raw))
        self.assertEqual(len(diag.browser_failure_counter_lines(bounded, 101, record)), 1)
        for bad in [bounded + b"x", raw + b"\xff", raw + b"\r", None, raw.decode()]:
            self.assertEqual(diag.browser_failure_counter_lines(bad, 101, record), self.UNAVAILABLE)

    def test_source_pin_exit_inventory_and_record_binding_fail_closed(self):
        raw = self.raw()
        record, _ = parse_both(browser_raw=raw, browser_exit=101)
        self.assertTrue(diag._browser_native_call_source_current())
        with mock.patch.object(diag, "_browser_native_call_source_current", return_value=False):
            self.assertEqual(diag.browser_failure_counter_lines(raw, 101, record), self.UNAVAILABLE)
        for exit_code in [0, 1, 124, 137, True, "101"]:
            self.assertEqual(diag.browser_failure_counter_lines(raw, exit_code, record), self.UNAVAILABLE)
        for bad in [raw.replace(b"running 10 tests", b"running 0 tests"),
                    raw.replace(b"9 passed; 1 failed", b"10 passed; 0 failed"),
                    raw.replace(("test " + self.CASE + " ... FAILED").encode(),
                                ("test " + self.CASE + " ... ignored").encode())]:
            self.assertEqual(diag.browser_failure_counter_lines(bad, 101, record), self.UNAVAILABLE)
        altered = json.loads(json.dumps(record))
        altered["suites"]["browser"]["failed_cases"] = [self.WIRE_CASE]
        self.assertEqual(diag.browser_failure_counter_lines(raw, 101, altered), self.UNAVAILABLE)
        altered = dict(record, injected="PRIVATE_SENTINEL")
        self.assertEqual(diag.browser_failure_counter_lines(raw, 101, altered), self.UNAVAILABLE)
        altered = dict(record, marker_count=True)
        self.assertEqual(diag.browser_failure_counter_lines(raw, 101, altered), self.UNAVAILABLE)

    def test_cli_never_reprints_rejected_counter_or_private_log_text(self):
        for raw in [self.raw() + b"Cookie: PRIVATE_SENTINEL\n",
                    self.raw(markers={self.CASE: [self.marker() + " sql=PRIVATE_SENTINEL"]}),
                    self.raw(markers={}) + (self.marker() + "\n").encode()]:
            expected, expected_status = parse_both(browser_raw=raw, browser_exit=101)
            with tempfile.TemporaryDirectory() as folder:
                root = pathlib.Path(folder)
                (root / "session-deadline-session.raw").write_bytes(cargo_log(diag.SESSION_CASES))
                (root / "session-deadline-browser.raw").write_bytes(raw)
                output = root / "result.json"
                console = io.StringIO()
                with contextlib.redirect_stdout(console):
                    status = diag.main(["diag", str(root), "0", "101", str(output)])
                self.assertEqual((status, expected_status), (1, 1))
                self.assertEqual(json.loads(output.read_text()), expected)
                self.assertEqual([line for line in console.getvalue().splitlines() if line.startswith(self.PREFIX)], self.UNAVAILABLE)
                self.assertNotIn("PRIVATE_SENTINEL", console.getvalue() + output.read_text())


if __name__ == "__main__":
    unittest.main(verbosity=2)
