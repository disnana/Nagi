"""一つのtargetに依存crateを共有し、全サンプルを実際にbuild/runする。"""
import json,os,subprocess,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
TARGET=Path(os.environ.get('CARGO_TARGET_DIR',ROOT/'target')).resolve()
NATIVE_TARGET=Path(os.environ.get('NAGI_NATIVE_TARGET_DIR',ROOT/'native-target')).resolve()
EXE='.exe' if os.name=='nt' else ''
def run(args):
    r = subprocess.run([str(x) for x in args],cwd=ROOT,text=True,capture_output=True)
    if r.returncode:
        raise RuntimeError(r.stdout + r.stderr)
    return r
def build(args):
    r=run(args)
    if 'unused import' in r.stderr:
        raise RuntimeError(r.stderr)
    return r
def main():
    rows=[]
    nagic=TARGET/'release'/('nagic'+EXE)
    for name in ['hello','values','cpu','crud','async','actor','supervisor','queue','zero_copy','low_call','override']:
        args=[nagic,'build',ROOT/'examples'/f'{name}.nagi','--out',ROOT/'build'/name,'--cost-report']
        if name=='low_call':args+=['--native',ROOT/'examples/native/math.low']
        if name=='override':args+=['--native',ROOT/'examples/native/override.low']
        r=build(args)
        row={'sample':name,'build':'passed'}
        if name not in ['cpu','crud']:
            r=run([NATIVE_TARGET/'release'/('nagi-'+name.replace('_','-')+EXE)])
            stderr = r.stderr.replace(str(ROOT) + os.sep, '').replace(ROOT.as_posix() + '/', '')
            row.update(stdout=r.stdout,stderr=stderr,run='passed')
            if name in ['low_call','override']:assert r.stdout.strip()=='42'
        rows.append(row)
    r=build([nagic,'build',ROOT/'examples/hello.low','--out',ROOT/'build/hello_low'])
    r=run([NATIVE_TARGET/'release'/('nagi-hello'+EXE)])
    assert r.stdout.strip()=='4'
    rows.append({'sample':'hello.low','build':'passed','run':'passed','stdout':r.stdout})
    # 同じHighを再生成しても、手書き置換のbytesは変化しない。
    native=ROOT/'examples/native/override.low';before=native.read_bytes()
    build([nagic,'build',ROOT/'examples/override.nagi','--native',native,'--out',ROOT/'build/override'])
    assert native.read_bytes()==before
    (ROOT/'benchmarks/results').mkdir(parents=True,exist_ok=True)
    (ROOT/'benchmarks/results/examples.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2))
    print(json.dumps({'samples':len(rows),'builds':'passed','native_preserved':True}))
    verified = run([sys.executable, ROOT/'scripts/verify_library_examples.py', '--compiler', nagic])
    print(verified.stdout, end='')
if __name__=='__main__':main()
