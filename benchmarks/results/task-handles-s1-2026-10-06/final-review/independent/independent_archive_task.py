from pathlib import Path
import os,sys,tarfile,hashlib,json
root=Path('/workspace/Nagi')
p=Path('/tmp/nagi-s1-final-review')
sys.path.insert(0,str(root/'scripts/releases'))
import verify
archive=Path('/tmp/nagi-release-task-audit/distribution/nagi-0.1.10-linux-x86_64.tar.gz')
digest=hashlib.sha256(archive.read_bytes()).hexdigest()
assert archive.with_name(archive.name+'.sha256').read_text().strip()==f'{digest}  {archive.name}'
folder=p/'independent-archive-native'
folder.mkdir()
with tarfile.open(archive) as source:source.extractall(folder,filter='data')
installed=folder/'nagi-0.1.10-linux-x86_64'
exe=installed/'nagic'
environment=dict(os.environ)
environment.pop('NAGI_ROOT',None)
environment['NAGI_NATIVE_TARGET_DIR']='/tmp/nagi-container-flow-target'
environment['CARGO_NET_OFFLINE']='true'
metadata={'archive_sha256':digest,'compiler_sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'verifier_sha256':hashlib.sha256((root/'scripts/releases/verify.py').read_bytes()).hexdigest(),'runtime_task_sha256':hashlib.sha256((installed/'runtime/src/task.rs').read_bytes()).hexdigest(),'release_json':json.loads((installed/'release.json').read_text())}
(p/'independent-archive-identity.json').write_text(json.dumps(metadata,indent=2)+'\n')
verify.verify_task_handles(exe,folder,environment)
print('INDEPENDENT_BUNDLED_TASK_NATIVE_OK')
