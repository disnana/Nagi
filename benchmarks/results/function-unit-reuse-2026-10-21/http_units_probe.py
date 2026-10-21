import json, os, signal, socket, subprocess, sys, time, urllib.error, urllib.request
from pathlib import Path
repo=Path(__file__).resolve().parents[3]; base=repo/'local-only/function-delta-probe-20261021'
report=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else base/'print-results.json'
r=json.loads(report.read_text()); root=Path(r['work_directory'])
fixture=root/'http-project'; fixture.mkdir(exist_ok=True)
with socket.socket() as sock:
 sock.bind(('127.0.0.1',0)); port=sock.getsockname()[1]
source=(repo/'examples/tutorial/http.nagi').read_text().replace('8094',str(port)).replace('    if id < 1:', '    if not valid_id(id):')
source='def valid_id(value: i64) -> bool:\n    return value > 0\n\n'+source
(fixture/'main.nagi').write_text(source)
cache=root/'current-units-dev/target'; out=fixture/'out'; out.mkdir(exist_ok=True); (out/'Cargo.lock').write_bytes((repo/'Cargo.lock').read_bytes())
env=dict(os.environ,NAGI_ROOT=str(repo),NAGI_NATIVE_TARGET_DIR=str(cache),CARGO_NET_OFFLINE='true')
command=[str(repo/'target/release/nagic'),'build',str(fixture/'main.nagi'),'--no-project','--out',str(out),'--profile','dev','--incremental=units','--build-stats']
result=subprocess.run(command,env=env,capture_output=True,text=True,timeout=180)
(base/'http-build.log').write_text(result.stdout+result.stderr)
assert result.returncode==0, result.stderr
stats=json.loads(next(x.removeprefix('nagi-build-stats: ') for x in result.stderr.splitlines() if x.startswith('nagi-build-stats: ')))
assert stats['checks']=='full' and stats['generated_body']['mode']=='cargo_units'
assert any(n.endswith('valid_id') for u in stats['generated_body']['units'] for n in u['functions'])
native=next(x.removeprefix('native: ') for x in (result.stdout+result.stderr).splitlines() if x.startswith('native: '))
process=subprocess.Popen([native],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env=env)
observations=[]
try:
 for i in range(100):
  assert process.poll() is None,process.communicate()
  try:
   with urllib.request.urlopen(f'http://127.0.0.1:{port}/greet/41',timeout=.5) as response:
    value=json.load(response); assert response.status==200 and value=={'id':41,'name':'Nagi'}
    observations.append({'path':'/greet/41','status':200,'value':value}); break
  except (urllib.error.URLError,TimeoutError): time.sleep(.05)
 else: raise AssertionError('HTTP readiness deadline')
 try: urllib.request.urlopen(f'http://127.0.0.1:{port}/greet/0',timeout=2)
 except urllib.error.HTTPError as error:
  assert error.code==400; observations.append({'path':'/greet/0','status':error.code})
 else: raise AssertionError('invalid input accepted')
finally:
 process.send_signal(signal.SIGINT); stdout,stderr=process.communicate(timeout=10)
 assert process.returncode==0,(stdout,stderr)
verified=subprocess.run([sys.executable,str(repo/'scripts/verify_build_receipt.py'),stats['receipt'],'--compiler',str(repo/'target/release/nagic')],capture_output=True,text=True)
assert verified.returncode==0,verified.stderr
(base/'http-results.json').write_text(json.dumps({'source':source,'observations':observations,'shutdown_exit':process.returncode,'stats':stats,'receipt_verified':True},indent=2)+'\n')
print('Real runtime HTTP: pure helper extracted; GET success and invalid input; SIGINT shutdown; schema 3 verified.')
