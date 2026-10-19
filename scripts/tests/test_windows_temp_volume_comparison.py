"""Pure mocks: no sockets, Windows calls, native test binaries, or network."""
import copy
import contextlib
import ctypes
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

PACKET = Path(__file__).resolve().parents[1]
ROOT = Path(os.environ.get('NAGI_VOLUME_TEST_REPO', Path(__file__).resolve().parents[2]))
sys.path.insert(0, str(ROOT / 'scripts/ci'))
import session_deadline_diagnostic as diag
spec = importlib.util.spec_from_file_location('volume_helper', Path(os.environ.get('NAGI_VOLUME_TEST_HELPER', ROOT / 'scripts/ci/windows_temp_volume_comparison.py')))
h = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = h
spec.loader.exec_module(h)


def raw_log(cases, failed=()):
    failed = set(failed)
    lines = [f'running {len(cases)} tests']
    lines += [f'test {name} ... ' + ('FAILED' if name in failed else 'ok') for name in sorted(cases)]
    if failed:
        lines += ['failures:', '', *sorted(failed), '']
    lines += [f'test result: {"FAILED" if failed else "ok"}. {len(cases)-len(failed)} passed; {len(failed)} failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.10s', '']
    return '\n'.join(lines).encode()


class VolumeTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        # Bind fixture Cargo IDs and fake volume roots to the identity helpers resolve.
        self.repo = Path(self.tmp.name).resolve(strict=True) / 'private-sentinel-repo'
        self.target = self.repo / 'target'
        self.exe = self.target / 'debug/deps/session_csrf_public_api-unit.exe'
        self.exe.parent.mkdir(parents=True)
        self.exe.write_bytes(b'MZ-unit-native-binary')
        for name in ['runtime/tests/session_csrf_public_api.rs', 'runtime/tests/browser_login_public_api.rs', 'runtime/Cargo.toml', 'Cargo.toml']:
            destination = self.repo / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((ROOT / name).read_bytes())
        self.runner = self.repo / 'private-sentinel-runner'
        self.original = self.repo / 'private-sentinel-original'
        self.runner.mkdir(); self.original.mkdir()
        self.env = {'RUNNER_TEMP': str(self.runner), 'TMP': str(self.original), 'TEMP': str(self.original), 'PRIVATE_SENTINEL': 'must-not-appear'}
        self.messages = self.repo / 'cargo-messages.jsonl'
        self.artifact = {
            'reason': 'compiler-artifact', 'package_id': 'path+' + (self.repo / 'runtime').as_uri() + '#nagi-runtime@0.2.0',
            'manifest_path': str(self.repo / 'runtime/Cargo.toml'),
            'target': {'name': 'session_csrf_public_api', 'kind': ['test'], 'crate_types': ['bin'], 'src_path': str(self.repo / 'runtime/tests/session_csrf_public_api.rs')},
            'profile': {'test': True}, 'executable': str(self.exe), 'filenames': [str(self.exe)], 'fresh': True,
        }
        self.write_messages([self.artifact, {'reason': 'build-finished', 'success': True}])
        self.browser = self.repo / 'browser.raw'
        self.browser.write_bytes(raw_log(diag.BROWSER_CASES))
        self.native = SimpleNamespace(api_kind='get_temp_path2', temp_root=lambda: str(self.original), volume_id=lambda path: 'volume-A' if Path(path) == self.original else 'volume-B')
        self.calls = []
        self.a_failed = True
        self.b_failed = False
        self.bind = b'1\n'

    def write_messages(self, values):
        self.messages.write_text(''.join(json.dumps(value) + '\n' for value in values))

    def capture(self, argv, *, cwd, env, timeout):
        self.calls.append((list(argv), Path(cwd), dict(env), timeout))
        if '--list' in argv:
            raw = ('\n'.join(name + ': test' for name in sorted(diag.SESSION_CASES)) + '\n\n11 tests, 0 benchmarks\n').encode()
            return SimpleNamespace(raw=raw, returncode=0, elapsed_ms=1, closed=True)
        if '-c' in argv:
            return SimpleNamespace(raw=self.bind, returncode=0, elapsed_ms=1, closed=True)
        arm = 'A' if env['TMP'] == str(self.original) else 'B'
        failed = self.a_failed if arm == 'A' else self.b_failed
        return SimpleNamespace(raw=raw_log(diag.SESSION_CASES, [diag.CSRF_REUSABLE_CASE] if failed else []), returncode=101 if failed else 0, elapsed_ms=1100, closed=True)

    def run_pair(self, **overrides):
        args = dict(repo=self.repo, target_root=self.target, messages=self.messages, cargo_exit=0,
                    browser_raw=self.browser, browser_exit=0, env=self.env,
                    native=self.native, capture=self.capture, collector=diag)
        args.update(overrides)
        return h.compare(**args)

    def assert_private(self, result):
        encoded = json.dumps(result)
        for forbidden in ['PRIVATE_SENTINEL', 'private-sentinel-', 'volume-A', 'volume-B', str(self.repo)]:
            self.assertNotIn(forbidden, encoded)

    def test_fixture_paths_and_volume_roots_share_the_canonical_temp_identity(self):
        self.assertEqual(self.repo, Path(self.tmp.name).resolve(strict=True) / 'private-sentinel-repo')
        expected_package = 'path+' + (self.repo / 'runtime').resolve(strict=True).as_uri() + '#nagi-runtime@0.2.0'
        self.assertEqual(self.artifact['package_id'], expected_package)
        self.assertEqual(Path(self.native.temp_root()).resolve(strict=True), self.original)

    def test_unresolved_alias_package_identity_refuses_when_metadata_paths_resolve_to_fixture(self):
        # Model a short-path alias without relying on symlink creation privileges.
        alias_repo = self.repo.parent / (self.repo.name + '-short')
        alias_runtime = alias_repo / 'runtime'
        artifact = copy.deepcopy(self.artifact)
        artifact['package_id'] = 'path+' + alias_runtime.as_uri() + '#nagi-runtime@0.2.0'
        artifact['manifest_path'] = str(alias_runtime / 'Cargo.toml')
        artifact['target']['src_path'] = str(alias_runtime / 'tests/session_csrf_public_api.rs')
        self.write_messages([artifact, {'reason': 'build-finished', 'success': True}])

        original_resolve = Path.resolve
        def canonicalize_alias(path, strict=False):
            try:
                relative = path.relative_to(alias_repo)
            except ValueError:
                return original_resolve(path, strict=strict)
            return original_resolve(self.repo / relative, strict=strict)

        with mock.patch.object(Path, 'resolve', autospec=True, side_effect=canonicalize_alias):
            self.assertEqual(Path(artifact['manifest_path']).resolve(strict=True), self.repo / 'runtime/Cargo.toml')
            self.assertEqual(Path(artifact['target']['src_path']).resolve(strict=True), self.repo / 'runtime/tests/session_csrf_public_api.rs')
            with self.assertRaises(h.Refused) as caught:
                h.select_binary(alias_repo, self.target, self.messages, 0)
        self.assertEqual(caught.exception.reason, 'artifact_refused')

    def test_fixed_reverse_pair_preserves_failed_A_and_runs_both_exactly_once(self):
        result, code = self.run_pair()
        self.assertEqual([arm['arm'] for arm in result['arms']], ['B', 'A'])
        self.assertEqual(result['status'], 'tests_failed')
        self.assertEqual(code, 1, 'a failed/504 arm can never become successful experiment acceptance')
        runs = [call for call in self.calls if call[0] == [str(self.exe)]]
        self.assertEqual(len(runs), 2)
        self.assertEqual(result['order'], 'B_then_A')
        self.assertEqual(runs[1][2], self.env)
        self.assertEqual({key for key in self.env if runs[0][2].get(key) != self.env[key]}, {'TMP', 'TEMP'})
        self.assertEqual(runs[0][2]['TMP'], runs[0][2]['TEMP'])
        self.assertTrue(Path(runs[0][2]['TMP']).is_relative_to(self.runner))
        self.assertTrue(all(call[3] == 30 for call in runs))
        self.assertTrue(result['order_confound'])
        self.assertEqual(result['causality'], 'unproven')
        self.assertNotIn('collector_failure', result)
        self.assert_private(result)


    def test_same_or_unknown_volume_refuses_without_running_binary_or_creating_workdir(self):
        for volume in [lambda _: 'same', lambda _: None, lambda _: '']:
            self.native.volume_id = volume
            with mock.patch.object(h.tempfile, 'mkdtemp', side_effect=AssertionError('unexpected workdir')):
                result, code = self.run_pair()
            self.assertEqual(code, 1)
            self.assertEqual(result['arms'], [])
            self.assertEqual(self.calls, [])
            self.assert_private(result)
        self.native.volume_id = lambda _: (_ for _ in ()).throw(OSError('PRIVATE_SENTINEL'))
        result, code = self.run_pair()
        self.assertEqual(code, 1); self.assert_private(result)

    def test_precheck_is_read_only_and_main_rechecks_instead_of_caching(self):
        with mock.patch.object(h.tempfile, 'mkdtemp', side_effect=AssertionError('precheck must not create directory')):
            result, code = h.check_platform(env=self.env, native=self.native, capture=self.capture)
        self.assertEqual(code, 0)
        self.assertEqual(result['topology'], 'distinct_volumes')
        self.assertEqual([p for p in self.runner.iterdir()], [])
        self.assertTrue(all('-c' in call[0] for call in self.calls))
        self.assert_private(result)
        self.calls.clear()
        self.native.volume_id = lambda _: 'same'
        result, code = self.run_pair()
        self.assertEqual(result['reason'], 'same_volume')
        self.assertEqual(code, 1)
        self.assertEqual(self.calls, [])

    def test_precheck_fallback_same_unknown_and_binding_failure_are_nonzero_and_private(self):
        for kind, volume, binding, expected in [
            ('get_temp_path', lambda _: 'same', b'1\n', 'temp_api_fallback'),
            ('get_temp_path2', lambda _: 'same', b'1\n', 'same_volume'),
            ('get_temp_path2', lambda _: None, b'1\n', 'topology_unknown'),
            ('get_temp_path2', lambda p: 'a' if Path(p)==self.original else 'b', b'0\n', 'temp_binding'),
        ]:
            self.native.api_kind, self.native.volume_id, self.bind = kind, volume, binding
            result, code = h.check_platform(env=self.env, native=self.native, capture=self.capture)
            self.assertEqual(code, 1)
            self.assertEqual(result['reason'], expected)
            self.assert_private(result)
        self.native.temp_root = lambda: (_ for _ in ()).throw(OSError('PRIVATE_SENTINEL'))
        result, code = h.check_platform(env=self.env, native=self.native, capture=self.capture)
        self.assertEqual(code, 1); self.assert_private(result)

    def test_binding_requires_zero_exit_and_exact_boolean_without_leaking_raw(self):
        original = self.capture
        for raw, code in [(b'0\n', 0), (b'1\nPRIVATE_SENTINEL', 0), (b'1\n', 101)]:
            def capture(argv, **kwargs):
                value = original(argv, **kwargs)
                if '-c' in argv: value.raw, value.returncode = raw, code
                return value
            result, code = self.run_pair(capture=capture)
            self.assertEqual(code, 1)
            self.assertEqual(result['arms'], [])
            self.assert_private(result)
        self.assertTrue(all('--list' not in call[0] for call in self.calls))

    def test_passing_A_does_not_skip_B_and_all_pass_is_only_test_success(self):
        self.a_failed = False
        result, code = self.run_pair()
        self.assertEqual(code, 0)
        self.assertEqual(result['status'], 'tests_passed')
        self.assertEqual([arm['arm'] for arm in result['arms']], ['B', 'A'])
        self.assertEqual(result['causality'], 'unproven')
        self.assertNotIn('collector_failure', result)
        self.b_failed = True
        result, code = self.run_pair()
        self.assertEqual(code, 1)
        self.assertEqual(result['status'], 'tests_failed')
        self.assertEqual([arm['arm'] for arm in result['arms']], ['B', 'A'])
        self.assert_private(result)

    def test_source_and_binary_mutation_stop_before_A(self):
        original = self.capture
        def capture(argv, **kwargs):
            value = original(argv, **kwargs)
            if argv == [str(self.exe)]: self.exe.write_bytes(b'MZ-changed')
            return value
        result, code = self.run_pair(capture=capture)
        self.assertEqual(code, 1)
        self.assertEqual(result['reason'], 'binary_changed')
        self.assertEqual(len([call for call in self.calls if call[0] == [str(self.exe)]]), 1)
        self.assert_private(result)
        self.calls.clear()
        source = self.repo / 'runtime/tests/session_csrf_public_api.rs'
        source.write_bytes(b'PRIVATE_SENTINEL')
        result, code = self.run_pair()
        self.assertEqual(code, 1)
        self.assertEqual(result['reason'], 'source_refused')
        self.assertEqual(self.calls, [])
        self.assert_private(result)

    def test_browser_is_strict_common_input_and_never_executed_by_helper(self):
        for raw in [raw_log(diag.BROWSER_CASES, [next(iter(diag.BROWSER_CASES))]),
                    raw_log(set(diag.BROWSER_CASES) - {next(iter(diag.BROWSER_CASES))}),
                    b'PRIVATE_SENTINEL', b'\xff', b'x' * (h.RAW_LIMIT+1)]:
            self.browser.write_bytes(raw)
            result, code = self.run_pair()
            self.assertEqual(code, 1)
            self.assertEqual(self.calls, [])
            self.assert_private(result)

    def test_wrong_inventory_oversize_invalid_utf8_and_watchdog_cannot_be_pass(self):
        original = self.capture
        variants = [
            ('raw', raw_log(set(diag.SESSION_CASES) - {diag.CSRF_REUSABLE_CASE})),
            ('raw', b'x' * (h.RAW_LIMIT+1)), ('raw', b'\xff'), ('returncode', 1),
            ('returncode', True), ('fault', 'watchdog'), ('closed', False),
            ('elapsed_ms', 33001), ('elapsed_ms', True),
        ]
        for field, changed in variants:
            with self.subTest(field=field):
                self.calls.clear()
                def capture(argv, **kwargs):
                    value = original(argv, **kwargs)
                    if argv == [str(self.exe)]: setattr(value, field, changed)
                    return value
                result, code = self.run_pair(capture=capture)
                self.assertEqual(code, 1)
                self.assertNotEqual(result['status'], 'tests_passed')
                self.assertEqual(len([call for call in self.calls if call[0] == [str(self.exe)]]), 1)
                if field == 'closed': self.assertEqual(result['cleanup'], 'retained')
                self.assert_private(result)

    def test_collector_output_must_remain_existing_closed_shapes(self):
        for name, changed in [('csrf_reusable_observation', {'PRIVATE_SENTINEL': 'private-sentinel-path'}),
                              ('session_native_call_lines', ['session_native_call=PRIVATE_SENTINEL']),
                              ('session_native_call_lines', None),
                              ('session_native_call_lines', ['session_native_call=unavailable'] * (diag.NATIVE_CALL_FACT_LIMIT + 1)),
                              ('session_native_call_lines', [None]),
                              ('session_native_call_lines', ['PRIVATE_SENTINEL']),
                              ('session_native_call_lines', ['session_native_call={"PRIVATE_SENTINEL":"private-sentinel-path"}'])]:
            self.calls.clear()
            with mock.patch.object(diag, name, return_value=changed):
                result, code = self.run_pair()
            self.assertEqual(code, 1)
            self.assertEqual(result['reason'], 'collector_refused')
            self.assertEqual(result['collector_failure']['stage'], 'csrf' if name == 'csrf_reusable_observation' else 'native')
            self.assertEqual(result['collector_failure']['exit'], 0)
            self.assert_private(result)

    def test_incomplete_A_records_refusal_stage_without_claiming_an_arm_verdict(self):
        original = self.capture
        def capture(argv, **kwargs):
            value = original(argv, **kwargs)
            if argv == [str(self.exe)] and kwargs['env']['TMP'] == str(self.original):
                value.raw = b'running 11 tests\nPRIVATE_SENTINEL\n'
            return value
        result, code = self.run_pair(capture=capture)
        self.assertEqual((code, result['status'], result['reason']), (1, 'infra_failed', 'collector_refused'))
        self.assertEqual([arm['arm'] for arm in result['arms']], ['B'])
        failure = result['collector_failure']
        self.assertEqual({key: failure[key] for key in ('arm', 'stage', 'exit', 'record_shape_valid', 'record_status')},
                         {'arm': 'A', 'stage': 'record', 'exit': 101, 'record_shape_valid': True,
                          'record_status': 'diagnostic_invalid'})
        self.assertEqual(failure['session'], {'input_status': 'valid', 'diagnostic_status': 'valid',
            'inventory_status': 'incomplete', 'running_count': 11, 'passed_count': None,
            'failed_count': None, 'ignored_count': None, 'failed_cases': []})
        self.assertEqual(result['cleanup'], 'removed')
        self.assert_private(result)

    def test_collector_refusals_identify_each_fixed_stage_and_both_arms(self):
        original_diagnose = diag.diagnose
        cases = [('suite', 'diagnose'), ('csrf', 'csrf_reusable_observation'),
                 ('native', 'session_native_call_lines')]
        for arm in ('B', 'A'):
            for stage, method in cases:
                self.calls.clear()
                self.b_failed = True
                native = diag.session_native_call_lines
                csrf = diag.csrf_reusable_observation
                count = 0
                def changed(*args, **kwargs):
                    nonlocal count
                    if method == 'diagnose' and args[0] is None:
                        return original_diagnose(*args, **kwargs)
                    count += 1
                    selected = count == (1 if arm == 'B' else 2)
                    if method == 'diagnose':
                        record, code = original_diagnose(*args, **kwargs)
                        if selected: record['suites']['session']['ignored_count'] = 1
                        return record, code
                    if selected:
                        return {'PRIVATE_SENTINEL': 'private-sentinel-path'} if stage == 'csrf' else ['session_native_call=PRIVATE_SENTINEL']
                    return (csrf if stage == 'csrf' else native)(*args, **kwargs)
                with mock.patch.object(diag, method, side_effect=changed):
                    result, code = self.run_pair()
                self.assertEqual((code, result['status'], result['reason']), (1, 'infra_failed', 'collector_refused'))
                failure = result['collector_failure']
                self.assertEqual((failure['arm'], failure['stage'], failure['exit']), (arm, stage, 101))
                self.assertEqual(failure['session']['failed_cases'], [diag.CSRF_REUSABLE_CASE])
                self.assertEqual([item['arm'] for item in result['arms']], [] if arm == 'B' else ['B'])
                self.assert_private(result)

    def test_malformed_refusal_record_rebuilds_only_bounded_known_inventory_fields(self):
        original = diag.diagnose
        for counts in [(True, -1, 12, 10**100), (0, 11, 1, None)]:
            def diagnose(*args, **kwargs):
                record, code = original(*args, **kwargs)
                if args[0] is not None:
                    record['PRIVATE_SENTINEL'] = {'path': 'private-sentinel-path'}
                    record['status'] = ['PRIVATE_SENTINEL']
                    suite = record['suites']['session']
                    suite.update(input_status={'PRIVATE_SENTINEL': True}, diagnostic_status='PRIVATE_SENTINEL',
                        inventory_status='PRIVATE_SENTINEL', failed_cases=[diag.CSRF_REUSABLE_CASE,
                        'PRIVATE_SENTINEL', {'PRIVATE_SENTINEL': True}, diag.CSRF_REUSABLE_CASE],
                        extra='PRIVATE_SENTINEL')
                    suite.update(zip(('running_count', 'passed_count', 'failed_count', 'ignored_count'), counts))
                return record, code
            with mock.patch.object(diag, 'diagnose', side_effect=diagnose):
                result, code = self.run_pair()
            self.assertEqual((code, result['status'], result['reason']), (1, 'infra_failed', 'collector_refused'))
            failure = result['collector_failure']
            self.assertEqual(set(failure), {'arm', 'stage', 'exit', 'record_shape_valid', 'record_status', 'session'})
            self.assertEqual((failure['stage'], failure['record_shape_valid'], failure['record_status']), ('record', False, None))
            expected = {key: None for key in ('input_status', 'diagnostic_status', 'inventory_status')}
            expected.update(zip(('running_count', 'passed_count', 'failed_count', 'ignored_count'),
                [number if type(number) is int and 0 <= number <= 11 else None for number in counts]))
            expected['failed_cases'] = [diag.CSRF_REUSABLE_CASE]
            self.assertEqual(failure['session'], expected)
            self.assertLess(len(json.dumps(failure)), 2048)
            self.assert_private(result)

    def test_noncollector_refusal_and_bad_record_containers_never_project_raw_values(self):
        original = diag.diagnose
        unknown_case, _ = original(raw_log(diag.SESSION_CASES), 'valid', 0,
                                  raw_log(diag.BROWSER_CASES), 'valid', 0)
        unknown_case['suites']['session']['failed_cases'] = ['PRIVATE_SENTINEL']
        for broken in [None, ['PRIVATE_SENTINEL'], {'suites': ['PRIVATE_SENTINEL']},
                       {'suites': {'session': 'PRIVATE_SENTINEL'}},
                       {'suites': {'session': {'failed_cases': ['PRIVATE_SENTINEL'] * 12}}}, unknown_case]:
            def diagnose(*args, **kwargs):
                return (broken, 1) if args[0] is not None else original(*args, **kwargs)
            with mock.patch.object(diag, 'diagnose', side_effect=diagnose):
                result, code = self.run_pair()
            self.assertEqual((code, result['reason']), (1, 'collector_refused'))
            self.assertEqual(result['collector_failure']['session']['failed_cases'], [])
            self.assert_private(result)
        self.bind = b'PRIVATE_SENTINEL'
        result, code = self.run_pair()
        self.assertEqual((code, result['reason']), (1, 'temp_binding'))
        self.assertNotIn('collector_failure', result)
        self.assert_private(result)

    def test_cargo_json_requires_single_exact_target_and_success(self):
        fields = [
            ('target', {**self.artifact['target'], 'name': 'other'}),
            ('target', {**self.artifact['target'], 'kind': ['lib']}),
            ('target', {**self.artifact['target'], 'crate_types': ['lib']}),
            ('target', {**self.artifact['target'], 'src_path': str(self.repo/'Cargo.toml')}),
            ('manifest_path', str(self.repo/'Cargo.toml')),
            ('package_id', 'path+file:///PRIVATE_SENTINEL#nagi-runtime@0.2.0'),
            ('profile', {'test': False}), ('executable', str(self.exe)+'missing'),
            ('filenames', []),
        ]
        for key, value in fields:
            with self.subTest(key=key):
                row = copy.deepcopy(self.artifact); row[key] = value
                self.write_messages([row, {'reason': 'build-finished', 'success': True}])
                with self.assertRaises(h.Refused): h.select_binary(self.repo, self.target, self.messages, 0)
        for records in [[self.artifact, self.artifact, {'reason': 'build-finished', 'success': True}],
                        [self.artifact, {'reason': 'build-finished', 'success': 1}],
                        [self.artifact, {'reason': 'build-finished', 'success': False}], [self.artifact],
                        [self.artifact, {'reason': 'build-finished', 'success': True}, {'reason': 'build-finished', 'success': True}]]:
            self.write_messages(records)
            with self.assertRaises(h.Refused): h.select_binary(self.repo, self.target, self.messages, 0)
        self.write_messages([self.artifact, {'reason': 'build-finished', 'success': True}])
        for code in [1, 101, True]:
            with self.assertRaises(h.Refused): h.select_binary(self.repo, self.target, self.messages, code)
        elsewhere = self.repo/'elsewhere'; elsewhere.mkdir()
        with self.assertRaises(h.Refused): h.select_binary(self.repo, elsewhere, self.messages, 0)

    def test_cargo_large_machine_stream_duplicate_key_and_extra_executable(self):
        dependency = {'reason': 'compiler-artifact', 'target': {'name': 'dep'}, 'executable': None, 'padding': 'x' * 1024}
        self.write_messages([dependency] * 300 + [self.artifact, {'reason': 'build-finished', 'success': True}])
        self.assertGreater(self.messages.stat().st_size, h.RAW_LIMIT)
        executable, digest = h.select_binary(self.repo, self.target, self.messages, 0)
        self.assertEqual(executable, self.exe)
        self.assertRegex(digest, r'^[0-9a-f]{64}$')
        self.messages.write_text('{"reason":"build-finished","success":false,"success":true}\n')
        with self.assertRaises(h.Refused): h.select_binary(self.repo, self.target, self.messages, 0)
        self.write_messages([self.artifact, {**self.artifact, 'target': {**self.artifact['target'], 'name': 'browser_login_public_api'}}, {'reason': 'build-finished', 'success': True}])
        with self.assertRaises(h.Refused): h.select_binary(self.repo, self.target, self.messages, 0)

    def test_cli_precheck_is_closed_and_mixed_arguments_usage_never_echoes_values(self):
        closed = {'schema': 'nagi-windows-temp-volume-platform-v1', 'status': 'ready', 'reason': 'complete', 'topology': 'distinct_volumes', 'temp_api': 'get_temp_path2'}
        output = io.StringIO()
        with mock.patch.object(h, 'check_platform', return_value=(closed, 0)), contextlib.redirect_stdout(output):
            self.assertEqual(h.main(['--check-platform']), 0)
        self.assertEqual(json.loads(output.getvalue()), closed)
        output = io.StringIO()
        with contextlib.redirect_stderr(output), self.assertRaises(SystemExit) as caught:
            h.main(['--check-platform', '--repo-root', 'PRIVATE_SENTINEL'])
        self.assertEqual(caught.exception.code, 2)
        self.assertNotIn('PRIVATE_SENTINEL', output.getvalue())


    def test_existing_output_directory_refuses_before_any_experiment(self):
        destination = self.repo / 'already-owned-output'
        destination.mkdir()
        output = io.StringIO()
        argv = ['--repo-root', str(self.repo), '--target-root', str(self.target),
            '--cargo-json', str(self.messages), '--cargo-exit-code', '0',
            '--browser-raw', str(self.browser), '--browser-exit-code', '0',
            '--output-dir', str(destination)]
        with mock.patch.object(h, 'compare', side_effect=AssertionError('must not execute')), contextlib.redirect_stdout(output):
            self.assertEqual(h.main(argv), 1)
        self.assertNotIn('private-sentinel', output.getvalue())

    def test_non_json_constants_are_refused(self):
        self.messages.write_text('{"reason":"compiler-message","extra":NaN}\n' +
            json.dumps(self.artifact) + '\n' + '{"reason":"build-finished","success":true}\n')
        with self.assertRaises(h.Refused):
            h.select_binary(self.repo, self.target, self.messages, 0)


class NativeTests(unittest.TestCase):
    def library(self, *, newer=True, root='C:\\private-sentinel\\', count=None, mount_ok=True, volume_ok=True, volume=None):
        calls = []
        class Function:
            def __init__(self, callback): self.callback = callback
            def __call__(self, *args): return self.callback(*args)
        def temp(size, buf):
            calls.append(('temp', size)); buf.value = root
            return len(root) if count is None else count
        def mount(path, buf, size):
            calls.append(('mount', path)); buf.value = 'C:\\'
            return mount_ok
        def identity(path, buf, size):
            calls.append(('volume', path)); buf.value = volume or '\\\\?\\Volume{11111111-2222-3333-4444-555555555555}\\'
            return volume_ok
        library = SimpleNamespace(GetTempPathW=Function(temp), GetVolumePathNameW=Function(mount), GetVolumeNameForVolumeMountPointW=Function(identity))
        if newer: library.GetTempPath2W = Function(temp)
        return library, calls

    def test_actual_api_binding_configuration_and_volume_identity_are_private(self):
        lib, calls = self.library()
        native = h.NativeWindows(lib)
        self.assertEqual(native.api_kind, 'get_temp_path2')
        root = native.temp_root()
        self.assertTrue(root.startswith('C:'))
        identity = native.volume_id(root)
        self.assertTrue(identity.startswith('\\\\?\\volume{'))
        self.assertEqual([call[0] for call in calls], ['temp', 'mount', 'volume'])
        self.assertEqual(calls[0][1], 32768)
        self.assertEqual(len(lib.GetTempPath2W.argtypes), 2)
        fallback, _ = self.library(newer=False)
        self.assertEqual(h.NativeWindows(fallback).api_kind, 'get_temp_path')

    def test_native_api_failure_and_oversize_never_fall_back_or_render_error(self):
        for options in [{'count': 0}, {'count': 32768}, {'root': ''}, {'root': 'relative'}, {'mount_ok': False}, {'volume_ok': False}, {'volume': 'PRIVATE_SENTINEL'}]:
            lib, _ = self.library(**options)
            native = h.NativeWindows(lib)
            with self.assertRaises(h.Refused) as caught:
                native.volume_id(native.temp_root())
            self.assertNotIn('PRIVATE_SENTINEL', str(caught.exception))


class CaptureTests(unittest.TestCase):
    def test_raw_reader_requests_only_256k_plus_one_and_overflow_kills(self):
        process = SimpleNamespace(stdout=io.BytesIO(b'x' * (h.RAW_LIMIT+1)), returncode=None)
        def kill(): process.returncode = 1
        process.kill = mock.Mock(side_effect=kill)
        process.wait = lambda timeout: process.returncode
        process.poll = lambda: process.returncode
        with mock.patch.object(h.subprocess, 'Popen', return_value=process):
            result = h.capture_process(['owned.exe'], cwd='.', env={}, timeout=30)
        self.assertEqual(len(result['raw']), h.RAW_LIMIT+1)
        process.kill.assert_called_once()

    def test_reader_start_failure_still_joins_child(self):
        process = SimpleNamespace(stdout=io.BytesIO(), returncode=1)
        process.kill = mock.Mock(); process.wait = mock.Mock(return_value=1)
        with mock.patch.object(h.subprocess, 'Popen', return_value=process), mock.patch.object(h.threading.Thread, 'start', side_effect=RuntimeError('PRIVATE_SENTINEL')):
            result = h.capture_process(['owned.exe'], cwd='.', env={}, timeout=30)
        self.assertEqual(result['fault'], 'spawn_failed')
        self.assertTrue(result['closed'])
        process.kill.assert_called_once()
        process.wait.assert_called_once_with(timeout=3)

    def test_watchdog_is_infra_even_if_a_child_later_claims_pass(self):
        process = SimpleNamespace(stdout=io.BytesIO(b'PRIVATE_SENTINEL'), returncode=0)
        process.wait = mock.Mock(side_effect=[h.subprocess.TimeoutExpired('owned.exe', 30), 0])
        process.kill = mock.Mock(); process.poll = lambda: 0
        with mock.patch.object(h.subprocess, 'Popen', return_value=process):
            result = h.capture_process(['owned.exe'], cwd='.', env={}, timeout=30)
        self.assertEqual(result['fault'], 'watchdog')
        process.kill.assert_called_once()


if __name__ == '__main__':
    unittest.main()
