from pathlib import Path
import subprocess
p=Path(__file__).parent
accepted=['nested-move-receive','self-move-alias','nested-move-discard','branch-alias-transfer','same-name-user-discard']
base=(p/'Cargo.toml').read_text()
for name in accepted:
 (p/'Cargo.toml').write_text(base+'\n[lib]\npath="'+name+'.rs"\n')
 print('PROBE',name,flush=True)
 result=subprocess.run(['cargo','test','--offline','--manifest-path',str(p/'Cargo.toml'),'--lib','--','--nocapture'])
 if result.returncode: raise SystemExit(result.returncode)
(p/'Cargo.toml').write_text(base)
