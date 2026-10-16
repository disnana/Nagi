"""Rate checkpoint registration and fake CI controls; never native evidence."""
import importlib.util
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
