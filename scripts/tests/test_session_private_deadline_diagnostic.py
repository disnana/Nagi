import contextlib
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
                return raw(markers=[witness().replace("terminal=1", "terminal=3")]), 0, False

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
                        return raw(markers=[]), 0, False
                    return raw(markers=[witness().replace("terminal=1", "terminal=3")]), 0, False
                with mock.patch.object(diag, "capture", side_effect=fake) as capture, contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(diag.wiring_controls(root, {}), accepted)
                    self.assertEqual(capture.call_count, 2 if accepted else 0)
                self.assertEqual(target.read_bytes(), original)


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


if __name__ == "__main__":
    unittest.main()
