import json, os, shutil, subprocess, sys
from pathlib import Path
repo=Path(__file__).resolve().parents[3]; sys.path.insert(0,str(repo/'scripts'))
from benchmark_project_builds import measure
p=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else repo/'local-only/function-delta-probe-20261021/print-results.json'
r=json.loads(p.read_text()); assert r['complete']; root=Path(r['work_directory']); project=root/'project'
expected=[str(42+i) if i<5 else str(i) for i in range(1000)]
result={'conditions':'real runtime already compiled in each independent measured target; fresh generated app/output identity; same 1000-function source and seed lock; one sample each; dev profile unchanged','modes':{}}
for label,c in r['configurations'].items():
 folder=root/label; out=folder/'dependency-warm-out'; out.mkdir(); shutil.copyfile(repo/'Cargo.lock',out/'Cargo.lock')
 mode=c['mode']; env=dict(os.environ,NAGI_ROOT=str(repo),NAGI_NATIVE_TARGET_DIR=str(folder/'target'),CARGO_NET_OFFLINE='true')
 sample=measure([str(repo/'target/release/nagic'),'build','--project',str(project),'--out',str(out),'--profile','dev','--build-stats','--incremental'+('=units' if mode=='units' else '')],project,env,folder/'dependency-warm.log')
 run=subprocess.run([sample['native']],capture_output=True,text=True,check=True)
 assert run.stdout.splitlines()==expected
 result['modes'][mode]=sample
p.with_name('dependency-warm-results.json').write_text(json.dumps(result,indent=2)+'\n')
print({mode:x['elapsed_s'] for mode,x in result['modes'].items()})
