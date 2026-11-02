"""Once-per-platform relocation, unchanged commands, cache and closed gates."""
from collections import Counter
import copy
import hashlib
import json
from pathlib import Path
import re
import unittest

import gate
import levels
import levels_projection
import platform_contract_projection as projection
import publication_gate
import sf07_request_projection as historical
from sf07_projection_contract import BASE_WORKFLOW_SHA256
import test_levels
import test_publication_gate
import test_publish_conditions
import windows_contract_projection

ROOT = Path(__file__).resolve().parents[2]
COMMON = tuple(body.replace("        if: runner.os != 'Windows'\n", '')
               for body, _ in projection.MOVES[11:15])


def steps(block):
    return re.findall(r'(?ms)^      - .*?(?=^      - |\Z)', block)


def selected_steps(source, os_name, current):
    blocks = [projection.job(source, 'nagi-package').group()]
    if current:
        blocks.append(projection.job(source, 'platform-contracts').group())
    elif os_name == 'Windows':
        blocks.append(projection.job(source, 'windows-contracts').group())
    titles = {body.splitlines()[0] for body, _ in projection.MOVES}
    titles.update((windows_contract_projection.SEND_STEP.splitlines()[0],
                   windows_contract_projection.PRIVATE_STEP.splitlines()[0]))
    result = Counter()
    for block in blocks:
        for body in steps(block):
            if body.splitlines()[0] not in titles:
                continue
            if "        if: runner.os != 'Windows'\n" in body:
                if os_name == 'Windows':
                    continue
                body = body.replace("        if: runner.os != 'Windows'\n", '', 1)
            elif re.search(r"(?m)^        if: (?:always\(\) && )?runner.os == 'Windows'$", body):
                if os_name != 'Windows':
                    continue
            result[body] += 1
    return result


class PlatformContractWiringTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = (ROOT / '.github/workflows/ci.yml').read_text()
        cls.base = projection.inverse_workflow(cls.source)

    def test_exact_base_inverse_and_frozen_historical_proofs_compose(self):
        self.assertEqual(hashlib.sha256(self.base.encode()).hexdigest(), projection.BASE_WORKFLOW_SHA256)
        self.assertEqual(projection.apply_workflow(self.base), self.source)
        self.assertEqual(projection.inverse_workflow(self.base), self.base)
        self.assertEqual(hashlib.sha256(historical.inverse_workflow(self.source).encode()).hexdigest(),
                         BASE_WORKFLOW_SHA256)
        self.assertEqual(levels_projection.inverse_workflow(self.source),
                         levels_projection.inverse_workflow(self.base))

    def test_full_step_inventory_and_commands_are_identical_on_all_four_platforms(self):
        for os_name in ('Linux', 'Windows', 'macos-arm64', 'macos-x86_64'):
            with self.subTest(platform=os_name):
                before = selected_steps(self.base, os_name, False)
                after = selected_steps(self.source, os_name, True)
                self.assertEqual(before, after)
                self.assertTrue(all(count == 1 for count in after.values()))
                for body in COMMON:
                    self.assertEqual(after[body], 1)
        # These jobs and matrix authority were not changed by this optimization.
        for name in ('nagi-regression', 'ci-plan', 'release-plan', 'linux-workspace-shards'):
            self.assertEqual(projection.job(self.source, name).group(),
                             projection.job(self.base, name).group())
        package = projection.job(self.source, 'nagi-package').group()
        for marker in ('cargo build --release --locked -p nagic', 'Check demo packaging with Windows PowerShell',
                       'Verify an extracted distribution outside the checkout',
                       'Verify High, saved Low, and standalone Low applications',
                       'Verify the PowerShell installer and User PATH',
                       'id: sf07_checkpoint', 'id: b1_csrf_runtime', 'name: release-${{ matrix.platform }}'):
            self.assertIn(marker, package)
        for body, _ in projection.MOVES:
            self.assertNotIn(body.splitlines()[0] + '\n', package)

    def test_moved_body_loss_duplicates_order_guards_cfg_counts_and_failure_evidence_reject(self):
        job = projection.PLATFORM_JOB
        mutations = [self.source.replace(job, '', 1), self.source.replace(job, job * 2, 1)]
        for body, _ in projection.MOVES:
            body = body.replace("        if: runner.os != 'Windows'\n", '')
            self.assertEqual(job.count(body), 1)
            for replacement in ('', body * 2, body.replace('cargo test --locked', 'cargo test', 1)):
                if replacement != body:
                    mutations.append(self.source.replace(job, job.replace(body, replacement, 1), 1))
        first, second = [body for body, _ in projection.MOVES[:2]]
        mutations.append(self.source.replace(job, job.replace(first, '').replace(second, second + first, 1), 1))
        changes = (
            ("if: runner.os == 'Windows'", "if: runner.os != 'Windows'"),
            ("if: always() && runner.os == 'Windows'", "if: runner.os == 'Windows'"),
            ('timeout-minutes: 30', 'timeout-minutes: 10'),
            ('export TMP="$RUNNER_TEMP" TEMP="$RUNNER_TEMP"', ':'),
            ('--cfg nagi_auth_stage_diag --cfg nagi_browser_login_diag', '--cfg nagi_auth_stage_diag'),
            ('--cfg nagi_sf01_diag_oracle', '--cfg wrong_oracle'),
            ('2 passed; 0 failed; 0 ignored;', '1 passed; 0 failed; 0 ignored;'),
            ('3 passed; 0 failed; 0 ignored;', '2 passed; 0 failed; 0 ignored;'),
            ('"sf01-auth.log":9,"sf01-http.log":11', '"sf01-auth.log":8,"sf01-http.log":11'),
            ('"host-authority.log": 5, "host-http-entrypoint.log": 4', '"host-authority.log": 4, "host-http-entrypoint.log": 4'),
            ('"runtime-http-client": (222,', '"runtime-http-client": (221,'),
            ('if: always()', 'if: success()'),
            ('continue-on-error: true', 'continue-on-error: false'),
            ("steps.auth_stage_diag.outcome != 'skipped'", "steps.auth_stage_diag.outcome == 'success'"),
            ('${{ steps.sf06_public_client.outputs.closed_failure_diagnostic }}', '${{ runner.temp }}/wrong'),
            ('retention-days: 7 # ci-retention', 'retention-days: 1 # ci-retention'),
            ('name: compiler-failures-platform-contracts-${{ matrix.platform }}', 'name: compiler-failures-shared'),
            ('bb45b069b1bd8584ad19c80ce29b622759df4e2a91e28848e330ee4fba86581b', '0' * 64),
        )
        for old, new in changes:
            self.assertIn(old, job)
            mutations.append(self.source.replace(job, job.replace(old, new, 1), 1))
        for index, source in enumerate(mutations):
            with self.subTest(mutation=index), self.assertRaises(ValueError):
                projection.inverse_workflow(source)

    def test_original_package_duplicate_and_new_matrix_or_cache_mutation_reject(self):
        job = projection.PLATFORM_JOB
        package = projection.job(self.source, 'nagi-package').group()
        body = projection.MOVES[0][0]
        mutations = [self.source.replace(package, package + body, 1)]
        changes = (
            ('fail-fast: false', 'fail-fast: true'),
            ('matrix: ${{ fromJSON(needs.ci-plan.outputs.matrix) }}', 'matrix: {os: [windows-latest]}'),
            ('runs-on: ${{ matrix.os }}', 'runs-on: windows-latest'),
            ('RELEASE_RUST_HOST: ${{ matrix.rust_host }}', 'RELEASE_RUST_HOST: x86_64-pc-windows-msvc'),
            ('nagi-platform-contracts-v1-', 'nagi-rust-v1-package-'),
            ('-config-default-auth-browser-sf01-oracle-sf06-nodebug-noincremental-v1-', '-flags-default-'),
            ("hashFiles('Cargo.lock')", "hashFiles('Cargo.toml')"),
            ('${{ runner.arch }}', 'x64'),
            ('${{ steps.rust.outputs.cachekey }}', 'stable'),
            ("'scripts/ci/**', 'scripts/tests/**'", "'scripts/ci/levels.py'"),
            ('            native-target\n', ''),
            ("github.event_name == 'push' && github.ref == 'refs/heads/main'", "github.event_name == 'pull_request'"),
        )
        for old, new in changes:
            self.assertIn(old, job)
            mutations.append(self.source.replace(job, job.replace(old, new, 1), 1))
        mutations.append(self.source.replace(job, job.replace('      - uses: actions/setup-python@',
            '          restore-keys: nagi-rust-v1-package-\n      - uses: actions/setup-python@', 1), 1))
        for name in ('ready', 'publish-release'):
            block = projection.job(self.source, name).group()
            mutations.append(self.source.replace(block, block.replace(', platform-contracts', '', 1), 1))
        for index, source in enumerate(mutations):
            with self.subTest(mutation=index), self.assertRaises(ValueError):
                projection.inverse_workflow(source)
        self.assertNotIn('restore-keys:', projection.NEW_CACHE)
        for marker in ('${{ matrix.platform }}', '${{ runner.os }}', '${{ runner.arch }}',
                       '${{ steps.rust.outputs.cachekey }}', "hashFiles('Cargo.lock')", "hashFiles('.github/workflows/ci.yml'",
                       "'compiler/**', 'runtime/**', 'runtime-core/**'"):
            self.assertIn(marker, projection.NEW_CACHE)
        self.assertNotIn('CARGO_ENCODED_RUSTFLAGS:', job)

    def test_job_selector_and_current_aggregate_dependencies(self):
        condition = test_publish_conditions.job_expression('platform-contracts', current=True)
        for name in ('changes', 'release-plan', 'ci-plan'):
            for status in ('success', 'failure', 'cancelled', 'skipped', None):
                for selected in ('true', 'false'):
                    for cancelled in (False, True):
                        needs = {key: {'result': 'success'} for key in ('changes', 'release-plan', 'ci-plan')}
                        needs[name]['result'] = status
                        needs['ci-plan']['outputs'] = {'package_checks': selected}
                        actual = test_publish_conditions.evaluate(condition, dict(needs=needs, cancelled=cancelled))
                        self.assertEqual(actual, status == 'success' and selected == 'true' and not cancelled)
        for name in ('ready', 'publish-release'):
            block = projection.job(self.source, name).group()
            needs = re.search(r'(?m)^    needs: \[([^\n]+)\]$', block)[1].split(', ')
            self.assertEqual(needs.count('platform-contracts'), 1)
            self.assertNotIn('windows-contracts', needs)


class PlatformContractGateTests(unittest.TestCase):
    def test_selected_skipped_missing_failed_cancelled_partial_matrix_are_closed(self):
        for level, full, package in (('1','true','true'), ('1','false','false'), ('2','true','true'),
                                     ('2','false','true'), ('3','true','true'), ('3','false','false')):
            original = test_levels.needs(level, full, test_levels.release(package_nagi=package))
            expected = original['platform-contracts']['result']
            for result in ('success', 'skipped', 'failure', 'cancelled', 'partial', 'queued', '', None):
                changed = copy.deepcopy(original)
                changed['platform-contracts']['result'] = result
                self.assertEqual(gate.errors(changed) == [], result == expected)
            del original['platform-contracts']
            self.assertTrue(gate.errors(original))
        for level in ('2', '3'):
            original = test_levels.needs(level)
            rows = json.loads(original['ci-plan']['outputs']['matrix'])['include']
            for bad in (rows[:-1], rows + rows[:1], [], rows[::-1]):
                changed = copy.deepcopy(original)
                changed['ci-plan']['outputs']['matrix'] = json.dumps({'include': bad}, separators=(',', ':'))
                self.assertTrue(gate.errors(changed))

    def test_independent_publication_rejects_fictional_ready_with_failed_or_missing_job(self):
        for component in ('nagi', 'vscode', 'jetbrains'):
            original = test_publication_gate.valid(component)
            expected = original['platform-contracts']['result']
            for result in ('success', 'skipped', 'failure', 'cancelled', 'partial', '', None):
                changed = copy.deepcopy(original)
                changed['platform-contracts']['result'] = result
                self.assertEqual(changed['ready']['result'], 'success')
                self.assertEqual(publication_gate.errors(changed, 'push', 'refs/heads/main',
                    test_publication_gate.SHA) == [], result == expected)
            del original['platform-contracts']
            self.assertTrue(publication_gate.errors(original, 'push', 'refs/heads/main', test_publication_gate.SHA))


if __name__ == '__main__':
    unittest.main()
