import ast,json,os
from pathlib import Path
import tempfile,time
from types import SimpleNamespace as NS
import unittest
from unittest.mock import patch
import browser_coordinator as c
import browser_budget as b
import failure_checkpoint as f


class R2Tests(unittest.TestCase):
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

if __name__=='__main__':unittest.main(verbosity=2)
