"""Compare unchanged default release code generation and native execution."""
import hashlib, json, os, statistics, subprocess, sys, tempfile
from pathlib import Path
repo = next(parent for parent in Path(__file__).resolve().parents if (parent / 'compiler/Cargo.toml').is_file() and (parent / 'runtime/Cargo.toml').is_file())
sys.path.insert(0, str(repo / 'scripts'))
from benchmark_project_builds import measure
root = Path(tempfile.mkdtemp(prefix='release-native-', dir=Path(__file__).resolve().parent))
project=root/'project'; project.mkdir()
(project/'main.nagi').write_text('@rust("native::iterations")\nextern def iterations() -> i64\ndef main():\n    n = iterations()\n    i = 0\n    checksum = 0\n    while i < n:\n        checksum = (checksum + i) % 1000000007\n        i += 1\n    print(checksum)\n')
(project/'native.rs').write_text('pub fn iterations() -> i64 { std::hint::black_box(50_000_000) }\n')
expected = str((50_000_000 * 49_999_999 // 2) % 1_000_000_007)
report={'complete':False,'conditions':'default release profile; shared native target (build cache warmth may differ; do not compare build times); sequential builds; alternating fresh-process execution order; real runtime; Rust adapter black_box iteration count; not HTTP throughput','work_directory':str(root),'expected':expected,'configurations':{}}
env=dict(os.environ,NAGI_ROOT=str(repo),NAGI_NATIVE_TARGET_DIR=str(repo/'native-target'),CARGO_NET_OFFLINE='true')
for name,compiler in [('baseline',repo/'local-only/release-0.2.1/baseline/nagic-release'),('current',repo/'target/release/nagic')]:
    out=root/name;out.mkdir();(out/'Cargo.lock').write_bytes((repo/'Cargo.lock').read_bytes())
    result=measure([str(compiler),'build','main.nagi','--no-project','--rust','native.rs','--out',str(out),'--build-stats'],project,env,root/f'{name}-build.log')
    executable=Path(result['native'])
    report['configurations'][name]={'compiler_sha256':hashlib.sha256(compiler.read_bytes()).hexdigest(),'build':result,'native_sha256':hashlib.sha256(executable.read_bytes()).hexdigest(),'generated_rust_sha256':hashlib.sha256((executable.parent/'src/main.rs').read_bytes()).hexdigest(),'samples':[]}
for i in range(7):
    for name in (['baseline','current'] if i%2==0 else ['current','baseline']):
        c=report['configurations'][name];log=root/f'{name}-run-{i}.log'
        c['samples'].append(measure([c['build']['native']],project,env,log))
        assert log.read_text().strip()==expected
        assert hashlib.sha256(Path(c['build']['native']).read_bytes()).hexdigest()==c['native_sha256']
for c in report['configurations'].values():
    c['median_s']=statistics.median(s['elapsed_s'] for s in c['samples'])
    c['min_s']=min(s['elapsed_s'] for s in c['samples']);c['max_s']=max(s['elapsed_s'] for s in c['samples'])
assert len({c['generated_rust_sha256'] for c in report['configurations'].values()})==1
report['complete']=True
Path(__file__).with_name('release-native.json').write_text(json.dumps(report,indent=2)+'\n')
print({name:c['median_s'] for name,c in report['configurations'].items()})
