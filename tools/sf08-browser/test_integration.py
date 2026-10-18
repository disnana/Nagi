import base64
import fcntl
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
from types import SimpleNamespace as NS
import unittest
from unittest.mock import patch, Mock

import browser_coordinator as c
import ci_entrypoint as ci
import negative_endpoint as ne
import browser_budget as bb
import failure_checkpoint as fc

core = c.load('unit_protocol', c.HERE/'protocol.py')
certificate_fixture = c.load('unit_certificate_fixture', c.HERE/'context'/'fixture.py')
loaded = core.components({n:{'path':str(c.HERE/'context'/f'{n}.py')} for n in
    ('owned_json','owned_processes','report_reader','report_validator')})
OwnedRoot = loaded['owned_json'].OwnedRoot
SUMMARY = {'accepted':True,'cases':4,'projects':3,'executions':12}
PREFLIGHT = {'schema_version':1,'engines':3,'positive_sans':9,'wrong_ca_refusals':3,'wrong_host_refusals':3}


def facts(resets=12):
    return dict(schema_version=1, frontends_joined=True,http_joined=True,prelogin_closed=True,
        auth_closed=True,runtime_exit_zero=True,fixture_failed=False,resets=resets)


class Owner:
    def __init__(self, work, fail_browser=False, resets=12, closure=True, failure_stage='engine_wrong_ca',
                 browser_status=0, browser_report=None):
        self.work,self.fail_browser,self.resets,self.closure = work,fail_browser,resets,closure
        self.failure_stage = failure_stage
        self.browser_status,self.browser_report = browser_status,browser_report
        self.cancelled = False
        self.leaders = {}
        self.calls = []
        self.closed = 0
    def spawn(self, name, argv, **kwargs):
        self.calls.append((name,argv,kwargs))
        leader = NS(name=name,pid=123,returncode=None)
        self.leaders[name]=leader
        if name == 'fixture':
            put(self.work/'closed.json',facts(self.resets))
        elif name == 'negative':
            put(self.work/'negative.json',{'schema_version':1,'ca_origin':'https://localhost:11004',
                'ca_pem':base64.b64encode(b'other-public-ca').decode()})
        elif name == 'provision':
            put(self.work/'trust.json',dict(schema_version=1,home='/run/nagi-sf08/trust-unit/home',
                ca='/run/nagi-sf08/trust-unit/ca.pem',ca_sha256=c.hashlib.sha256(b'unit-public-ca').hexdigest(),
                firefox_policy='/run/nagi-sf08/trust-unit/firefox-policy.json',declared_image='unit-image',
                stores_provisioned=True,browser_trust_verified=False))
        elif name == 'browser':
            put(self.work/'preflight.json',PREFLIGHT)
            if self.browser_report is not None:
                put(self.work/'browser-report.json',self.browser_report)
            if self.fail_browser:
                put(self.work/'stage-failure.json',{'schema_version':1,'stage':self.failure_stage,'engine':'firefox'})
        return leader
    def peek(self, leader):
        return None if leader.name=='negative' else NS(si_code=os.CLD_EXITED,
            si_status=self.browser_status if leader.name=='browser' else 0)
    def close_all(self, **kwargs):
        self.closed+=1
        for leader in self.leaders.values(): leader.returncode=0
        return NS(closed=self.closure,echild_observed=self.closure)


def put(path,value):
    path.write_text(json.dumps(value)); path.chmod(0o600)


class CertificateRoleFixtureTests(unittest.TestCase):
    SUBJECTS = {
        'positive': '/CN=Nagi owned ephemeral SF08 positive CA',
        'wrong-ca': '/CN=Nagi owned ephemeral SF08 wrong-ca CA',
    }

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='unit-cert-',dir=os.environ.get('SF08_UNIT_ROOT','/tmp'))
        self.directory = Path(self.temp.name)
        self.addCleanup(self.temp.cleanup)

    def run_certificates(self, role):
        calls = []
        server = NS(set_alpn_protocols=Mock(), load_cert_chain=Mock())
        def capture(argv, **kwargs):
            calls.append((list(argv), dict(kwargs)))
        with patch.object(certificate_fixture.subprocess, 'run', side_effect=capture), \
             patch.object(certificate_fixture.ssl, 'SSLContext', return_value=server) as make_context:
            context = certificate_fixture.certificates(self.directory, role)
        make_context.assert_called_once_with(certificate_fixture.ssl.PROTOCOL_TLS_SERVER)
        return calls, server, context

    def test_positive_and_wrong_ca_roles_change_only_fixed_ca_subject(self):
        positive, positive_server, positive_context = self.run_certificates('positive')
        wrong, wrong_server, wrong_context = self.run_certificates('wrong-ca')
        self.assertEqual(len(positive), 3)
        self.assertEqual(len(wrong), 3)
        self.assertEqual(positive[0][0][positive[0][0].index('-subj') + 1], self.SUBJECTS['positive'])
        self.assertEqual(wrong[0][0][wrong[0][0].index('-subj') + 1], self.SUBJECTS['wrong-ca'])

        def normalized(calls):
            result = []
            for argv, kwargs in calls:
                if argv[1:3] == ['req', '-x509']:
                    argv = list(argv)
                    argv[argv.index('-subj') + 1] = '<closed-ca-role>'
                result.append((argv, kwargs))
            return result

        self.assertNotEqual(self.SUBJECTS['positive'], self.SUBJECTS['wrong-ca'])
        self.assertEqual(normalized(positive), normalized(wrong))
        self.assertIs(positive_context, positive_server)
        self.assertIs(wrong_context, wrong_server)
        self.assertEqual(positive_server.minimum_version, certificate_fixture.ssl.TLSVersion.TLSv1_2)
        self.assertEqual(wrong_server.minimum_version, certificate_fixture.ssl.TLSVersion.TLSv1_2)
        positive_server.set_alpn_protocols.assert_called_once_with(['http/1.1'])
        wrong_server.set_alpn_protocols.assert_called_once_with(['http/1.1'])
        positive_server.load_cert_chain.assert_called_once_with(self.directory/'leaf.pem', self.directory/'leaf.key')
        wrong_server.load_cert_chain.assert_called_once_with(self.directory/'leaf.pem', self.directory/'leaf.key')
        self.assertEqual((self.directory/'leaf.ext').read_text(),
            'basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\n'
            'extendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost,DNS:app.example.invalid,IP:127.0.0.1\n')

    def test_unknown_role_is_rejected_before_umask_or_openssl(self):
        with patch.object(certificate_fixture.os, 'umask') as umask, \
             patch.object(certificate_fixture.subprocess, 'run') as run:
            for role in ('Positive', 'positive ', 'wrong-ca-extra', None, 7):
                with self.subTest(role_type=type(role).__name__), self.assertRaisesRegex(ValueError, 'invalid certificate role'):
                    certificate_fixture.certificates(self.directory, role)
            with self.assertRaises(TypeError):
                certificate_fixture.certificates(self.directory)
            umask.assert_not_called()
            run.assert_not_called()
        self.assertEqual(list(self.directory.iterdir()), [])

    def test_supervisor_selects_positive_certificate_role(self):
        owned = self.directory/'owned'
        owned.mkdir()
        with patch.object(certificate_fixture.tempfile, 'mkdtemp', return_value=str(owned)), \
             patch.object(certificate_fixture, 'certificates', return_value=object()) as certificates:
            certificate_fixture.Supervisor('/unit/runtime', self.directory)
        certificates.assert_called_once_with(owned, 'positive')


class IntegrationTests(unittest.TestCase):
    def setUp(self):
        # Retained owned fake data only; no real CA/key/DB or input deletion.
        self.work=Path(tempfile.mkdtemp(prefix='unit-',dir=os.environ.get('SF08_UNIT_ROOT','/tmp')))

    def fake(self, fail_browser=False, bad_ack=False, resets=12, closure=True, cancel=False,
             failure_stage='engine_wrong_ca',browser_status=0,browser_report=None,wait_error=None):
        owner=Owner(self.work,fail_browser,resets,closure,failure_stage,browser_status,browser_report)
        deps={**loaded,'owned_processes':NS(LinuxProcessOwner=NS(activate_in_dedicated_cli=lambda:owner))}
        files={n:{'path':'/unit/'+n} for n in ('protocol','budget','provision','fixture','negative','provision_worker','stage','spec','playwright_module','playwright_cli')}
        binding={'files':files,'binary':{'path':'/unit/binary'},'node_modules':'/unit/node_modules'}
        ready={'app':'https://localhost:11001','ui':'https://app.example.invalid:11002',
            'denied':'https://127.0.0.1:11003','control':str(self.work/'control.sock')}
        provision=NS(check_scope=lambda:None,IMAGE='unit-image')
        def wait(_owner,leader,*args,**kwargs):
            if leader.name=='browser' and (fail_browser or cancel):
                owner.cancelled=cancel
                raise wait_error if wait_error is not None else ValueError('fixed unit failure')
        budget=NS(check=lambda *args,**kwargs:None,add_trust=lambda *_:None)
        resources=NS(BrowserBudget=lambda *_:budget)
        with patch.object(c,'check_bindings',return_value=binding), \
            patch.object(c,'load',side_effect=lambda n,p:core if n=='sf08_bound_protocol' else resources if n=='sf08_bound_budget' else provision), \
            patch.object(c.tempfile,'mkdtemp',return_value=str(self.work)), \
            patch.object(core,'components',return_value=deps), \
            patch.object(core,'wait_ready',return_value=ready),patch.object(core,'ready_ca_bytes',return_value=b'unit-public-ca'), \
            patch.object(core,'tls_probes'),patch.object(core,'wait_exit',side_effect=wait), \
            patch.object(core,'accept_browser_report',return_value=SUMMARY) as report, \
            patch.object(core,'stop_fixture',side_effect=ValueError('fixed') if bad_ack else None) as stop:
            result,exit_code=c.run(self.work/'bindings.json','0'*64)
        return result,exit_code,owner,stop,report

    def test_order_exact_fixture_cli_passfds_report12_and_globalwait_fake(self):
        r,exit_code,owner,stop,report=self.fake()
        self.assertEqual(exit_code,0)
        self.assertEqual([x[0] for x in owner.calls],['fixture','negative','provision','browser'])
        self.assertEqual(owner.calls[0][1][2:4],['--runtime','/unit/binary'])
        self.assertEqual(owner.calls[0][1][4:],['--work-dir',str(self.work)])
        self.assertEqual(len(owner.calls[1][2]['pass_fds']),1)
        self.assertTrue(r['actual_echild'] and r['preflight_verified'])
        self.assertEqual((r['browser_executions_accepted'],r['resets_accepted'],owner.closed,stop.call_count,report.call_count),(12,12,1,1,1))

    def test_js_python_failure_stage_whitelists_match_and_positive_stages_validate(self):
        source=(Path(__file__).resolve().parent/'browser_stage.mjs').read_text(encoding='utf-8')
        match=re.search(r'export const FAILURE_STAGES = \[(.*?)\];',source,re.S)
        self.assertIsNotNone(match)
        js_stages=set(re.findall(r"'([a-z_]+)'",match.group(1)))
        self.assertEqual(js_stages,fc.STAGES)
        positive={'engine_positive_ca','engine_positive_host','engine_positive_timeout',
                  'engine_positive_response','engine_positive_tls_unknown','engine_positive_unknown'}
        self.assertTrue(positive <= js_stages)
        for stage in positive:
            value={'schema_version':1,'stage':stage,'engine':'firefox'}
            self.assertEqual(fc.validate(value),value)
        with self.assertRaises(ValueError):
            fc.validate({'schema_version':1,'stage':'engine_positive_private_path','engine':'firefox'})

    def test_positive_failure_stage_survives_checkpoint_and_remains_failure(self):
        r,code,owner,stop,report=self.fake(fail_browser=True,failure_stage='engine_positive_timeout')
        self.assertEqual(code,1)
        self.assertEqual((r['failure_stage'],r['failure_engine']),('engine_positive_timeout','firefox'))
        self.assertTrue(r['actual_echild'])
        self.assertEqual((stop.call_count,owner.closed,report.call_count),(1,1,0))

    def test_browser_failure_and_cancel_do_not_accept_report_but_stop_and_wait(self):
        for kwargs in ({'fail_browser':True},{'cancel':True}):
            self.setUp()
            r,code,owner,stop,report=self.fake(**kwargs)
            self.assertEqual(code,1); self.assertTrue(r['actual_echild'])
            self.assertEqual((stop.call_count,owner.closed,report.call_count),(1,1,0))
            self.assertEqual(r['browser_executions_accepted'],0)
            if kwargs.get('fail_browser'):
                self.assertEqual((r['failure_stage'],r['failure_engine']),('engine_wrong_ca','firefox'))
                self.assertEqual(json.loads((self.work/'browser-failure.json').read_text())['engine'],'firefox')

        self.setUp()
        stage='engine_wrong_ca_diag_bad_signature_present'
        r,code,owner,stop,report=self.fake(fail_browser=True,failure_stage=stage)
        self.assertEqual((code,r['failure_stage'],r['failure_engine']),(1,stage,'firefox'))
        self.assertEqual((r['browser_executions_accepted'],r['resets_accepted']), (0,0))
        self.assertTrue(r['actual_echild'])
        self.assertEqual((stop.call_count,owner.closed,report.call_count),(1,1,0))

    def test_business_failure_projection_waits_for_browser_exit_and_echild(self):
        from test_r2_regressions import business_failure_report
        report_value=business_failure_report()
        report_projection=loaded['report_validator'].project_failure(report_value)
        record,code,owner,stop,accept=self.fake(fail_browser=True,browser_status=1,
            browser_report=report_value,wait_error=core.Rejected('child_exit'))
        self.assertEqual(code,1)
        self.assertTrue(record['actual_echild'])
        self.assertEqual((record['browser_executions_accepted'],record['resets_accepted']),(0,0))
        self.assertEqual(json.loads((self.work/'browser-report-diagnostic.json').read_text()),report_projection)
        self.assertNotIn('unit-private-sentinel',(self.work/'browser-report-diagnostic.json').read_text())
        self.assertEqual((stop.call_count,owner.closed,accept.call_count),(1,1,0))

        for kwargs in (
            {'fail_browser':True,'browser_status':1,'browser_report':report_value},
            {'fail_browser':True,'browser_status':1,'browser_report':report_value,
             'wait_error':core.Rejected('child_exit'),'closure':False},
            {'cancel':True,'browser_status':1,'browser_report':report_value},
        ):
            self.setUp()
            record,code,owner,stop,accept=self.fake(**kwargs)
            self.assertEqual(code,1)
            self.assertFalse((self.work/'browser-report-diagnostic.json').exists())
            self.assertEqual((record['browser_executions_accepted'],record['resets_accepted']),(0,0))

    def test_ci_browser_diagnostic_is_closed_and_attaches_only_to_nonzero_closed_browser_failure(self):
        from test_r2_regressions import business_failure_report
        good=loaded['report_validator'].project_failure(business_failure_report())
        reads=[]
        class SidecarRoot:
            def __init__(self,_path): pass
            def __enter__(self): return self
            def __exit__(self,*_args): pass
            def _read_json(self,name):
                reads.append(name)
                if name=='browser-report-diagnostic.json':
                    return self.value
                raise FileNotFoundError
        closure=NS(closed=True,echild_observed=True)
        outer={'schema_version':1,'phase':'browser','stage':'business','engine':'none'}
        failure={'class':'child_exit','status':1}
        root_holder=SidecarRoot
        def load(name,_path):
            if name=='sf08_ci_report_validator': return loaded['report_validator']
            if name=='owned_json': return NS(OwnedRoot=root_holder)
            raise AssertionError('unexpected dynamic module')
        with patch.object(ci.co,'load',side_effect=load):
            SidecarRoot.value=good
            self.assertEqual(ci.browser_report_diagnostic(failure,'browser',closure,outer,self.work),good)
            self.assertEqual(reads,['browser-report-diagnostic.json'])
            for rejected in (
                ({'class':'child_exit','status':0},'browser',closure,outer),
                ({'class':'child_exit','status':True},'browser',closure,outer),
                (failure,'build',closure,outer),
                (failure,'browser',NS(closed=False,echild_observed=False),outer),
                (failure,'browser',closure,{'schema_version':1,'phase':'tls','stage':'none','engine':'none'}),
            ):
                before=len(reads)
                self.assertIsNone(ci.browser_report_diagnostic(*rejected,self.work))
                self.assertEqual(len(reads),before)
            report_outer={'schema_version':1,'phase':'report','stage':'business','engine':'none'}
            self.assertEqual(ci.browser_report_diagnostic(failure,'browser',closure,report_outer,self.work),good)

            SidecarRoot.value={**good,'rows':[dict(good['rows'][0],count=True)] + good['rows'][1:]}
            self.assertEqual(ci.browser_report_diagnostic(failure,'browser',closure,outer,self.work),
                {'schema_version':1,'status':'unavailable','rows':[]})
            SidecarRoot.value={'schema_version':1,'status':'available','rows':[
                dict(good['rows'][0],extra='unit-private-sentinel')] + good['rows'][1:]}
            sanitized=ci.browser_report_diagnostic(failure,'browser',closure,outer,self.work)
            self.assertEqual(sanitized,{'schema_version':1,'status':'unavailable','rows':[]})
            self.assertNotIn('unit-private-sentinel',json.dumps(sanitized))

            SidecarRoot._read_json=lambda self,_name: (_ for _ in ()).throw(FileNotFoundError())
            self.assertEqual(ci.browser_report_diagnostic(failure,'browser',closure,outer,self.work),
                {'schema_version':1,'status':'unavailable','rows':[]})

    def test_bad_ack_is_not_retried_and_closed_is_not_pass(self):
        r,code,owner,stop,report=self.fake(bad_ack=True)
        self.assertEqual((code,stop.call_count,owner.closed),(1,1,1))
        self.assertFalse(r['stop_ack_verified'])

    def test_close_resets_and_global_echild_are_required(self):
        for kwargs in ({'resets':0},{'resets':11},{'resets':True},{'closure':False}):
            self.setUp()
            r,code,owner,stop,report=self.fake(**kwargs)
            self.assertEqual(code,1); self.assertEqual(r['browser_executions_accepted'],0)

    def test_guard_refusal_no_work_owner_or_spawn(self):
        provision=NS(check_scope=Mock(side_effect=ValueError('guard')))
        with patch.object(c,'check_bindings',return_value={'files':{'protocol':{'path':'x'},'budget':{'path':'b'},'provision':{'path':'y'}}}), \
             patch.object(c,'load',side_effect=[core,NS(),provision]), patch.object(core,'components',return_value={}), \
             patch.object(c.tempfile,'mkdtemp') as work:
            record,code=c.run(Path('/unit/file'),'0'*64)
        self.assertEqual((code,record['code']),(1,'scope')); work.assert_not_called()

    def test_binding_refusal_precedes_any_import_or_work(self):
        with patch.object(c,'check_bindings',side_effect=ValueError()),patch.object(c,'load') as load, \
             patch.object(c.tempfile,'mkdtemp') as work:
            record,code=c.run(Path('/unit/file'),'0'*64)
        self.assertEqual((code,record['code']),(1,'binding')); load.assert_not_called(); work.assert_not_called()

    def test_negative_metadata_different_ca_distinct_owned_port_and_exact_shape(self):
        ready={'app':'https://localhost:11001','ui':'https://app.example.invalid:11002','denied':'https://127.0.0.1:11003'}
        good={'schema_version':1,'ca_origin':'https://localhost:11004','ca_pem':base64.b64encode(b'other').decode()}
        self.assertEqual(c.negative_receipt(good,ready,b'main')['host_origin'],'https://sf08-wronghost.invalid:11001')
        for change in ({'schema_version':True},{'ca_origin':ready['app']},{'ca_origin':'https://localhost:80'},
                       {'ca_pem':'invalid?syntax'},{'ca_pem':base64.b64encode(b'main').decode()},{'extra':True}):
            with self.assertRaises((ValueError,c.Refused)): c.negative_receipt({**good,**change},ready,b'main')

    def test_existing_fifo_is_rejected_not_polled_forever(self):
        root=OwnedRoot(self.work)
        try:
            os.mkfifo(self.work/'negative.json',0o600)
            with self.assertRaises(ValueError):
                c.wait_object(root,'negative.json',core,NS(peek=lambda _:None),object(),NS(check=lambda *_:None),1)
        finally: root.close()

    def test_projection_canonical_only_actual_runtime_including_six_files(self):
        checkout=self.work/'checkout'; checkout.mkdir(mode=0o700)
        required=json.loads((c.HERE/'projection-required.json').read_bytes())
        for name in set(required):
            p=checkout/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(b'unit actual checkout')
        extra=checkout/'runtime/src/unit_new_current_runtime.rs';extra.write_bytes(b'unit current addition')
        before=ci.projection(checkout,self.work/'projection')
        self.assertEqual(set(before),set(required)|{'runtime/src/unit_new_current_runtime.rs'})
        self.assertEqual((self.work/'projection'/extra.relative_to(checkout)).read_bytes(),extra.read_bytes())
        self.assertEqual((self.work/'projection/runtime/src/auth.rs').read_bytes(),b'unit actual checkout')
        for name in ci.CANONICAL_FILES:
            self.assertEqual((self.work/'projection'/name).read_bytes(),(checkout/name).read_bytes())
        with self.assertRaises(FileExistsError): ci.projection(checkout,self.work/'projection')

    def test_negative_endpoint_reuses_cert_helper_and_actual_eof_contract_fake_server(self):
        read_fd, write_fd = os.pipe2(os.O_CLOEXEC)
        os.close(write_fd)
        closed = []
        class FakeServer:
            server_address = ('127.0.0.1',11004)
            def __enter__(self): return self
            def __exit__(self,*_): closed.append(True)
            def handle_request(self): raise AssertionError('EOF must stop before serving')
        certificate_roles=[]
        def fake_certificates(path, role):
            certificate_roles.append(role)
            (path/'ca.pem').write_bytes(b'unit-public-ca')
            return object()
        helper = NS(certificates=fake_certificates,
                    atomic_json=lambda path,value: put(path,value))
        with patch.object(ne,'load_fixture',return_value=helper),patch.object(ne,'Server',return_value=FakeServer()):
            ne.run(self.work,'/unit/fixture.py',read_fd)
        self.assertEqual(certificate_roles,['wrong-ca'])
        self.assertEqual(closed,[True])
        with self.assertRaises(OSError): os.fstat(read_fd)
        meta=json.loads((self.work/'negative.json').read_text())
        self.assertEqual(meta['ca_origin'],'https://localhost:11004')
        self.assertEqual(base64.b64decode(meta['ca_pem']),b'unit-public-ca')

    def test_native_tool_security_deadlines_and_r8_contract_source_unchanged(self):
        source=(c.HERE.parent/'sf08-fixture-runtime/src/main.rs').read_text()
        self.assertIn(
            'login::checked_for_origin(4096, 5000, 5000, 2000, app_origin)', source)
        helper=(c.HERE.parents[1]/'runtime/tests/support/browser_login.rs').read_text()
        self.assertRegex(helper, r'pub\(crate\) fn checked_for_origin\(\s*'
            r'body: i64,\s*body_ms: i64,\s*handler_ms: i64,\s*'
            r'shutdown_ms: i64,\s*app_origin: &str,\s*\) -> http::Options')
        self.assertIn(
            'response_rate_options(body, body_ms, handler_ms, shutdown_ms)', helper)
        self.assertIn('app_origin.strip_prefix(\"https://\").unwrap().to_owned()', helper)
        self.assertIn('http::with_response_limits(', helper)
        self.assertIn('response_limits(body)', helper)
        self.assertIn('http::rate_limits(64, 64, 1, 67_108_864, 65_536)', helper)
        self.assertIn('http::with_rate_limits(options, limits).unwrap()', helper)
        spec=(c.HERE/'business/security-foundation.browser.spec.ts').read_text()
        self.assertEqual(spec.count('test("'),4)
        self.assertIn('await resetFixture()',spec)
        self.assertIn('originalCookieSnapshot',spec)
        self.assertIn('original_prelogin_unexpired',spec)

    def test_ci_wait_failure_keeps_only_closed_waitid_facts(self):
        class WaitOwner:
            def __init__(self, status=None, cancelled=False, blocked=None):
                self.status,self.cancelled,self.peek_calls=status,cancelled,0
                self.blocked,self.drain_calls=blocked,0
            def peek(self, _leader):
                self.peek_calls+=1
                return self.status
            def drain_captured_stderr(self):
                self.drain_calls+=1
                return self.blocked

        cases=(
            (NS(si_code=os.CLD_EXITED,si_status=101),False,{'class':'child_exit','status':101}),
            (NS(si_code=os.CLD_KILLED,si_status=9),False,{'class':'child_signal','status':9}),
            (NS(si_code=os.CLD_DUMPED,si_status=11),False,{'class':'child_signal','status':11}),
            (NS(si_code=999,si_status=0),False,{'class':'unknown'}),
            (None,True,{'class':'cancelled'}),
        )
        for status,cancelled,expected in cases:
            owner=WaitOwner(status,cancelled)
            with patch.object(ci.time,'monotonic',side_effect=(0.0,0.01)):
                with self.assertRaises(Exception) as caught:
                    ci.await_zero(owner,NS(),1)
            self.assertEqual(type(caught.exception).__name__,'ChildWaitFailure')
            self.assertEqual(getattr(caught.exception,'detail',None),expected)
            self.assertEqual(owner.peek_calls,0 if cancelled else 1)
            self.assertEqual(owner.drain_calls,0 if cancelled else 1)

        owner=WaitOwner(NS(si_code=os.CLD_EXITED,si_status=0))
        with patch.object(ci.time,'monotonic',side_effect=(0.0,0.01)):
            self.assertIsNone(ci.await_zero(owner,NS(),1))

        owner=WaitOwner()
        with patch.object(ci.time,'monotonic',side_effect=(0.0,2.0)):
            with self.assertRaises(Exception) as caught:
                ci.await_zero(owner,NS(),1)
        self.assertEqual(type(caught.exception).__name__,'ChildWaitFailure')
        self.assertEqual(getattr(caught.exception,'detail',None),{'class':'timeout'})

        blocked=NS(name='build')
        owner=WaitOwner(blocked=blocked)
        with patch.object(ci.time,'monotonic',side_effect=(0.0,0.01)):
            with self.assertRaises(Exception) as caught: ci.await_zero(owner,NS(),1)
        self.assertEqual(getattr(caught.exception,'detail',None),{'class':'capture_refused'})
        self.assertEqual((owner.peek_calls,owner.drain_calls),(0,1))

    def test_cargo_error_codes_are_bounded_fixed_and_unknown_on_incomplete_input(self):
        cases=(
            (b'error[E0432]: fixed\nerror[E0599]: fixed\nerror[E0432]: duplicate\n',False,True,False,
             {'class':'rustc_error','rust_codes':['E0432','E0599']}),
            (b'error: linker refused',False,True,False,
             {'class':'unknown','rust_codes':[]}),
            (b'error[E12345]: invalid-width',False,True,False,
             {'class':'unknown','rust_codes':[]}),
            (b'error[E0432',False,True,False,
             {'class':'unknown','rust_codes':[]}),
            (b'error[E0432]: partial',True,True,False,
             {'class':'unknown','rust_codes':[]}),
            (b'error[E0432]: no-eof',False,False,False,
             {'class':'unknown','rust_codes':[]}),
            (b'\x1b[31merror[E0432]\x1b[0m',False,True,False,
             {'class':'unknown','rust_codes':[]}),
            (b'error[E0432]: invalid-\xff',False,True,False,
             {'class':'unknown','rust_codes':[]}),
            (b'error[E0432]: capture-error',False,True,True,
             {'class':'unknown','rust_codes':[]}),
            (b'x'*(ci.MAX_CARGO_DIAGNOSTIC_BYTES+1),False,True,False,
             {'class':'unknown','rust_codes':[]}),
            (b' '.join(f'error[E{i:04d}]'.encode() for i in range(9)),False,True,False,
             {'class':'unknown','rust_codes':[]}),
        )
        for sample,truncated,eof,error,expected in cases:
            self.assertEqual(ci.cargo_diagnostic(sample,truncated,eof,error),expected)

    def test_cargo_failure_classes_are_closed_and_never_echo_input(self):
        cases=(
            (b'error: the lock file /private/secret/Cargo.lock needs to be updated but --locked was passed to prevent this', 'lockfile_stale'),
            (b"current package believes it's in a workspace when it's not /private", 'manifest_or_workspace'),
            (b'error: failed to download from `https://private.invalid/token=secret`\nCaused by: Could not resolve proxy: private-secret', 'registry_or_transport'),
            (b'error: could not execute process `/opt/private/rustc -vV` (never executed)', 'toolchain'),
            (b'error: failed to run custom build command for `private-native`', 'native_toolchain_or_build_script'),
            (b'error: lock file /private/Cargo.lock needs to be updated but --locked was passed to prevent this\nfailed to download from https://private.invalid', 'unknown'),
        )
        secrets=('/private','private.invalid','token=secret','private-secret')
        for sample,expected in cases:
            diagnostic=ci.cargo_diagnostic(sample,False,True,False)
            self.assertEqual(diagnostic,{'class':expected,'rust_codes':[]})
            self.assertFalse(any(secret in json.dumps(diagnostic) for secret in secrets))

    def test_owner_capture_is_nonblocking_bounded_and_registered_before_popen(self):
        ops=loaded['owned_processes']
        self.assertEqual(ops.STDERR_CAPTURE_BYTES,ci.MAX_CARGO_DIAGNOSTIC_BYTES)
        owner=object.__new__(ops.LinuxProcessOwner)
        owner.pid=os.getpid();owner.leaders={};owner._pending_stderr=[];owner.closing=False;owner.cancelled=False;owner.fault=''
        payload=b'error[E0432]: unit-only diagnostic'
        seen=[]
        def fake_popen(_argv,**kwargs):
            leader=owner.leaders.get('build')
            self.assertIsNotNone(leader)
            self.assertTrue(leader.capture_stderr)
            self.assertIsNone(leader._process)
            self.assertTrue(fcntl.fcntl(leader._stderr_reader.fileno(),fcntl.F_GETFL)&os.O_NONBLOCK)
            self.assertFalse(fcntl.fcntl(kwargs['stderr'].fileno(),fcntl.F_GETFL)&os.O_NONBLOCK)
            seen.append(kwargs['stderr'].fileno())
            os.write(kwargs['stderr'].fileno(),payload)
            return NS(pid=123,returncode=None)
        with patch.object(ops.subprocess,'Popen',side_effect=fake_popen):
            leader=owner.spawn('build',['/unit/cargo'],capture_stderr=True)
        self.assertIs(owner.leaders['build'],leader)
        self.assertEqual(len(seen),1)
        with self.assertRaisesRegex(ops.OwnershipError,'^diagnostic_capture_scope$'):
            owner.spawn('other',['/unit/other'],capture_stderr=True)
        self.assertIsNone(owner.drain_captured_stderr())
        self.assertEqual(owner.stderr_capture(leader),(payload,False,True,False))
        owner._close_stderr();owner._close_stderr()
        self.assertIsNone(leader._stderr_reader)
        self.assertIsNone(leader._stderr_writer)

        ordinary=object.__new__(ops.LinuxProcessOwner)
        ordinary.pid=os.getpid();ordinary.leaders={};ordinary._pending_stderr=[];ordinary.closing=False;ordinary.cancelled=False;ordinary.fault=''
        ordinary_calls=[]
        def ordinary_popen(_argv,**kwargs):
            ordinary_calls.append(kwargs)
            return NS(pid=126,returncode=None)
        with patch.object(ops.subprocess,'Popen',side_effect=ordinary_popen):
            ordinary_leader=ordinary.spawn('npm',['/unit/npm'])
        self.assertFalse(ordinary_leader.capture_stderr)
        self.assertIs(ordinary_calls[0]['stderr'],ops.subprocess.DEVNULL)

        failed=object.__new__(ops.LinuxProcessOwner)
        failed.pid=os.getpid();failed.leaders={};failed._pending_stderr=[];failed.closing=False;failed.cancelled=False;failed.fault=''
        with patch.object(ops.subprocess,'Popen',side_effect=OSError('fixed')):
            with self.assertRaises(OSError): failed.spawn('build',['/unit/cargo'],capture_stderr=True)
        self.assertEqual(failed.leaders,{})

        post=object.__new__(ops.LinuxProcessOwner)
        post.pid=os.getpid();post.leaders={};post._pending_stderr=[];post.closing=False;post.cancelled=False;post.fault=''
        writers=[];first_close=[]
        def post_popen(_argv,**kwargs):
            self.assertIn('build',post.leaders)
            writers.append(kwargs['stderr'])
            return NS(pid=124,returncode=None)
        close_capture=ops._close_capture_stream
        def fail_one_close(stream):
            if writers and stream is writers[0] and not first_close:
                first_close.append(True)
                return False
            return close_capture(stream)
        with patch.object(ops.subprocess,'Popen',side_effect=post_popen), \
             patch.object(ops,'_close_capture_stream',side_effect=fail_one_close):
            tracked=post.spawn('build',['/unit/cargo'],capture_stderr=True)
            self.assertIs(post.drain_captured_stderr(),tracked)
            self.assertIs(post.leaders['build'],tracked)
            post._close_stderr()
        self.assertIsNone(tracked._stderr_writer)

    def test_failed_capture_setup_retains_bounded_stream_cleanup_ownership(self):
        ops=loaded['owned_processes']
        owner=object.__new__(ops.LinuxProcessOwner)
        owner.pid=os.getpid();owner.leaders={};owner._pending_stderr=[]
        owner.adopted={};owner.adopted_reaped=0
        owner.closing=False;owner.closed=False;owner.in_cleanup=False
        owner.cancelled=False;owner.fault=''
        streams=[]
        def refuse_close(stream):
            if not any(item is stream for item in streams): streams.append(stream)
            return False
        try:
            with patch.object(ops.subprocess,'Popen',side_effect=OSError('fixed')), \
                 patch.object(ops,'_close_capture_stream',side_effect=refuse_close):
                with self.assertRaises(OSError):
                    owner.spawn('build',['/unit/cargo'],capture_stderr=True)
                self.assertEqual(owner.leaders,{})
                self.assertEqual(len(owner._pending_stderr),2)
                self.assertTrue(all(not stream.closed for stream in streams))
                owner._close_stderr()
                self.assertEqual(len(owner._pending_stderr),2)
                with self.assertRaises(ops.OwnershipError):
                    owner.spawn('build',['/unit/cargo'],capture_stderr=True)
                with patch.object(owner,'_finish_groups',return_value=True), \
                     patch.object(owner,'_drain',return_value=True):
                    first=owner.close_all(timeout=1)
                self.assertTrue(first.closed and first.echild_observed)
                self.assertEqual(len(owner._pending_stderr),2)
            # Child closure is separate from stream cleanup: retry even after
            # the fake child ledger already observed ECHILD.
            second=owner.close_all(timeout=1)
            self.assertTrue(second.closed and second.echild_observed)
            self.assertEqual(owner._pending_stderr,[])
            self.assertTrue(all(stream.closed for stream in streams))
        finally:
            for stream in streams: stream.close()

    def test_owner_global_closure_drains_once_and_closes_capture_descriptors(self):
        ops=loaded['owned_processes']
        owner=object.__new__(ops.LinuxProcessOwner)
        owner.pid=os.getpid();owner.leaders={};owner._pending_stderr=[];owner.adopted={};owner.adopted_reaped=0
        owner.closing=False;owner.closed=False;owner.in_cleanup=False;owner.cancelled=False;owner.fault=''
        read_fd,write_fd=os.pipe2(os.O_CLOEXEC)
        flags=fcntl.fcntl(read_fd,fcntl.F_GETFL)
        fcntl.fcntl(read_fd,fcntl.F_SETFL,flags|os.O_NONBLOCK)
        reader=os.fdopen(read_fd,'rb',buffering=0)
        writer=os.fdopen(write_fd,'wb',buffering=0)
        payload=b'error[E0599]: final-buffer'
        writer.write(payload)
        writer.close()
        leader=ops.Leader('build',NS(pid=125,returncode=None),capture_stderr=True,
                          _stderr_reader=reader,_stderr_writer=None)
        owner.leaders['build']=leader
        with patch.object(owner,'_finish_groups',return_value=True),patch.object(owner,'_drain',return_value=True):
            closed=owner.close_all(timeout=1)
        self.assertTrue(closed.closed and closed.echild_observed)
        self.assertEqual(owner.stderr_capture(leader),(payload,False,True,False))
        self.assertIsNone(leader._stderr_reader)
        self.assertTrue(owner.close_all(timeout=1).closed)

    def test_owner_stop_failure_and_teardown_exception_close_capture_without_fake_echild(self):
        ops=loaded['owned_processes']
        owner=object.__new__(ops.LinuxProcessOwner)
        owner.pid=os.getpid();owner.leaders={};owner._pending_stderr=[];owner.adopted={};owner.adopted_reaped=0
        owner.closing=False;owner.closed=False;owner.in_cleanup=False;owner.cancelled=False;owner.fault=''
        read_fd,write_fd=os.pipe2(os.O_CLOEXEC|os.O_NONBLOCK)
        reader=os.fdopen(read_fd,'rb',buffering=0)
        writer=os.fdopen(write_fd,'wb',buffering=0)
        leader=ops.Leader('build',NS(pid=127,returncode=None),capture_stderr=True,
                          _stderr_reader=reader,_stderr_writer=writer)
        owner.leaders['build']=leader
        with patch.object(owner,'_finish_groups',return_value=False):
            self.assertFalse(owner.stop_group(leader,timeout=1))
        self.assertIs(leader._stderr_reader,reader)
        with patch.object(owner,'_finish_groups',side_effect=OSError('fixed')):
            closure=owner.close_all(timeout=1)
        self.assertFalse(closure.closed or closure.echild_observed)
        self.assertIsNone(leader._stderr_reader)
        self.assertIsNone(leader._stderr_writer)

    def test_close_all_retries_idempotent_capture_close_after_owner_closed(self):
        ops=loaded['owned_processes']
        owner=object.__new__(ops.LinuxProcessOwner)
        owner.pid=os.getpid();owner.leaders={};owner._pending_stderr=[];owner.adopted={};owner.adopted_reaped=0
        owner.closing=False;owner.closed=False;owner.in_cleanup=False;owner.cancelled=False;owner.fault=''
        read_fd,write_fd=os.pipe2(os.O_CLOEXEC|os.O_NONBLOCK)
        reader=os.fdopen(read_fd,'rb',buffering=0)
        os.close(write_fd)
        leader=ops.Leader('build',NS(pid=128,returncode=None),capture_stderr=True,
                          _stderr_reader=reader,_stderr_writer=None)
        owner.leaders['build']=leader
        close_capture=ops._close_capture_stream
        failed=[]
        def fail_first_close(stream):
            if stream is reader and not failed:
                failed.append(True)
                return False
            return close_capture(stream)
        with patch.object(owner,'_finish_groups',return_value=True), \
             patch.object(owner,'_drain',return_value=True), \
             patch.object(ops,'_close_capture_stream',side_effect=fail_first_close):
            first=owner.close_all(timeout=1)
            self.assertTrue(first.closed and first.echild_observed)
            self.assertIs(leader._stderr_reader,reader)
            second=owner.close_all(timeout=1)
        self.assertTrue(second.closed and second.echild_observed)
        self.assertIsNone(leader._stderr_reader)

    def test_ci_cancel_precedes_capture_drain_and_child_status(self):
        class CancelOwner:
            cancelled=True
            drain_calls=peek_calls=0
            def drain_captured_stderr(self): self.drain_calls+=1
            def peek(self,_leader): self.peek_calls+=1
        owner=CancelOwner()
        with patch.object(ci.time,'monotonic',side_effect=(0.0,0.01)):
            with self.assertRaises(Exception) as caught: ci.await_zero(owner,NS(),1)
        self.assertEqual(getattr(caught.exception,'detail',None),{'class':'cancelled'})
        self.assertEqual((owner.drain_calls,owner.peek_calls),(0,0))

    def test_owner_capture_poll_and_total_read_work_have_hard_limits(self):
        ops=loaded['owned_processes']
        owner=object.__new__(ops.LinuxProcessOwner)
        owner.pid=os.getpid();owner.leaders={};owner._pending_stderr=[];owner.closing=False;owner.cancelled=False;owner.fault=''
        leader=ops.Leader('build',None,capture_stderr=True,_stderr_reader=NS(fileno=lambda:99))
        owner.leaders['build']=leader
        requests=[]
        def flood(_fd,count):
            requests.append(count)
            return b'x'*count
        with patch.object(ops.os,'read',side_effect=flood):
            self.assertFalse(owner._drain_stderr(leader))
            self.assertEqual(len(requests),ops.STDERR_READS_PER_POLL)
            self.assertLessEqual(max(requests),ops.STDERR_READ_CHUNK)
            self.assertEqual(sum(requests),ops.STDERR_BYTES_PER_POLL)
            refused=None
            while refused is None:
                refused=owner.drain_captured_stderr()
        self.assertIs(refused,leader)
        self.assertEqual(leader._stderr_total,ops.STDERR_TOTAL_BYTES)
        self.assertTrue(leader._stderr_truncated and leader._stderr_error)

    def test_owner_capture_does_not_wait_for_inherited_writer_eof(self):
        ops=loaded['owned_processes']
        owner=object.__new__(ops.LinuxProcessOwner)
        owner.pid=os.getpid();owner.leaders={};owner._pending_stderr=[];owner.closing=False;owner.cancelled=False;owner.fault=''
        read_fd,write_fd=os.pipe2(os.O_CLOEXEC|os.O_NONBLOCK)
        reader=os.fdopen(read_fd,'rb',buffering=0)
        writer=os.fdopen(write_fd,'wb',buffering=0)
        leader=ops.Leader('build',None,capture_stderr=True,_stderr_reader=reader,_stderr_writer=writer)
        owner.leaders['build']=leader
        writer.write(b'one-buffered-chunk')
        self.assertFalse(owner._drain_stderr(leader))
        sample,truncated,eof,error=owner.stderr_capture(leader)
        self.assertEqual(sample,b'one-buffered-chunk')
        self.assertFalse(truncated or eof or error)
        owner._close_stderr()
        self.assertIsNone(leader._stderr_reader)
        self.assertIsNone(leader._stderr_writer)

    def test_ci_entrypoint_emits_failure_facts_without_changing_failure_or_closure(self):
        class RunOwner:
            cancelled=False
            def spawn(self,name,_argv,**_kwargs): return NS(name=name,capture_stderr=name=='build')
            def peek(self,leader):
                return NS(si_code=os.CLD_EXITED,si_status=101 if leader.name=='build' else 0)
            def drain_captured_stderr(self): return None
            def stderr_capture(self,_leader):
                return (b'error[E0432]: /private/unit https://unit.invalid token=unit-secret',False,True,False)
            def close_all(self,**_kwargs): return NS(closed=True,echild_observed=True)

        class MissingDiagnosticRoot:
            def __init__(self,_path): pass
            def __enter__(self): return self
            def __exit__(self,*_args): pass
            def _read_json(self,_name): raise FileNotFoundError

        owner=RunOwner()
        run_root=self.work/'ci-run';run_root.mkdir(mode=0o700)
        def load(name,_path):
            if name=='owned_processes':
                return NS(LinuxProcessOwner=NS(activate_in_dedicated_cli=lambda:owner))
            if name=='owned_json': return NS(OwnedRoot=MissingDiagnosticRoot)
            raise AssertionError('unexpected dynamic module')
        def project(_checkout,target):
            target.mkdir(mode=0o700)
            return {'unit':'fixed'}

        with patch.object(ci.provision,'check_scope'), \
             patch.object(ci.tempfile,'mkdtemp',return_value=str(run_root)), \
             patch.object(ci,'projection',side_effect=project), \
             patch.object(ci.co,'load',side_effect=load), \
             patch('builtins.print') as emitted:
            ci.run(self.work)
        record=json.loads(emitted.call_args.args[0])
        self.assertEqual(record['code'],'build')
        self.assertEqual(record['status'],'failed')
        self.assertEqual(record['failure'],{'class':'child_exit','status':101,
            'diagnostic':{'class':'rustc_error','rust_codes':['E0432']}})
        self.assertNotIn('/private/unit',emitted.call_args.args[0])
        self.assertNotIn('unit-secret',emitted.call_args.args[0])
        self.assertEqual((record['browser_executions_accepted'],record['resets_accepted']), (0,0))
        self.assertTrue(record['actual_echild'] and record['retain_data'])

    def test_ci_capture_limit_refuses_instead_of_succeeding(self):
        class RefusingOwner:
            cancelled=False
            drains=0
            build=None
            def spawn(self,name,_argv,**_kwargs):
                leader=NS(name=name,capture_stderr=name=='build')
                if name=='build': self.build=leader
                return leader
            def peek(self,leader):
                return NS(si_code=os.CLD_EXITED,si_status=0)
            def drain_captured_stderr(self):
                self.drains+=1
                return self.build if self.build is not None else None
            def stderr_capture(self,_leader): return (b'',True,False,True)
            def close_all(self,**_kwargs): return NS(closed=True,echild_observed=True)

        class MissingDiagnosticRoot:
            def __init__(self,_path): pass
            def __enter__(self): return self
            def __exit__(self,*_args): pass
            def _read_json(self,_name): raise FileNotFoundError

        owner=RefusingOwner()
        run_root=self.work/'ci-capture-refused';run_root.mkdir(mode=0o700)
        def load(name,_path):
            if name=='owned_processes':
                return NS(LinuxProcessOwner=NS(activate_in_dedicated_cli=lambda:owner))
            if name=='owned_json': return NS(OwnedRoot=MissingDiagnosticRoot)
            raise AssertionError('unexpected dynamic module')
        def project(_checkout,target):
            target.mkdir(mode=0o700)
            return {'unit':'fixed'}

        with patch.object(ci.provision,'check_scope'), \
             patch.object(ci.tempfile,'mkdtemp',return_value=str(run_root)), \
             patch.object(ci,'projection',side_effect=project), \
             patch.object(ci.co,'load',side_effect=load), \
             patch('builtins.print') as emitted:
            ci.run(self.work)
        record=json.loads(emitted.call_args.args[0])
        self.assertEqual(record['status'],'failed')
        self.assertEqual(record['code'],'build')
        self.assertEqual(record['failure'],{'class':'capture_refused',
            'diagnostic':{'class':'unknown','rust_codes':[]}})
        self.assertEqual((record['browser_executions_accepted'],record['resets_accepted']), (0,0))
        self.assertTrue(record['actual_echild'])


    def test_registered_js_python_stage_engine_cross_product_parity(self):
        source = (c.HERE / 'browser_stage.mjs').read_text(encoding='utf-8')
        stage_match = re.search(r'export const FAILURE_STAGES = \[(.*?)\];', source, re.S)
        engine_match = re.search(r'export const FAILURE_ENGINES = \[(.*?)\];', source, re.S)
        self.assertIsNotNone(stage_match)
        self.assertIsNotNone(engine_match)
        js_stages = set(re.findall(r"'([a-z_]+)'", stage_match.group(1)))
        js_engines = set(re.findall(r"'([a-z]+)'", engine_match.group(1)))
        self.assertEqual(js_stages, fc.STAGES)
        self.assertEqual(js_engines, fc.ENGINES)

        cases = [(stage, engine) for stage in sorted(js_stages | {'engine_wrong_ca_diag_unlisted'})
                 for engine in sorted(js_engines)]
        source_url = (c.HERE / 'browser_stage.mjs').as_uri()
        script = (
            'const {checkedFailure}=await import(' + json.dumps(source_url) + ');'
            'const cases=JSON.parse(process.argv[1]);'
            'const accepted=cases.map(([stage,engine])=>{try{checkedFailure(stage,engine);return true;}catch{return false;}});'
            'console.log(JSON.stringify(accepted));'
        )
        result = subprocess.run(['node', '--input-type=module', '-e', script, json.dumps(cases)],
                                check=True, capture_output=True, text=True)
        js_accepted = json.loads(result.stdout)
        py_accepted = []
        for stage, engine in cases:
            try:
                fc.validate({'schema_version': 1, 'stage': stage, 'engine': engine})
                py_accepted.append(True)
            except ValueError:
                py_accepted.append(False)
        self.assertEqual(js_accepted, py_accepted)
        for (stage, engine), js_ok in zip(cases, js_accepted):
            try:
                fc.validate({'schema_version': 1, 'stage': stage, 'engine': engine,
                             'phase': 'browser'}, outer=True)
                outer_ok = True
            except ValueError:
                outer_ok = False
            self.assertEqual(outer_ok, js_ok)

    def test_registered_python_checkpoint_exact_schema_and_no_input_echo(self):
        good = {'schema_version': 1, 'stage': 'engine_wrong_ca_diag_bad_signature_present',
                'engine': 'firefox'}
        self.assertEqual(fc.validate(good), good)
        for value in (dict(good, extra='PRIVATE_SENTINEL'),
                      dict(good, phase='browser'),
                      dict(good, path='https://private.invalid/?token=PRIVATE_SENTINEL'),
                      dict(good, message='PRIVATE_SENTINEL')):
            with self.assertRaisesRegex(ValueError, '^invalid failure checkpoint$') as caught:
                fc.validate(value)
            self.assertNotIn('PRIVATE_SENTINEL', str(caught.exception))

    def test_registered_diagnostic_checkpoints_are_firefox_only_and_fixed_shape(self):
        for stage in fc.FIREFOX_WRONG_CA_DIAGNOSTIC_STAGES:
            self.assertIn(stage, fc.STAGES)
            for engine in ('chromium', 'webkit', 'none'):
                with self.assertRaisesRegex(ValueError, '^invalid failure checkpoint$'):
                    fc.validate({'schema_version': 1, 'stage': stage, 'engine': engine})

    def test_registered_webkit_wrong_host_diagnostics_are_webkit_only(self):
        for stage in fc.WEBKIT_WRONG_HOST_DIAGNOSTIC_STAGES:
            self.assertIn(stage, fc.STAGES)
            good = {'schema_version': 1, 'stage': stage, 'engine': 'webkit'}
            self.assertEqual(fc.validate(good), good)
            for engine in ('chromium', 'firefox', 'none'):
                with self.assertRaisesRegex(ValueError, '^invalid failure checkpoint$') as caught:
                    fc.validate(dict(good, engine=engine))
                self.assertNotIn('PRIVATE_SENTINEL', str(caught.exception))


class ProvisionMountFlagsTests(unittest.TestCase):
    def check_scope_with_mount_flags(self, base_flags, duplicate_base=False):
        provision = ci.provision
        io = __import__('io')
        stat = __import__('stat')
        base_line = f'2 1 0:2 / {provision.BASE} {base_flags} - tmpfs tmpfs rw,size=4g\n'
        mountinfo = (
            '1 0 0:1 / / rw,relatime - overlay overlay rw\n'
            + base_line + (base_line if duplicate_base else '')
        ).encode()

        def fake_lstat(path):
            if str(path) == '/.dockerenv':
                return NS(st_mode=stat.S_IFREG | 0o600, st_uid=0)
            if str(path) == str(provision.BASE):
                return NS(st_mode=stat.S_IFDIR | 0o700, st_uid=0)
            raise AssertionError('unexpected lstat path')

        def fake_open(path, mode='r', *_args, **_kwargs):
            if path != '/proc/self/mountinfo' or mode != 'rb':
                raise AssertionError('unexpected file access')
            return io.BytesIO(mountinfo)

        env = {'GITHUB_ACTIONS': 'true', 'NAGI_SF08_EPHEMERAL': provision.IMAGE}
        with patch.object(provision.os, 'uname', return_value=NS(sysname='Linux')), \
             patch.object(provision.os, 'lstat', side_effect=fake_lstat), \
             patch.object(provision.os, 'geteuid', return_value=0), \
             patch.object(provision.os, 'environ', env), \
             patch.object(provision.Path, 'exists', return_value=False), \
             patch('builtins.open', side_effect=fake_open):
            provision.check_scope()

    def test_mountinfo_allows_exec_or_default_exec_mount(self):
        self.check_scope_with_mount_flags('rw,exec,nosuid,nodev,relatime')
        self.check_scope_with_mount_flags('rw,nosuid,nodev,relatime')

    def test_mountinfo_refuses_noexec_before_provisioning(self):
        with self.assertRaisesRegex(ci.provision.Refused, 'trust prerequisite refused'):
            self.check_scope_with_mount_flags('rw,nosuid,nodev,noexec,relatime')

    def test_mountinfo_refuses_ambiguous_or_malformed_exec_flags(self):
        for flags in ('rw,exec,noexec,nosuid,nodev', 'rw,nosuid,,nodev,relatime'):
            with self.subTest(flags=flags), self.assertRaises(ci.provision.Refused):
                self.check_scope_with_mount_flags(flags)
        with self.assertRaises(ci.provision.Refused):
            self.check_scope_with_mount_flags('rw,nosuid,nodev,relatime', duplicate_base=True)


if __name__=='__main__': unittest.main(verbosity=2)
