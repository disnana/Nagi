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


def normalize_cli_stdout(raw):
    output = raw.replace(b"\r\n", b"\n")
    return None if b"\r" in output else output


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
        raw = fixture("wire").replace(b"ok. 38 passed; 0 failed", b"FAILED. 37 passed; 1 failed")
        self.assertEqual(summary.classify(raw, "wire"), "test_failed")

    def test_zero_wrong_count_ignored_or_missing_rows_are_invalid(self):
        raw = fixture("wire")
        bad = (raw.replace(b"38 passed", b"0 passed"),
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
            expected = f"sf07_checkpoint scope={label} state={state}\n".encode()
            self.assertEqual(normalize_cli_stdout(result.stdout), expected)
            self.assertEqual(result.stderr, b"")

        # Simulate Windows text-stream translation while running the real CLI.
        wrapper = (
            "import runpy,sys; script=sys.argv[1]; mode=sys.argv[2]; "
            "sys.stdout.reconfigure(newline='\\r\\n'); "
            "sys.argv=[script,mode]; runpy.run_path(script,run_name='__main__')"
        )
        result = subprocess.run(
            [sys.executable, "-B", "-c", wrapper, str(SCRIPT), "wire"],
            input=fixture("wire"), capture_output=True,
        )
        self.assertEqual(result.returncode, 0)
        expected = b"sf07_checkpoint scope=wire state=passed\n"
        self.assertEqual(normalize_cli_stdout(result.stdout), expected)
        self.assertEqual(result.stderr, b"")
        for malformed in (
            b"sf07_checkpoint scope=wire state=passed\r\r\n",
            expected + b"\r",
            expected[:-1] + b"\x00\n",
            expected[:-1] + b"unknown\n",
        ):
            self.assertNotEqual(normalize_cli_stdout(malformed), expected)

    def test_closed_inventories_match_all_original_native_oracles(self):
        def native(path, prefix):
            text = (ROOT/path).read_text()
            return {prefix + n for n in re.findall(r"#\[test\]\s*fn ([a-zA-Z0-9_]+)\(", text)}
        wire = native("runtime/src/http_server/response_budget_tests.rs", "http_server::response_budget::tests::")
        wire |= native("runtime/src/http_server/response_wire_tests.rs", "http_server::response_budget::wire::tests::")
        quota = native("runtime/src/security_sf07_private/quota.rs", "security_sf07_private::quota::tests::")
        wire |= native("runtime/src/http_server/response_finalize_tests.rs", "http_server::response_budget::finalize::tests::")
        self.assertEqual(set(summary.EXPECTED["wire"]), wire)
        self.assertEqual(set(summary.EXPECTED["quota"]), quota)
        self.assertEqual((len(wire), len(quota)), (38, 9))

    def test_previous_59_names_and_new_20_are_exact_and_required(self):
        import hashlib
        import json
        old = {k: list(summary.EXPECTED[k]) for k in ("wire", "quota", "transport", "request", "rate-startup")}
        old["wire"] = old["wire"][:22]
        old["transport"] = old["transport"][:8]
        self.assertEqual(hashlib.sha256(json.dumps(old, sort_keys=True, separators=(",", ":")).encode()).hexdigest(), '91fc28d8e6cff5efe7f56b95eae0b838766c1835eee0f5fbb0d34f3a65a71552')
        self.assertEqual(sum(map(len, summary.EXPECTED.values())), 83)
        paths = {
            "catalog-retained": ("runtime/src/security_sf07_private/quota_retention_tests.rs", "security_sf07_private::quota::retained_catalog_tests::"),
            "transport": ("runtime/src/http_server/response_send_tests.rs", "http_server::response_send::tests::"),
            "fixed-owner": ("runtime/src/auth/csrf/fixed_body.rs", "auth::csrf::fixed_body::tests::"),
            "csrf-issuer": ("runtime/src/auth/csrf/fixed_wire_owner_tests.rs", "auth::csrf::tests::fixed_wire_owner_tests::"),
            "prelogin-issuer": ("runtime/src/auth/prelogin/fixed_wire_owner_tests.rs", "auth::prelogin::tests::fixed_wire_owner_tests::"),
        }
        for mode, (path, prefix) in paths.items():
            names = re.findall(r"#\[(?:tokio::)?test(?:\([^\]]*\))?\]\s*(?:async\s+)?fn ([a-zA-Z0-9_]+)\(", (ROOT/path).read_text())
            self.assertEqual(set(summary.EXPECTED[mode]), {prefix + name for name in names})
        for mode in ("fixed-owner", "csrf-issuer", "prelogin-issuer", "catalog-retained"):
            for raw in (fixture(mode), fixture(mode).replace(b"\n", b"\r\n")):
                self.assertEqual(summary.classify(raw, mode), "passed")
                self.assertNotEqual(summary.classify(raw.replace(b"0 ignored", b"1 ignored"), mode), "passed")
                first = summary.EXPECTED[mode][0].encode()
                self.assertNotEqual(summary.classify(raw.replace(first, b"foreign::test"), mode), "passed")
        ci = (ROOT / ".github/workflows/ci.yml").read_text()
        for mode in ("fixed-owner", "csrf-issuer", "prelogin-issuer", "catalog-retained"):
            self.assertEqual(ci.count("| python -B scripts/ci/sf07_checkpoint_summary.py " + mode), 2)

    def test_both_required_ci_steps_register_source_parser_and_all_native_tests(self):
        text = (ROOT/".github/workflows/ci.yml").read_text()
        for command in ("python -B scripts/tests/test_sf07_checkpoint_summary.py",
                        "| python -B scripts/ci/sf07_checkpoint_summary.py wire",
                        "| python -B scripts/ci/sf07_checkpoint_summary.py quota"):
            self.assertEqual(text.count(command), 2)



class CatalogFailureDiagnosticTests(unittest.TestCase):
    def failed(self, index):
        raw = fixture("catalog-retained")
        name = summary.EXPECTED["catalog-retained"][index].encode()
        raw = raw.replace(b"test " + name + b" ... ok", b"test " + name + b" ... FAILED")
        return raw.replace(b"ok. 3 passed; 0 failed", b"FAILED. 2 passed; 1 failed")

    def test_each_fixed_case_and_crlf_project_without_acceptance(self):
        for index in range(3):
            expected = tuple("failed" if i == index else "passed" for i in range(3))
            for raw in (self.failed(index), self.failed(index).replace(b"\n", b"\r\n")):
                self.assertEqual(summary.catalog_failure_projection(raw), expected)
                self.assertEqual(summary.classify(raw, "catalog-retained"), "test_failed")

    def test_all_finite_failure_combinations_match_their_summary(self):
        import itertools
        for states in itertools.product(("ok", "FAILED"), repeat=3):
            failures = states.count("FAILED")
            if not failures:
                continue
            raw = fixture("catalog-retained")
            for name, state in zip(summary.EXPECTED["catalog-retained"], states):
                raw = raw.replace(("test " + name + " ... ok").encode(),
                                  ("test " + name + " ... " + state).encode())
            raw = raw.replace(b"ok. 3 passed; 0 failed",
                              f"FAILED. {3-failures} passed; {failures} failed".encode())
            expected = tuple("failed" if state == "FAILED" else "passed" for state in states)
            self.assertEqual(summary.catalog_failure_projection(raw), expected)

    def test_malformed_inventory_is_unavailable(self):
        raw = self.failed(0)
        first, second = (n.encode() for n in summary.EXPECTED["catalog-retained"][:2])
        for bad in (raw.replace(second, first), raw.replace(first, b"foreign::secret"),
                    raw.replace(b" ... FAILED", b" ... ignored"),
                    raw.replace(b"2 passed", b"1 passed"),
                    raw.replace(b"running 3", b"running 0"),
                    raw + raw, raw + b"test malformed row\n"):
            self.assertIsNone(summary.catalog_failure_projection(bad))

    def test_private_text_is_never_projected(self):
        raw = self.failed(1) + b"SECRET/path/sql/cookie arbitrary assertion\n"
        self.assertEqual(summary.catalog_failure_projection(raw), ("passed", "failed", "passed"))
        result = subprocess.run([sys.executable, "-B", str(SCRIPT), "catalog-retained"],
                                input=raw, capture_output=True)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stderr, b"")
        self.assertEqual(normalize_cli_stdout(result.stdout),
            b"sf07_checkpoint scope=catalog-retained state=test_failed\n"
            b"sf07_catalog_diagnostic state=classified receipt=passed preallocation=failed excess_capacity=passed\n")

    def test_no_projection_from_green_compile_or_oversize(self):
        for bad in (fixture("catalog-retained"), b"error[E0382]: SECRET/path\n",
                    self.failed(0) + b"\r", self.failed(0) + b"\xff", b"x" * 65537):
            self.assertIsNone(summary.catalog_failure_projection(bad))

    def test_cli_unavailable_is_fixed_and_stays_failure(self):
        raw = self.failed(0).replace(b"running 3", b"running 2")
        result = subprocess.run([sys.executable, "-B", str(SCRIPT), "catalog-retained"],
                                input=raw, capture_output=True)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(normalize_cli_stdout(result.stdout),
            b"sf07_checkpoint scope=catalog-retained state=test_failed\n"
            b"sf07_catalog_diagnostic state=unavailable\n")
        self.assertEqual(result.stderr, b"")


if __name__ == "__main__":
    unittest.main()
