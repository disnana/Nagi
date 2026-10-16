"""Rate checkpoint registration and fake CI controls; never native evidence."""
import ast
import hashlib
import importlib.util
import json
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
        self.assertEqual(len(additions), 1)
        old = ast.literal_eval(original[0].value)
        new = ast.literal_eval(additions[0].value)
        self.assertEqual(len(old), 55)
        self.assertEqual(hashlib.sha256(json.dumps(old, separators=(',', ':')).encode()).hexdigest(),
                         'a9494c0c4080e9bf75d4c631f23e873fd37380ada1dffdf908f87c571a9df985')
        self.assertIsInstance(additions[0].op, ast.Add)
        self.assertEqual(new, sorted(summary.EXPECTED['rate-startup']))
        self.assertEqual(len(set(old + new)), 67)
        # Run the actual broad checker: registration must not merely agree in
        # two source lists while its fixed count still rejects the new suite.
        code = compile(tree, 'check_sf07_private_tests.py', 'exec')
        from unittest.mock import patch
        text = ''.join('test ' + n + ' ... ok\n' for n in old + new)
        text += 'test result: ok. 67 passed; 0 failed; 0 ignored;\n'
        with patch.object(Path, 'read_text', return_value=text), patch.object(sys, 'argv', ['checker', 'fixture']):
            exec(code, {'__name__': '__main__'})

    def test_exact_twelve_native_cases_and_both_required_steps(self):
        source = (ROOT/'runtime/src/security_sf07_private/startup_rate_tests.rs').read_text()
        names = re.findall(r'#\[test\]\s*fn (\w+)\(', source)
        self.assertEqual(len(names), 12)
        self.assertEqual({'security_sf07_private::startup_rate::tests::' + n for n in names},
                         set(summary.EXPECTED['rate-startup']))
        self.assertNotIn('#[ignore', source)
        self.assertIn('#[cfg(test)]\nmod startup_rate;',
                      (ROOT/'runtime/src/security_sf07_private/mod.rs').read_text())
        workflow = (ROOT/'.github/workflows/ci.yml').read_text()
        self.assertEqual(workflow.count('python -B scripts/tests/test_sf07_rate_source.py'), 2)
        self.assertEqual(workflow.count('| python -B scripts/ci/sf07_checkpoint_summary.py rate-startup'), 2)

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
