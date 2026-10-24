"""One independent runner; unchanged coverage, strict aggregate and historical proof."""
import copy
import hashlib
from pathlib import Path
import re
import unittest

import gate
import levels_projection
import linux_workspace_projection as shard
import optimization_projection
import publication_gate
import sf07_request_projection as historical
from sf07_projection_contract import BASE_WORKFLOW_SHA256
import test_levels
import test_publication_gate
import test_publish_conditions
import windows_contract_projection
import parallel_cache_projection

ROOT = Path(__file__).resolve().parents[2]


def prove(source):
    source = parallel_cache_projection.apply_workflow(source)
    digest = hashlib.sha256(historical.inverse_workflow(source).encode()).hexdigest()
    if digest != BASE_WORKFLOW_SHA256:
        raise AssertionError("historical whole-workflow identity changed")


class LinuxWorkspaceSourceTests(unittest.TestCase):
    def setUp(self):
        self.current_source = (ROOT / '.github/workflows/ci.yml').read_text()
        self.source = parallel_cache_projection.inverse_workflow(self.current_source)
        self.linux_source = windows_contract_projection.inverse_workflow(self.source)

    def test_one_command_moves_and_all_original_linux_checks_are_restored(self):
        prove(self.source)
        self.assertEqual(self.source.count(shard.WORKSPACE_STEP), 1)
        linux = re.search(r'(?ms)^  linux:\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)', self.source).group()
        self.assertNotIn(shard.WORKSPACE_STEP, linux)
        self.assertIn('steps.auth_stage_diag.outcome', linux)
        self.assertIn('steps.session_deadline_diag.outcome', linux)
        self.assertIn("steps.sf07_prelogin_owner_diag.outputs.sanitized == 'true'", linux)
        self.assertIn('Compile and run CSRF deadline semantic mutants', linux)
        self.assertIn('python scripts/build_examples.py', linux)
        self.assertEqual(len(re.findall(r'^      - run:', shard.JOB, re.M)), 1)
        self.assertNotIn('continue-on-error:', shard.JOB)
        self.assertNotIn('needs: [linux', shard.JOB)
        restored = shard.inverse_workflow(self.linux_source)
        self.assertIn(shard.WORKSPACE_STEP + shard.ANCHOR, restored)
        self.assertNotIn('linux-workspace', restored)
        # Both direct source guards and the historical levels/parallel callers
        # must see the same inherited command body and aggregate graph.
        inherited = levels_projection.inverse_workflow(self.current_source)
        self.assertIn(shard.WORKSPACE_STEP + shard.ANCHOR, inherited)

    def test_raw_guard_composes_with_existing_infrastructure(self):
        plain = optimization_projection.inverse_infrastructure(self.current_source)
        with self.assertRaises(ValueError):
            shard.inverse_workflow(plain)
        self.assertEqual(optimization_projection.infrastructure(plain), self.current_source)
        restored = shard.inverse_workflow(self.linux_source)
        self.assertNotIn('linux-workspace', restored)
        plain_restored = optimization_projection.inverse_infrastructure(restored)
        self.assertEqual(optimization_projection.infrastructure(plain_restored), restored)

    def test_missing_duplicate_moved_or_weakened_job_never_passes(self):
        mutations = [self.source.replace(shard.JOB, '', 1),
                     self.source.replace(shard.JOB, shard.JOB * 2, 1),
                     self.source.replace(shard.JOB, '', 1) + shard.JOB,
                     self.source.replace(shard.ANCHOR, shard.WORKSPACE_STEP + shard.ANCHOR, 1)]
        for old, new in (
            ('cargo test --locked', 'cargo test --locked -p nagic --lib'),
            ('cargo test --locked', 'cargo test --locked -- --ignored'),
            ('cargo test --locked', 'cargo test'),
            (shard.WORKSPACE_STEP, '      - run: true\n'),
            (shard.WORKSPACE_STEP, shard.WORKSPACE_STEP * 2),
            ('needs: [changes, ci-plan]', 'needs: [changes, ci-plan, linux]'),
            ("needs.ci-plan.outputs.comprehensive_checks == 'true'", 'false'),
            ("needs.changes.result == 'success'", "needs.changes.result != 'failure'"),
            ('runs-on: ubuntu-latest', 'runs-on: windows-latest'),
            ("python-version: '3.12'", "python-version: '3.11'"),
            ('# ci-pin:stable', '# ci-pin:nightly'),
            ('persist-credentials: false', 'persist-credentials: true'),
            ('NAGI_FAILURE_DIR:', 'OTHER_FAILURE_DIR:'),
            ('NAGI_GOLDEN_CAPTURE_DIR:', 'OTHER_GOLDEN_CAPTURE_DIR:'),
            ('            native-target\n', ''),
            ('key: nagi-rust-v1-linux-workspace-', 'key: nagi-rust-v1-checks-'),
            ('hashFiles(\'Cargo.lock\')', 'hashFiles(\'wrong.lock\')'),
            ('            nagi-rust-v1-checks-', '            wrong-'),
            ('if: failure()', 'if: success()'),
            ('retention-days: 7', 'retention-days: 14'),
            ('name: compiler-failures-linux-workspace', 'name: compiler-failures-linux'),
            ('path: build/compiler-failures/', 'path: build/other/'),
            ('if-no-files-found: ignore', 'if-no-files-found: warn'),
            ("success() && github.event_name == 'push'", "always() && github.event_name == 'push'"),
            ("github.event_name == 'push'", "github.event_name == 'pull_request'"),
            ("github.ref == 'refs/heads/main'", 'true'),
            ('steps.nagi_rust_cache.outputs.cache-primary-key', 'steps.other.outputs.key'),
        ):
            self.assertIn(old, shard.JOB)
            mutations.append(self.source.replace(shard.JOB, shard.JOB.replace(old, new, 1), 1))
        for new in (windows_contract_projection.READY_NEW, windows_contract_projection.PUBLISH_NEW):
            mutations.append(self.source.replace(new, new.replace(' linux-workspace,', '', 1), 1))
        for index, changed in enumerate(mutations):
            with self.subTest(mutation=index), self.assertRaises((ValueError, AssertionError)):
                prove(changed)

    def test_original_diagnostic_enforcement_cannot_be_masked_by_new_job(self):
        for old in ("run: test '${{ steps.auth_stage_diag.outcome }}' = success",
                    "run: test '${{ steps.session_deadline_diag.outcome }}' = success"):
            self.assertIn(old, self.source)
            with self.assertRaises((ValueError, AssertionError)):
                prove(self.source.replace(old, 'run: true', 1))


class LinuxWorkspaceGateTests(unittest.TestCase):
    def test_handwritten_level_result_matrix_and_missing_job_are_strict(self):
        for level, full, package, expected in [
                ('1', 'false', 'false', 'skipped'), ('1', 'true', 'true', 'skipped'),
                ('2', 'true', 'true', 'success'), ('3', 'true', 'true', 'success'),
                ('2', 'false', 'true', 'success'), ('3', 'false', 'true', 'success'),
                ('3', 'false', 'false', 'skipped')]:
            plan = test_levels.release(package_nagi=package)
            original = test_levels.needs(level, full, plan)
            self.assertEqual(original['linux-workspace']['result'], expected)
            self.assertEqual(gate.errors(original), [])
            for result in ('success', 'skipped', 'failure', 'cancelled', 'queued', '', None):
                needs = copy.deepcopy(original)
                needs['linux-workspace']['result'] = result
                self.assertEqual(gate.errors(needs) == [], result == expected, (level, full, result))
            del original['linux-workspace']
            self.assertTrue(gate.errors(original))

    def test_source_guard_or_invalid_plan_never_makes_shard_success_sufficient(self):
        original = test_levels.needs('2')
        for job in ('linux', 'changes', 'ci-plan'):
            for result in ('failure', 'cancelled', 'skipped', None):
                needs = copy.deepcopy(original)
                needs[job]['result'] = result
                self.assertEqual(needs['linux-workspace']['result'], 'success')
                self.assertTrue(gate.errors(needs))
        for value in (None, '', 'TRUE', True, False):
            needs = copy.deepcopy(original)
            needs['ci-plan']['outputs']['comprehensive_checks'] = value
            self.assertTrue(gate.errors(needs))

    def test_job_condition_has_same_selector_and_no_linux_dependency(self):
        condition = test_publish_conditions.job_expression('linux-workspace')
        for changes in ('success', 'failure', 'cancelled', 'skipped'):
            for plan in ('success', 'failure', 'cancelled', 'skipped'):
                for selected in ('true', 'false'):
                    context = {'needs': {'changes': {'result': changes},
                                        'ci-plan': {'result': plan, 'outputs': {'comprehensive_checks': selected}}}}
                    actual = test_publish_conditions.evaluate(condition, context)
                    self.assertEqual(actual, (changes, plan, selected) == ('success', 'success', 'true'))

    def test_fictional_ready_success_cannot_publish_failed_or_missing_workspace(self):
        for component in ('nagi', 'vscode', 'jetbrains'):
            original = test_publication_gate.valid(component)
            expected = 'success' if component == 'nagi' else 'skipped'
            for result in ('success', 'skipped', 'failure', 'cancelled', '', None):
                needs = copy.deepcopy(original)
                needs['linux-workspace']['result'] = result
                self.assertEqual(needs['ready']['result'], 'success')
                errors = publication_gate.errors(needs, 'push', 'refs/heads/main', test_publication_gate.SHA)
                self.assertEqual(errors == [], result == expected, (component, result))
            del original['linux-workspace']
            self.assertTrue(publication_gate.errors(original, 'push', 'refs/heads/main', test_publication_gate.SHA))


if __name__ == '__main__':
    unittest.main()
