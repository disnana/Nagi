"""Synthetic inventory controls; not execution evidence for Rust/native cases."""
import ast
import hashlib
import importlib.util
import io
from pathlib import Path
import re
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
CI = ROOT / "scripts/ci"
sys.path.insert(0, str(CI))
import session_native_finish_summary as gate


def load_projection():
    spec = importlib.util.spec_from_file_location("native_finish_projection", CI / "sf07_request_projection.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def green():
    return ("running 12 tests\n" + "".join("test " + name + " ... ok\n" for name in sorted(gate.REQUIRED))
            + "test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 999 filtered out; finished in 0.01s\n").encode()


class NativeFinishSummaryTests(unittest.TestCase):
    def test_exact_twelve_source_names_match_independent_handwritten_expectation(self):
        expected = {
            "auth::session_deadline_observer::native_finish::tests::": {
                "unseen_sentinel_is_distinct_from_zero_and_clamped_elapsed",
                "result_reason_and_checkout_are_closed_independent_boundaries",
                "serial_tx_resets_and_overlapping_or_saturated_tx_is_invalid",
                "duplicate_or_out_of_order_marker_never_fabricates_completion",
                "delayed_worker_updates_only_original_owner_and_not_copied_terminal",
                "closed_numeric_line_and_poisoned_record_cannot_expose_payload",
            },
            "sqlite::session::native_finish_tests::": {
                "real_pool_commit_ack_follows_original_checkout_drop",
                "old_commit_gate_is_after_native_return_and_before_checkout",
                "denied_commit_records_err_then_actual_cleanup_ok_or_err",
                "eof_cleanup_retains_reason_and_retirement_without_retry",
                "old_management_panic_has_unseen_commit_and_after_finish_cleanup",
                "precommit_expired_observer_never_invents_commit_entry",
            },
        }
        paths = ("runtime/src/auth/session_native_finish_tests.rs", "runtime/src/sqlite/session_native_finish_tests.rs")
        for (prefix, names), path in zip(expected.items(), paths):
            source = (ROOT / path).read_text()
            actual = re.findall(r"#\[(?:tokio::)?test\]\s*(?:async )?fn (\w+)\(", source)
            self.assertEqual(len(actual), 6)
            self.assertEqual(set(actual), names)
            self.assertNotIn("#[ignore", source)
        self.assertEqual(gate.REQUIRED, frozenset(prefix + name for prefix, names in expected.items() for name in names))
        self.assertEqual(len(gate.REQUIRED), 12)

    def test_green_lf_crlf_and_reordered_test_rows(self):
        self.assertTrue(gate.accepts(green()))
        self.assertTrue(gate.accepts(green().replace(b"\n", b"\r\n")))
        self.assertTrue(gate.accepts(green(), 0))

    def test_zero_missing_duplicate_foreign_and_ignored_never_pass(self):
        row = ("test " + sorted(gate.REQUIRED)[0] + " ... ok\n").encode()
        for raw in (b"", b"running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.0s\n",
                    green().replace(row, b""), green() + row,
                    green().replace(row, b"test foreign::case ... ok\n"),
                    green().replace(row, row.replace(b"ok", b"ignored")),
                    green().replace(b"0 ignored", b"1 ignored")):
            self.assertFalse(gate.accepts(raw))

    def test_nonzero_exit_failed_summary_and_duplicate_summaries_never_pass(self):
        for code in (-1, 1, 101, True):
            self.assertFalse(gate.accepts(green(), code))
        for raw in (green().replace(b"12 passed", b"11 passed"),
                    green().replace(b"0 failed", b"1 failed"),
                    green().replace(b"... ok", b"... FAILED", 1),
                    green() + green(), b"error[E0308]: private/path secret\n"):
            self.assertFalse(gate.accepts(raw))

    def test_max_bound_utf8_controls_and_actual_stream_draining(self):
        padded = green() + b" " * (gate.MAX_BYTES - len(green()))
        self.assertTrue(gate.accepts(padded))
        for raw in (padded + b"x", green() + b"\xff", green() + b"\r", green() + b"\x1b[0m"):
            self.assertFalse(gate.accepts(raw))
        stream = io.BytesIO(padded + b"overflow" * 10000)
        retained = gate.capture(stream)
        self.assertEqual(len(retained), gate.MAX_BYTES + 1)
        self.assertEqual(stream.read(), b"")
        self.assertFalse(gate.accepts(retained))

    def test_cli_is_closed_and_invalid_input_returns_failure(self):
        script = str(CI / "session_native_finish_summary.py")
        driver = '''import pathlib, runpy, sys
script = sys.argv[1]
sys.path.insert(0, str(pathlib.Path(script).parent))
sys.argv = [script]
sys.stdout.reconfigure(newline="\\r\\n")
runpy.run_path(script, run_name="__main__")
'''
        commands = ([sys.executable, "-B", script],
                    [sys.executable, "-B", "-c", driver, script])
        for raw, code, output in ((green(), 0, b"session_native_finish_inventory=passed\n"),
                                  (b"private/path cookie=SECRET\n", 1, b"session_native_finish_inventory=invalid\n"),
                                  (b"x" * (gate.MAX_BYTES + 20), 1, b"session_native_finish_inventory=invalid\n")):
            for command in commands:
                result = subprocess.run(command, input=raw, capture_output=True, timeout=15)
                # Python text stdout uses CRLF on Windows. Canonicalize pairs
                # only; lone CR, extra lines and all other bytes stay invalid.
                canonical = result.stdout.replace(b"\r\n", b"\n")
                self.assertEqual((result.returncode, canonical, result.stderr), (code, output, b""))
                if command is commands[1]:
                    self.assertEqual(result.stdout, output.replace(b"\n", b"\r\n"))
            for malformed in (output.replace(b"\n", b"\r"),
                              output.replace(b"\n", b"\r\r\n"),
                              output + b"\n", output + b"extra\n"):
                self.assertNotEqual(malformed.replace(b"\r\n", b"\n"), output)

    def test_import_does_not_start_process_signal_or_native_calls(self):
        code = '''import sys
from unittest.mock import patch
sys.path.insert(0, sys.argv[1])
with patch("subprocess.Popen", side_effect=AssertionError("process")), patch("ctypes.CDLL", side_effect=AssertionError("native")), patch("signal.signal", side_effect=AssertionError("signal")):
 import session_native_finish_summary
'''
        result = subprocess.run([sys.executable, "-B", "-c", code, str(CI)], capture_output=True, timeout=15)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (0, b"", b""))

    def test_reused_inventory_ast_remains_frozen(self):
        module = ast.parse((CI / "csrf_deadline_mutations.py").read_text())
        function = next(node for node in module.body if isinstance(node, ast.FunctionDef) and node.name == "inventory")
        self.assertEqual(hashlib.sha256(ast.dump(function, include_attributes=False).encode()).hexdigest(),
                         "66bf327fb6bc2348e418c0d82a3d6695e6495a4649afbfb46915a879838d74ca")

    def test_workflow_exact_additions_inverse_and_failure_propagation(self):
        projection = load_projection()
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        old_fake = "          python scripts/tests/test_session_deadline_diagnostic.py\n"
        additions = projection.SESSION_NATIVE_FINISH_ADDITIONS
        self.assertEqual(workflow.count(old_fake + additions[0] + additions[1] + "          set +e\n"), 2)
        self.assertEqual(workflow.count("CARGO_TERM_COLOR=never RUSTFLAGS='--cfg nagi_auth_stage_diag' cargo test --locked -p nagi-runtime --lib native_finish"), 2)
        self.assertEqual(workflow.count('if [ "${native_finish_status[0]}" -ne 0 ] || [ "${native_finish_status[1]}" -ne 0 ]; then diagnose_rc=1; fi'), 2)
        self.assertEqual(workflow.count("if: always() && steps.session_deadline_diag.outcome != 'skipped'"), 2)
        self.assertEqual(workflow.count("run: test '${{ steps.session_deadline_diag.outcome }}' = success"), 2)
        self.assertEqual(hashlib.sha256(projection.inverse_workflow(workflow).encode()).hexdigest(),
                         "6113a9a6f22f9ffc2189316391cac6b35c024f52e854c65d387c925c835bca79")
        for added in additions:
            for mutated in (workflow.replace(added, "", 1), workflow.replace(added, added + added, 1),
                            workflow.replace(added, added.replace("python", "python -c pass", 1), 1)):
                with self.assertRaises(ValueError):
                    projection.inverse_workflow(mutated)

    def test_collector_inverse_preserves_old_frozen_bytes_and_refuses_changed_added_regions(self):
        projection = load_projection()
        source = (CI / "session_private_deadline_diagnostic.py").read_text()
        self.assertEqual(hashlib.sha256(projection.inverse_collector(source).encode()).hexdigest(),
                         "1d0dd3c92e07c374de80d4f968c101141eb6290ba2b935bf9c06dcb85ac60913")
        for added, inherited in projection.SESSION_NATIVE_COLLECTOR_INVERSE:
            for mutated in (source.replace(added, "", 1), source.replace(added, added + added, 1),
                            source.replace(added, inherited, 1)):
                with self.assertRaises(ValueError):
                    projection.inverse_collector(mutated)
        for before, after in (("256 * 1024", "512 * 1024"),
                              ("process.wait(timeout=600)", "process.wait(timeout=601)"),
                              ("reader.join(timeout=10)", "reader.join(timeout=0)"),
                              ('"--", "--exact", "--show-output"', '"--", "--show-output"')):
            self.assertIn(before, source)
            mutated = source.replace(before, after, 1)
            try:
                restored = projection.inverse_collector(mutated)
            except ValueError:
                continue
            self.assertNotEqual(hashlib.sha256(restored.encode()).hexdigest(),
                                "1d0dd3c92e07c374de80d4f968c101141eb6290ba2b935bf9c06dcb85ac60913")


if __name__ == "__main__":
    unittest.main()
