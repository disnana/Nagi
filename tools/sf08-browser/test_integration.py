import base64
import json
import os
from pathlib import Path
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
loaded = core.components({n:{'path':str(c.HERE/'context'/f'{n}.py')} for n in
    ('owned_json','owned_processes','report_reader','report_validator')})
OwnedRoot = loaded['owned_json'].OwnedRoot
SUMMARY = {'accepted':True,'cases':4,'projects':3,'executions':12}
PREFLIGHT = {'schema_version':1,'engines':3,'positive_sans':9,'wrong_ca_refusals':3,'wrong_host_refusals':3}


def facts(resets=12):
    return dict(schema_version=1, frontends_joined=True,http_joined=True,prelogin_closed=True,
        auth_closed=True,runtime_exit_zero=True,fixture_failed=False,resets=resets)


class Owner:
    def __init__(self, work, fail_browser=False, resets=12, closure=True):
        self.work,self.fail_browser,self.resets,self.closure = work,fail_browser,resets,closure
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
            if self.fail_browser:
                put(self.work/'stage-failure.json',{'schema_version':1,'stage':'engine_wrong_ca','engine':'firefox'})
        return leader
    def peek(self, leader):
        return None if leader.name=='negative' else NS(si_code=os.CLD_EXITED,si_status=0)
    def close_all(self, **kwargs):
        self.closed+=1
        for leader in self.leaders.values(): leader.returncode=0
        return NS(closed=self.closure,echild_observed=self.closure)


def put(path,value):
    path.write_text(json.dumps(value)); path.chmod(0o600)


class IntegrationTests(unittest.TestCase):
    def setUp(self):
        # Retained owned fake data only; no real CA/key/DB or input deletion.
        self.work=Path(tempfile.mkdtemp(prefix='unit-',dir=os.environ.get('SF08_UNIT_ROOT','/tmp')))

    def fake(self, fail_browser=False, bad_ack=False, resets=12, closure=True, cancel=False):
        owner=Owner(self.work,fail_browser,resets,closure)
        deps={**loaded,'owned_processes':NS(LinuxProcessOwner=NS(activate_in_dedicated_cli=lambda:owner))}
        files={n:{'path':'/unit/'+n} for n in ('protocol','budget','provision','fixture','negative','provision_worker','stage','spec','playwright_module','playwright_cli')}
        binding={'files':files,'binary':{'path':'/unit/binary'},'node_modules':'/unit/node_modules'}
        ready={'app':'https://localhost:11001','ui':'https://app.example.invalid:11002',
            'denied':'https://127.0.0.1:11003','control':str(self.work/'control.sock')}
        provision=NS(check_scope=lambda:None,IMAGE='unit-image')
        def wait(_owner,leader,*args,**kwargs):
            if leader.name=='browser' and (fail_browser or cancel):
                owner.cancelled=cancel
                raise ValueError('fixed unit failure')
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
        helper = NS(certificates=lambda path: (path/'ca.pem').write_bytes(b'unit-public-ca'),
                    atomic_json=lambda path,value: put(path,value))
        with patch.object(ne,'load_fixture',return_value=helper),patch.object(ne,'Server',return_value=FakeServer()):
            ne.run(self.work,'/unit/fixture.py',read_fd)
        self.assertEqual(closed,[True])
        with self.assertRaises(OSError): os.fstat(read_fd)
        meta=json.loads((self.work/'negative.json').read_text())
        self.assertEqual(meta['ca_origin'],'https://localhost:11004')
        self.assertEqual(base64.b64decode(meta['ca_pem']),b'unit-public-ca')

    def test_native_tool_security_deadlines_and_r8_contract_source_unchanged(self):
        source=(c.HERE.parent/'sf08-fixture-runtime/src/main.rs').read_text()
        self.assertIn('http::options(4096, 5000, 5000, 2000)',source)
        spec=(c.HERE/'business/security-foundation.browser.spec.ts').read_text()
        self.assertEqual(spec.count('test("'),4)
        self.assertIn('await resetFixture()',spec)
        self.assertIn('originalCookieSnapshot',spec)
        self.assertIn('original_prelogin_unexpired',spec)


if __name__=='__main__': unittest.main(verbosity=2)
