import re
import unittest
from pathlib import Path

import levels_projection
import optimization_projection as opt
import publication_projection
import linux_workspace_projection
import windows_contract_projection
import parallel_cache_projection
import go_backend_projection

ROOT = Path(__file__).resolve().parents[2]


class OptimizationTests(unittest.TestCase):
    def source(self):
        return (ROOT / '.github/workflows/ci.yml').read_text()

    def test_checked_in_workflows_have_pinned_actions_and_owned_caches(self):
        for path in sorted((ROOT / '.github/workflows').glob('*.yml')):
            with self.subTest(workflow=path.name):
                source = path.read_text()
                opt.validate_infrastructure(source)
                if 'actions/cache/restore@' in source:
                    self.assertIn("github.event_name == 'push'", source)
                    self.assertIn("github.ref == 'refs/heads/main'", source)

    def test_extra_historical_diagnostics_are_explicit_manual_work(self):
        for name in ('browser-login-observer.yml', 'editor-stage-observation.yml'):
            source = (ROOT / '.github/workflows' / name).read_text()
            events = source.split('on:\n', 1)[1].split('permissions:', 1)[0]
            self.assertIn('workflow_dispatch:', events)
            self.assertNotIn('pull_request:', events)
            self.assertNotIn('workflow_run:', events)
        # Required ordinary runtime/browser verification remains in the core
        # workflow; this removes duplicate investigative runs, not coverage.
        source = self.source()
        self.assertIn('  sf08-browser:\n', source)
        self.assertIn('cargo test --locked -p nagi-runtime --test browser_login_public_api', source)

    def test_infrastructure_exact_roundtrip_and_reapplication(self):
        for path in sorted((ROOT / '.github/workflows').glob('*.yml')):
            with self.subTest(workflow=path.name):
                source = path.read_text()
                restored = opt.inverse_infrastructure(source)
                self.assertEqual(opt.infrastructure(restored), source)

    def test_pin_mutation_is_rejected(self):
        source = self.source()
        bad = source.replace(opt.PINS['actions/checkout', 'v4'], '0' * 40, 1)
        with self.assertRaises(ValueError):
            opt.inverse_infrastructure(bad)
        with self.assertRaises(ValueError):
            opt.validate_infrastructure(source.replace('uses: actions/checkout@', 'uses: malicious/checkout@', 1))

    def test_job_cancellation_guards_roundtrip_without_changing_cleanup_steps(self):
        source = self.source()
        seen = set()
        for match in opt.JOB.finditer(source):
            block = match.group()
            name = block.split(':', 1)[0].strip()
            if name not in opt.CANCELLABLE_JOBS:
                self.assertEqual(opt.cancellation_edit(block, inverse=True), block)
                continue
            seen.add(name)
            restored = opt.cancellation_edit(block, inverse=True)
            self.assertEqual(opt.cancellation_edit(restored), block)
            self.assertEqual(restored.partition('    steps:\n')[2], block.partition('    steps:\n')[2])
            for guard in ('always()', 'always() && cancelled()',
                          '!cancelled() && always()', 'always() && !cancelled() && !cancelled()'):
                changed = block.replace('always() && !cancelled()', guard, 1)
                mutant = source[:match.start()] + changed + source[match.end():]
                with self.subTest(job=name, guard=guard), self.assertRaises(ValueError):
                    opt.inverse_infrastructure(mutant)
        self.assertEqual(seen, {'ci-plan', 'jetbrains', 'sf08-browser', 'nagi-package', 'ready', 'publish-release'})

    def test_cache_save_is_successful_main_only_and_not_a_test_skip(self):
        source = self.source()
        for before, after in (("success() && github.event_name", "always() && github.event_name"),
                              ("github.ref == 'refs/heads/main'", "github.ref != 'refs/heads/main'"),
                              ("steps.nagi_rust_cache.outputs.cache-primary-key", "steps.other.outputs.key")):
            # Mutate the cache guard, not an unrelated main-ref expression in
            # workflow concurrency or publication policy.
            bad = source.replace(opt.SAVE_IF, opt.SAVE_IF.replace(before, after, 1), 1)
            with self.subTest(change=after), self.assertRaises(ValueError):
                opt.inverse_infrastructure(bad)
        first = re.search(r'      - name: Save validated Rust cache.*?(?=^  [a-z]|\Z)', source, re.M | re.S).group()
        for bad in (source.replace(first, '', 1), source.replace(first, first + first, 1)):
            with self.assertRaises(ValueError):
                opt.inverse_infrastructure(bad)

    def test_credential_and_retention_weakening_is_rejected(self):
        for before, after in (('persist-credentials: false # ci-isolation', 'persist-credentials: true # ci-isolation'),
                              ('retention-days: 7 # ci-retention', 'retention-days: 90 # ci-retention')):
            with self.subTest(change=after), self.assertRaises(ValueError):
                opt.inverse_infrastructure(self.source().replace(before, after, 1))

    def test_critical_cache_is_separate_pr_local_without_native_app_artifacts(self):
        source = self.source()
        critical = re.search(r'^  critical:\n.*?(?=^  jetbrains:)', source, re.M | re.S).group()
        self.assertIn(opt.CRITICAL_SAVE_IF, critical)
        self.assertIn('key: nagi-critical-v1-', critical)
        self.assertNotIn('native-target', critical)
        for name in ('linux', 'nagi-package', 'nagi-regression'):
            job = re.search(r'^  '+name+r':\n.*?(?=^  [a-z]|\Z)', source, re.M | re.S).group()
            self.assertNotIn('key: nagi-critical-v1-', job)
            self.assertIn(opt.SAVE_IF, job)
        for name in ('linux-workspace', 'windows-contracts'):
            job = re.search(r'^  '+name+r':\n.*?(?=^  [a-z]|\Z)', source, re.M | re.S).group()
            parallel_cache_projection.validate_restore_only_job(job, opt.unpin)
            self.assertNotIn('actions/cache/save@', job)

    def test_publication_aggregate_source_and_level_controls_cannot_drift(self):
        source = self.source()
        for before, after in (("needs.ready.result == 'success'", "needs.ready.result != 'failure'"),
                              ("needs.ci-plan.outputs.level == '3'", "needs.ci-plan.outputs.level == '2'"),
                              ('python -B scripts/ci/publication_gate.py', 'echo bypass')):
            with self.subTest(change=after), self.assertRaises(ValueError):
                levels_projection.inverse_workflow(source.replace(before, after, 1))
        historical_caches = parallel_cache_projection.inverse_workflow(
            go_backend_projection.inverse_workflow(source))
        without_windows = windows_contract_projection.inverse_workflow(historical_caches)
        plain = opt.inverse_infrastructure(linux_workspace_projection.inverse_workflow(without_windows))
        self.assertEqual(publication_projection.inverse_publication(plain).count('  publish-release:\n'), 1)

    def test_critical_command_removal_is_not_hidden_by_infrastructure_projection(self):
        source = self.source()
        with self.assertRaises(ValueError):
            levels_projection.inverse_workflow(source.replace('      - run: cargo test --locked -p nagic --lib\n', '', 1))


if __name__ == '__main__':
    unittest.main()
