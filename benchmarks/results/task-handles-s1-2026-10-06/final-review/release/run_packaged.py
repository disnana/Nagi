import hashlib
import json
import subprocess
import sys
from pathlib import Path

root = Path('/workspace/Nagi')
sys.path.insert(0, str(root / 'scripts/releases'))
import package
import verify

sha = 'a9a0d42e94d6c7518bd7d955c9400299e554888c'
binary = Path('/tmp/nagi-container-flow-target/release/nagic')
diff = subprocess.check_output(['git', 'diff', '--name-only', sha, '--', 'compiler/src'], cwd=root, text=True)
assert not diff, diff
archive = package.package(binary, '0.1.10', 'linux-x86_64',
                          Path('/tmp/nagi-release-task-audit/distribution'), sha)
print(json.dumps({'packaged_commit': sha, 'version': '0.1.10',
                  'compiler_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                  'archive': str(archive),
                  'archive_sha256': hashlib.sha256(archive.read_bytes()).hexdigest()}, indent=2), flush=True)
try:
    verify.verify(archive, '0.1.10', 'linux-x86_64', Path('/tmp/nagi-container-flow-target'))
except subprocess.CalledProcessError as error:
    print(error.stdout or '', flush=True)
    print(error.stderr or '', flush=True)
    raise
print('PACKAGED_TASK_FULL_VERIFICATION_OK', flush=True)
