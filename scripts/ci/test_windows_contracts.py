"""Windows-only relocation keeps per-platform native evidence and strict gates."""
from collections import Counter
import copy
import hashlib
from pathlib import Path
import re
import unittest

import gate
import optimization_projection as infrastructure
import publication_gate
import sf07_request_projection as historical
from sf07_projection_contract import BASE_WORKFLOW_SHA256
import test_levels
import test_publication_gate
import test_publish_conditions
import windows_contract_projection as shard
import parallel_cache_projection
import sf06_failure_projection
import browser_failure_projection

ROOT = Path(__file__).resolve().parents[2]
# Immutable Linux-only commit 0e233491; its workflow must be restored exactly.
LINUX_WORKFLOW_SHA256 = '8dd517948186a68b37130190d75fc2e843e74a7230622f310150723038609720'
COMMON = (
    'Check bounded SF01 diagnostic observer',
    'Check SF01 High, saved Low, handwritten Low and native policy contracts',
    'Check SF01 request lease and permit lifecycle through the public runtime',
    'Check Host authority and HTTP entrypoint contracts on the target platform',
)
WINDOWS_ONLY = (
    'Check native buffered send deadline and idle Keep-Alive early on Windows',
    'Diagnose private Session phase deadlines on Windows',
)


def job(source, name):
    found = re.findall(r'(?ms)^  ' + re.escape(name) + r':\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)', source)
    if len(found) != 1:
        raise ValueError('job missing or duplicated')
    return found[0]


def effective_inventory(source, platform):
    """Evaluate only the three reviewed OS guards, with preceding checks green.

    Compare the full step (including shell, environment and every assertion),
    removing only the new non-Windows guard. Unknown guards are never ignored.
    """
    blocks = [job(source, 'nagi-package')]
    if platform == 'Windows':
        blocks.append(job(source, 'windows-contracts'))
    result = Counter()
    for block in blocks:
        for step in re.findall(r'(?ms)^      - .*?(?=^      - |\Z)', block):
            name = re.match(r'      - name: ([^\n]+)\n', step)
            if name is None or name[1] not in (*COMMON, *WINDOWS_ONLY):
                continue
            guards = re.findall(r'^        if: (.+)$', step, re.M)
            if len(guards) > 1:
                raise ValueError('duplicate guard')
            selected = True
            if guards:
                if guards[0] == "runner.os != 'Windows'":
                    selected = platform != 'Windows'
                    step = step.replace("        if: runner.os != 'Windows'\n", '', 1)
                elif guards[0] in ("runner.os == 'Windows'", "always() && runner.os == 'Windows'"):
                    selected = platform == 'Windows'
                else:
                    raise ValueError('unknown platform guard')
            if selected:
                result[step] += 1
    return result


def prove(source):
    restored = sf06_failure_projection.inverse_workflow(shard.inverse_workflow(
        browser_failure_projection.inverse_workflow(source)))
    if hashlib.sha256(restored.encode()).hexdigest() != LINUX_WORKFLOW_SHA256:
        raise AssertionError('Linux-only workflow identity changed')
    current = parallel_cache_projection.apply_workflow(source)
    if hashlib.sha256(historical.inverse_workflow(current).encode()).hexdigest() != BASE_WORKFLOW_SHA256:
        raise AssertionError('historical frozen workflow identity changed')


class WindowsContractSourceTests(unittest.TestCase):
    def setUp(self):
        self.current_source = (ROOT / '.github/workflows/ci.yml').read_text()
        self.source = parallel_cache_projection.inverse_workflow(self.current_source)

    def test_effective_inventory_is_once_on_all_four_release_platforms(self):
        prove(self.source)
        for platform in ('Windows', 'Linux', 'macos-arm64', 'macos-x86_64'):
            expected = Counter(shard.COMMON_STEPS)
            if platform == 'Windows':
                expected.update((shard.SEND_STEP, shard.PRIVATE_STEP))
            self.assertEqual(effective_inventory(self.source, platform), expected, platform)
        package = job(self.source, 'nagi-package')
        self.assertNotIn(shard.SEND_STEP, package)
        self.assertNotIn(shard.PRIVATE_STEP, package)
        for step in shard.COMMON_STEPS:
            self.assertEqual(package.count(shard.guarded(step)), 1)
        for retained in ('Check stalled send permit release and graceful join early on Windows',
                         'Capture bounded Session auth stages before broad checks',
                         'Enforce Session auth-stage supplement after normal checks',
                         'Enforce Session deadline diagnostic after normal checks',
                         'Package tracked source and the compiler',
                         'Verify an extracted distribution outside the checkout',
                         'Check demo packaging with Windows PowerShell',
                         'Check demo packaging with PowerShell 7'):
            self.assertIn(retained, package)
        self.assertEqual(infrastructure.infrastructure(infrastructure.inverse_infrastructure(self.current_source)), self.current_source)

    def test_private_controls_and_native_send_oracles_remain_strict(self):
        self.assertIn("if: always() && runner.os == 'Windows'", shard.PRIVATE_STEP)
        self.assertIn('timeout-minutes: 30', shard.PRIVATE_STEP)
        for text in ('export TMP="$RUNNER_TEMP" TEMP="$RUNNER_TEMP"',
                     'python -B scripts/tests/test_session_private_deadline_diagnostic.py\n          fake_rc=$?',
                     'python -B scripts/ci/session_private_deadline_diagnostic.py --run\n          native_rc=$?',
                     'if [ "$fake_rc" -ne 0 ] || [ "$native_rc" -ne 0 ]; then exit 1; fi'):
            self.assertIn(text, shard.PRIVATE_STEP)
        self.assertNotIn('--run-case', shard.PRIVATE_STEP)
        hashes = re.findall(r"^              'runtime/src/[^']+': '[a-f0-9]{64}',$", shard.SEND_STEP, re.M)
        self.assertEqual(len(hashes), 5)
        self.assertIn('3 passed; 0 failed; 0 ignored;', shard.SEND_STEP)
        for name in ('header_flush_before_buffered_frame_keeps_original_send_end',
                     'empty_response_completion_does_not_expire_idle_keep_alive',
                     'buffered_response_completion_does_not_expire_idle_keep_alive'):
            self.assertEqual(shard.SEND_STEP.count('"' + name + '"'), 1)
        self.assertIn('--cfg nagi_sf01_diag_oracle', shard.OBSERVER_STEP)
        self.assertIn('--cfg nagi_sf01_diagnostic --check-cfg=', shard.POLICY_STEP)
        self.assertIn('--test security_sf01 --test security_sf01_native', shard.POLICY_STEP)
        self.assertIn('"sf01-auth.log":9,"sf01-http.log":11', shard.LEASE_STEP)
        self.assertIn('"host-authority.log": 5, "host-http-entrypoint.log": 4', shard.HOST_STEP)
        self.assertNotIn('continue-on-error', shard.JOB)
        self.assertNotIn('strategy:', shard.JOB)
        self.assertNotIn('target/release', shard.JOB)

    def test_missing_duplicated_moved_and_weakened_controls_fail_closed(self):
        mutations = [self.source.replace(shard.JOB, '', 1),
                     self.source.replace(shard.JOB, shard.JOB * 2, 1),
                     self.source.replace(shard.JOB, '', 1) + shard.JOB]
        for old, new in (
            ('needs: [changes, release-plan, ci-plan]', 'needs: [changes, release-plan, ci-plan, nagi-package]'),
            ('runs-on: windows-latest', 'runs-on: ubuntu-latest'),
            ("package_checks == 'true'", "comprehensive_checks == 'true'"),
            ("needs.release-plan.result == 'success'", "needs.release-plan.result != 'failure'"),
            ('always() && !cancelled()', 'always()'),
            ('x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu'),
            ('cargo fetch --locked', 'cargo fetch'),
            ('ref: ${{ github.event.pull_request.head.sha || github.sha }}', 'ref: main'),
            ('persist-credentials: false', 'persist-credentials: true'),
            ("python-version: '3.12'", "python-version: '3.11'"),
            ('NAGI_FAILURE_DIR:', 'OTHER_FAILURE_DIR:'),
            ('NAGI_GOLDEN_CAPTURE_DIR:', 'OTHER_GOLDEN_CAPTURE_DIR:'),
            ('            native-target\n', ''),
            ('key: nagi-rust-v1-windows-contracts-', 'key: nagi-rust-v1-package-'),
            ('            nagi-rust-v1-package-', '            wrong-'),
            ('hashFiles(\'Cargo.lock\')', 'hashFiles(\'wrong.lock\')'),
            ('retention-days: 7', 'retention-days: 14'),
            ('name: compiler-failures-windows-contracts', 'name: compiler-failures-windows-x86_64'),
            ('path: build/compiler-failures/', 'path: build/other/'),
            ('if: failure()', 'if: success()'),
            ("success() && github.event_name == 'push'", "always() && github.event_name == 'push'"),
            ("github.event_name == 'push'", "github.event_name == 'pull_request'"),
            ("github.ref == 'refs/heads/main'", 'true'),
            ('timeout-minutes: 30', 'timeout-minutes: 60'),
            ("if: always() && runner.os == 'Windows'", "if: runner.os == 'Windows'"),
            (' || [ "$native_rc" -ne 0 ]', ''),
            ('0 failed; 0 ignored;', '0 failed; 1 ignored;'),
            ('--test security_sf01 --test security_sf01_native', '--test security_sf01'),
            ('--cfg nagi_sf01_diag_oracle', '--cfg other'),
        ):
            self.assertIn(old, shard.JOB)
            mutations.append(self.source.replace(shard.JOB, shard.JOB.replace(old, new, 1), 1))
        for step in (shard.SEND_STEP, shard.PRIVATE_STEP, *shard.COMMON_STEPS):
            mutations.append(self.source.replace(shard.JOB, shard.JOB.replace(step, '', 1), 1))
        for step in shard.COMMON_STEPS:
            guarded = shard.guarded(step)
            mutations.extend((self.source.replace(guarded, step, 1),
                              self.source.replace(guarded, '', 1),
                              self.source.replace(guarded, guarded * 2, 1),
                              self.source.replace(guarded, guarded.replace("!= 'Windows'", "== 'Windows'"), 1)))
        for new, old in ((shard.READY_NEW, shard.READY_OLD), (shard.PUBLISH_NEW, shard.PUBLISH_OLD)):
            mutations.append(self.source.replace(new, old, 1))
        for index, changed in enumerate(mutations):
            self.assertNotEqual(changed, self.source, index)
            with self.subTest(mutation=index), self.assertRaises((ValueError, AssertionError)):
                prove(changed)


class WindowsContractGateTests(unittest.TestCase):
    def test_handwritten_selected_and_skipped_profiles_are_strict(self):
        for level, full, package, expected in (
                ('1', 'true', 'true', 'skipped'), ('1', 'false', 'false', 'skipped'),
                ('2', 'true', 'true', 'success'), ('3', 'true', 'true', 'success'),
                ('2', 'false', 'true', 'success'), ('3', 'false', 'true', 'success'),
                ('3', 'false', 'false', 'skipped')):
            needs = test_levels.needs(level, full, test_levels.release(package_nagi=package))
            self.assertEqual(gate.errors(needs), [])
            for status in ('success', 'skipped', 'failure', 'cancelled', 'queued', '', None):
                changed = copy.deepcopy(needs)
                changed['windows-contracts']['result'] = status
                self.assertEqual(gate.errors(changed) == [], status == expected, (level, full, package, status))
            del needs['windows-contracts']
            self.assertTrue(gate.errors(needs))

    def test_package_failure_and_invalid_four_os_plan_never_pass(self):
        needs = test_levels.needs('3')
        self.assertEqual(needs['windows-contracts']['result'], 'success')
        for name in ('nagi-package', 'changes', 'ci-plan', 'release-plan'):
            for status in ('failure', 'cancelled', 'skipped', None):
                changed = copy.deepcopy(needs)
                changed[name]['result'] = status
                self.assertTrue(gate.errors(changed))
        import json
        for rows in ([], [row for row in test_levels.levels.PLATFORMS if row['os'] != 'windows-latest'],
                     test_levels.levels.PLATFORMS[:3]):
            changed = copy.deepcopy(needs)
            changed['ci-plan']['outputs']['matrix'] = json.dumps({'include': rows})
            self.assertTrue(gate.errors(changed))

    def test_actual_job_selector_rejects_failed_dependencies_and_cancellation(self):
        condition = test_publish_conditions.job_expression('windows-contracts')
        for name in ('changes', 'release-plan', 'ci-plan'):
            for status in ('success', 'failure', 'cancelled', 'skipped'):
                for selected in ('true', 'false'):
                    for cancelled in (True, False):
                        needs = {job: {'result': 'success'} for job in ('changes', 'release-plan', 'ci-plan')}
                        needs[name]['result'] = status
                        needs['ci-plan']['outputs'] = {'package_checks': selected}
                        context = {'needs': needs, 'cancelled': cancelled}
                        result = test_publish_conditions.evaluate(condition, context)
                        self.assertEqual(result, status == 'success' and selected == 'true' and not cancelled)

    def test_fictional_successful_ready_never_hides_missing_or_failed_windows_job(self):
        for component in ('nagi', 'vscode', 'jetbrains'):
            original = test_publication_gate.valid(component)
            for status in ('success', 'skipped', 'failure', 'cancelled', '', None):
                needs = copy.deepcopy(original)
                needs['windows-contracts']['result'] = status
                self.assertEqual(needs['ready']['result'], 'success')
                result = publication_gate.errors(needs, 'push', 'refs/heads/main', test_publication_gate.SHA)
                self.assertEqual(result == [], status == ('success' if component == 'nagi' else 'skipped'))
            del original['windows-contracts']
            self.assertTrue(publication_gate.errors(original, 'push', 'refs/heads/main', test_publication_gate.SHA))


if __name__ == '__main__':
    unittest.main()
