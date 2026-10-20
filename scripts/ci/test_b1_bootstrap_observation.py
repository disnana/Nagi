"""Prepared pure unit cases; no native executions or workflow mutations."""
import contextlib
import io
import os
import pathlib
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from b1_bootstrap_observation import B1_IDENTIFIED, B1_NAME, B1_NEXT, STEP, RESOURCE_STEP, add_failure_diagnostic, inverse_failure_diagnostic


class TemporaryB1WorkflowTests(unittest.TestCase):
    def setUp(self):
        body = '        shell: bash\n        run: |\n          cargo test --locked unchanged\n'
        self.before = 'jobs:\n  linux:\n    steps:\n' + B1_NAME + body + B1_NEXT + '        run: keep1\n  nagi-package:\n    steps:\n' + B1_NAME + body + B1_NEXT + '        run: keep2\n'
        self.after = add_failure_diagnostic(self.before)

    def test_exact_roundtrip(self):
        self.assertEqual(inverse_failure_diagnostic(self.after), self.before)
        self.assertEqual(self.after.count(STEP), 2)
        self.assertEqual(self.after.count(B1_IDENTIFIED), 2)

    def test_missing_duplicate_moved_or_changed_steps_are_rejected(self):
        mutants = (
            self.after.replace(STEP, '', 1),
            self.after.replace(STEP, STEP + STEP, 1),
            self.after.replace(STEP + B1_NEXT, B1_NEXT + STEP, 1),
            self.after.replace("if: failure() && steps.b1_csrf_runtime.outcome == 'failure'", 'if: always()', 1),
            self.after.replace('-- --show-output', '-- --test-threads=1', 1),
            self.after.replace('256*1024+1', '512*1024+1', 1),
            self.after.replace('        id: b1_csrf_runtime\n', '', 1),
            self.after.replace('        id: b1_csrf_runtime\n', '        id: b1_csrf_runtime\n        id: b1_csrf_runtime\n', 1),
        )
        for mutant in mutants:
            with self.subTest(mutant=mutants.index(mutant)):
                with self.assertRaises(ValueError):
                    inverse_failure_diagnostic(mutant)

    def test_original_body_mutation_survives_for_inherited_digest_rejection(self):
        mutant = self.after.replace('cargo test --locked unchanged', 'cargo test --locked WEAKENED', 1)
        self.assertNotEqual(inverse_failure_diagnostic(mutant), self.before)

    def test_add_rejects_partial_duplicate_and_reapplication(self):
        for value in (self.before.replace(B1_NAME, '', 1), self.before + B1_NAME, self.after):
            with self.assertRaises(ValueError):
                add_failure_diagnostic(value)


    def test_linux_mutation_step_boundary_is_preserved(self):
        linux_next = '      - name: Compile and run CSRF deadline semantic mutants\n'
        before = self.before.replace(B1_NEXT, linux_next, 1)
        after = add_failure_diagnostic(before)
        self.assertEqual(inverse_failure_diagnostic(after), before)
        self.assertEqual(after.count(RESOURCE_STEP), 2)
        for mutant in (
            after.replace(RESOURCE_STEP, '', 1),
            after.replace(RESOURCE_STEP, RESOURCE_STEP + RESOURCE_STEP, 1),
            after.replace(RESOURCE_STEP + B1_IDENTIFIED, B1_IDENTIFIED + RESOURCE_STEP, 1),
        ):
            with self.assertRaises(ValueError):
                inverse_failure_diagnostic(mutant)

    def test_unsupported_resource_fields_are_explicit_and_do_not_skip_acceptance(self):
        body = RESOURCE_STEP.split("<<'PYB1RESOURCE'\n", 1)[1].split('          PYB1RESOURCE\n', 1)[0]
        code = '\n'.join(line[10:] for line in body.splitlines()) + '\n'
        output = io.StringIO()
        with patch('os.cpu_count', return_value=None), patch('os.getloadavg', side_effect=OSError, create=True), \
                patch('builtins.open', side_effect=OSError), patch('shutil.disk_usage', side_effect=OSError), \
                patch.dict(os.environ, {'RUNNER_TEMP': 'unavailable'}), contextlib.redirect_stdout(output):
            exec(compile(code, '<b1-resource-unavailable>', 'exec'), {})
        self.assertIn('cpu=0 load_supported=0', output.getvalue())
        self.assertIn('memory_supported=0', output.getvalue())
        self.assertIn('disk_supported=0', output.getvalue())


class TemporaryB1LogTests(unittest.TestCase):
    # Exact emitted key orders, copied from pinned Rust fmt::Display sources.
    # Millisecond fields are u16 from bounded_ms(saturating_duration_since).
    RECORDS = (
        'session_deadline_diag status=504 phase=2 event=1 db=10 exit=1 polls=2 begins=1 commands=4 commits=0 budget_ms=1000 entry_remaining_ms=999 phase_remaining_ms=980 exit_remaining_ms=0',
        'session_private_deadline_diag status=504 phase=6 event=1 db=10 polls=2 begins=1 commands=4 commits=0 budget_ms=1000 entry_remaining_ms=1000 phase_remaining_ms=1000 exit_remaining_ms=0 terminal=3',
        'csrf_token_deadline_diag status=504 phase=2 event=1 db=10 exit=1 polls=2 begins=2 commands=10 commits=1 budget_ms=1000 entry_remaining_ms=999 phase_remaining_ms=980 exit_remaining_ms=0 terminal=5',
        'csrf_guarded_deadline_diag status=504 phase=10 event=1 db=1 exit=0 polls=1 begins=0 commands=0 commits=0 budget_ms=1000 entry_remaining_ms=1000 phase_remaining_ms=1000 exit_remaining_ms=0 selected_deadline_passed=1 terminal=1',
        'session_native_finish_diag status=504 origin=3 terminal=3 diagnostic_valid=1 native_state=1 commit_result=0 cleanup_result=0 cleanup_reason=0 commit_enter_us=100 commit_return_us=60000001 cleanup_enter_us=60000001 cleanup_return_us=60000001 settled_us=60000001 begins=1 commits=0 selected_deadline_passed=1',
        'session_native_call_diag status=504 origin=3 terminal=3 diagnostic_valid=1 native_call_state=1 generation=1 call_enter_us=100 call_return_us=60000001 native_state=1 commit_result=0 settled_us=60000001 begins=1 commits=0 selected_deadline_passed=1',
    )
    ROOT = 'http_server::csrf_http::deadline_tests::pool_readiness_tests::'
    CASES = ('cold_worker2_startup_spends_original_guarded_http_budget', 'ready_worker2_setup_preserves_same_guarded_http_budget')
    BOOTSTRAP = ('b1_startup_bootstrap stage=issue status=200 created=1 idle=1 elapsed_us=100', 'b1_startup_bootstrap stage=csrf status=200 created=1 idle=1 elapsed_us=200') * 2

    def run_logger(self, raw, cargo_rc='0', capture_rc='0'):
        body = STEP.split("          B1_DIAGNOSTIC_CARGO_RC=", 1)[1].split("<<'PYB1LOG'\n", 1)[1].split('          PYB1LOG\n', 1)[0]
        code = '\n'.join(line[10:] for line in body.splitlines()) + '\n'
        output = io.StringIO()
        with tempfile.TemporaryDirectory(prefix='b1-log-vector-') as owned:
            raw_path = pathlib.Path(owned) / 'b1-bootstrap-supplement.raw'
            raw_path.write_bytes(raw)
            environment = {'RUNNER_TEMP': owned, 'B1_DIAGNOSTIC_CARGO_RC': cargo_rc, 'B1_DIAGNOSTIC_CAPTURE_RC': capture_rc}
            with patch.dict(os.environ, environment), contextlib.redirect_stdout(output):
                try:
                    exec(compile(code, '<temporary-b1-log-vector>', 'exec'), {})
                except SystemExit as result:
                    status = result.code
                else:
                    self.fail('inline logger must return an explicit status')
            self.assertFalse(raw_path.exists())
        return status, output.getvalue()

    def raw(self, records=(), failed=False):
        states = ('FAILED', 'ok') if failed else ('ok', 'ok')
        rows = ['running 2 tests']
        rows.extend('test ' + self.ROOT + name + ' ... ' + state for name, state in zip(self.CASES, states))
        rows.extend(self.BOOTSTRAP)
        rows.extend(records)
        rows.append('test result: {}. {} passed; {} failed; 0 ignored; 0 measured; 861 filtered out; finished in 1.46s'.format('FAILED' if failed else 'ok', 1 if failed else 2, 1 if failed else 0))
        return ('\n'.join(rows) + '\n').encode()

    def test_source_derived_numeric_records_and_zero_remaining_are_retained(self):
        status, output = self.run_logger(self.raw(self.RECORDS))
        self.assertEqual(status, 0)
        for record in self.RECORDS:
            self.assertIn(record + '\n', output)
        self.assertIn('bootstrap_ok=1 diagnostic_available=1', output)

    def test_only_known_case_headers_bind_records_and_unknown_headers_reset_binding(self):
        record = self.RECORDS[0]
        rows = (
            '---- ' + self.ROOT + self.CASES[0] + ' stdout ----', record,
            '---- owned-secret stdout ----', record,
            '---- ' + self.ROOT + self.CASES[1] + ' stdout ----', record,
        )
        status, output = self.run_logger(self.raw(rows))
        self.assertEqual(status, 0)
        for arm in (0, 1, 2):
            self.assertIn('b1_supplement_record arm={} {}\n'.format(arm, record), output)
        self.assertNotIn('owned-secret', output)

    def test_failed_supplement_preserves_its_actual_exit_code(self):
        status, output = self.run_logger(self.raw(self.RECORDS, failed=True), cargo_rc='101')
        self.assertEqual(status, 101)
        self.assertIn('cargo_rc=101 capture_rc=0', output)
        self.assertIn('diagnostic_available=1', output)

    def test_gate_failure_record_is_retained_without_raw_panic(self):
        gate = 'b1_startup_bootstrap_gate stage=issue created=2 idle=0 elapsed_us=123'
        raw = self.raw((gate,), failed=True) + b'bootstrap worker2 gate: private panic owned-secret\n'
        status, output = self.run_logger(raw, cargo_rc='101')
        self.assertEqual(status, 101)
        self.assertIn(gate + '\n', output)
        self.assertNotIn('owned-secret', output)
        self.assertIn('diagnostic_available=1', output)

    def test_unknown_compile_panic_url_cookie_token_sql_and_path_are_never_echoed(self):
        secret = b'Cookie: owned-secret\ncsrf_token=owned-secret\nSELECT owned-secret\n/tmp/owned-secret\nhttps://owned-secret.invalid\nthread panicked: owned-secret\n'
        status, output = self.run_logger(self.raw() + secret)
        self.assertEqual(status, 0)
        self.assertNotIn('owned-secret', output)
        self.assertNotIn('Cookie', output)
        self.assertNotIn('SELECT', output)

    def test_changed_signed_duplicate_extra_long_or_excess_records_are_unavailable(self):
        base = self.RECORDS[1]
        mutants = (
            (base.replace('exit_remaining_ms=0', 'exit_remaining_ms=-1'),),
            (base.replace('db=10', 'db=10 db=10'),),
            (base + ' token=owned-secret',),
            (base + ' ' + '7' * 1024,),
            (base,) * 65,
        )
        for records in mutants:
            with self.subTest(index=mutants.index(records)):
                status, output = self.run_logger(self.raw(records))
                self.assertEqual(status, 1)
                self.assertIn('malformed=1', output)
                self.assertIn('diagnostic_available=0', output)
                self.assertNotIn('owned-secret', output)
                self.assertNotIn(base + '\n', output)

    def test_overflow_capture_failure_or_missing_bootstrap_is_unavailable(self):
        cases = (
            (self.raw() + b'x' * (256 * 1024), '0'),
            (self.raw(), '1'),
            (self.raw().replace(('\n'.join(self.BOOTSTRAP) + '\n').encode(), b''), '0'),
            (b'error: private compile output owned-secret\n', '0'),
        )
        for raw, capture_rc in cases:
            with self.subTest(capture_rc=capture_rc, size=len(raw)):
                status, output = self.run_logger(raw, capture_rc=capture_rc)
                self.assertEqual(status, 1)
                self.assertIn('diagnostic_available=0', output)
                self.assertNotIn('owned-secret', output)


if __name__ == '__main__':
    unittest.main()
