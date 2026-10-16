"""Fake summaries verify the bounded CI parser, never native execution."""
import importlib.util
from pathlib import Path
import re
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/ci/sf07_checkpoint_summary.py"
spec = importlib.util.spec_from_file_location("sf07_summary", SCRIPT)
summary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(summary)


def fixture(mode):
    names = summary.EXPECTED[mode]
    return ("running " + str(len(names)) + " tests\n" +
            "".join("test " + name + " ... ok\n" for name in names) +
            f"\ntest result: ok. {len(names)} passed; 0 failed; 0 ignored; 0 measured; 123 filtered out; finished in 0.01s\n").encode()


class SummaryTests(unittest.TestCase):
    def test_exact_lf_and_crlf_pass_both_registered_scopes(self):
        for mode in summary.EXPECTED:
            for raw in (fixture(mode), fixture(mode).replace(b"\n", b"\r\n")):
                self.assertEqual(summary.classify(raw, mode), "passed")

    def test_old_gate_rejects_complete_crlf_native_rows(self):
        # Reproduce the published ok$ Windows defect using its twelve wire names.
        names = [n for n in summary.EXPECTED["wire"] if "::wire::tests::" in n]
        lf = "".join("test " + n + " ... ok\n" for n in names)
        old = r"(?m)^test http_server::response_budget::wire::tests::[^ ]+ \.\.\. ok$"
        self.assertEqual(len(re.findall(old, lf)), 12)
        self.assertEqual(len(re.findall(old, lf.replace("\n", "\r\n"))), 0)

    def test_lone_cr_invalid_utf8_and_oversize_are_rejected(self):
        for raw in (fixture("wire") + b"\r", fixture("wire") + b"\xff", b"x" * 65537):
            self.assertEqual(summary.classify(raw, "wire"), "invalid")

    def test_missing_empty_or_duplicate_summary_is_unavailable(self):
        for raw in (b"", b"random", fixture("wire") + fixture("wire")):
            self.assertEqual(summary.classify(raw, "wire"), "unavailable")

    def test_compiler_failure_is_separate_from_semantic_red(self):
        for raw in (b"error[E0382]: secret\n", b"error: arbitrary path\n"):
            self.assertEqual(summary.classify(raw, "wire"), "compile_failed")

    def test_native_failure_is_classified_without_acceptance(self):
        raw = fixture("wire").replace(b"ok. 22 passed; 0 failed", b"FAILED. 21 passed; 1 failed")
        self.assertEqual(summary.classify(raw, "wire"), "test_failed")

    def test_zero_wrong_count_ignored_or_missing_rows_are_invalid(self):
        raw = fixture("wire")
        bad = (raw.replace(b"22 passed", b"0 passed"),
               raw.replace(b"0 ignored", b"1 ignored"),
               b"\n".join(raw.split(b"\n")[2:]))
        for mutated in bad:
            self.assertEqual(summary.classify(mutated, "wire"), "invalid")

    def test_duplicate_foreign_or_non_ok_test_row_is_invalid(self):
        raw = fixture("wire")
        first, second = (n.encode() for n in summary.EXPECTED["wire"][:2])
        for mutated in (raw.replace(second, first), raw.replace(first, b"unbound::test"),
                        raw.replace(b" ... ok\n", b" ... ignored\n", 1),
                        raw + b"test unbound::test ... ok\n", raw + b"test malformed row\n"):
            self.assertEqual(summary.classify(mutated, "wire"), "invalid")

    def test_mode_cannot_accept_other_scope_or_unknown_value(self):
        self.assertEqual(summary.classify(fixture("wire"), "quota"), "invalid")
        self.assertEqual(summary.classify(fixture("wire"), "secret-mode"), "invalid")

    def test_closed_cli_output_exit_and_bounded_input(self):
        for mode, raw, state in (("wire", fixture("wire"), "passed"),
                                 ("quota", fixture("quota").replace(b"\n", b"\r\n"), "passed"),
                                 ("wire", b"error: SECRET/path/key\n", "compile_failed"),
                                 ("secret-mode", b"SECRET/path/key", "invalid"),
                                 ("wire", b"x" * 65537, "invalid")):
            result = subprocess.run([sys.executable, "-B", str(SCRIPT), mode], input=raw, capture_output=True)
            label = mode if mode in summary.EXPECTED else "invalid"
            self.assertEqual(result.returncode, 0 if state == "passed" else 1)
            self.assertEqual(result.stdout, f"sf07_checkpoint scope={label} state={state}\n".encode())
            self.assertEqual(result.stderr, b"")

    def test_closed_inventories_match_all_original_native_oracles(self):
        def native(path, prefix):
            text = (ROOT/path).read_text()
            return {prefix + n for n in re.findall(r"#\[test\]\s*fn ([a-zA-Z0-9_]+)\(", text)}
        wire = native("runtime/src/http_server/response_budget_tests.rs", "http_server::response_budget::tests::")
        wire |= native("runtime/src/http_server/response_wire_tests.rs", "http_server::response_budget::wire::tests::")
        quota = native("runtime/src/security_sf07_private/quota.rs", "security_sf07_private::quota::tests::")
        self.assertEqual(set(summary.EXPECTED["wire"]), wire)
        self.assertEqual(set(summary.EXPECTED["quota"]), quota)
        self.assertEqual((len(wire), len(quota)), (22, 9))

    def test_both_required_ci_steps_register_source_parser_and_all_native_tests(self):
        text = (ROOT/".github/workflows/ci.yml").read_text()
        for command in ("python -B scripts/tests/test_sf07_checkpoint_summary.py",
                        "| python -B scripts/ci/sf07_checkpoint_summary.py wire",
                        "| python -B scripts/ci/sf07_checkpoint_summary.py quota"):
            self.assertEqual(text.count(command), 2)


if __name__ == "__main__":
    unittest.main()
