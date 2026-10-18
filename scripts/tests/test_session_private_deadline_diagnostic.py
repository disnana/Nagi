import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location(
    "private_deadline", ROOT / "scripts/ci/session_private_deadline_diagnostic.py")
diag = importlib.util.module_from_spec(spec)
spec.loader.exec_module(diag)


def witness(terminal=1, event=1):
    phase = 9 if terminal <= 2 else 6
    budget = 2000 if phase == 9 else 20
    return (f"{diag.PREFIX} status=504 phase={phase} event={event} db=0 polls=1 "
            f"begins=0 commands=0 commits=0 budget_ms={budget} entry_remaining_ms={budget} "
            f"phase_remaining_ms={budget} exit_remaining_ms=0 terminal={terminal}")


def native_witness(terminal=3, **changes):
    values = {"status": 504, "origin": 3, "terminal": terminal, "diagnostic_valid": 1,
              "native_state": 0, "commit_result": 0, "cleanup_result": 0, "cleanup_reason": 0,
              **{name: diag.native_finish.UNSEEN_US for name in diag.native_finish.TIMES},
              "begins": 0, "commits": 0, "selected_deadline_passed": 1}
    values.update(changes)
    return diag.native_finish.PREFIX + " " + " ".join(
        f"{name}={values[name]}" for name in diag.native_finish.FIELDS)


def paired_raw(legacy_terminal=3, native_terminal=3, native_changes=None, extra=()):
    legacy = witness(1, 1).replace("terminal=1", f"terminal={legacy_terminal}")
    markers = [legacy, native_witness(native_terminal, **(native_changes or {}))]
    return raw(markers=markers + list(extra))


def raw(case="prep_timer", markers=None, failed=False):
    name, expected = diag.CASES[case]
    if markers is None:
        markers = [] if expected is None else [witness(t, e) for t, e in expected]
    outcome = "FAILED" if failed else "ok"
    summary = (f"test result: {outcome}. {0 if failed else 1} passed; {1 if failed else 0} failed; "
               "0 ignored; 0 measured; 200 filtered out; finished in 2.00s")
    block = (f"\n{'failures' if failed else 'successes'}:\n\n---- {name} stdout ----\n" +
             "\n".join(markers) + "\n") if markers else "\n"
    return (f"running 1 test\ntest {name} ... {outcome}\n" + block + summary + "\n").encode()


class ClosedCollectorTests(unittest.TestCase):
    def test_all_exact_cases_and_terminal_event_pairs(self):
        self.assertEqual(len(diag.CASES), 12)
        for case in diag.CASES:
            with self.subTest(case=case):
                result = diag.collect(case, raw(case), 0)
                self.assertEqual(result["state"], "passed")
                self.assertLess(len(json.dumps(result).encode()), diag.JSON_LIMIT)
        self.assertEqual({diag.marker(witness(t, e))["phase"]
                          for t, e in ((1, 1), (2, 2), (3, 1), (4, 2))}, {6, 9})

    def test_selected_terminal_does_not_infer_poll_event(self):
        for terminal in (1, 2, 3, 4):
            for event in (0, 1, 2):
                self.assertIsNotNone(diag.marker(witness(terminal, event)))
        self.assertEqual(diag.collect("prep_timer", raw(markers=[witness(1, 2)]), 0)["reason"], 6)

    def test_functional_failures_retain_numeric_witness_and_failure(self):
        result = diag.collect("foreign_grant", raw("foreign_grant", [witness()], True), 101)
        self.assertEqual(result["state"], "tests_failed")
        self.assertEqual(result["facts"][0]["terminal"], 1)
        self.assertEqual(diag.collect("foreign_grant", raw("foreign_grant", [], True), 101)["state"],
                         "unavailable")

    def test_no_output_positive_and_body408_controls(self):
        for case in ("positive_200", "positive_400", "positive_401", "positive_500", "body_408"):
            self.assertEqual(diag.collect(case, raw(case), 0)["facts"], [])
            self.assertEqual(diag.collect(case, raw(case, [witness()]), 0)["reason"], 6)

    def test_omission_swapped_terminal_and_event_are_not_native_success(self):
        self.assertEqual(diag.collect("prep_timer", raw(markers=[]), 0)["reason"], 6)
        swapped = witness().replace("terminal=1", "terminal=3")
        self.assertEqual(diag.collect("prep_timer", raw(markers=[swapped]), 0)["reason"], 4)
        self.assertEqual(diag.collect("prep_timer", raw(markers=[witness(2, 2)]), 0)["reason"], 6)

    def test_unknown_fields_codes_and_secret_tail_refuse_without_echo(self):
        secret = "SECRET_cookie_SQL_/private/path_https://invalid/"
        variants = [witness() + " secret=" + secret, witness().replace("terminal=1", "terminal=5"),
                    witness().replace("phase=9", "phase=8"), witness().replace("event=1", "event=3"),
                    witness().replace("db=0", "db=15"), witness().replace("polls=1", "polls=1024"),
                    witness().replace("budget_ms=2000", "budget_ms=60001"),
                    witness().replace("entry_remaining_ms=2000", "entry_remaining_ms=2001"),
                    witness().replace("phase_remaining_ms=2000", "phase_remaining_ms=2001"),
                    witness().replace("exit_remaining_ms=0", "exit_remaining_ms=2001"),
                    witness().replace("polls=1", "polls=01"), secret + witness(), witness() + "\x00"]
        for line in variants:
            with self.subTest(line=line):
                result = diag.collect("prep_timer", raw(markers=[line]), 0)
                self.assertEqual(result["state"], "invalid")
                self.assertNotIn(secret, json.dumps(result))

    def test_bounds_duplicate_raw_json_and_marker_counts(self):
        self.assertEqual(diag.collect("foreign_grant", b"x" * (diag.RAW_LIMIT + 1), 0)["state"], "invalid")
        self.assertEqual(diag.collect("prep_timer", raw(), 0, oversized=True)["state"], "invalid")
        self.assertEqual(diag.collect("prep_timer", raw(markers=[witness(), witness()]), 0)["reason"], 4)
        markers = [witness().replace("polls=1", f"polls={n}") for n in range(1, 18)]
        self.assertEqual(diag.collect("foreign_grant", raw("foreign_grant", markers[:16]), 0)["state"], "passed")
        self.assertEqual(diag.collect("foreign_grant", raw("foreign_grant", markers), 0)["reason"], 4)
        self.assertEqual(diag.collect("prep_timer", b"\xff", 0)["state"], "invalid")

    def test_paired_control_requires_exact_pair_and_repairs_only_terminals(self):
        valid = paired_raw()
        repaired = valid.replace(b" terminal=3", b" terminal=1")
        for observed in (valid, valid.replace(b"\n", b"\r\n")):
            expected = observed.replace(b" terminal=3", b" terminal=1")
            captured = []
            original_collect = diag.collect

            def capture_collect(case, data, code, **kwargs):
                captured.append(data)
                return original_collect(case, data, code, **kwargs)

            with mock.patch.object(diag, "collect", side_effect=capture_collect):
                self.assertTrue(diag._prep_timer_pair_control_passes(observed, 0))
            self.assertEqual(captured, [expected])
            self.assertEqual(captured[0].count(b" terminal=1"), 2)
        self.assertEqual(diag.collect("prep_timer", repaired, 0)["state"], "passed")

        legacy = witness(1, 1).replace("terminal=1", "terminal=3")
        native = native_witness()
        counter_mismatch = native_witness(
            native_state=1, begins=1, commit_enter_us=0)
        commit_mismatch = native_witness(
            native_state=2, commit_result=1, commit_enter_us=0, commit_return_us=0,
            begins=1, commits=1)
        invalid_fields = native.replace("diagnostic_valid=1", "diagnostic_valid=0")
        variants = [
            raw(markers=[legacy]),                         # native record missing
            raw(markers=[native]),                         # legacy record missing
            raw(markers=[native, legacy]),                 # reversed pair
            raw(markers=[legacy, native, native]),         # duplicate native
            raw(markers=[legacy, native, legacy]),         # duplicate legacy
            raw(markers=[legacy, native_witness(origin=1)]),  # wrong origin
            raw(markers=[legacy, native_witness(2)]),      # wrong native terminal
            raw(markers=[witness(4, 1), native]),          # wrong private terminal
            raw(markers=[witness(3, 2), native]),          # wrong private event
            raw(markers=[legacy, counter_mismatch]),       # begins mismatch
            raw(markers=[legacy, commit_mismatch]),        # commits mismatch
            raw(markers=[legacy, native + " extra=1"]),   # unknown field
            raw(markers=[legacy, invalid_fields]),         # invalid closed value
            raw(markers=[legacy, native, "other terminal=3"]),  # extra terminal
            raw(markers=[legacy, native]).replace(b"running 1 test", b"running 0 tests"),
            raw(markers=[legacy, native]).replace(b"test result: ok", b"test result: FAILED"),
            b"compile failed\n",
            b"\xff" + valid,
        ]
        for sample in variants:
            with self.subTest(sample_length=len(sample)):
                self.assertFalse(diag._prep_timer_pair_control_passes(sample, 0))
        self.assertFalse(diag._prep_timer_pair_control_passes(valid, 101))
        self.assertFalse(diag._prep_timer_pair_control_passes(valid, -1))  # timed out
        self.assertFalse(diag._prep_timer_pair_control_passes(valid, 0, oversized=True))
        self.assertFalse(diag._prep_timer_pair_control_passes(b"x" * (diag.RAW_LIMIT + 1), 0))

    def test_exact_single_execution_summary_returncode_and_identity(self):
        samples = [raw().replace(b"running 1 test", b"running 0 tests"),
                   raw().replace(b"1 passed; 0 failed", b"0 passed; 0 failed"),
                   raw() + raw(), raw().replace(b"0 ignored", b"1 ignored"),
                   raw().replace(diag.CASES["prep_timer"][0].encode(), b"unknown"),
                   b"compiler failed\n", b""]
        for sample in samples:
            self.assertEqual(diag.collect("prep_timer", sample, 0)["state"], "invalid")
        self.assertEqual(diag.collect("prep_timer", raw(), 101)["state"], "invalid")
        self.assertEqual(diag.collect("prep_timer", raw(failed=True), 0)["state"], "invalid")
        self.assertEqual(diag.collect("SECRET_PATH", raw(), 0)["case"], "unknown")
        unbound = (witness() + "\n").encode() + raw(markers=[])
        self.assertEqual(diag.collect("prep_timer", unbound, 0)["reason"], 4)
        other_case = raw().replace(b"---- " + diag.CASES["prep_timer"][0].encode(), b"---- unknown")
        self.assertEqual(diag.collect("prep_timer", other_case, 0)["reason"], 4)

    def test_command_exact_native_assertions_no_ignored_or_threads_flag(self):
        command = diag.native_command(diag.CASES["foreign_grant"][0])
        self.assertIn("--exact", command)
        self.assertIn("--show-output", command)
        self.assertNotIn("--nocapture", command)
        self.assertNotIn("--test-threads", " ".join(command))
        self.assertNotIn("--ignored", command)

    def test_capture_is_bounded_in_memory(self):
        captured, code, oversized = diag.capture(
            [sys.executable, "-B", "-c", "import sys; sys.stdout.write('x'*300000)"], dict(os.environ))
        self.assertEqual(code, 0)
        self.assertTrue(oversized)
        self.assertEqual(len(captured), diag.RAW_LIMIT + 1)

    def test_cli_attempts_all_cases_and_preserves_native_failure_no_echo(self):
        calls = []

        def fake(command, env):
            case = next(k for k, (name, _) in diag.CASES.items() if name in command)
            calls.append(case)
            failed = case == "foreign_grant"
            data = raw(case, [witness()] if failed else None, failed)
            return b"SECRET arbitrary native log\n" + data, 101 if failed else 0, False

        output = io.StringIO()
        with mock.patch.object(sys, "argv", ["private", "--run"]), \
                mock.patch.object(diag, "source_inventory", return_value=True), \
                mock.patch.object(diag, "capture", side_effect=fake), \
                mock.patch.object(diag, "wiring_controls", return_value=True), \
                contextlib.redirect_stdout(output):
            self.assertEqual(diag.main(), 1)
        self.assertEqual(calls, list(diag.CASES))
        self.assertNotIn("SECRET", output.getvalue())
        self.assertLess(len(output.getvalue()), 32768)
        self.assertEqual(len(output.getvalue().splitlines()), 12)

    def test_wiring_controls_targeted_red_and_finally_restore(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "runtime/src/http_server.rs"
            target.parent.mkdir(parents=True)
            original = (ROOT / "runtime/src/http_server.rs").read_bytes()
            target.write_bytes(original)

            def fake(command, env):
                source = target.read_bytes()
                if b"let _ = observer;" in source:
                    return raw(markers=[]), 0, False
                return paired_raw(), 0, False

            with mock.patch.object(diag, "capture", side_effect=fake), contextlib.redirect_stdout(io.StringIO()):
                self.assertTrue(diag.wiring_controls(root, {}))
            self.assertEqual(target.read_bytes(), original)
            for bad in ((b"compile error", 101, False), (b"running 0 tests", 0, False),
                        (raw(), 0, False)):
                with mock.patch.object(diag, "capture", return_value=bad), contextlib.redirect_stdout(io.StringIO()):
                    self.assertFalse(diag.wiring_controls(root, {}))
                self.assertEqual(target.read_bytes(), original)
            with mock.patch.object(diag, "capture", side_effect=OSError), contextlib.redirect_stdout(io.StringIO()):
                self.assertFalse(diag.wiring_controls(root, {}))
            self.assertEqual(target.read_bytes(), original)


    def test_wiring_controls_canonical_line_endings_and_exact_raw_restore(self):
        canonical = (ROOT / "runtime/src/http_server.rs").read_bytes().replace(b"\r\n", b"\n")
        variants = [(canonical, True), (canonical.replace(b"\n", b"\r\n"), True),
                    (canonical + b" ", False), (canonical.replace(b"\n", b"\r\n") + b" ", False),
                    (canonical.replace(b"\n", b"\r"), False)]
        for original, accepted in variants:
            with self.subTest(accepted=accepted, crlf=b"\r\n" in original), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                target = root / "runtime/src/http_server.rs"
                target.parent.mkdir(parents=True)
                target.write_bytes(original)
                def fake(command, env):
                    source = target.read_bytes()
                    if b"let _ = observer;" in source:
                        output = raw(markers=[])
                    else:
                        output = paired_raw()
                    if b"\r\n" in original:
                        output = output.replace(b"\n", b"\r\n")
                    return output, 0, False
                with mock.patch.object(diag, "capture", side_effect=fake) as capture, contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(diag.wiring_controls(root, {}), accepted)
                    self.assertEqual(capture.call_count, 2 if accepted else 0)
                self.assertEqual(target.read_bytes(), original)



class HeadFinalizerObservationTests(unittest.TestCase):
    @staticmethod
    def sample(*, line=282, column=5, actual="504", expected="400", thread_id=True,
               source="D:\\private\\runtime\\src\\auth\\session\\http_bridge_tests.rs"):
        name = diag.CASES["head_finalizer"][0]
        thread = " (123)" if thread_id else ""
        return [f"thread '{name}'{thread} panicked at {source}:{line}:{column}:",
                "assertion `left == right` failed", f"  left: {actual}", f" right: {expected}"]

    def test_failure_status_and_fixed_source_only_lf_crlf_thread_id(self):
        for thread_id in (False, True):
            data = raw("head_finalizer", [witness(3, 1)] + self.sample(thread_id=thread_id), True)
            for sample in (data, data.replace(b"\n", b"\r\n")):
                result = diag.collect("head_finalizer", sample, 101)
                self.assertEqual(result["state"], "tests_failed")
                self.assertEqual(result["assertions"], [{"source": 1, "line": 282,
                    "column": 5, "assertion": 1, "actual": 504, "expected": 400}])
                self.assertNotIn("private", json.dumps(result))

    def test_expected_panic_unknown_source_range_secret_and_duplicate_unavailable(self):
        variants = [self.sample(line=435), self.sample(line=486), self.sample(column=4097),
                    self.sample(source="runtime/src/auth/prelogin/http_bridge_tests.rs"),
                    self.sample() + self.sample()]
        for markers in variants:
            result = diag.collect("head_finalizer", raw("head_finalizer", [witness(3, 1)] + markers, True), 101)
            self.assertEqual(result["state"], "tests_failed")
            self.assertEqual(result["assertions"], [])
        for actual, expected in (("SECRET_cookie", "400"), ("504", "SECRET_cookie")):
            result = diag.collect("head_finalizer", raw("head_finalizer",
                [witness(3, 1)] + self.sample(actual=actual, expected=expected), True), 101)
            self.assertEqual(result["assertions"][0], {"source": 1, "line": 282,
                                                       "column": 5, "assertion": 1})
            self.assertNotIn("SECRET", json.dumps(result))
        for line, assertion in ((283, 2), (284, 3), (285, 4), (424, 5)):
            result = diag.collect("head_finalizer", raw("head_finalizer",
                [witness(3, 1)] + self.sample(line=line, actual="SECRET_cookie"), True), 101)
            self.assertEqual(result["assertions"][0], {"source": 1, "line": line,
                                                       "column": 5, "assertion": assertion})

    def test_position_never_changes_exit_and_requires_own_failed_block(self):
        markers = [witness(3, 1)] + self.sample()
        self.assertNotIn("assertions", diag.collect("head_finalizer", raw("head_finalizer", markers), 0))
        self.assertEqual(diag.collect("head_finalizer", raw("head_finalizer", markers, True), 0)["state"], "invalid")
        foreign = raw("head_finalizer", markers, True).replace(
            ("---- " + diag.CASES["head_finalizer"][0]).encode(), b"---- foreign")
        self.assertNotIn("assertions", diag.collect("head_finalizer", foreign, 101))
        outside = ("\n".join(self.sample()) + "\n").encode() + raw("head_finalizer", [witness(3, 1)], True)
        self.assertEqual(diag.collect("head_finalizer", outside, 101)["assertions"], [])
        malformed = diag.collect("head_finalizer", raw("head_finalizer",
            [witness(3, 1) + " secret=SECRET_cookie"] + self.sample(), True), 101)
        self.assertEqual(malformed["state"], "invalid")
        self.assertEqual(malformed["assertions"][0]["actual"], 504)
        missing = diag.collect("head_finalizer", raw("head_finalizer", self.sample(), True), 101)
        self.assertEqual(missing["state"], "unavailable")
        self.assertEqual(missing["assertions"][0]["expected"], 400)

    def test_exact_only_cli_attempts_once_no_controls_and_preserves_failure(self):
        for failed in (False, True):
            output = io.StringIO()
            data = raw("head_finalizer", [witness(3, 1)] + self.sample(), failed)
            with mock.patch.object(sys, "argv", ["private", "--run-case", "head_finalizer"]), \
                    mock.patch.object(diag, "source_inventory", return_value=True), \
                    mock.patch.object(diag, "capture", return_value=(data, 101 if failed else 0, False)) as capture, \
                    mock.patch.object(diag, "wiring_controls") as controls, contextlib.redirect_stdout(output):
                self.assertEqual(diag.main(), int(failed))
            self.assertEqual(capture.call_count, 1)
            self.assertEqual(capture.call_args[0][0], diag.native_command(diag.CASES["head_finalizer"][0]))
            controls.assert_not_called()
            self.assertEqual(len(output.getvalue().splitlines()), 1)
            self.assertNotIn("private\\", output.getvalue())
        with mock.patch.object(sys, "argv", ["private", "--run-case", "prep_timer"]), \
                mock.patch.object(diag, "capture") as capture, contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(diag.main(), 1)
            capture.assert_not_called()

    def test_exact_source_binding_lf_crlf_and_mutation_rejected(self):
        self.assertTrue(diag.source_inventory(ROOT))
        for input_newline in (b"\n", b"\r\n"):
            for output_newline in (b"\n", b"\r\n"):
                with self.subTest(input_newline=input_newline, output_newline=output_newline), \
                        tempfile.TemporaryDirectory() as directory:
                    fixture = Path(directory)
                    for path in ("runtime/src/auth/session/http_bridge_tests.rs",
                                 "runtime/src/http_server/private_deadline_calibration.rs"):
                        target = fixture / path
                        target.parent.mkdir(parents=True, exist_ok=True)
                        canonical = (ROOT / path).read_bytes().replace(b"\r\n", b"\n")
                        supplied = canonical.replace(b"\n", input_newline)
                        rendered = supplied.replace(b"\r\n", b"\n").replace(b"\n", output_newline)
                        self.assertNotIn(b"\r\r\n", rendered)
                        target.write_bytes(rendered)
                    self.assertTrue(diag.source_inventory(fixture))
                    target = fixture / "runtime/src/auth/session/http_bridge_tests.rs"
                    target.write_bytes(target.read_bytes() + b" ")
                    self.assertFalse(diag.source_inventory(fixture))


class SourceWiringControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = Path(os.environ.get("SESSION_PRIVATE_SOURCE_ROOT", ROOT))

    def test_preparation_wrapper_reaches_pre_body_actual_timeout(self):
        source = (self.root / "runtime/src/http_server.rs").read_text()
        self.assertTrue("Phase::Preparation" in source, "preparation observation missing")
        self.assertLess(source.index("Phase::Preparation"), source.index("tokio::time::timeout(options.security_deadline"))

    def test_four_selected_private_emit_arms(self):
        source = (self.root / "runtime/src/http_server.rs").read_text()
        for label in ("PreparationTimer", "PreparationLateReady", "HandlerTimer", "HandlerLateReady"):
            self.assertEqual(source.count("PrivateTerminal::" + label), 1)
        self.assertEqual(source.count("observer.emit_private("), 4)

    def test_private_calibration_reachable_exact_inventory(self):
        self.assertTrue(diag.source_inventory(self.root))
        source = (self.root / "runtime/src/http_server.rs").read_text()
        self.assertTrue('#[path = "http_server/private_deadline_calibration.rs"]' in source, "calibration registration missing")

    def test_private_prefix_formatter_retained_owner_not_drop(self):
        source = (self.root / "runtime/src/auth/session_deadline_observer.rs").read_text()
        self.assertTrue("session_private_deadline_diag status=504" in source, "private formatter missing")
        method = source.split("pub(crate) fn emit_private", 1)[1].split("pub(crate) fn emit_token", 1)[0]
        self.assertIn("self.closed(504)", method)
        self.assertNotIn("ACTIVE", method.replace("ACTIVE is deliberately not consulted here", ""))

    def test_paired_control_inverse_is_exact_and_rejects_helper_or_branch_mutants(self):
        projection_spec = importlib.util.spec_from_file_location(
            "request_projection", self.root / "scripts/ci/sf07_request_projection.py")
        projection = importlib.util.module_from_spec(projection_spec)
        projection_spec.loader.exec_module(projection)
        source = (self.root / "scripts/ci/session_private_deadline_diagnostic.py").read_text()
        restored = projection.inverse_collector(source)
        self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(),
                         "1d0dd3c92e07c374de80d4f968c101141eb6290ba2b935bf9c06dcb85ac60913")

        helper_start = source.index(projection.PRIVATE_PAIR_HELPER_START)
        helper_end = source.index(projection.PRIVATE_PAIR_HELPER_END, helper_start)
        helper = source[helper_start:helper_end]
        branch = projection.PRIVATE_PAIR_CONTROL_AFTER
        mutants = (
            source.replace(helper, "", 1),
            source.replace(helper, helper + helper, 1),
            source.replace("return collect(\"prep_timer\", repaired, returncode)[\"state\"] == \"passed\"",
                           "return False", 1),
            source.replace(branch, "", 1),
            source.replace(branch, branch + branch, 1),
            source.replace(branch, branch.replace("_prep_timer_pair_control_passes", "_other_control"), 1),
            source.replace("\ndef _head_finalizer_assertion(", "\ndef _missing_assertion(", 1),
            source.replace("assertions = {282: 1", "assertions = {283: 1", 1),
            source.replace("    if focused:\n        return int(failed)\n", "", 1),
            source.replace('if bad and case == "head_finalizer"', 'if True', 1),
        )
        for mutant in mutants:
            with self.subTest(mutant_hash=hashlib.sha256(mutant.encode()).hexdigest()):
                with self.assertRaises(ValueError):
                    projection.inverse_collector(mutant)


if __name__ == "__main__":
    unittest.main()
