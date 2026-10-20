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
projection_spec = importlib.util.spec_from_file_location(
    "checkpoint_projection", ROOT / "scripts/ci/sf07_request_projection.py")
projection = importlib.util.module_from_spec(projection_spec)
projection_spec.loader.exec_module(projection)


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
        # Reproduce the published ok$ Windows defect using all thirteen wire tests.
        names = [n for n in summary.EXPECTED["wire"] if "::wire::tests::" in n]
        lf = "".join("test " + n + " ... ok\n" for n in names)
        old = r"(?m)^test http_server::response_budget::wire::tests::[^ ]+ \.\.\. ok$"
        self.assertEqual(len(re.findall(old, lf)), 13)
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
        original = fixture("wire")
        count = len(summary.EXPECTED["wire"])
        raw = original.replace(f"ok. {count} passed; 0 failed".encode(),
                               f"FAILED. {count - 1} passed; 1 failed".encode())
        self.assertNotEqual(raw, original)
        self.assertEqual(summary.classify(raw, "wire"), "test_failed")

    def test_zero_wrong_count_ignored_or_missing_rows_are_invalid(self):
        raw = fixture("wire")
        bad = (raw.replace(f"{len(summary.EXPECTED['wire'])} passed".encode(), b"0 passed"),
               raw.replace(b"0 ignored", b"1 ignored"),
               b"\n".join(raw.split(b"\n")[2:]))
        for mutated in bad:
            self.assertNotEqual(mutated, raw)
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
        self.assertEqual((len(wire), len(quota)), (40, 9))

    def test_previous_59_names_and_current_inventory_are_exact_and_required(self):
        import hashlib
        import json
        old = {k: list(summary.EXPECTED[k]) for k in ("wire", "quota", "transport", "request", "rate-startup")}
        old["wire"] = old["wire"][:22]
        old["transport"] = old["transport"][:8]
        self.assertEqual(hashlib.sha256(json.dumps(old, sort_keys=True, separators=(",", ":")).encode()).hexdigest(), '91fc28d8e6cff5efe7f56b95eae0b838766c1835eee0f5fbb0d34f3a65a71552')
        self.assertEqual(sum(map(len, summary.EXPECTED.values())), 85)
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
        restored = projection.inverse_source_guard_dedup(text)
        for command in ("python -B scripts/tests/test_sf07_checkpoint_summary.py",
                        "| python -B scripts/ci/sf07_checkpoint_summary.py wire",
                        "| python -B scripts/ci/sf07_checkpoint_summary.py quota"):
            current_count = 1 if command.startswith("python -B scripts/tests/") else 2
            self.assertEqual(text.count(command), current_count)
            self.assertEqual(restored.count(command), 2)



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



def browser_list_fixture(names=None, benchmarks=0):
    names = summary.BROWSER_COUNTER_CASES if names is None else tuple(names)
    rows = "".join(name + ": test\n" for name in names)
    return (rows + f"{len(names)} tests, {benchmarks} benchmarks\n").encode()


def browser_profile_fixture(mode, failures=()):
    names = summary.BROWSER_COUNTER_PROFILES[mode]
    failures = set(failures)
    count = len(names)
    filtered = len(summary.BROWSER_COUNTER_CASES) - count
    running = "test" if count == 1 else "tests"
    rows = "".join("test " + name + " ... " + ("FAILED" if name in failures else "ok") + "\n"
                    for name in names)
    if failures:
        status = f"FAILED. {count-len(failures)} passed; {len(failures)} failed"
    else:
        status = f"ok. {count} passed; 0 failed"
    return (f"running {count} {running}\n" + rows
            + f"test result: {status}; 0 ignored; 0 measured; {filtered} filtered out; finished in 0.01s\n").encode()


def browser_strict_counter_line():
    return ("sf03_browser_failure_counters_v1 case=strict_json_valid phase=final_response "
            "status=403 status_overflow=false body_bytes=2 body_bytes_overflow=false "
            "set_cookie_count=0 set_cookie_count_overflow=false auth_cookie=false deletion=false "
            "verifier_calls=0 verifier_calls_overflow=false handler_calls=0 handler_calls_overflow=false "
            "claim_committed=false issue_committed=false mode6_cancel_seen=0 mode6_cancel_seen_overflow=false")


def browser_wire_counter_line(iteration=0):
    return ("sf03_browser_failure_counters_v1 case=wire_body_cap phase=exact_body_response "
            f"iteration={iteration} status=200 status_overflow=false body_bytes=192 body_bytes_overflow=false "
            "expected_body_bytes=192 expected_body_bytes_overflow=false body_matches=true "
            "set_cookie_count=2 set_cookie_count_overflow=false auth_cookie=true deletion=true "
            "verifier_calls=1 verifier_calls_overflow=false handler_calls=1 handler_calls_overflow=false "
            "claim_committed=true issue_committed=true mode6_cancel_seen=0 mode6_cancel_seen_overflow=false")


def run_browser_cli(mode, raw):
    return subprocess.run([sys.executable, "-B", str(SCRIPT), mode], input=raw, capture_output=True)


class BrowserCounterProfileTests(unittest.TestCase):
    def test_independent_inventory_keeps_original_sf07_expectation_count(self):
        self.assertEqual(sum(map(len, summary.EXPECTED.values())), 85)
        self.assertEqual(len(summary.BROWSER_COUNTER_CASES), 10)
        self.assertEqual(len(summary.BROWSER_COUNTER_PROFILES["browser-counter-strict"]), 1)
        self.assertEqual(len(summary.BROWSER_COUNTER_PROFILES["browser-counter-wire"]), 1)
        self.assertEqual(summary.BROWSER_COUNTER_PROFILES["browser-counter-strict"][0],
                         "input_and_strict_json_rejections_do_not_consume_or_call_verifier")
        self.assertEqual(summary.BROWSER_COUNTER_PROFILES["browser-counter-wire"][0],
                         "wire_body_cap_is_independent_of_db_quota_and_checked_before_claim")

    def test_closed_inventory_matches_all_ten_public_rust_tests(self):
        source = (ROOT / "runtime/tests/browser_login_public_api.rs").read_text()
        names = re.findall(r"#\[tokio::test\]\s*async fn ([a-zA-Z0-9_]+)\(", source)
        self.assertEqual(len(names), 10)
        self.assertEqual(set(names), set(summary.BROWSER_COUNTER_CASES))

    def test_list_requires_all_ten_exact_unique_names_and_no_benchmarks(self):
        raw = browser_list_fixture()
        for value in (raw, raw.replace(b"\n", b"\r\n")):
            self.assertEqual(summary.classify(raw if value == raw else value, summary.BROWSER_COUNTER_LIST), "passed")
        first, second = summary.BROWSER_COUNTER_CASES[:2]
        malformed = (
            raw.replace(second.encode(), b"renamed_test"),
            raw.replace(second.encode(), first.encode()),
            raw.replace((summary.BROWSER_COUNTER_CASES[-1] + ": test\n").encode(), b""),
            raw + b"foreign_test: test\n",
            raw.replace(b"10 tests, 0 benchmarks", b"0 tests, 0 benchmarks"),
            raw.replace(b"10 tests, 0 benchmarks", b"10 tests, 1 benchmarks"),
        )
        for value in malformed:
            self.assertEqual(summary.classify(value, summary.BROWSER_COUNTER_LIST), "invalid")

    def test_all_three_profiles_require_exact_rows_and_exact_summaries(self):
        for mode in summary.BROWSER_COUNTER_PROFILES:
            raw = browser_profile_fixture(mode)
            for value in (raw, raw.replace(b"\n", b"\r\n")):
                self.assertEqual(summary.classify(value, mode), "passed")
        strict = browser_profile_fixture("browser-counter-strict")
        single_bad = (
            strict.replace(b"running 1 test", b"running 0 tests"),
            strict.replace(summary.BROWSER_COUNTER_PROFILES["browser-counter-strict"][0].encode(), b"foreign_test"),
            strict.replace(b"9 filtered out", b"8 filtered out"),
            strict.replace(b"0 ignored", b"1 ignored"),
            strict + b"test duplicate ... ok\n",
            strict.replace(b"test result: ok.", b"test result: ok.", 1) + b"test result: ok. duplicate\n",
        )
        for value in single_bad:
            self.assertEqual(summary.classify(value, "browser-counter-strict"), "invalid")
        parallel = browser_profile_fixture("browser-counter-parallel")
        self.assertEqual(summary.classify(parallel.replace(b"running 10 tests", b"running 9 tests"),
                                          "browser-counter-parallel"), "invalid")
        self.assertEqual(summary.classify(parallel.replace(b"10 passed; 0 failed", b"9 passed; 0 failed"),
                                          "browser-counter-parallel"), "invalid")

    def test_failure_profiles_remain_failures_only_after_complete_inventory(self):
        strict = summary.BROWSER_COUNTER_PROFILES["browser-counter-strict"][0]
        wire = summary.BROWSER_COUNTER_PROFILES["browser-counter-wire"][0]
        self.assertEqual(summary.classify(browser_profile_fixture("browser-counter-strict", (strict,)),
                                          "browser-counter-strict"), "test_failed")
        self.assertEqual(summary.classify(browser_profile_fixture("browser-counter-parallel", (wire,)),
                                          "browser-counter-parallel"), "test_failed")
        failed = browser_profile_fixture("browser-counter-parallel", (wire,))
        for value in (failed.replace(b"0 ignored", b"1 ignored"),
                      failed.replace(b"wire_body_cap_is_independent_of_db_quota_and_checked_before_claim", b"foreign_test"),
                      failed.replace(b"9 passed; 1 failed", b"10 passed; 1 failed"),
                      failed.replace(b"0 filtered out", b"1 filtered out")):
            self.assertEqual(summary.classify(value, "browser-counter-parallel"), "invalid")

    def test_cli_renders_exact_failed_case_and_validated_strict_sample(self):
        name = summary.BROWSER_COUNTER_PROFILES["browser-counter-strict"][0]
        raw = (browser_profile_fixture("browser-counter-strict", (name,))
               + browser_strict_counter_line().encode() + b"\n"
               + summary.BROWSER_COUNTER_CARGO_TEST_FAILURE.encode() + b"\n")
        expected = ("sf07_checkpoint scope=browser-counter-strict state=test_failed\n"
                    "sf07_browser_cases state=complete count=1\n"
                    "sf07_browser_case name=input_and_strict_json_rejections_do_not_consume_or_call_verifier state=failed\n"
                    "sf07_browser_counters state=present count=1\n"
                    "sf07_browser_counter_sample case=strict_json_valid phase=final_response status=403 status_overflow=false body_bytes=2 body_bytes_overflow=false set_cookie_count=0 set_cookie_count_overflow=false auth_cookie=false deletion=false verifier_calls=0 verifier_calls_overflow=false handler_calls=0 handler_calls_overflow=false claim_committed=false issue_committed=false mode6_cancel_seen=0 mode6_cancel_seen_overflow=false\n").encode()
        for value in (raw, raw.replace(b"\n", b"\r\n")):
            result = run_browser_cli("browser-counter-strict", value)
            self.assertEqual(result.returncode, 1)
            self.assertEqual(normalize_cli_stdout(result.stdout), expected)
            self.assertEqual(result.stderr, b"")

    def test_cli_parallel_inventory_is_complete_and_absent_counters_are_explicit(self):
        raw = browser_profile_fixture("browser-counter-parallel")
        expected = [
            "sf07_checkpoint scope=browser-counter-parallel state=passed",
            "sf07_browser_cases state=complete count=10",
        ]
        expected.extend("sf07_browser_case name=" + name + " state=passed"
                        for name in summary.BROWSER_COUNTER_CASES)
        expected.append("sf07_browser_counters state=absent count=0")
        result = run_browser_cli("browser-counter-parallel", raw.replace(b"\n", b"\r\n"))
        self.assertEqual(result.returncode, 0)
        self.assertEqual(normalize_cli_stdout(result.stdout), ("\n".join(expected) + "\n").encode())
        self.assertEqual(result.stderr, b"")

    def test_cli_parallel_failures_show_both_schemas_without_cross_field_inference(self):
        strict_name, wire_name = (summary.BROWSER_COUNTER_CASES[index] for index in (1, 4))
        raw = (browser_profile_fixture("browser-counter-parallel", (strict_name, wire_name))
               + (browser_strict_counter_line() + "\n" + browser_wire_counter_line(1) + "\n"
                  + browser_wire_counter_line(0) + "\n").encode()
               + summary.BROWSER_COUNTER_CARGO_TEST_FAILURE.encode() + b"\n")
        result = run_browser_cli("browser-counter-parallel", raw)
        output = normalize_cli_stdout(result.stdout)
        self.assertEqual(result.returncode, 1)
        self.assertIsNotNone(output)
        lines = output.decode().splitlines()
        self.assertEqual(lines[0], "sf07_checkpoint scope=browser-counter-parallel state=test_failed")
        self.assertEqual(lines[1], "sf07_browser_cases state=complete count=10")
        cases = lines[2:12]
        self.assertEqual(cases, ["sf07_browser_case name=" + name + " state=" +
                                 ("failed" if name in (strict_name, wire_name) else "passed")
                                 for name in summary.BROWSER_COUNTER_CASES])
        self.assertEqual(lines[12], "sf07_browser_counters state=present count=3")
        samples = lines[13:]
        self.assertEqual(len(samples), 3)
        self.assertTrue(samples[0].startswith("sf07_browser_counter_sample case=strict_json_valid "))
        self.assertIn("iteration=0 ", samples[1])
        self.assertIn("iteration=1 ", samples[2])
        self.assertEqual(result.stderr, b"")

    def test_cli_rejects_secret_duplicate_unknown_and_out_of_range_counter_lines(self):
        strict_name, wire_name = (summary.BROWSER_COUNTER_CASES[index] for index in (1, 4))
        base = (browser_profile_fixture("browser-counter-parallel", (strict_name, wire_name))
                + (browser_strict_counter_line() + "\n" + browser_wire_counter_line(0) + "\n").encode())
        valid_wire = browser_wire_counter_line(0).encode()
        bad_inputs = (
            base + b"SECRET cookie=/private/raw\n",
            base + valid_wire + b"\n",
            base + b"sf03_browser_failure_counters_v1 case=unknown phase=final_response secret=/private\n",
            base + valid_wire.replace(b"status=200", b"status=1000") + b"\n",
            base + b"sf03_browser_failure_counters_v2 case=wire_body_cap secret=/private\n",
            base + b"sf03_browser_failure_counters_v1 case=wire_body_cap phase=exact_body_response iteration=0 status=200 status_overflow=false secret=/private\n",
            base + b"sf03_browser_failure_counters_v1 case=wire_body_cap phase=exact_body_response iteration=0 status=200\n",
            base + valid_wire.replace(b"status=200", b"status=" + b"9" * 5000) + b"\n",
        )
        for raw in bad_inputs:
            with self.subTest(input_sha256=__import__("hashlib").sha256(raw).hexdigest()):
                result = run_browser_cli("browser-counter-parallel", raw)
                output = normalize_cli_stdout(result.stdout)
                self.assertEqual(result.returncode, 1)
                self.assertIsNotNone(output)
                self.assertIn(b"sf07_checkpoint scope=browser-counter-parallel state=invalid\n", output)
                self.assertIn(b"sf07_browser_cases state=complete count=10\n", output)
                self.assertIn(b"sf07_browser_counters state=unavailable\n", output)
                self.assertNotIn(b"/private", output + result.stderr)
                self.assertNotIn(b"SECRET", output + result.stderr)
                self.assertNotIn(b"secret=", output + result.stderr)
                self.assertEqual(result.stderr, b"")

    def test_cli_compile_error_and_oversize_remain_closed_failures(self):
        self.assertEqual(summary.classify_browser_counter_compile(b"Finished test profile\n"), "compiled")
        self.assertEqual(summary.classify_browser_counter_compile(b"error[E0382]: private/path\n"), "compile_failed")
        self.assertEqual(summary.classify_browser_counter_compile(b"x" * (summary.MAX_BYTES + 1)), "invalid")
        compile_result = run_browser_cli("browser-counter-compile", b"error[E0382]: SECRET/private/path\n")
        self.assertEqual(compile_result.returncode, 1)
        self.assertEqual(normalize_cli_stdout(compile_result.stdout),
                         b"sf07_checkpoint scope=browser-counter-compile state=compile_failed\n")
        self.assertEqual(compile_result.stderr, b"")
        raw = b"x" * (summary.MAX_BYTES + 1)
        result = run_browser_cli("browser-counter-parallel", raw)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(normalize_cli_stdout(result.stdout),
                         b"sf07_checkpoint scope=browser-counter-parallel state=invalid\n"
                         b"sf07_browser_cases state=unavailable\n"
                         b"sf07_browser_counters state=unavailable\n")
        self.assertNotIn(b"x", result.stdout + result.stderr)
        self.assertEqual(result.stderr, b"")



if __name__ == "__main__":
    unittest.main()
