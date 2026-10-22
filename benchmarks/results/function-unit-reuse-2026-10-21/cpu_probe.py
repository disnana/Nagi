"""Owned dev-only native comparison after build timing has completed."""
import hashlib, json, os, statistics, subprocess, sys
from pathlib import Path
repo = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(repo/'scripts'))
from benchmark_project_builds import measure
report_path = Path(sys.argv[1]).resolve()
raw = json.loads(report_path.read_text())
assert raw['complete']
root = Path(raw['work_directory'])
project = root/'cpu-project'; project.mkdir()
source = ('def step(n: i64) -> i64:\n    return (n * 31 + 7) % 1009\n'
          'def calculate(n: i64) -> i64:\n    i = 0\n    checksum = 0\n'
          '    while i < n:\n        checksum = (checksum + step(i)) % 1000000007\n'
          '        i += 1\n    return checksum\ndef main():\n    print(calculate(50000000))\n')
(project/'main.nagi').write_text(source)
compiler=repo/'target/release/nagic'
cycles, tail = divmod(50_000_000,1009)
expected = str((cycles*(1009*1008//2)+sum((n*31+7)%1009 for n in range(tail)))%1_000_000_007)
report={'conditions':'dev opt-level=0; real runtime and existing measured configuration target; seven fresh process executions; no other local build/test/benchmark; constant loop bound retained by unoptimized dev code; startup included; not production throughput', 'source':source,'expected':expected,'compiler_sha256':hashlib.sha256(compiler.read_bytes()).hexdigest(),'modes':{}}
for label, config in raw['configurations'].items():
    assert report['compiler_sha256'] == config['compiler_sha256']
    folder=root/label
    out=folder/'cpu-out'; out.mkdir(); (out/'Cargo.lock').write_bytes((repo/'Cargo.lock').read_bytes())
    env=dict(os.environ,NAGI_ROOT=str(repo),NAGI_NATIVE_TARGET_DIR=str(folder/'target'),CARGO_NET_OFFLINE='true')
    mode=config['mode']; assert mode in ('body','units')
    build=measure([str(compiler),'build',str(project/'main.nagi'),'--no-project','--out',str(out),'--profile','dev','--build-stats','--incremental' + ('=units' if mode=='units' else '')],project,env,folder/'cpu-build.log')
    assert build['stats']['generated_body']['mode']==('cargo_units' if mode=='units' else 'cargo_library')
    samples=[]
    for i in range(7):
        log=folder/f'cpu-{i}.log'; sample=measure([build['native']],project,env,log)
        assert log.read_text().splitlines()==[expected]
        samples.append(sample)
    report['modes'][mode]={'build':build,'samples':samples,'median_s':statistics.median(x['elapsed_s'] for x in samples)}
report_path.with_name('cpu-results.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({mode:x['median_s'] for mode,x in report['modes'].items()}))
