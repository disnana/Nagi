import ast,json,os,subprocess
from pathlib import Path
import tempfile,time
from types import SimpleNamespace as NS
import unittest
from unittest.mock import patch
import browser_coordinator as c
import browser_budget as b
import failure_checkpoint as f


def business_failure_report(secret='unit-private-sentinel'):
    from report_validator import PROJECTS, CASES
    project_ids = {name: 'unit-id-' + name for name in PROJECTS}
    projects = [{'name': name, 'id': project_ids[name], 'outputDir': 'private-output',
        'testDir': 'private-testdir', 'repeatEach': 1, 'retries': 0, 'timeout': 45000,
        'metadata': {}, 'testIgnore': [], 'testMatch': []} for name in PROJECTS]
    suites = []
    failed_pair = ('firefox', CASES[1])
    for project in PROJECTS:
        specs = []
        for case in CASES:
            failed = (project, case) == failed_pair
            error = {'message': secret, 'stack': 'private stack ' + secret,
                     'expected': secret, 'actual': secret,
                     'location': {'file': 'private path ' + secret, 'line': 999999999, 'column': True}}
            result = {'workerIndex': 0, 'parallelIndex': 0, 'duration': 1,
                'status': 'failed' if failed else 'passed', 'startTime': 'fixed-time',
                'retry': 0, 'errors': [error] if failed else [], 'stdout': [], 'stderr': [],
                'attachments': [], 'annotations': []}
            specs.append({'title': case, 'id': 'unit-spec', 'file': 'private-spec-path',
                'line': 1, 'column': 1, 'tags': [], 'ok': not failed,
                'tests': [{'timeout': 45000, 'annotations': [], 'expectedStatus': 'passed',
                    'status': 'unexpected' if failed else 'expected', 'projectName': project,
                    'projectId': project_ids[project], 'results': [result]}]})
        suites.append({'title': project, 'file': 'private-suite-path', 'line': 1,
                       'column': 1, 'specs': specs})
    return {'errors': [], 'stats': {'startTime': 'fixed-time', 'duration': 12,
            'expected': 11, 'unexpected': 1, 'flaky': 0, 'skipped': 0},
        'config': {'projects': projects}, 'suites': suites}


class R2Tests(unittest.TestCase):
    def test_temporary_wrong_host_format_suffix_is_failure_only_and_private(self):
        base = 'engine_wrong_host_one_shot_console_malformed_events_GRMNXSFWJKO_0_text_'+'a'*64
        for label in f.WRONG_HOST_CONSOLE_FORMATS:
            stage = base+'_fmt_'+label
            value = dict(schema_version=1, stage=stage, engine='webkit')
            self.assertEqual(f.validate(value), value)
            self.assertEqual(f.validate(dict(value, phase='browser'), outer=True), dict(value, phase='browser'))
            for engine in ('none', 'chromium', 'firefox'):
                with self.assertRaises(ValueError): f.validate(dict(value, engine=engine))
            for invalid in (stage+'\n', stage+'_extra', stage.replace('wrong_host', 'wrong_ca'),
                            stage.replace('GRMNXSFWJKO', 'GRMNSFWJKO'),
                            stage.replace('console_malformed', 'accepted')):
                with self.assertRaises(ValueError): f.validate(dict(value, stage=invalid))
        for label in ('', 'PRIVATE_SENTINEL', 'https://secret.invalid/', 'a'*193):
            with self.assertRaises(ValueError):
                f.validate(dict(schema_version=1, stage=base+'_fmt_'+label, engine='webkit'))

    def test_text_identity_suffix_is_failure_only_bounded_and_closed(self):
        digest = 'a'*64
        for role in ('ca', 'host'):
            base = f'engine_wrong_{role}_one_shot_console_malformed'
            stage = base+'_events_GRMNXJKO_0_text_'+digest
            value = dict(schema_version=1,stage=stage,engine='webkit')
            self.assertEqual(f.validate(value),value)
            self.assertEqual(f.validate(dict(value,phase='browser'),outer=True),dict(value,phase='browser'))
            for engine in ('none','chromium','firefox'):
                with self.assertRaises(ValueError): f.validate(dict(value,engine=engine))
            for invalid in (stage+'\n',stage+'_extra',stage.replace('GRMNXJKO','GRMNJKO'),
                stage.replace('console_malformed','accepted'),stage.replace('console_malformed','deadline'),
                stage.replace(digest,'A'*64),stage.replace(digest,'a'*63),
                stage.replace(digest,'PRIVATE_SENTINEL'),stage.replace('GRMNXJKO_0','X_1')):
                with self.assertRaisesRegex(ValueError,'^invalid failure checkpoint$'):
                    f.validate(dict(value,stage=invalid))

    def test_bounded_one_shot_event_suffix_is_strict_and_keeps_exact_schema(self):
        expected_codes = 'ETXLAZUIRMNGVSFBYWDPCJKHOQ'
        self.assertEqual(f.WEBKIT_EVENT_CODES, expected_codes)
        js = (c.HERE/'preflight.mjs').read_text()
        self.assertIn("export const WEBKIT_EVENT_CODES = '" + expected_codes + "'", js)
        for role in ('ca', 'host'):
            base = f'engine_wrong_{role}_one_shot_deadline'
            for trace in ('_0', 'GRMNSFWDJHO_0', 'GRMQNSFWDJHO_0', 'GRMNSFWXJKO_0', 'E'*32+'_1'):
                stage = base + '_events_' + trace
                value = dict(schema_version=1, stage=stage, engine='webkit')
                self.assertEqual(f.validate(value), value)
                self.assertEqual(f.validate(dict(value, phase='browser'), outer=True), dict(value, phase='browser'))
                for engine in ('none', 'chromium', 'firefox'):
                    with self.assertRaises(ValueError): f.validate(dict(value, engine=engine))
                with self.assertRaises(ValueError): f.validate(dict(value, extra='PRIVATE_SENTINEL'))
            invalid = [base+'_events_'+trace for trace in ('PRIVATE_SENTINEL_0', 'https://private.invalid/_0',
                'E'*33+'_0', 'E_1', '_1', 'E_2', 'E_00', 'E_0\n', 'E_0_suffix')]
            invalid += [f'engine_wrong_{role}_one_shot_accepted_events_E_0',
                f'engine_wrong_{role}_one_shot_peer_code_sha256_'+('a'*64)+'_events_E_0',
                f'engine_wrong_{role}_events_E_0']
            for stage in invalid:
                with self.assertRaisesRegex(ValueError, '^invalid failure checkpoint$'):
                    f.validate(dict(schema_version=1, stage=stage, engine='webkit'))

    def setUp(self):
        self.base=Path(tempfile.mkdtemp(prefix='sf08-bi-r2-unit-',dir=os.environ.get('SF08_UNIT_ROOT','/tmp')))
        self.work=self.base/'run-unit';self.work.mkdir(mode=0o700)

    def test_f1_checkout_credential_persistence_disabled(self):
        workflow=c.HERE.parents[1]/'.github/workflows/sf08-browser.yml'
        self.assertIn('      - uses: actions/checkout@v4\n        with:\n          persist-credentials: false\n',workflow.read_text())

    def test_f3_owned_filesystem_budget_no_declared_host_dependency(self):
        owner=NS(cancelled=False)
        budget=b.BrowserBudget(owner,self.work,self.base)
        seen=[]
        def free(path):
            self.assertEqual(Path(path),self.base);seen.append(path);return NS(free=b.FREE_FLOOR+1)
        with patch.object(b.shutil,'disk_usage',side_effect=free):budget.check(time.monotonic()+1)
        self.assertTrue(seen)
        text=(c.HERE/'protocol.py').read_text()
        for forbidden in ('/tmp/','/workspace'):
            self.assertNotIn(forbidden,text)
        self.assertFalse((c.HERE/'context/core.py').exists())
        self.assertFalse((c.HERE/'overlay').exists())

    def test_budget_free_floor_writecap_cancel_timeout_and_root_identity(self):
        owner=NS(cancelled=False);budget=b.BrowserBudget(owner,self.work,self.base)
        with patch.object(b.shutil,'disk_usage',return_value=NS(free=b.FREE_FLOOR-1)),self.assertRaisesRegex(b.ResourceRefused,'capacity'):
            budget.check(time.monotonic()+1)
        with patch.object(b.shutil,'disk_usage',return_value=NS(free=b.FREE_FLOOR+1)):
            data=self.work/'unit';data.write_bytes(b'12345')
            with patch.object(b,'WRITE_CAP',4),self.assertRaisesRegex(b.ResourceRefused,'capacity'):budget.check(time.monotonic()+1)
            owner.cancelled=True
            with self.assertRaisesRegex(b.ResourceRefused,'cancelled'):budget.check(time.monotonic()+1)
            budget.check(time.monotonic()+1,cleanup=True)
            with self.assertRaisesRegex(b.ResourceRefused,'timeout'):budget.check(time.monotonic()-1,cleanup=True)
            self.work.chmod(0o755)
            with self.assertRaisesRegex(b.ResourceRefused,'capacity'):budget.check(time.monotonic()+1,cleanup=True)

    def test_budget_counts_trust_and_symlinks_without_following(self):
        budget=b.BrowserBudget(NS(cancelled=False),self.work,self.base)
        trust=self.base/'trust-unit';trust.mkdir(mode=0o700)
        (trust/'owned-public-ca').write_bytes(b'unit')
        (self.work/'browser-profile-lock').symlink_to('/unavailable-outside-target')
        budget.add_trust(trust)
        self.assertGreaterEqual(budget.used(time.monotonic()+1),4+len('/unavailable-outside-target'))
        with self.assertRaisesRegex(b.ResourceRefused,'capacity'):budget.add_trust(trust)

    def test_failure_checkpoint_strict_schema_stage_engine_and_no_echo(self):
        good={'schema_version':1,'phase':'browser','stage':'engine_wrong_ca','engine':'firefox'}
        self.assertEqual(f.validate(good,outer=True),good)
        for change in ({'schema_version':True},{'stage':'https://secret.invalid/raw-cookie'},{'engine':'raw-token'},
                       {'phase':'raw-stack'},{'extra':'unit-secret'},{'stage':'business','engine':'firefox'}):
            with self.assertRaisesRegex(ValueError,'^invalid failure checkpoint$'):
                f.validate({**good,**change},outer=True)
        node=(c.HERE/'browser_stage.mjs').read_text()
        self.assertIn("stage-failure.json",node)
        self.assertIn('O_EXCL',node)
        for stage in f.STAGES:self.assertIn("'"+stage+"'",node)

    def test_one_shot_failure_categories_are_finite_webkit_only_and_not_acceptance(self):
        categories = {'credential', 'setup', 'navigation', 'request', 'response',
                      'console_missing', 'console_malformed',
                      'peer_legacy_certificate', 'peer_connection_reset', 'peer_handshake_timeout',
                      'peer_no_native_code', 'peer_non_error', 'peer_other_code', 'peer_other', 'peer_binding',
                      'socket', 'socket_eof', 'socket_close_unproved', 'socket_error_reset_raw',
                      'socket_error_reset_tls', 'socket_error_other', 'socket_destroy_error',
                      'lifecycle', 'deadline', 'join', 'cleanup', 'unknown'}
        prefix = 'engine_wrong_host_one_shot_'
        expected = {prefix + category for category in categories}
        self.assertEqual({stage for stage in f.STAGES if stage.startswith(prefix)}, expected)
        for stage in expected:
            value = {'schema_version': 1, 'stage': stage, 'engine': 'webkit'}
            self.assertEqual(f.validate(value), value)
            for engine in ('none', 'chromium', 'firefox'):
                with self.assertRaisesRegex(ValueError, '^invalid failure checkpoint$'):
                    f.validate({**value, 'engine': engine})
            with self.assertRaisesRegex(ValueError, '^invalid failure checkpoint$'):
                f.validate({**value, 'accepted': True})
        for suffix in ('', 'PRIVATE_SENTINEL', 'peer_other_extra', 'https://private.invalid/'):
            with self.assertRaisesRegex(ValueError, '^invalid failure checkpoint$') as caught:
                f.validate({'schema_version': 1, 'stage': prefix + suffix, 'engine': 'webkit'})
            self.assertNotIn('PRIVATE_SENTINEL', str(caught.exception))

    def test_wrong_ca_one_shot_categories_are_finite_webkit_only_and_not_acceptance(self):
        categories = {'credential', 'setup', 'navigation', 'request', 'response',
                      'console_missing', 'console_malformed',
                      'peer_legacy_certificate', 'peer_connection_reset', 'peer_handshake_timeout',
                      'peer_no_native_code', 'peer_non_error', 'peer_other_code', 'peer_other', 'peer_binding',
                      'socket', 'socket_eof', 'socket_close_unproved', 'socket_error_reset_raw',
                      'socket_error_reset_tls', 'socket_error_other', 'socket_destroy_error',
                      'lifecycle', 'deadline', 'join', 'cleanup', 'unknown'}
        prefix = 'engine_wrong_ca_one_shot_'
        expected = {prefix + category for category in categories}
        self.assertEqual({stage for stage in f.STAGES if stage.startswith(prefix)}, expected)
        node=(c.HERE/'browser_stage.mjs').read_text()
        for stage in expected:
            self.assertIn("'" + stage + "'", node)
            value = {'schema_version': 1, 'stage': stage, 'engine': 'webkit'}
            self.assertEqual(f.validate(value), value)
            for engine in ('none', 'chromium', 'firefox'):
                with self.assertRaisesRegex(ValueError, '^invalid failure checkpoint$'):
                    f.validate({**value, 'engine': engine})
            with self.assertRaisesRegex(ValueError, '^invalid failure checkpoint$'):
                f.validate({**value, 'accepted': True})
        for suffix in ('', 'PRIVATE_SENTINEL', 'peer_other_extra', 'https://private.invalid/'):
            with self.assertRaisesRegex(ValueError, '^invalid failure checkpoint$') as caught:
                f.validate({'schema_version': 1, 'stage': prefix + suffix, 'engine': 'webkit'})
            self.assertNotIn('PRIVATE_SENTINEL', str(caught.exception))

    def test_native_peer_fingerprint_checkpoints_are_exact_webkit_only(self):
        for role in ('ca', 'host'):
            prefix = f'engine_wrong_{role}_one_shot_peer_code_sha256_'
            value = {'schema_version': 1, 'stage': prefix + 'a' * 64, 'engine': 'webkit'}
            self.assertEqual(f.validate(value), value)
            self.assertNotIn(value['stage'], f.STAGES)
            for engine in ('none', 'chromium', 'firefox'):
                with self.assertRaisesRegex(ValueError, '^invalid failure checkpoint$'):
                    f.validate({**value, 'engine': engine})
            for suffix in ('', 'a' * 63, 'a' * 65, 'A' * 64, 'g' * 64,
                           'a' * 64 + '\n', 'a' * 64 + '\r', 'a' * 64 + '\u2028', 'a' * 64 + '\u2029', 'a' * 64 + '_PRIVATE_SENTINEL', 'https://private.invalid/'):
                with self.assertRaisesRegex(ValueError, '^invalid failure checkpoint$') as caught:
                    f.validate({**value, 'stage': prefix + suffix})
                self.assertNotIn('PRIVATE_SENTINEL', str(caught.exception))
            for extra in ({'accepted': True}, {'code': 'ERR_SSL_PRIVATE'}):
                with self.assertRaisesRegex(ValueError, '^invalid failure checkpoint$'):
                    f.validate({**value, **extra})
            for wrong_role in ('ca_extra', 'host_extra', 'other'):
                with self.assertRaisesRegex(ValueError, '^invalid failure checkpoint$'):
                    f.validate({**value, 'stage': f'engine_wrong_{wrong_role}_one_shot_peer_code_sha256_' + 'a' * 64})

    def test_webkit_witness_checkpoints_are_bounded_and_webkit_only(self):
        self.assertEqual(len(f.WEBKIT_WITNESS_DIAGNOSTICS), 25)
        self.assertEqual({value for value in f.WEBKIT_WITNESS_DIAGNOSTICS
                          if value.startswith('not_certificate_')}, {
            'not_certificate_empty','not_certificate_oversize','not_certificate_cancelled',
            'not_certificate_provisional_cancelled','not_certificate_certificate_form',
            'not_certificate_certificate_token','not_certificate_dns_token',
            'not_certificate_connection_token','not_certificate_cancel_token',
            'not_certificate_tls_token','not_certificate_multiple_tokens'})
        self.assertEqual(len(f.WEBKIT_WITNESS_STAGES),
                         len(f.WEBKIT_WITNESS_BASE_STAGES) * len(f.WEBKIT_WITNESS_DIAGNOSTICS))
        node=(c.HERE/'browser_stage.mjs').read_text()
        self.assertIn('WEBKIT_WITNESS_BASE_STAGES.flatMap(base =>',node)
        self.assertIn('WEBKIT_WITNESS_DIAGNOSTICS.map(diagnostic =>',node)
        script="""import { WEBKIT_WITNESS_STAGE_LIST, checkedFailure } from './browser_stage.mjs';
const engines=['none','chromium','firefox','webkit'];
const rows=WEBKIT_WITNESS_STAGE_LIST.flatMap(stage=>engines.map(engine=>{
  try { checkedFailure(stage,engine); return [stage,engine,true]; }
  catch { return [stage,engine,false]; }
}));
process.stdout.write(JSON.stringify({stages:WEBKIT_WITNESS_STAGE_LIST,rows}));"""
        result=subprocess.run(['node','--input-type=module','-e',script],cwd=c.HERE,
                              check=True,capture_output=True,text=True,timeout=10)
        generated=json.loads(result.stdout)
        self.assertEqual(set(generated['stages']),f.WEBKIT_WITNESS_STAGES)
        self.assertEqual(len(generated['stages']),len(f.WEBKIT_WITNESS_STAGES))
        expected={(stage,engine,engine=='webkit')
                  for stage in f.WEBKIT_WITNESS_STAGES
                  for engine in ('none','chromium','firefox','webkit')}
        self.assertEqual({tuple(row) for row in generated['rows']},expected)
        for stage in f.WEBKIT_WITNESS_STAGES:
            good={'schema_version':1,'stage':stage,'engine':'webkit'}
            self.assertEqual(f.validate(good),good)
            for engine in ('none','chromium','firefox'):
                with self.assertRaisesRegex(ValueError,'^invalid failure checkpoint$'):
                    f.validate({**good,'engine':engine})
        for stage in ('engine_wrong_host_webkit_cancelled_present_witness_PRIVATE_SENTINEL',
                      'engine_wrong_host_webkit_cancelled_present_witness_unknown',
                      'engine_wrong_host_webkit_cancelled_present_witness_missing_event_extra',
                      'engine_wrong_host_witness_missing_event'):
            with self.assertRaisesRegex(ValueError,'^invalid failure checkpoint$') as caught:
                f.validate({'schema_version':1,'stage':stage,'engine':'webkit'})
            self.assertNotIn('PRIVATE_SENTINEL',str(caught.exception))

    def test_failure_checkpoint_samefd_owned_regular_reader_rejects_fifo_mode_and_keys(self):
        import protocol
        module=protocol.components({n:{'path':str(c.HERE/'context'/f'{n}.py')} for n in
            ('owned_json','owned_processes','report_reader','report_validator')})
        with module['owned_json'].OwnedRoot(self.work) as root:
            target=self.work/'stage-failure.json'
            target.write_text(json.dumps({'schema_version':1,'stage':'business','engine':'none'}));target.chmod(0o600)
            self.assertEqual(f.validate(root._read_json('stage-failure.json'))['stage'],'business')
            target.chmod(0o644)
            with self.assertRaises(ValueError):root._read_json('stage-failure.json')
        fifo=self.base/'run-fifo';fifo.mkdir(mode=0o700)
        os.mkfifo(fifo/'stage-failure.json',0o600)
        with module['owned_json'].OwnedRoot(fifo) as root:
            with self.assertRaises(ValueError):root._read_json('stage-failure.json')

    def test_business_failure_projection_uses_owned_reader_and_redacts_error_text(self):
        import protocol
        module=protocol.components({n:{'path':str(c.HERE/'context'/f'{n}.py')} for n in
            ('owned_json','owned_processes','report_reader','report_validator')})
        expected={'schema_version':1,'status':'available','rows':[
            {'project_index':0,'case_index':0,'status':'passed','count':1},
            {'project_index':0,'case_index':1,'status':'passed','count':1},
            {'project_index':0,'case_index':2,'status':'passed','count':1},
            {'project_index':0,'case_index':3,'status':'passed','count':1},
            {'project_index':1,'case_index':0,'status':'passed','count':1},
            {'project_index':1,'case_index':1,'status':'failed','count':1},
            {'project_index':1,'case_index':2,'status':'passed','count':1},
            {'project_index':1,'case_index':3,'status':'passed','count':1},
            {'project_index':2,'case_index':0,'status':'passed','count':1},
            {'project_index':2,'case_index':1,'status':'passed','count':1},
            {'project_index':2,'case_index':2,'status':'passed','count':1},
            {'project_index':2,'case_index':3,'status':'passed','count':1}]}
        with module['owned_json'].OwnedRoot(self.work) as root:
            armed=module['report_reader'].FreshPlaywrightReport(root)
            target=self.work/'browser-report.json'
            target.write_text(json.dumps(business_failure_report()),encoding='utf8')
            target.chmod(0o600)
            observed=armed.failure_projection()
            self.assertEqual(observed,expected)
            self.assertNotIn('unit-private-sentinel',json.dumps(observed))
            self.assertEqual(armed.failure_projection(),{'schema_version':1,'status':'unavailable','rows':[]})

    def test_existing_owned_accept_path_keeps_exact_success_summary(self):
        import protocol
        module=protocol.components({n:{'path':str(c.HERE/'context'/f'{n}.py')} for n in
            ('owned_json','owned_processes','report_reader','report_validator')})
        report=business_failure_report()
        report['stats'].update(expected=12,unexpected=0)
        for suite in report['suites']:
            for spec in suite['specs']:
                spec['ok']=True
                test=spec['tests'][0]
                test['status']='expected'
                test['results'][0].update(status='passed',errors=[])
        with module['owned_json'].OwnedRoot(self.work) as root:
            armed=module['report_reader'].FreshPlaywrightReport(root)
            target=self.work/'browser-report.json'
            target.write_text(json.dumps(report),encoding='utf8');target.chmod(0o600)
            self.assertEqual(armed.accept(),{'accepted':True,'cases':4,'projects':3,'executions':12})
            self.assertEqual(armed.failure_projection(),{'schema_version':1,'status':'unavailable','rows':[]})

    def test_failure_projection_rejects_unknown_contradictory_and_pass_only_reports(self):
        import protocol
        module=protocol.components({n:{'path':str(c.HERE/'context'/f'{n}.py')} for n in
            ('owned_json','owned_processes','report_reader','report_validator')})
        validator=module['report_validator']
        good=business_failure_report()
        projected=validator.project_failure(good)
        self.assertEqual(validator.validate_failure_projection(projected),projected)

        unknown=business_failure_report()
        unknown['suites'][0]['specs'][0]['title']='private-unknown-case'
        duplicate=business_failure_report()
        duplicate['suites'][0]['specs'].append(dict(duplicate['suites'][0]['specs'][0]))
        contradictory=business_failure_report()
        contradictory['stats']['unexpected']=True
        pass_only=business_failure_report()
        pass_only['stats'].update(expected=12,unexpected=0)
        for suite in pass_only['suites']:
            for spec in suite['specs']:
                spec['ok']=True
                spec['tests'][0]['status']='expected'
                spec['tests'][0]['results'][0].update(status='passed',errors=[])
        for value in (unknown,duplicate,contradictory,pass_only,{'sentinel':'unit-private-sentinel'}):
            self.assertEqual(validator.project_failure(value),
                             {'schema_version':1,'status':'unavailable','rows':[]})

    def test_failure_projection_schema_rejects_bool_order_unknown_and_extra_fields(self):
        import protocol
        module=protocol.components({n:{'path':str(c.HERE/'context'/f'{n}.py')} for n in
            ('owned_json','owned_processes','report_reader','report_validator')})
        validator=module['report_validator']
        good=validator.project_failure(business_failure_report())
        mutants=[]
        for change in (
            lambda row: row.update(project_index=True),
            lambda row: row.update(case_index=False),
            lambda row: row.update(count=True),
            lambda row: row.update(status='private-unknown-status'),
            lambda row: row.update(extra='unit-private-sentinel'),
        ):
            mutant=json.loads(json.dumps(good));change(mutant['rows'][5]);mutants.append(mutant)
        reordered=json.loads(json.dumps(good));reordered['rows'][0],reordered['rows'][1]=reordered['rows'][1],reordered['rows'][0]
        mutants.append(reordered)
        for value in mutants:
            with self.assertRaises(validator.ReportValidationError):
                validator.validate_failure_projection(value)

    def test_failure_projection_reader_rejects_stale_missing_late_duplicate_oversize_and_replacement(self):
        import protocol
        module=protocol.components({n:{'path':str(c.HERE/'context'/f'{n}.py')} for n in
            ('owned_json','owned_processes','report_reader','report_validator')})
        report_bytes=json.dumps(business_failure_report()).encode()

        stale=self.work/'stale';stale.mkdir(mode=0o700)
        target=stale/'browser-report.json';target.write_bytes(report_bytes);target.chmod(0o600)
        with module['owned_json'].OwnedRoot(stale) as root:
            with self.assertRaises(ValueError):module['report_reader'].FreshPlaywrightReport(root)

        late=self.work/'late';late.mkdir(mode=0o700)
        with module['owned_json'].OwnedRoot(late) as root:
            armed=module['report_reader'].FreshPlaywrightReport(root)
            self.assertEqual(armed.failure_projection(),{'schema_version':1,'status':'unavailable','rows':[]})
            target=late/'browser-report.json';target.write_bytes(report_bytes);target.chmod(0o600)
            self.assertEqual(armed.failure_projection(),{'schema_version':1,'status':'unavailable','rows':[]})

        duplicate=self.work/'duplicate';duplicate.mkdir(mode=0o700)
        duplicate_bytes=report_bytes[:-1]+b',"errors":[]}'
        with module['owned_json'].OwnedRoot(duplicate) as root:
            armed=module['report_reader'].FreshPlaywrightReport(root)
            target=duplicate/'browser-report.json';target.write_bytes(duplicate_bytes);target.chmod(0o600)
            self.assertEqual(armed.failure_projection(),{'schema_version':1,'status':'unavailable','rows':[]})

        oversize=self.work/'oversize';oversize.mkdir(mode=0o700)
        with module['owned_json'].OwnedRoot(oversize) as root:
            armed=module['report_reader'].FreshPlaywrightReport(root)
            target=oversize/'browser-report.json';target.write_bytes(b'{}'+b' '*module['report_reader'].REPORT_LIMIT);target.chmod(0o600)
            self.assertEqual(armed.failure_projection(),{'schema_version':1,'status':'unavailable','rows':[]})

        replacement=self.work/'replacement';replacement.mkdir(mode=0o700)
        secret=self.base/'private-report-target';secret.write_bytes(report_bytes)
        with module['owned_json'].OwnedRoot(replacement) as root:
            armed=module['report_reader'].FreshPlaywrightReport(root)
            target=replacement/'browser-report.json';target.write_bytes(report_bytes);target.chmod(0o600)
            target.unlink();target.symlink_to(secret)
            self.assertEqual(armed.failure_projection(),{'schema_version':1,'status':'unavailable','rows':[]})

if __name__=='__main__':unittest.main(verbosity=2)
