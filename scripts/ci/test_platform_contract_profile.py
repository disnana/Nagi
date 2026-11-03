"""Profile/cache experiment over the exact verified #220 platform job."""
from pathlib import Path
import re
import unittest

import platform_contract_profile_projection as projection
import platform_contract_projection as historical
import windows_regression_sharding_projection as regression_sharding


ROOT = Path(__file__).resolve().parents[2]
STEP = re.compile(r'(?ms)^      - .*?(?=^      - |\Z)')


class PlatformContractProfileTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.current = regression_sharding.inverse_workflow(
            (ROOT / '.github/workflows/ci.yml').read_text())
        cls.base = projection.inverse_workflow(cls.current)
        cls.current_job = re.search(
            r'(?ms)^  platform-contracts:\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)', cls.current).group()
        cls.base_job = historical.job(cls.base, 'platform-contracts').group()

    def test_exact_outer_inverse_restores_verified_838_workflow(self):
        self.assertEqual(projection.apply_workflow(self.base), self.current)
        self.assertEqual(projection.inverse_workflow(self.current), self.base)
        self.assertEqual(projection._workflow_hash(self.base), projection.BASE_WORKFLOW_SHA256)
        self.assertEqual(self.base_job, historical.PLATFORM_JOB)
        self.assertEqual(
            re.search(r'(?ms)^  ready:\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)', self.current).group(),
            re.search(r'(?ms)^  ready:\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)', self.base).group(),
        )
        self.assertEqual(
            re.search(r'(?ms)^  publish-release:\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)', self.current).group(),
            re.search(r'(?ms)^  publish-release:\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)', self.base).group(),
        )

    def test_profile_scope_and_unchanged_steps_matrix_and_commands(self):
        job_env = self.current_job.split('    steps:\n', 1)[0]
        for field in ('CARGO_PROFILE_DEV_DEBUG', 'CARGO_PROFILE_TEST_DEBUG', 'CARGO_INCREMENTAL'):
            with self.subTest(field=field):
                self.assertEqual(len(re.findall(r'(?m)^      ' + field + r": '0'$", job_env)), 1)
        self.assertIn('matrix: ${{ fromJSON(needs.ci-plan.outputs.matrix) }}', self.current_job)
        self.assertIn('runs-on: ${{ matrix.os }}', self.current_job)
        self.assertIn("RUSTFLAGS: --cfg nagi_sf01_diagnostic --cfg nagi_sf01_diag_oracle", self.current_job)
        self.assertIn("RUSTFLAGS='--cfg nagi_auth_stage_diag --cfg nagi_browser_login_diag'", self.current_job)

        before = STEP.findall(self.base_job)
        after = STEP.findall(self.current_job)
        self.assertEqual(len(before), len(after))
        for index, (old, new) in enumerate(zip(before, after)):
            if 'Restore isolated platform contract builds' in old:
                self.assertEqual(old.replace(projection.OLD_CACHE_KEY, projection.NEW_CACHE_KEY), new)
            else:
                with self.subTest(step=index):
                    self.assertEqual(old, new)
        self.assertEqual(self.base_job.count('cargo test --locked'), self.current_job.count('cargo test --locked'))
        self.assertEqual(self.base_job.count('cargo check --locked'), self.current_job.count('cargo check --locked'))
        self.assertEqual(self.base_job.count('RUSTFLAGS'), self.current_job.count('RUSTFLAGS'))

    def test_cache_namespace_records_profile_and_has_no_fallback(self):
        self.assertEqual(self.current_job.count(projection.NEW_CACHE_KEY), 1)
        self.assertNotIn(projection.OLD_CACHE_KEY, self.current_job)
        self.assertIn('nagi-platform-contracts-v2-', projection.NEW_CACHE_KEY)
        for marker in (
            '${{ matrix.platform }}', '${{ runner.os }}', '${{ runner.arch }}',
            '${{ steps.rust.outputs.cachekey }}', 'debug0-devtest-incr0',
            'cfg-default-auth-browser-sf01-oracle-sf06-native-target-v2',
            "hashFiles('Cargo.lock')", "hashFiles('.github/workflows/ci.yml'",
            "'scripts/ci/**'", "'scripts/tests/**'", "hashFiles('Cargo.toml'",
            "'compiler/**'", "'runtime/**'", "'runtime-core/**'",
        ):
            with self.subTest(marker=marker):
                self.assertIn(marker, projection.NEW_CACHE_KEY)
        self.assertIn('            target\n            native-target\n', self.current_job)
        cache_step = next(step for step in STEP.findall(self.current_job)
                          if 'Restore isolated platform contract builds' in step)
        self.assertNotIn('restore-keys:', cache_step)
        self.assertIn("github.event_name == 'push' && github.ref == 'refs/heads/main'", self.current_job)
        self.assertIn('steps.nagi_rust_cache.outputs.cache-primary-key', self.current_job)

    def test_outer_inverse_preserves_unrelated_text_for_historical_checks(self):
        suffix = '\n# unrelated workflow drift remains visible\n'
        changed = self.current + suffix
        self.assertEqual(projection.inverse_workflow(changed), self.base + suffix)

    def test_profile_key_command_matrix_and_gate_mutations_fail_closed(self):
        job = projection.PROFILE_JOB
        mutations = []
        for old, new in (
            ("      CARGO_PROFILE_DEV_DEBUG: '0'", "      CARGO_PROFILE_DEV_DEBUG: '1'"),
            ("      CARGO_PROFILE_TEST_DEBUG: '0'", "      CARGO_PROFILE_TEST_DEBUG: '1'"),
            ("      CARGO_INCREMENTAL: '0'", "      CARGO_INCREMENTAL: '1'"),
            (projection.PROFILE_ENV, projection.OLD_ENV),
            ("      NAGI_GOLDEN_CAPTURE_DIR: ${{ github.workspace }}/build/compiler-failures/goldens\n",
             "      CARGO_PROFILE_RELEASE_DEBUG: '0'\n      NAGI_GOLDEN_CAPTURE_DIR: ${{ github.workspace }}/build/compiler-failures/goldens\n"),
            (projection.NEW_CACHE_KEY, projection.OLD_CACHE_KEY),
            ('debug0-devtest-incr0', 'debug1-devtest-incr0'),
            ('native-target\n', ''),
            ('      - name: Save validated Rust cache (main only)', '      - name: Save cache'),
            ('      - name: Check native buffered send deadline', '      - name: Missing send deadline'),
            ('matrix: ${{ fromJSON(needs.ci-plan.outputs.matrix) }}', 'matrix: {os: [windows-latest]}'),
        ):
            self.assertIn(old, job)
            changed = job.replace(old, new, 1)
            mutations.append(self.current.replace(job, changed, 1))
        mutations.extend((
            self.current.replace(job, '', 1),
            self.current.replace(job, job + job, 1),
            self.current.replace(job, job.replace('          native-target\n', '', 1), 1),
            self.current.replace(job, job.replace(
                '      - uses: actions/setup-python@',
                '          restore-keys: nagi-rust-v1-package-\n      - uses: actions/setup-python@', 1), 1),
        ))
        for index, mutant in enumerate(mutations):
            with self.subTest(mutation=index), self.assertRaises(ValueError):
                projection.inverse_workflow(mutant)

    def test_existing_contract_inverse_rejects_aggregate_gate_mutations(self):
        changed = self.current.replace('  ready:\n', '  ready:\n    needs: [] # mutated\n', 1)
        with self.assertRaises(ValueError):
            historical.inverse_workflow(changed)


if __name__ == '__main__':
    unittest.main()
