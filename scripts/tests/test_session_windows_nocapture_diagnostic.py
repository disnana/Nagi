"""Focused tests for the same-executable --nocapture comparison wrapper."""
import pathlib
import tempfile
import unittest
from unittest import mock

from scripts.ci import auth_stage_diagnostic as parser
from scripts.ci import session_windows_nocapture_diagnostic as diag
from scripts.ci import session_windows_same_exe_diagnostic as base


KNOWN = "auth::prelogin::tests::one_known_control"
OTHER = "auth::prelogin::tests::another_control"
RECORD = (b"op=Claim stage=Return event=Success elapsed_us=None "
          b"utc_remaining_ms=None mono=None kind=None outcome=None\n")
GUARDS = (
    b"auth-stage slot=1 (test-only; scheduling perturbed)\n" + RECORD + b"lost=false\n"
    + b"auth-stage slot=2 (test-only; scheduling perturbed)\n" + RECORD + b"lost=false\n"
)
ZERO_RECORD_GUARD = (
    b"auth-stage slot=1 (test-only; scheduling perturbed)\nlost=false\n"
    + b"auth-stage slot=2 (test-only; scheduling perturbed)\n" + RECORD + b"lost=false\n"
)
SUMMARY = (b"test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; "
           b"0 filtered out; finished in 0.01s\n")


class CaptureProjectionTests(unittest.TestCase):
    def test_broad_parser_emits_closed_records_and_drops_unrelated_secret_text(self):
        stdout = b"private-token-should-not-echo\n" + SUMMARY
        valid, bounded, lines, summary, parser_valid = diag._project_broad(
            parser, stdout, GUARDS, 0, False, False, {KNOWN: KNOWN, OTHER: OTHER}, base.SUMMARY
        )
        self.assertTrue(valid)
        self.assertTrue(parser_valid)
        self.assertTrue(bounded)
        self.assertEqual(summary, {
            "valid": True, "tests": 2, "passed": 1, "failed": 0, "ignored": 1,
            "measured": 0, "filtered": 0,
        })
        self.assertTrue(any(line == "test=broad cargo_exit=0 inventory=pass" for line in lines))
        self.assertTrue(any(line.startswith("op=") for line in lines))
        rendered = "\n".join(lines)
        self.assertNotIn("private-token", rendered)
        self.assertNotIn("test result:", rendered)

    def test_failed_broad_capture_projects_only_an_inventory_bound_failure_name(self):
        failed = (
            f"test {KNOWN} ... FAILED\nfailures:\n    {KNOWN}\n"
            "test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; "
            "0 filtered out; finished in 0.01s\n"
        ).encode()
        valid, bounded, lines, summary, parser_valid = diag._project_broad(
            parser, failed + b"secret-panic-payload\n", GUARDS, 101, False, False,
            {KNOWN: KNOWN, OTHER: OTHER}, base.SUMMARY,
        )
        self.assertFalse(valid)
        self.assertFalse(parser_valid)
        self.assertTrue(bounded)
        self.assertTrue(summary["valid"])
        self.assertIn(f"failed_case={KNOWN}", lines)
        self.assertNotIn("secret-panic-payload", "\n".join(lines))

    def test_zero_record_guard_is_rejected_by_the_reused_auth_parser(self):
        valid, bounded, lines, summary, parser_valid = diag._project_broad(
            parser, SUMMARY, ZERO_RECORD_GUARD, 0, False, False,
            {KNOWN: KNOWN, OTHER: OTHER}, base.SUMMARY,
        )
        self.assertFalse(valid)
        self.assertFalse(parser_valid)
        self.assertTrue(bounded)
        self.assertTrue(summary["valid"])
        self.assertIn("trace_validation=failed", lines)

    def test_summary_must_match_inventory_and_reject_filtering_or_duplicates(self):
        cases = {KNOWN: KNOWN, OTHER: OTHER}
        variants = (
            b"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
            b"0 filtered out; finished in 0.01s\n",
            b"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
            b"1 filtered out; finished in 0.01s\n",
            SUMMARY + SUMMARY,
        )
        for raw in variants:
            with self.subTest(raw_length=len(raw)):
                valid, bounded, _lines, summary, parser_valid = diag._project_broad(
                    parser, raw, GUARDS, 0, False, False, cases, base.SUMMARY,
                )
                self.assertFalse(valid)
                self.assertTrue(bounded)
                self.assertTrue(parser_valid)
                if raw == SUMMARY + SUMMARY:
                    self.assertIsNone(summary)
                else:
                    self.assertFalse(summary["valid"])

        failed_summary = (
            b"test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; "
            b"0 filtered out; finished in 0.01s\n"
        )
        for raw, exit_code in ((failed_summary, 0), (SUMMARY, 101)):
            valid, _bounded, _lines, summary, _parser_valid = diag._project_broad(
                parser, raw, GUARDS, exit_code, False, False, cases, base.SUMMARY,
            )
            self.assertFalse(valid)
            self.assertFalse(summary["valid"])

    def test_timeout_and_oversize_are_closed_without_invoking_parser(self):
        rejecting = mock.Mock(side_effect=AssertionError("must not parse incomplete bytes"))
        valid, bounded, lines, summary, parser_valid = diag._project_broad(
            rejecting, b"partial", b"", 1, True, False, {KNOWN: KNOWN}, base.SUMMARY
        )
        self.assertFalse(valid)
        self.assertFalse(parser_valid)
        self.assertFalse(bounded)
        self.assertEqual(lines, ())
        self.assertIsNone(summary)
        rejecting.assert_not_called()
        valid, bounded, lines, summary, parser_valid = diag._project_broad(
            rejecting, b"partial", b"", -1, False, True, {KNOWN: KNOWN}, base.SUMMARY
        )
        self.assertFalse(valid)
        self.assertFalse(parser_valid)
        self.assertFalse(bounded)
        self.assertEqual(lines, ())
        self.assertIsNone(summary)
        rejecting.assert_not_called()


class ProtocolPathTests(unittest.TestCase):
    def test_alias_component_and_parent_component_resolve_to_runtime(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / "runtime").mkdir()
            (root / "alias-component").mkdir()
            self.assertTrue(diag._same_runtime_cwd(
                root / "alias-component" / ".." / "runtime", root
            ))
            self.assertFalse(diag._same_runtime_cwd(root, root))

    def test_show_result_requires_one_closed_summary(self):
        self.assertEqual(diag._show_state(0, "result=pass_with_ignored\n"), "pass_with_ignored")
        self.assertEqual(diag._show_state(0, "result=pass\nresult=pass\n"), "unavailable")
        self.assertEqual(diag._show_state(1, "result=pass\n"), "fail")


class KnownFailurePositionTests(unittest.TestCase):
    def test_inventory_bound_failure_uses_r6_windows_panic_matcher(self):
        known = base.CASE_NAMES[0]
        panic = (f"thread '{known}' (321) panicked at "
                 r"C:\ci\work\runtime\src\auth\prelogin\tests.rs:456:5:")
        diagnostics = (f"failed_case={known}", "failed_case=unlisted::panic")
        self.assertEqual(diag._failed_inventory_names(diagnostics, base), (known,))
        projected = diag._known_failure_lines(base, diagnostics, panic.encode(), b"")
        self.assertEqual(projected, (
            f"nocapture_known_case={known} state=fail source=tests line=456 column=5",
        ))
        self.assertNotIn("C:\\ci", "\n".join(projected))

    def test_wrong_source_and_out_of_range_lines_are_unavailable(self):
        known = base.CASE_NAMES[0]
        diagnostics = (f"failed_case={known}",)
        for path, number in (
            (r"C:\ci\runtime\src\auth\prelogin\observer_tests.rs", 70),
            (r"C:\ci\runtime\src\auth\prelogin\tests.rs", 509),
        ):
            panic = f"thread '{known}' panicked at {path}:{number}:5:"
            with self.subTest(path=path, number=number):
                self.assertEqual(diag._known_failure_lines(
                    base, diagnostics, panic.encode(), b""
                ), (f"nocapture_known_case={known} state=fail source=unavailable",))

    def test_expected_or_unrelated_panic_alone_never_creates_a_failed_case(self):
        known = base.CASE_NAMES[0]
        panic = (f"thread '{known}' (17) panicked at "
                 r"C:\ci\runtime\src\auth\prelogin\tests.rs:456:5:")
        projected = diag._known_failure_lines(
            base, (), (panic + "\nsecret-panic-payload").encode(), b""
        )
        self.assertEqual(projected, ())
        self.assertNotIn("secret-panic-payload", "\n".join(projected))

    def test_known_failure_position_survives_invalid_guard_projection(self):
        known = base.CASE_NAMES[0]
        rows = [f"test {name} ... {'FAILED' if name == known else 'ok'}"
                for name in base.CASE_NAMES]
        stdout = ("\n".join(rows + ["failures:", f"    {known}",
                                     "test result: FAILED. 4 passed; 1 failed; 0 ignored; "
                                     "0 measured; 0 filtered out; finished in 0.01s", ""])).encode()
        stderr = (GUARDS +
                  b"auth-stage slot=9 (unexpected; test-only)\nsecret-guard-payload\n")
        cases = {name: name for name in base.CASE_NAMES}
        _valid, _bounded, diagnostics, summary, parser_valid = diag._project_broad(
            parser, stdout, stderr, 101, False, False, cases, base.SUMMARY,
        )
        self.assertTrue(summary["valid"])
        self.assertFalse(parser_valid)
        self.assertIn(f"failed_case={known}", diagnostics)
        panic = (f"thread '{known}' (4) panicked at "
                 r"C:\ci\runtime\src\auth\prelogin\tests.rs:456:5:")
        locations = diag._known_failure_lines(
            base, diagnostics, stdout + panic.encode() + b"\n", stderr
        )
        self.assertEqual(locations, (
            f"nocapture_known_case={known} state=fail source=tests line=456 column=5",
        ))
        self.assertNotIn("secret-guard-payload", "\n".join(locations))


class WorkflowTests(unittest.TestCase):
    def test_workflow_is_create_only_for_the_exact_private_branch(self):
        path = pathlib.Path(__file__).resolve().parents[2] / ".github/workflows/session-windows-nocapture-diagnostic.yml"
        text = path.read_text(encoding="utf-8")
        self.assertRegex(text, r"(?m)^on:\n  create:\s*$")
        for event in ("push:", "pull_request:", "workflow_dispatch:", "schedule:"):
            self.assertNotIn(event, text)
        branch = "diag/session-windows-nocapture-20261018"
        self.assertIn(f"github.event.ref == '{branch}'", text)
        self.assertIn(f"github.ref == 'refs/heads/{branch}'", text)


if __name__ == "__main__":
    unittest.main()
