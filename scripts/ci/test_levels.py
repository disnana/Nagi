"""Current policy, fail-closed selectors, and unchanged comprehensive command proof."""
import copy
import json
import re
import unittest
from pathlib import Path
from unittest.mock import patch
import levels
import gate
import levels_projection
import critical_inventory
from optimization_projection import unpin

ROOT = Path(__file__).resolve().parents[2]


def release(**overrides):
    result = {prefix + name: 'false' for prefix in ('package_', 'release_') for name in ('nagi', 'vscode', 'jetbrains')}
    result.update(overrides)
    return result


def needs(level='1', full='true', plan=None):
    plan = plan or release(package_nagi='true')
    selected = levels.expected(level, full, plan['package_nagi'])
    yes = lambda flag: {'result': 'success' if flag else 'skipped'}
    return {'ci-plan': {'result': 'success', 'outputs': selected},
            'changes': {'result': 'success', 'outputs': {'full_checks': full, 'jetbrains_checks': 'false'}},
            'release-plan': {'result': 'success', 'outputs': plan},
            'critical': yes(selected['critical_checks'] == 'true'),
            'linux': yes(selected['comprehensive_checks'] == 'true'),
            'rust-1-88': yes(selected['comprehensive_checks'] == 'true'),
            'nagi-package': yes(selected['package_checks'] == 'true'),
            'nagi-regression': yes(selected['package_checks'] == 'true'),
            'sf08-browser': yes(full == 'true' or plan['package_nagi'] == 'true' or selected['comprehensive_checks'] == 'true'),
            'jetbrains': yes(plan['package_jetbrains'] == 'true'),
            'vscode-package': yes(plan['package_vscode'] == 'true')}


class LevelsTests(unittest.TestCase):
    def test_recognized_pr_critical_and_unknown_security_dependency_escalation(self):
        for path in ['compiler/src/lexer.rs', 'compiler/src/parser.rs', 'compiler/tests/enum_parser.rs']:
            value = levels.select('pull_request', [path], 'true', release(package_nagi='true'))
            self.assertEqual((value['level'], value['critical_checks'], value['package_checks']), ('1', 'true', 'false'))
        for paths in [None, [], ['new/unknown'], ['Cargo.lock'], ['.github/workflows/ci.yml'],
                      ['runtime/src/auth/session.rs'], ['runtime/src/http_server.rs'], ['compiler/src/check/security.rs'],
                      ['compiler/src/parser.rs', 'unknown.rs'], ['compiler/../README.md'],
                      ['compiler/src/check.rs'], ['compiler/src/check/checked.rs'],
                      ['compiler/src/emit.rs'], ['compiler/src/lowering.rs'],
                      ['runtime/src/task.rs'], ['runtime/src/actor.rs'], ['runtime/src/owned.rs'],
                      ['compiler/src/new_unreviewed.rs']]:
            self.assertEqual(levels.select('pull_request', paths, 'true', release())['level'], '2')

    def test_main_schedule_and_formal_release_platforms(self):
        for event in ['push', 'schedule', 'workflow_dispatch', 'unfamiliar']:
            value = levels.select(event, ['compiler/src/parser.rs'], 'true', release())
            self.assertEqual(value['level'], '2')
            self.assertEqual(len(json.loads(value['matrix'])['include']), 3)
        for component in ['nagi', 'vscode', 'jetbrains']:
            value = levels.select('pull_request', ['README.md'], 'false', release(**{'release_' + component: 'true'}))
            self.assertEqual(value['level'], '3')
            self.assertEqual(len(json.loads(value['matrix'])['include']), 4)
        self.assertEqual(levels.select('workflow_dispatch', None, 'true', release(), 'release')['level'], '3')
        self.assertEqual(levels.select('pull_request', ['compiler/src/parser.rs'], 'true', release(), 'release')['level'], '1')

    def test_invalid_inputs_do_not_select_empty_or_optional_coverage(self):
        for bad in [None, '', 'TRUE', True, False]:
            for key in ['package_nagi', 'release_nagi', 'release_vscode', 'release_jetbrains']:
                with self.assertRaises(ValueError):levels.select('push', None, 'true', release(**{key: bad}))
        with self.assertRaises(ValueError):levels.select('pull_request', [], 'true', release(), 'skip')
        # Validate package independently even when full already selects Nagi.
        for bad in [None, '', 'TRUE', True, False]:
            for level in ['1', '2', '3']:
                with self.assertRaises(ValueError):levels.expected(level, 'true', bad)
            broken = needs('2')
            broken['release-plan']['outputs']['package_nagi'] = bad
            self.assertTrue(levels.validation_errors(broken))

    def test_gate_every_level_component_and_selected_job(self):
        for level in ['1', '2', '3']:
            good = needs(level)
            self.assertEqual(gate.errors(good), [])
            for name, job in good.items():
                for state in ['failure', 'cancelled', 'missing', 'skipped']:
                    if state == job['result']:continue
                    broken = copy.deepcopy(good)
                    if state == 'missing':del broken[name]
                    else:broken[name]['result'] = state
                    self.assertTrue(gate.errors(broken), (level, name, state))
        ide = needs('3', 'false', release(package_jetbrains='true', release_jetbrains='true'))
        self.assertEqual(gate.errors(ide), [])
        self.assertEqual(ide['nagi-package']['result'], 'skipped')

    def test_gate_rejects_matrix_level_boolean_and_release_drift(self):
        good = needs('3', plan=release(package_nagi='true', release_nagi='true'))
        for key, bad in [('level', '1'), ('matrix', '{"include":[]}'), ('matrix', json.dumps({'include':levels.PLATFORMS[:3]})),
                         ('critical_checks', 'true'), ('package_checks', 'false'), ('comprehensive_checks', None)]:
            broken=copy.deepcopy(good);broken['ci-plan']['outputs'][key]=bad
            self.assertTrue(gate.errors(broken))
        for name in ['nagi','vscode','jetbrains']:
            broken=copy.deepcopy(good);del broken['release-plan']['outputs']['release_'+name]
            self.assertTrue(gate.errors(broken))

    def test_missing_or_malformed_git_range_escalates(self):
        self.assertIsNone(levels.complete_paths('', 'a'*40))
        with patch('changes.git', side_effect=OSError('unavailable')):
            self.assertIsNone(levels.complete_paths('b'*40, 'a'*40))

    def test_critical_cache_binds_key_and_restore_prefix_to_rust_toolchain(self):
        source=(ROOT/'.github/workflows/ci.yml').read_text()
        critical=unpin(re.search(r'^  critical:\n.*?(?=^  [a-z][a-z0-9-]*:)', source, re.M | re.S).group())
        self.assertIn('      - uses: dtolnay/rust-toolchain@stable\n        id: rust\n', critical)
        prefix = "nagi-critical-v1-${{ runner.os }}-${{ runner.arch }}-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('Cargo.lock') }}"
        self.assertIn('key: ' + prefix + '-', critical)
        self.assertIn('restore-keys: ' + prefix + '-', critical)

    def test_critical_inventory_requires_all_targets_and_nonempty_unique_names(self):
        artifact = lambda name: dict(reason='compiler-artifact', profile={'test':True}, target={'name':name}, executable='/tmp/'+name)
        text = json.dumps(artifact('nagic'))
        self.assertEqual(critical_inventory.binaries(text, {'nagic'}), {'nagic':'/tmp/nagic'})
        for value in ['', text+'\n'+text, json.dumps(artifact('other'))]:
            with self.assertRaises(ValueError):critical_inventory.binaries(value, {'nagic'})
        self.assertEqual(critical_inventory.cases('a: test\nb: test\n'), ['a','b'])
        for value in ['', 'a: test\na: test\n', 'unknown']:
            with self.assertRaises(ValueError):critical_inventory.cases(value)

    def test_policy_inverse_preserves_original_commands_and_rejects_mutants(self):
        source=(ROOT/'.github/workflows/ci.yml').read_text()
        restored=levels_projection.inverse_workflow(source)
        self.assertNotIn('\n  ci-plan:\n', restored)
        self.assertEqual(restored.count('          - os: macos-15-intel\n'),2)
        self.assertIn("    if: github.event_name == 'pull_request' && always()",restored)
        for original, bad in [
            ('    needs: [changes, ci-plan, critical, linux,', '    needs: [changes, linux,'),
            ('      matrix: ${{ fromJSON(needs.ci-plan.outputs.matrix) }}', '      matrix: {}'),
            ("    name: Ready to merge\n    if: always() && !cancelled()\n", "    name: Ready to merge\n    if: false\n"),
            ('      - run: cargo test --locked --doc', '      - run: true'),
            ('  ci-plan:\n','  no-plan:\n')]:
            mutated = source.replace(original, bad, 1)
            self.assertNotEqual(mutated, source, 'mutation anchor must match the current workflow')
            with self.assertRaises(ValueError):levels_projection.inverse_workflow(mutated)
        # Unchanged original command drift remains visible to the existing historical guard.
        mutated=source.replace('cargo test --locked -p nagi-runtime --lib auth::ed25519::', 'cargo test --locked -p nagi-runtime --lib typo::',1)
        self.assertNotEqual(levels_projection.inverse_workflow(mutated),restored)

if __name__ == '__main__':unittest.main()
