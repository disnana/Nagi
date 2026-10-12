import contextlib
import importlib.util
import io
import os
import pathlib
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).parents[1] / "ci" / "auth_stage_diagnostic.py"
spec = importlib.util.spec_from_file_location("diagnostic", SCRIPT)
diagnostic = importlib.util.module_from_spec(spec)
spec.loader.exec_module(diagnostic)


class DiagnosticCases(unittest.TestCase):
    def setUp(self):
        self.owner = tempfile.TemporaryDirectory(dir=os.environ.get("NAGI_DIAGNOSTIC_TEST_TMP"))
        self.root = pathlib.Path(self.owner.name)
        self.addCleanup(self.owner.cleanup)

    def catalogue(self, text, code=0):
        (self.root / "auth-stage-cases.raw").write_bytes(text.encode("utf-8"))
        return diagnostic.load_cases(self.root, code)

    def test_exact_listing_and_unique_footer(self):
        self.assertEqual(self.catalogue("auth::a: test\nhttp::b: test\n\n2 tests, 0 benchmarks\n"),
                         {"auth::a": "auth::a", "http::b": "http::b"})

    def test_duplicate_mismatched_and_zero_footer_reject(self):
        for text in ["auth::a: test\nauth::a: test\n2 tests, 0 benchmarks\n",
                     "auth::a: test\n2 tests, 0 benchmarks\n",
                     "0 tests, 0 benchmarks\n",
                     "auth::a: test\n1 tests, 0 benchmarks\n1 tests, 0 benchmarks\n"]:
            self.assertIsNone(self.catalogue(text))

    def test_listing_exit_io_utf8_and_byte_limit_reject(self):
        self.assertIsNone(diagnostic.load_cases(self.root, 0))
        self.assertIsNone(self.catalogue("auth::a: test\n1 tests, 0 benchmarks\n", 101))
        (self.root / "auth-stage-cases.raw").write_bytes(b"\xff")
        self.assertIsNone(diagnostic.load_cases(self.root, 0))
        (self.root / "auth-stage-cases.raw").write_bytes(b"a" * (diagnostic.RAW_LIMIT + 1))
        self.assertIsNone(diagnostic.load_cases(self.root, 0))

    def test_listing_identifier_and_entry_limits(self):
        long_name = "a" * (diagnostic.CASE_NAME_LIMIT + 1)
        self.assertIsNone(self.catalogue(long_name + ": test\n1 tests, 0 benchmarks\n"))
        lines = [f"case{i}: test" for i in range(diagnostic.CASE_LIMIT + 1)]
        self.assertIsNone(self.catalogue("\n".join(lines) + "\n1025 tests, 0 benchmarks\n"))

    def test_known_failure_is_canonical_and_deduplicated(self):
        cases = {"auth::a": "auth::a", "http::b": "http::b"}
        raw = "test auth::a ... FAILED\nfailures:\n\n    http::b\n    auth::a\n"
        self.assertEqual(diagnostic.failed_cases(raw, cases),
                         ["failed_case=auth::a", "failed_case=http::b"])

    def test_qualified_secret_prefix_is_not_copied(self):
        cases = {"auth::a": "auth::a"}
        raw = "test secret_credential_value::a ... FAILED\n"
        output = "\n".join(diagnostic.failed_cases(raw, cases))
        self.assertEqual(output, "failure_cases=unavailable")
        self.assertNotIn("secret_credential_value", output)

    def test_unknown_wire_path_unicode_ansi_and_oversized_never_output(self):
        cases = {"auth::a": "auth::a"}
        for value in ["wireCookieSecret", "/private/path", "ユーザー秘密", "\x1b[31msecret",
                      "a" * (diagnostic.CASE_NAME_LIMIT + 1)]:
            output = "\n".join(diagnostic.failed_cases("test " + value + " ... FAILED\n", cases))
            self.assertEqual(output, "failure_cases=unavailable")
            self.assertNotIn(value, output)

    def test_names_only_in_an_explicit_failure_section(self):
        cases = {"auth::a": "auth::a"}
        self.assertEqual(diagnostic.failed_cases("    auth::a\n", cases),
                         ["failure_cases=unavailable"])
        self.assertEqual(diagnostic.failed_cases("failures:\nother output\n    auth::a\n", cases),
                         ["failure_cases=unavailable"])

    def test_sixteen_case_output_is_bounded_and_reports_loss(self):
        cases = {f"case{i}": f"case{i}" for i in range(17)}
        raw = "".join(f"test {name} ... FAILED\n" for name in cases)
        output = diagnostic.failed_cases(raw, cases)
        self.assertEqual(sum(line.startswith("failed_case=") for line in output), 16)
        self.assertIn("failure_case_loss=observed", output)

    def raw(self, name="broad"):
        slots = [1, 2] if name == "broad" else [1 if name == "csrf" else 2]
        return "".join(f"auth-stage slot={slot} (test-only; scheduling perturbed)\n"
                       "op=Claim stage=Return event=Success elapsed_us=Some(1) "
                       "utc_remaining_ms=Some(2) mono=Some(Before(3)) kind=None outcome=None\n"
                       "lost=false\n" for slot in slots)

    def capture(self, text, exit_code, cases=None, name="broad"):
        (self.root / f"auth-stage-{name}.raw").write_bytes(text.encode("utf-8"))
        sink = io.TextIOWrapper(io.BytesIO(), encoding="utf-8")
        with contextlib.redirect_stdout(sink):
            accepted = diagnostic.capture(name, self.root, exit_code, cases)
        return accepted, (self.root / f"auth-stage-{name}.trace").read_bytes()

    def test_exit_101_remains_failure_with_known_identity(self):
        accepted, output = self.capture(self.raw() + "test auth::a ... FAILED\n", 101,
                                       {"auth::a": "auth::a"})
        self.assertFalse(accepted)
        self.assertIn(b"failed_case=auth::a", output)
        self.assertIn(b"cargo_exit=101 inventory=fail", output)

    def test_success_still_requires_exact_slots_and_summary(self):
        accepted, output = self.capture(self.raw() + "test result: ok. 427 passed; 0 failed; 1 ignored;\n", 0)
        self.assertTrue(accepted)
        accepted, output = self.capture(self.raw("csrf") + "test result: ok. 427 passed; 0 failed; 1 ignored;\n", 0)
        self.assertFalse(accepted)

    def test_missing_and_oversized_raw_reject_without_private_echo(self):
        sink = io.TextIOWrapper(io.BytesIO(), encoding="utf-8")
        with contextlib.redirect_stdout(sink):
            self.assertFalse(diagnostic.capture("broad", self.root, 101))
        output = (self.root / "auth-stage-broad.trace").read_bytes()
        self.assertIn(b"raw_file=unavailable", output)
        accepted, output = self.capture("wireSecret" * (diagnostic.RAW_LIMIT // 10 + 2), 101)
        self.assertFalse(accepted)
        self.assertIn(b"raw_limit=exceeded", output)
        self.assertNotIn(b"wireSecret", output)

    def test_known_plus_unknown_does_not_claim_full_inventory(self):
        output = diagnostic.failed_cases("test auth::a ... FAILED\ntest private::secret ... FAILED\n",
                                         {"auth::a": "auth::a"})
        self.assertEqual(output, ["failed_case=auth::a", "failure_cases=unavailable"])


    def test_malformed_failed_rows_mark_catalogue_unavailable_without_echo(self):
        cases = {"auth::a": "auth::a"}
        variants = [
            "test auth::a ... FAILED\ntest auth::bad-name ... FAILED\n",
            "test auth::a ... FAILED\ntest auth::a ... FAILED trailing\n",
            "test auth::a ... FAILED\nfailures:\n\n    auth::a\n    auth::bad-name\n",
        ]
        for index, raw in enumerate(variants):
            output = diagnostic.failed_cases(raw, cases)
            self.assertEqual(output,
                             ["failed_case=auth::a", "failure_cases=unavailable"],
                             f"malformed case {index}")
            self.assertNotIn("bad-name", "\n".join(output))
            self.assertNotIn("trailing", "\n".join(output))

    def test_mixed_known_and_invalid_utf8_failed_row_marks_unavailable(self):
        raw = (self.raw().encode("ascii") +
               b"test auth::a ... FAILED\n" +
               b"test auth::bad\xff ... FAILED\n")
        (self.root / "auth-stage-broad.raw").write_bytes(raw)
        sink = io.TextIOWrapper(io.BytesIO(), encoding="utf-8")
        with contextlib.redirect_stdout(sink):
            accepted = diagnostic.capture("broad", self.root, 101,
                                          {"auth::a": "auth::a"})
        output = (self.root / "auth-stage-broad.trace").read_bytes()
        self.assertFalse(accepted)
        self.assertIn(b"failed_case=auth::a", output)
        self.assertIn(b"failure_cases=unavailable", output)
        self.assertNotIn(b"bad", output)
        self.assertNotIn(b"\xff", output)

    def test_giant_digit_listing_footer_is_rejected_without_conversion(self):
        digits = "9" * 10000
        self.assertIsNone(self.catalogue("pkg::case: test\n" +
                                         digits + " tests, 0 benchmarks\n"))

    def test_raw_byte_limit_exact_boundary_and_plus_one(self):
        base = (self.raw() +
                "test result: ok. 427 passed; 0 failed; 1 ignored;\n")
        padding = diagnostic.RAW_LIMIT - len(base.encode("ascii")) - 1
        exact = "x" * padding + "\n" + base
        self.assertEqual(len(exact.encode("ascii")), diagnostic.RAW_LIMIT)
        accepted, output = self.capture(exact, 0)
        self.assertTrue(accepted)
        self.assertLess(len(output), diagnostic.OUTPUT_LIMIT["broad"])
        self.assertNotIn(b"x" * 32, output)
        over = exact + "x"
        self.assertEqual(len(over.encode("ascii")), diagnostic.RAW_LIMIT + 1)
        accepted, output = self.capture(over, 0)
        self.assertFalse(accepted)
        self.assertIn(b"raw_limit=exceeded", output)
        self.assertNotIn(b"x" * 32, output)

    def test_main_six_arguments_and_wrong_arity(self):
        (self.root / "auth-stage-cases.raw").write_text(
            "auth::a: test\n1 tests, 0 benchmarks\n")
        for name in ("csrf", "prelogin", "broad"):
            summary = ("test result: ok. 1 passed; 0 failed; 0 ignored;\n"
                       if name != "broad" else
                       "test result: ok. 427 passed; 0 failed; 1 ignored;\n")
            (self.root / f"auth-stage-{name}.raw").write_text(self.raw(name) + summary)

        class Sink:
            def __init__(self):
                self.buffer = io.BytesIO()

        old_argv, old_stdout = diagnostic.sys.argv, diagnostic.sys.stdout
        sink = Sink()
        try:
            diagnostic.sys.argv = ["diagnostic.py", str(self.root), "0", "0", "0", "0"]
            diagnostic.sys.stdout = sink
            six_argument_result = diagnostic.main()
            diagnostic.sys.argv = ["diagnostic.py", str(self.root), "0", "0", "0"]
            wrong_arity_result = diagnostic.main()
        finally:
            diagnostic.sys.argv, diagnostic.sys.stdout = old_argv, old_stdout
        self.assertEqual(six_argument_result, 0)
        self.assertEqual(wrong_arity_result, 2)
        self.assertNotIn(b"/private", sink.buffer.getvalue())


if __name__ == "__main__":
    unittest.main()
