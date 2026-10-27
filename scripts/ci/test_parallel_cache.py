"""Only the two parallel jobs restore shared caches; existing writers stay exact."""
import hashlib
from pathlib import Path
import re
import unittest

import optimization_projection as infrastructure
import parallel_cache_projection as policy
import sf07_request_projection as historical
import sf06_failure_projection
import browser_failure_projection
from sf07_projection_contract import BASE_WORKFLOW_SHA256

ROOT = Path(__file__).resolve().parents[2]
# Frozen pre-policy projection with the reviewed 9863a387 HTTP source identity.
PRE_POLICY_WORKFLOW_SHA256 = '0a0bfbbd4fa46ee3ef302c7f17a34b619e42de1748ef20231c6e4192f88b5cc4'


def job(source, name):
    found = re.findall(r'(?ms)^  ' + re.escape(name) + r':\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)', source)
    if len(found) != 1:
        raise ValueError('job missing or duplicated')
    return found[0]


def restore_step(block):
    found = [step.group() for step in infrastructure.STEP.finditer(block)
             if 'uses: actions/cache/restore@' in step.group()]
    if len(found) != 1:
        raise ValueError('restore missing or duplicated')
    return found[0]


def key_fields(step):
    # Retain exact paths, primary key and ordered restore prefixes, independent
    # of the display name on each role's cache step.
    return step[step.index('          path: |\n'):]


def prove(source):
    restored = sf06_failure_projection.inverse_workflow(policy.inverse_workflow(
        browser_failure_projection.inverse_workflow(source)))
    if hashlib.sha256(restored.encode()).hexdigest() != PRE_POLICY_WORKFLOW_SHA256:
        raise AssertionError('pre-policy workflow identity changed')
    if hashlib.sha256(historical.inverse_workflow(source).encode()).hexdigest() != BASE_WORKFLOW_SHA256:
        raise AssertionError('historical workflow identity changed')


class ParallelCacheTests(unittest.TestCase):
    def setUp(self):
        self.source = (ROOT / '.github/workflows/ci.yml').read_text()

    def test_two_restore_only_jobs_use_the_existing_writer_keys_and_paths(self):
        self.assertEqual(set(policy.NEW_JOBS), {'linux-workspace', 'windows-contracts'})
        linux = key_fields(restore_step(job(self.source, 'linux')))
        package = key_fields(restore_step(job(self.source, 'nagi-package'))).replace(
            '${{ matrix.platform }}', 'windows-x86_64')
        for name, expected in (('linux-workspace', linux), ('windows-contracts', package)):
            block = job(self.source, name)
            self.assertEqual(key_fields(restore_step(block)), expected)
            self.assertNotIn('uses: actions/cache/save@', block)
            self.assertNotIn('# ci-cache-save', block)
            self.assertNotIn('uses: actions/cache@', block)
            self.assertEqual(infrastructure.cache_edit(block, inverse=True), block)
            self.assertEqual(infrastructure.cache_edit(block), block)
        prove(self.source)

    def test_exact_roundtrips_keep_every_command_gate_and_existing_writer(self):
        before = policy.inverse_workflow(self.source)
        self.assertEqual(policy.apply_workflow(before), self.source)
        plain = infrastructure.inverse_infrastructure(self.source)
        self.assertEqual(infrastructure.infrastructure(plain), self.source)
        for name in policy.NEW_JOBS:
            infrastructure.cache_edit(job(plain, name), inverse=True)
        names = re.findall(r'^  ([a-z][a-z0-9-]*):$', self.source, re.M)
        for name in names:
            if name not in policy.NEW_JOBS:
                self.assertEqual(job(before, name), job(self.source, name), name)
        # Package includes its archive/installer/demo checks and original cache
        # policy. It is not rewritten by this change, including on warm reruns.
        self.assertIn(infrastructure.SAVE_IF, job(self.source, 'nagi-package'))
        self.assertIn(infrastructure.SAVE_IF, job(self.source, 'linux'))

    def test_unknown_jobs_cannot_use_the_restore_only_exception(self):
        for name, block in policy.NEW_JOBS.items():
            unknown = block.replace('  ' + name + ':\n', '  unreviewed-reader:\n', 1)
            with self.assertRaises(ValueError):
                infrastructure.cache_edit(unknown, inverse=True)
            with self.assertRaises(ValueError):
                policy.validate_restore_only_job(unknown, infrastructure.unpin)

    def test_reader_mutations_and_reintroduced_saves_fail_closed(self):
        for name, block in policy.NEW_JOBS.items():
            old = policy.OLD_JOBS[name]
            save = old[old.index('      - name: Save validated Rust cache'):]
            mutants = [self.source.replace(block, '', 1),
                       self.source.replace(block, block * 2, 1),
                       self.source.replace(block, '', 1) + block,
                       self.source.replace(block, block + save, 1),
                       self.source.replace(block, old, 1)]
            for before, after in (
                ('uses: actions/cache/restore@', 'uses: actions/cache@'),
                ('        id: nagi_rust_cache\n', ''),
                ('          key: nagi-rust-v1-', '          key: other-'),
                ('            native-target\n', ''),
                ('hashFiles(\'Cargo.lock\')', 'hashFiles(\'wrong.lock\')'),
                ('          restore-keys: |\n', '          restore-keys: |\n            other-platform-\n'),
                ('retention-days: 7', 'retention-days: 14'),
                ('cargo test --locked', 'cargo test'),
            ):
                self.assertIn(before, block)
                changed = block.replace(before, after, 1)
                mutants.append(self.source.replace(block, changed, 1))
                with self.assertRaises(ValueError):
                    infrastructure.cache_edit(changed, inverse=True)
            for index, changed in enumerate(mutants):
                with self.subTest(job=name, mutation=index), self.assertRaises((ValueError, AssertionError)):
                    prove(changed)

    def test_existing_writers_still_require_success_main_only_save(self):
        for name in ('linux', 'nagi-package', 'nagi-regression'):
            block = job(self.source, name)
            self.assertIn(infrastructure.SAVE_IF, block)
            mutants = [block.replace('success() && github.event_name', 'always() && github.event_name', 1),
                       block.replace(" && github.ref == 'refs/heads/main'", '', 1)]
            for changed in mutants:
                self.assertNotEqual(block, changed)
                with self.assertRaises(ValueError):
                    infrastructure.inverse_infrastructure(changed)
            with self.assertRaises(ValueError):
                infrastructure.inverse_infrastructure(block[:block.index('      - name: Save validated Rust cache')])


if __name__ == '__main__':
    unittest.main()
