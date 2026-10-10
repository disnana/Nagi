import ast
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path('/workspace/Nagi-http-limit-ci')
evidence = Path('/workspace/nagi-http413-ci-evidence')
sys.path.insert(0, str(root / 'scripts'))
import verify_application_examples as applications

project = root / 'test-nagi-code/application-examples/quote-api'
sf05_project = Path('/workspace/Nagi-security-sf05/test-nagi-code/application-examples/quote-api')
for filename in ('main.nagi', 'nagi.toml'):
    assert (project / filename).read_bytes() == (sf05_project / filename).read_bytes(), filename
assert (root / 'runtime/src/http_server.rs').read_bytes() == Path('/workspace/Nagi-security-sf05/runtime/src/http_server.rs').read_bytes()
relative = 'test-nagi-code/application-examples/quote-api/smoke.py'
baseline = ast.parse(subprocess.check_output(['git', 'show', '3b8da226eb26c27187f3b0bc4fa39639abe4ac57:' + relative], cwd=root))
current = ast.parse((root / relative).read_text())
def original_oracles(tree):
    return [ast.dump(node, include_attributes=False) for node in ast.walk(tree)
            if isinstance(node, ast.Assert) or (isinstance(node, ast.Call) and
            ((isinstance(node.func, ast.Name) and node.func.id == 'check') or
             (isinstance(node.func, ast.Attribute) and node.func.attr == 'endheaders')))]
assert original_oracles(baseline) == original_oracles(current), 'original request/assert AST changed'
executables = {
    'high': Path('/workspace/Nagi-security-sf05/build/application-example-verification/quote-api/high/nagi-b0d6b1d2345b6f51-g-e0668-18dce7cb14d41665-0'),
    'low': Path('/workspace/Nagi-security-sf05/build/application-example-verification/quote-api/low/nagi-a7e8585e68817ad2-g-e06a6-18dce7cd3f12c28e-0'),
}
rows = []
for mode, executable in executables.items():
    facts = applications.verify_smoke(applications.verifier(project), executable,
        dict(os.environ, NAGI_FAILURE_DIR=str(evidence / 'application-failures')),
        evidence / ('smoke-' + mode), 'quote-api', mode)
    rows.append({'source': mode, 'executable': str(executable),
                 'executable_sha256': hashlib.sha256(executable.read_bytes()).hexdigest(),
                 'source_and_runtime_match': True, 'original_request_and_asserts_match': True,
                 'reused_binary_without_rebuild': True, **facts})
(evidence / 'quote-reused-binaries.json').write_text(json.dumps(rows, indent=2) + '\n')
print(json.dumps(rows, indent=2))
