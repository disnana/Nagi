"""Strict target/case reconciliation and exact relocation of inherited coverage."""
import copy
import contextlib
import io
import hashlib
import json
import os
from pathlib import Path
import re
import unittest
import tempfile
import subprocess
from unittest.mock import patch

import linux_critical_path as policy
import linux_critical_path_projection as projection
import platform_contract_projection
import platform_contract_profile_projection
import gate
import test_levels
import test_publication_gate
import publication_gate

ROOT = Path(__file__).resolve().parents[2]
# The outer inverse binds to main 0b21a1d, without later private Go changes.
BASE_SHA256 = '60a2323d878944d0697ee5319e92ddcd93586766d1391b070ed90075ce1bc58a'


def metadata():
    root = str(ROOT)
    packages = []
    for number in range(8):
        name = f'package-{number}'
        packages.append(dict(id=name, name=name, targets=[dict(kind=['test'], name=f'target_{number}', test=True, src_path=f'{root}/{name}/tests/target_{number}.rs')]))
    packages[0]['targets'] += [dict(kind=['lib'], name='lib_name', test=True, doctest=True, src_path=f'{root}/package-0/src/lib.rs'), dict(kind=['example'], name='example', test=False, src_path=f'{root}/package-0/examples/example.rs')]
    members = [row['id'] for row in packages]
    return dict(workspace_default_members=members, workspace_members=members, workspace_root=root, packages=packages)


def receipts(items, identity):
    result = []
    for index, group in enumerate(policy.assignments(items)):
        rows = []
        for target in group:
            row = dict(target=target, passed=0, command=policy.command(target), exit=0)
            if target['mode'] == 'test':
                row['binaries'] = [dict(member=member, cases=['case', 'ignored_case'], ignored=['ignored_case'], ignored_reasons={'ignored_case': None}, passed=1) for member in target['members']]
                row['passed'] = len(row['binaries'])
            elif target['mode'] == 'doc':
                row.update(summaries=[['1', '0']], ignored_total=0, failed_total=0, passed=1)
            rows.append(row)
        result.append(dict(schema=1, shard=index, context=identity, inventory=policy.digest(items), targets=rows))
    return result


def artifact(member):
    return json.dumps(dict(reason='compiler-artifact', profile=dict(test=True), target=dict(kind=[member['kind']], name=member['name'], src_path=str(ROOT / member['source'])), executable='/tmp/' + member['package'] + '-' + member['name']))


class WorkspaceInventoryTests(unittest.TestCase):
    def setUp(self):
        self.items = policy.inventory(metadata())
        self.identity = dict(head='a' * 40, toolchain='rustc stable exact\nhost: x86_64-unknown-linux-gnu\n', flags='default', lock='lock', workflow='workflow')
        self.receipts = receipts(self.items, self.identity)

    def test_complete_stable_balancing_and_change_order_never_remove_targets(self):
        groups = policy.assignments(self.items)
        self.assertEqual(sorted(row['id'] for group in groups for row in group), sorted(row['id'] for row in self.items))
        self.assertEqual(groups, policy.assignments(list(reversed(self.items))))
        for paths in (None, [], ['runtime/src/lib.rs'], ['compiler/tests/new.rs'], ['unknown'], ['Cargo.lock']):
            for group in groups:
                self.assertCountEqual(policy.ordered(group, paths), group)
        policy.reconcile(self.receipts, self.items, self.identity)
        self.assertIn('workspace/doc', [row['id'] for row in groups[0]])

    def test_workspace_commands_group_libraries_and_prove_selector_members(self):
        data = metadata()
        data['packages'][1]['targets'] += [dict(kind=['lib'], name='second_lib', test=True, doctest=False, src_path=str(ROOT / 'package-1/src/lib.rs')), dict(kind=['test'], name='target_0', test=True, src_path=str(ROOT / 'package-1/tests/target_0.rs'))]
        items = policy.inventory(data)
        libs = next(row for row in items if row['kind'] == 'lib')
        self.assertEqual(len(libs['members']), 2)
        self.assertEqual(policy.command(libs), ['cargo', 'test', '--locked', '--workspace', '--lib'])
        shared = next(row for row in items if row['name'] == 'target_0')
        self.assertEqual(len(shared['members']), 2)
        self.assertEqual(policy.command(shared), ['cargo', 'test', '--locked', '--workspace', '--test', 'target_0'])
        example = next(row for row in items if row['mode'] == 'build')
        self.assertEqual(policy.command(example), ['cargo', 'build', '--locked', '--workspace', '--profile', 'test', '--example', 'example'])
        self.assertEqual(policy.command(next(row for row in items if row['mode'] == 'doc')), ['cargo', 'test', '--locked', '--doc'])
        self.assertTrue(all('-p' not in policy.command(row) for row in items))
        compiled = '\n'.join(artifact(member) for member in shared['members'])
        binaries = policy.compiled_binaries(compiled, shared)
        self.assertEqual(set(binaries), {member['id'] for member in shared['members']})
        for changed in ('', compiled + '\n' + compiled, artifact(shared['members'][0])):
            with self.assertRaises(ValueError):
                policy.compiled_binaries(changed, shared)
        for mutation in ('mixed-mode', 'duplicate', 'feature', 'defaults', 'ambiguous-source'):
            changed = copy.deepcopy(data)
            if mutation == 'mixed-mode': changed['packages'][1]['targets'][-1]['test'] = False
            elif mutation == 'duplicate': changed['packages'][1]['targets'].append(changed['packages'][1]['targets'][0])
            elif mutation == 'feature': changed['packages'][0]['targets'][0]['required-features'] = ['future']
            elif mutation == 'ambiguous-source': changed['packages'][1]['targets'][-1]['src_path'] = changed['packages'][0]['targets'][0]['src_path']
            else: changed['workspace_default_members'] = changed['workspace_default_members'][:-1]
            with self.assertRaises(ValueError): policy.inventory(changed)

    def test_missing_duplicate_unknown_identity_and_zero_execution_fail_closed(self):
        mutants = [self.receipts[:-1], self.receipts + [self.receipts[0]]]
        for index, receipt in enumerate(self.receipts):
            for key, value in [('shard', 99), ('shard', True), ('schema', 0), ('context', {}), ('inventory', 'wrong'), ('targets', []), ('targets', receipt['targets'] * 2)]:
                changed = copy.deepcopy(self.receipts); changed[index][key] = value; mutants.append(changed)
            for row_index, row in enumerate(receipt['targets']):
                replacements = [('passed', True), ('exit', 1), ('exit', True), ('exit', None), ('command', ['true']), ('command', None)]
                if row['target']['mode'] == 'test':
                    replacements += [('passed', 0), ('binaries', []), ('binaries', row['binaries'] * 2)]
                    for key, value in [('cases', []), ('cases', ['case', 'case']), ('ignored', ['unlisted']), ('ignored', ['ignored_case', 'ignored_case']), ('ignored_reasons', None), ('ignored_reasons', {}), ('ignored_reasons', {'unlisted': 'reason'}), ('ignored_reasons', {'ignored_case': True}), ('passed', True)]:
                        changed = copy.deepcopy(self.receipts); changed[index]['targets'][row_index]['binaries'][0][key] = value; mutants.append(changed)
                elif row['target']['mode'] == 'doc':
                    replacements += [('passed', 0), ('summaries', []), ('summaries', [['0', '0']]), ('failed_total', 1), ('ignored_total', 99)]
                else:
                    replacements += [('passed', 1)]
                for key, value in replacements:
                    changed = copy.deepcopy(self.receipts); changed[index]['targets'][row_index][key] = value; mutants.append(changed)
                for key in ('command', 'exit'):
                    changed = copy.deepcopy(self.receipts); del changed[index]['targets'][row_index][key]; mutants.append(changed)
        for changed in mutants:
            with self.subTest(receipts=changed), self.assertRaises(ValueError):
                policy.reconcile(changed, self.items, self.identity)

    def test_normal_ignored_should_panic_and_failed_pretty_cases(self):
        member = next(row for row in self.items if row['mode'] == 'test')['members'][0]
        panic = 'stdlib::tests::descriptor_rejects_missing_role'
        normal = 'stdlib::tests::legacy_views_borrow_the_registered_descriptor'
        ignored = 'stdlib::tests::descriptor_rejects_overlapping_roles'
        source = (ROOT / 'compiler/src/stdlib.rs').read_text()
        self.assertIn('fn descriptor_rejects_missing_role()', source)
        self.assertIn('fn legacy_views_borrow_the_registered_descriptor()', source)
        output = f'running 3 tests\ntest {normal} ... ok\ntest {panic} - should panic ... ok\ntest {ignored} - should panic ... ignored, fixture reason\ntest result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s\n'
        cases = {normal, panic, ignored}
        events = f'ok {normal}\nok {panic}\nignored {ignored}\n'
        checked = policy.check_execution(output, events, cases, {ignored}, member)
        self.assertEqual(checked['ignored_reasons'], {ignored: None})
        for changed in (output.replace('2 passed', '0 passed'), output.replace('0 filtered out', '1 filtered out'), output + 'test result: FAILED.\n'):
            with self.assertRaises(ValueError): policy.check_execution(changed, events, cases, {ignored}, member)
        for changed in (events.replace(f'ok {normal}\n', ''), events + f'ok {normal}\n', events + 'ok unknown\n', events.replace(f'ok {panic}', f'failed {panic}'), events.replace(f'ignored {ignored}', f'ok {ignored}'), events + 'unexpected output\n'):
            with self.assertRaises(ValueError): policy.check_execution(output, changed, cases, {ignored}, member)
        with self.assertRaises(ValueError): policy.check_execution('test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n', '', set(), set(), member)
        marker = 'a' * 64
        blocks = policy.execution_blocks(f'NAGI_CI_OUTER_EXECUTION {marker}\n' + output, {member['id']: marker})
        self.assertEqual(blocks[member['id']].strip(), output.strip())
        for changed in (output, f'NAGI_CI_OUTER_EXECUTION {"b" * 64}\n' + output, f'NAGI_CI_OUTER_EXECUTION {marker}\n' * 2 + output):
            with self.assertRaises(ValueError): policy.execution_blocks(changed, {member['id']: marker})

    def test_nested_self_harness_names_and_summaries_are_not_outer_events(self):
        # graph_render's three process tests call current_exe --exact image_child
        # with inherited stdout. image_child is also a listed outer test.
        member = dict(id='nagic/test/graph_render')
        names = {'process_tests::image_child', 'process_tests::export', 'process_tests::missing'}
        nested = 'running 1 test\ntest process_tests::image_child ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n'
        output = 'running 3 tests\n' + nested * 2 + '\n'.join('test ' + name + ' ... ok' for name in sorted(names)) + '\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n'
        events = ''.join('ok ' + name + '\n' for name in sorted(names))
        policy.check_execution(output, events, names, set(), member)
        for changed in (events + 'ok process_tests::image_child\n', events.replace('ok process_tests::image_child\n', ''), events + 'ok extra\n'):
            with self.assertRaises(ValueError): policy.check_execution(output, changed, names, set(), member)
        with patch.object(policy, 'MAX_EVENT_BYTES', 2), self.assertRaises(ValueError):
            policy.check_execution(output, events, names, set(), member)

    def test_reasoned_ignored_events_require_a_unique_known_ignored_suffix(self):
        ignored = 'task::cost_tests::native_task_handle_costs'
        normal, panic = 'normal', 'expected_panic'
        reason = 'native cost measurement; run explicitly with --ignored --nocapture'
        line = f'ignored: {reason} {ignored}\n'
        events = f'ok {normal}\nok {panic}\n' + line
        output = 'test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s\n'
        cases = {normal, panic, ignored}
        member = dict(id='nagi-runtime/lib/nagi_runtime')
        checked = policy.check_execution(output, events, cases, {ignored}, member)
        self.assertEqual(checked['ignored_reasons'], {ignored: reason})
        for changed in (events + line, events.replace(line, ''), events.replace(ignored, 'unknown'),
                        events.replace(line, f'ignored: {reason} {normal}\n'),
                        events.replace(line, f'ignored: {ignored}\n'),
                        events.replace(line, f'ignored: {reason}{ignored}\n'),
                        events.replace(f'ok {panic}', f'failed {panic}'),
                        events.replace(f'ok {panic}', f'failed: test did not panic as expected at fixture.rs:1:1 {panic}'),
                        events.replace(f'ok {panic}', f'failed: panic did not contain expected string\n      panic message: "wrong"\n expected substring: "required" {panic}'),
                        events.replace(f'ok {panic}', f'ignored {panic}')):
            with self.subTest(events=changed), self.assertRaises(ValueError):
                policy.check_execution(output, changed, cases, {ignored}, member)
        # A generated test name can itself contain spaces. Never guess which
        # listed ignored suffix belongs to a reasoned event.
        with self.assertRaises(ValueError):
            policy.check_execution(output, 'ignored: reason long short\n',
                                   {'long short', 'short'}, {'long short', 'short'}, member)

    def test_ignored_only_target_preserves_plain_empty_and_unicode_reasons(self):
        names = {'plain', 'empty', 'unicode'}
        output = 'test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s\n'
        events = 'ignored plain\nignored:  empty\nignored: 理由: café  unicode\n'
        checked = policy.check_execution(output, events, names, names, dict(id='probe/test/ignored'))
        self.assertEqual(checked['passed'], 0)
        self.assertEqual(checked['ignored_reasons'], {'plain': None, 'empty': '', 'unicode': '理由: café '})
        # An ignored-only binary is valid inside a nonzero shard; its reason
        # metadata remains mandatory in aggregation without becoming passes.
        changed = copy.deepcopy(self.receipts)
        target = next(row for receipt in changed for row in receipt['targets'] if row['target']['mode'] == 'test')
        target['binaries'][0].update(checked)
        target['passed'] -= 1
        policy.reconcile(changed, self.items, self.identity)

    def test_cargo_runner_exec_preserves_arguments_cwd_and_original_environment(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binaries = {'package/lib/one': '/tmp/binary-one', 'other/lib/two': '/tmp/binary-two'}
            plan, environment = policy.execution_plan(binaries, root / 'execution', self.identity['toolchain'])
            self.assertEqual(len(set(row['events'] for row in plan['members'].values())), 2)
            cwd = os.getcwd()
            with patch.dict(os.environ, {**environment, 'NAGI_FAILURE_DIR': 'preserved'}), patch.object(policy.os, 'execve') as execute, contextlib.redirect_stdout(io.StringIO()):
                for member in binaries:
                    row = plan['members'][member]
                    policy.record_binary([row['executable'], '--format=pretty'])
                    args = execute.call_args.args
                    self.assertEqual(args[:2], (row['executable'], [row['executable'], '--format=pretty', '--logfile', row['events']]))
                    self.assertNotIn(policy.EXECUTION_ENV, args[2])
                    self.assertNotIn(plan['runner_env'], args[2])
                    self.assertEqual(args[2]['NAGI_FAILURE_DIR'], 'preserved')
                    with self.assertRaises(FileExistsError): policy.record_binary([row['executable'], '--format=pretty'])
                with self.assertRaises(ValueError): policy.record_binary(['/tmp/unknown', '--format=pretty'])
                with self.assertRaises(ValueError): policy.record_binary(['/tmp/binary-one', '--exact', 'one'])
            self.assertEqual(os.getcwd(), cwd)

    def test_assigned_only_runner_writes_complete_receipts_after_real_case_checks(self):
        seen = []
        summary = 'test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s\n'
        def fake_run(args, merge=False, extra_env=None):
            seen.append(args)
            if '--no-run' in args:
                item = next(row for row in self.items if policy.command(row) == args[:-2])
                return '\n'.join(artifact(member) for member in item['members'])
            if '--list' in args:
                return ('ignored_case: test\n1 test, 0 benchmarks\n' if '--ignored' in args else 'case: test\nignored_case: test\n2 tests, 0 benchmarks\n')
            if '--doc' in args: return summary
            if args[1] == 'build': return ''
            self.assertTrue(merge)
            item = next(row for row in self.items if policy.command(row) == args[:-2])
            plan = json.loads(Path(extra_env[policy.EXECUTION_ENV]).read_text())
            for row in plan['members'].values(): Path(row['events']).write_text('ok case\nignored ignored_case\n')
            return ''.join(f"NAGI_CI_OUTER_EXECUTION {plan['members'][member['id']]['marker']}\ntest case ... ok\ntest ignored_case ... ignored\n" + summary for member in item['members'])
        with tempfile.TemporaryDirectory() as directory, patch.object(policy, 'context', return_value=self.identity), patch.object(policy, 'workspace_inventory', return_value=self.items), patch.object(policy.levels, 'complete_paths', return_value=None), patch.object(policy, 'run', side_effect=fake_run), contextlib.redirect_stdout(io.StringIO()):
            destination = Path(directory)
            for index in range(policy.SHARDS): policy.execute(index, destination, destination / 'raw')
            result = [json.loads(path.read_text()) for path in destination.glob('*.json')]
            policy.reconcile(result, self.items, self.identity)
        builds = [args for args in seen if '--no-run' in args]
        self.assertEqual(len(builds), len([row for row in self.items if row['mode'] == 'test']))
        self.assertTrue(all('--workspace' in args and '-p' not in args for args in builds))
        self.assertEqual(sum('--doc' in args for args in seen), 1)

    def test_compiler_flag_variants_cannot_reuse_workspace_context(self):
        for flag in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTDOCFLAGS', 'CARGO_ENCODED_RUSTDOCFLAGS'):
            with patch.dict('os.environ', {flag: '--cfg other'}), self.assertRaises(ValueError): policy.context()

    def test_orchestration_log_keeps_successful_command_output_when_parser_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            completed = subprocess.CompletedProcess(['cargo', 'test'], 0, 'nested output retained\n', 'compiler stderr retained\n')
            def broken(*unused):
                policy.run(['cargo', 'test'])
                raise ValueError('deliberate parser failure')
            with patch.object(policy, '_execute', side_effect=broken), patch.object(policy.subprocess, 'run', return_value=completed), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaisesRegex(ValueError, 'deliberate parser failure'):
                    policy.execute(0, root / 'receipts', root / 'logs')
            raw = (root / 'logs/shard-0/orchestration.log').read_text()
            for text in ('nested output retained', 'compiler stderr retained', 'EXIT 0', 'deliberate parser failure'):
                self.assertIn(text, raw)
            self.assertFalse((root / 'receipts/shard-0.json').exists())


class CriticalPathWiringTests(unittest.TestCase):
    def setUp(self):
        current = platform_contract_profile_projection.inverse_workflow(
            (ROOT / '.github/workflows/ci.yml').read_text())
        self.source = platform_contract_projection.inverse_workflow(current)

    def test_exact_outer_inverse_preserves_all_predecessor_commands_and_gates(self):
        restored = projection.inverse_workflow(self.source)
        self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(), BASE_SHA256)
        self.assertEqual(self.source.count(projection.MUTANTS), 1)
        self.assertEqual(self.source.count(projection.SAMPLES), 1)
        self.assertIn('shard: [0, 1, 2, 3]', self.source)
        for marker in ('steps.auth_stage_diag.outcome', 'steps.session_deadline_diag.outcome', 'steps.browser_counter_comparison.outcome'):
            self.assertIn(marker, self.source.split('  linux-workspace-shards:')[0])

    def test_pip_cache_follows_requirements_install_and_inverse_restores_main(self):
        linux = self.source.split('  linux:\n', 1)[1].split('  linux-workspace-shards:\n', 1)[0]
        samples = self.source.split('  linux-samples:\n', 1)[1].split('  rust-1-88:\n', 1)[0]
        self.assertNotIn('cache: pip', linux)
        self.assertNotIn('cache-dependency-path: tests/requirements.txt', linux)
        self.assertEqual(self.source.count('cache: pip'), 1)
        self.assertEqual(samples.count('cache-dependency-path: tests/requirements.txt'), 1)
        self.assertIn('pip install -r tests/requirements.txt', samples)
        self.assertIn('nagi-linux-v2-samples-', samples)

        restored = projection.inverse_workflow(self.source)
        restored_linux = restored.split('  linux:\n', 1)[1].split('  linux-workspace:\n', 1)[0]
        self.assertIn(projection.LINUX_PYTHON_PIP_CACHE, restored_linux)
        self.assertIn('pip install -r tests/requirements.txt', restored_linux)
        self.assertNotIn('  linux-samples:\n', restored)
        self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(), BASE_SHA256)

    def test_workspace_failure_upload_keeps_orchestration_evidence_without_compiler_failures(self):
        shards = self.source.split('  linux-workspace-shards:\n', 1)[1].split('  linux-workspace:\n', 1)[0]
        step = shards.split('      - name: Preserve workspace compiler and application failure evidence\n', 1)[1].split('      - name:', 1)[0]
        self.assertIn('if: failure()', step)
        self.assertIn('build/compiler-failures/', step)
        self.assertIn('build/ci-evidence/linux-workspace-logs/', step)
        self.assertIn('if-no-files-found: error', step)

    def test_real_new_jobs_are_required_and_the_aggregate_refuses_failed_shards(self):
        aggregate = self.source.split('  linux-workspace:\n', 1)[1].split('  linux-csrf-mutants:\n', 1)[0]
        self.assertIn("always() && !cancelled()", aggregate)
        self.assertIn('run: test "$SHARD_RESULT" = success', aggregate)
        self.assertIn('--aggregate', aggregate)
        for level in ('1', '2', '3'):
            original = test_levels.needs(level)
            for name in ('linux-csrf-mutants', 'linux-samples', 'linux-workspace'):
                for state in ('failure', 'cancelled', 'queued', None):
                    changed = copy.deepcopy(original)
                    changed[name]['result'] = state
                    self.assertTrue(gate.errors(changed))
                changed = copy.deepcopy(original)
                del changed[name]
                self.assertTrue(gate.errors(changed))
        for component in ('nagi', 'vscode', 'jetbrains'):
            original = test_publication_gate.valid(component)
            self.assertEqual(publication_gate.errors(original, 'push', 'refs/heads/main', test_publication_gate.SHA), [])
            for name in ('linux-csrf-mutants', 'linux-samples'):
                changed = copy.deepcopy(original)
                del changed[name]
                self.assertTrue(publication_gate.errors(changed, 'push', 'refs/heads/main', test_publication_gate.SHA))

    def test_removed_skipped_cache_bypassed_or_weakened_wiring_is_rejected(self):
        mutations = []
        for old, new in [('shard: [0, 1, 2, 3]', 'shard: [0, 1, 2]'), ('--shard "$SHARD"', '--shard 0'), ('--aggregate', 'true'), ('run: test "$SHARD_RESULT" = success', 'run: true'), ('if-no-files-found: error', 'if-no-files-found: ignore'), ('flags-default-', 'flags-other-'), ("hashFiles('.github/workflows/ci.yml', 'scripts/ci/linux_critical_path*')", "hashFiles('Cargo.lock')"), ('fail-fast: false', 'fail-fast: true')]:
            # Mutate precisely the reviewed added block, not a predecessor job.
            block = projection.NEW_WORKSPACE
            self.assertIn(old, block)
            mutations.append(self.source.replace(block, block.replace(old, new, 1), 1))
        mutations += [self.source.replace(projection.NEW_WORKSPACE, '', 1), self.source.replace(projection.NEW_WORKSPACE, projection.NEW_WORKSPACE * 2, 1)]
        for new, old in projection.NEEDS:
            mutations.append(self.source.replace(new, old, 1))
        for source in mutations:
            with self.assertRaises(ValueError):
                projection.inverse_workflow(source)


if __name__ == '__main__':
    unittest.main()
