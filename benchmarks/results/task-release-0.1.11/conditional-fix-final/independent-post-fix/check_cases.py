from pathlib import Path
import hashlib,json,os,re,shutil,subprocess
out=Path('/tmp/nagi-critical-release-review/post-fix')
old=out.parent
root=Path('/tmp/nagi-task-conditional-consumption')
(out/'bin').mkdir(exist_ok=True)
cli=out/'bin/nagic'
shutil.copy2('/tmp/nagi-container-flow-target/release/nagic',cli)
version=subprocess.run([str(cli),'--version'],capture_output=True,text=True,check=True)
assert version.stdout.strip()=='nagic 0.1.10',version.stdout
(out/'fixed-cli-version.log').write_text(version.stdout+version.stderr)
cases=[]
for case in ['lazy-task','short-task']:
    for form,p,line in [('high',old/(case+'.nagi'),16),('saved-low',old/(case+'-saved-source')/'saved.low',21),('handwritten-low',old/(case+'.low'),12)]:
        if form=='saved-low':
            assert not (p.parent/'main.nagi').exists()
        cases.append({'id':case+'-'+form,'source':str(p),'expected':'check-fail','line':line,'fragment':'未受取Task','group':'original-red'})
for row in json.loads((old/'lazy-symmetry-results.json').read_text()):
    cases.append({'id':'symmetry-'+row['case'],'source':str(old/'lazy-symmetry'/row['case']),'expected':'check-pass' if row['expected_after_fix']=='check-pass' else 'check-fail','line':row['expected_primary_line'],'fragment':row['expected_fragment'],'group':'original-symmetry'})
base=json.loads(subprocess.check_output(['git','show','f9b25782:tests/task-handles/contracts.json'],cwd=root,text=True))
current=json.loads((root/'tests/task-handles/contracts.json').read_text())
assert current[:len(base)]==base
for row in current[len(base):]:
    cases.append({'id':'added-'+row['name'],'source':str(root/'tests/task-handles'/row['source']),'expected':row['expected'],'line':row['primary_line'],'fragment':row['diagnostic'],'group':'added-contract'})
env={**os.environ,'PATH':'','NAGI_ROOT':str(out/'missing-runtime')}
records=[]
log=out/'check-logs'
log.mkdir(exist_ok=True)
for case in cases:
    path=Path(case['source'])
    output=out/'check-output'/case['id']
    command=[str(cli),'check',str(path),'--no-project','--out',str(output)]
    result=subprocess.run(command,cwd=out,env=env,capture_output=True,text=True)
    (log/(case['id']+'.stdout')).write_text(result.stdout)
    (log/(case['id']+'.stderr')).write_text(result.stderr)
    if case['expected']=='check-pass':
        assert result.returncode==0,(case,result.stderr)
    else:
        assert result.returncode==1,(case,result.stderr)
        assert case['fragment'] in result.stderr,(case,result.stderr)
        assert re.search(re.escape(str(path))+':'+str(case['line'])+r'(?::|\b)',result.stderr),(case,result.stderr)
        assert not output.exists(),(case,'generation written before rejection')
    records.append({**case,'returncode':result.returncode,'command':command,'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'native_build':False,'cargo_and_runtime_available':False})
    print(case['id']+': '+('accepted control' if case['expected']=='check-pass' else 'checker rejection with expected fragment and original line'))
(out/'checker-results.json').write_text(json.dumps({'examined_sha':'b7545d2839d8ac6fb22e1dcbd045b1dccc11107a','compiler_version':version.stdout.strip(),'compiler_sha256':hashlib.sha256(cli.read_bytes()).hexdigest(),'total':len(records),'original_red_checks':6,'original_symmetry_checks':12,'added_contract_checks':38,'accepted':sum(r['expected']=='check-pass' for r in records),'rejected':sum(r['expected']=='check-fail' for r in records),'checks':records},ensure_ascii=False,indent=2)+'\n')
