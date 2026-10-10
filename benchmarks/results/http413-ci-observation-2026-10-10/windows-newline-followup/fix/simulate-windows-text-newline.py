import sys, pathlib, unittest, json
from unittest.mock import patch
sys.path.insert(0, '/workspace/Nagi-http-limit-ci/scripts')
import test_application_native_tests as tests
original_open=pathlib.Path.open
def simulated_windows_open(path, mode='r', buffering=-1, encoding=None, errors=None, newline=None):
    if mode == 'w' and path.name.startswith('server-') and path.suffix in ('.stdout', '.stderr'):
        newline='\r\n'
    return original_open(path, mode, buffering, encoding, errors, newline)
name='test_application_native_tests.ApplicationFailureEvidenceTests.test_receive_failure_records_live_child_before_cleanup_and_propagates'
suite=unittest.defaultTestLoader.loadTestsFromName(name) if '--one' in sys.argv else unittest.defaultTestLoader.loadTestsFromModule(tests)
with patch.object(pathlib.Path, 'open', simulated_windows_open):
    result=unittest.TextTestRunner(verbosity=2).run(suite)
print(json.dumps({'simulated_text_writer_newline':'CRLF','tests':result.testsRun,'failures':len(result.failures),'errors':len(result.errors),'skipped':len(result.skipped),'real_Windows_execution':False}))
sys.exit(not result.wasSuccessful())
