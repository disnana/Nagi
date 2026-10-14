import contextlib
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
        secret_name = "secret_credential_value"
        marker = (
            "session_deadline_diag status=504 phase=1 event=1 db=4 exit=1 polls=1 begins=1 "
            "commands=0 commits=0 budget_ms=20 entry_remaining_ms=19 "
            "phase_remaining_ms=18 exit_remaining_ms=0"
        )

        known = "pending_guarded_authorizer_expires_at_bound_session_and_recovers_permit"
        unknown_cases = (set(diag.SESSION_CASES) - {known}) | {secret_name}
        unknown_row_log = cargo_log(
            unknown_cases, failed={secret_name}, markers={secret_name: [marker]}
        )
        record, status = parse_both(session_raw=unknown_row_log, session_exit=101)
        rendered = json.dumps(record)
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")
        self.assertEqual(record["suites"]["session"]["failed_cases"], [])
        self.assertEqual(record["suites"]["session"]["session_deadline"], [])
        self.assertNotIn(secret_name, rendered)

        known_failure = cargo_log(
            diag.SESSION_CASES, failed={known}, markers={known: [marker]}
        ).decode()
        unknown_stdout = known_failure.replace(
            "test result: FAILED.",
            f"---- {secret_name} stdout ----\n{marker}\nfailures:\ntest result: FAILED.",
            1,
        ).encode()
        record, status = parse_both(session_raw=unknown_stdout, session_exit=101)
        rendered = json.dumps(record)
        self.assertEqual(status, 1)
        self.assertEqual(record["status"], "diagnostic_invalid")
        self.assertEqual(record["suites"]["session"]["session_deadline"][0]["test"], known)
        self.assertEqual(len(record["suites"]["session"]["session_deadline"]), 1)
        self.assertNotIn(secret_name, rendered)

        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "session-deadline-session.raw").write_bytes(unknown_row_log)
            (root / "session-deadline-browser.raw").write_bytes(cargo_log(diag.BROWSER_CASES))
            output = root / "diagnostic.json"
            stdout, stderr = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                code = diag.main(["session_deadline_diagnostic.py", str(root), "101", "0", str(output)])
            self.assertEqual(code, 1)
            self.assertEqual(stdout.getvalue(), "")
            self.assertEqual(stderr.getvalue(), "")
            cli_json = output.read_text(encoding="utf-8")
            self.assertNotIn(secret_name, cli_json)
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


if __name__ == "__main__":
    unittest.main(verbosity=2)
