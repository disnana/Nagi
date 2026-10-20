"""Rate checkpoint registration and fake CI controls; never native evidence."""
import ast
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / 'scripts/ci/sf07_checkpoint_summary.py'
spec = importlib.util.spec_from_file_location('rate_summary', SCRIPT)
summary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(summary)
projection_spec = importlib.util.spec_from_file_location(
    'rate_projection', ROOT / 'scripts/ci/sf07_request_projection.py')
projection = importlib.util.module_from_spec(projection_spec)
projection_spec.loader.exec_module(projection)


def fixture():
    return ('running 12 tests\n' + ''.join('test ' + n + ' ... ok\n'
        for n in summary.EXPECTED['rate-startup']) +
        'test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out; finished in 0.01s\n').encode()


class RateSourceTests(unittest.TestCase):
    def test_broad_inventory_retains_frozen_fifty_five_and_adds_exact_twelve(self):
        tree = ast.parse((ROOT/'scripts/check_sf07_private_tests.py').read_text())
        original = [n for n in tree.body if isinstance(n, ast.Assign)
                    and any(isinstance(t, ast.Name) and t.id == 'EXPECTED' for t in n.targets)]
        additions = [n for n in tree.body if isinstance(n, ast.AugAssign)
                     and isinstance(n.target, ast.Name) and n.target.id == 'EXPECTED']
        self.assertEqual(len(original), 1)
        self.assertEqual(len(additions), 2)
        old = ast.literal_eval(original[0].value)
        new = ast.literal_eval(additions[0].value)
        self.assertEqual(len(old), 55)
        self.assertEqual(hashlib.sha256(json.dumps(old, separators=(',', ':')).encode()).hexdigest(),
                         'a9494c0c4080e9bf75d4c631f23e873fd37380ada1dffdf908f87c571a9df985')
        self.assertIsInstance(additions[0].op, ast.Add)
        self.assertEqual(new, sorted(summary.EXPECTED['rate-startup']))
        self.assertIsInstance(additions[1].op, ast.Add)
        retained = ast.literal_eval(additions[1].value)
        self.assertEqual(retained, sorted(summary.EXPECTED['catalog-retained']))
        self.assertEqual(len(set(old + new + retained)), 70)
        # Run the actual broad checker: registration must not merely agree in
        # two source lists while its fixed count still rejects the new suite.
        code = compile(tree, 'check_sf07_private_tests.py', 'exec')
        from unittest.mock import patch
        runner = {'linux': 'Linux', 'win32': 'Windows', 'darwin': 'macOS'}[sys.platform]
        with patch.object(sys, 'path', [str(ROOT/'scripts')] + sys.path), patch.dict(os.environ, {'RUNNER_OS': runner}, clear=True):
            from ci.sf07_owned_state_summary import expected
            names = old + new + retained + sorted(expected('private'))
            count = len(names)
            text = f'running {count} tests\n' + ''.join('test ' + n + ' ... ok\n' for n in names)
            text += f'test result: ok. {count} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'
            with patch.object(Path, 'open', return_value=io.BytesIO(text.encode())), patch.object(sys, 'argv', ['checker', 'fixture']):
                exec(code, {'__name__': '__main__'})

    def test_exact_twelve_native_cases_and_both_required_steps(self):
        source = (ROOT/'runtime/src/security_sf07_private/startup_rate_tests.rs').read_text()
        names = re.findall(r'#\[test\]\s*fn (\w+)\(', source)
        self.assertEqual(len(names), 12)
        self.assertEqual({'security_sf07_private::startup_rate::tests::' + n for n in names},
                         set(summary.EXPECTED['rate-startup']))
        self.assertNotIn('#[ignore', source)
        self.assertIn('pub(crate) mod startup_rate;',
                      (ROOT/'runtime/src/security_sf07_private/mod.rs').read_text())
        workflow = (ROOT/'.github/workflows/ci.yml').read_text()
        source_command = 'python -B scripts/tests/test_sf07_rate_source.py'
        self.assertEqual(workflow.count(source_command), 1)
        self.assertEqual(workflow.count('| python -B scripts/ci/sf07_checkpoint_summary.py rate-startup'), 2)
        restored = projection.inverse_source_guard_dedup(workflow)
        self.assertEqual(restored.count(source_command), 2)

    def test_closed_parser_rejects_missing_foreign_ignored_and_wrong_counts(self):
        raw = fixture()
        for value in (raw, raw.replace(b'\n', b'\r\n')):
            self.assertEqual(summary.classify(value, 'rate-startup'), 'passed')
        name = summary.EXPECTED['rate-startup'][0].encode()
        for value in (raw.replace(b'12 passed', b'0 passed'), raw.replace(b'0 ignored', b'1 ignored'),
                      raw.replace(name, b'unknown::test'), raw + b'test unknown::test ... ok\n',
                      raw + b'\r', raw + b'\xff', b'x' * 65537):
            self.assertEqual(summary.classify(value, 'rate-startup'), 'invalid')
        self.assertEqual(summary.classify(b'', 'rate-startup'), 'unavailable')
        self.assertEqual(summary.classify(b'error[E0382]: sentinel\n', 'rate-startup'), 'compile_failed')

    def test_actual_cli_normal_and_forced_crlf_preserve_closed_output(self):
        wrapper = "import runpy,sys; path=sys.argv[1]; sys.stdout.reconfigure(newline='\\r\\n'); sys.argv=[path,'rate-startup']; runpy.run_path(path,run_name='__main__')"
        commands = ([sys.executable, '-B', str(SCRIPT), 'rate-startup'],
                    [sys.executable, '-B', '-c', wrapper, str(SCRIPT)])
        for command in commands:
            for raw, code, state in ((fixture(), 0, 'passed'), (b'private sentinel', 1, 'unavailable')):
                result = subprocess.run(command, input=raw, capture_output=True, check=False)
                output = result.stdout.replace(b'\r\n', b'\n')
                self.assertNotIn(b'\r', output)
                self.assertEqual(result.returncode, code)
                self.assertEqual(output, ('sf07_checkpoint scope=rate-startup state=' + state + '\n').encode())
                self.assertEqual(result.stderr, b'')


if __name__ == '__main__':
    unittest.main()
