"""Negative controls for required native BrowserLogin stage coverage."""
import importlib.util
import pathlib
import unittest
import contextlib
import io
import hashlib
from unittest.mock import patch

SOURCE = pathlib.Path(__file__).resolve().parents[1] / "ci" / "browser_login_stage_coverage.py"
SPEC = importlib.util.spec_from_file_location("coverage_guard", SOURCE)
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


class CoverageTests(unittest.TestCase):
    def test_original_seventeen_stages_remain_exact_and_new_controls_are_additive(self):
        self.assertEqual(hashlib.sha256('\n'.join(guard.REQUIRED[:17]).encode()).hexdigest(),
                         'a4e5689b1e96e1c0712819c5b05c21c2d104085b05f22b6eb8c87b2aadf82e89')
        self.assertEqual(len(guard.REQUIRED), 22)
    def test_successful_expected_504_controls_use_broad_mode_without_publishing_raw(self):
        from test_session_deadline_diagnostic import BrowserHandlerDiagnosticTests as fixture
        path = SOURCE.with_name('auth_stage_diagnostic.py')
        spec = importlib.util.spec_from_file_location('broad_for_browser_controls', path)
        broad = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(broad)
        frames = ''.join(
            f'auth-stage slot={slot} (test-only; scheduling perturbed)\n'
            'op=Claim stage=Return event=Success elapsed_us=Some(1) utc_remaining_ms=Some(2) '
            'mono=Some(Before(3)) kind=None outcome=None\nlost=false\n' for slot in (1, 2))
        raw = self.raw() + frames.encode() + ('\n'.join((fixture.HEADER, fixture.CALL, fixture.PHASE)) + '\n').encode()
        sink = io.TextIOWrapper(io.BytesIO(), encoding='utf-8')
        written = []
        with patch.object(broad, 'read_bounded', return_value=(raw, False)), \
             patch.object(pathlib.Path, 'write_bytes', lambda _, data: written.append(data)), \
             contextlib.redirect_stdout(sink):
            self.assertTrue(broad.capture('broad', pathlib.Path('/owned/unused'), 0))
            sink.flush()
            output = sink.buffer.getvalue()
        self.assertEqual(guard.coverage(raw, 0)[1], 0)
        self.assertIn(b'auth-stage slot=1', output)
        self.assertIn(b'auth-stage slot=2', output)
        self.assertNotIn(b'browser_handler_', output + b''.join(written))

    def raw(self):
        return ("".join("test " + name + " ... ok\n" for name in guard.REQUIRED)
                + "test result: ok. 434 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s\n").encode()

    def test_complete_native_run_passes(self):
        record, status = guard.coverage(self.raw(), 0)
        self.assertEqual(status, 0)
        self.assertEqual(record["status"], "passed")
        self.assertEqual(record["required_count"], 22)

    def test_missing_or_ignored_stage_fails(self):
        original = ("test " + guard.REQUIRED[0] + " ... ok\n").encode()
        for replacement in (b"", original.replace(b"... ok", b"... ignored"),
                            original.replace(b"... ok", b"... FAILED")):
            with self.subTest(replacement=replacement):
                self.assertEqual(guard.coverage(self.raw().replace(original, replacement), 0)[1], 1)

    def test_existing_unrelated_ignored_measurement_does_not_hide_required_stages(self):
        raw = self.raw().replace(b"0 ignored", b"1 ignored")
        record, status = guard.coverage(raw, 0)
        self.assertEqual(status, 0)
        self.assertEqual(record["status"], "passed")
        for name in guard.REQUIRED:
            required = ("test " + name + " ... ok\n").encode()
            with self.subTest(required=name):
                ignored = raw.replace(required, required.replace(b"... ok", b"... ignored"))
                self.assertEqual(guard.coverage(ignored, 0)[1], 1)

    def test_duplicate_stage_fails(self):
        duplicate = ("test " + guard.REQUIRED[0] + " ... ok\n").encode()
        self.assertEqual(guard.coverage(self.raw() + duplicate, 0)[1], 1)

    def test_summary_exit_and_input_fail_closed(self):
        variants = [
            (self.raw(), 101), (None, 0), (b"\xff", 0),
            (b"x" * (guard.RAW_LIMIT + 1), 0),
            (self.raw().replace(b"0 failed", b"1 failed"), 0),
            (self.raw() + self.raw().splitlines(keepends=True)[-1], 0),
        ]
        for raw, exit_code in variants:
            with self.subTest(exit_code=exit_code, size=None if raw is None else len(raw)):
                self.assertEqual(guard.coverage(raw, exit_code)[1], 1)

    def test_socket_success_cannot_substitute_for_native_stages(self):
        raw = (b"running 10 tests\n"
               b"test committed_claim_stays_consumed_on_verifier_error_panic_and_deadline ... ok\n"
               b"test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s\n")
        self.assertEqual(guard.coverage(raw, 0)[1], 1)

    def test_old_composition_cannot_substitute_for_browser_dispatch_stages(self):
        raw = self.raw()
        for name in guard.REQUIRED:
            if name.startswith("http_server::browser_login::stage_tests::"):
                raw = raw.replace(("test " + name + " ... ok\n").encode(), b"")
        self.assertEqual(guard.coverage(raw, 0)[1], 1)

    def test_unknown_material_is_never_copied(self):
        secret = "Cookie=opaque-secret SQL /private/native/detail"
        record, status = guard.coverage(self.raw() + secret.encode(), 0)
        self.assertEqual(status, 0)
        self.assertNotIn(secret, str(record))

    def test_crlf_is_accepted(self):
        self.assertEqual(guard.coverage(self.raw().replace(b"\n", b"\r\n"), 0)[1], 0)


if __name__ == "__main__":
    unittest.main()
