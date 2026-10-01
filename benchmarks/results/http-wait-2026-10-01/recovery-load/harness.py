"""Loopback-only requests during incomplete/idle connection recovery."""
import argparse, asyncio, hashlib, json, os, signal, socket, subprocess, time
from pathlib import Path
p=argparse.ArgumentParser()
p.add_argument('--binary',type=Path,required=True)
p.add_argument('--vegeta',type=Path,required=True)
p.add_argument('--out',type=Path,required=True)
a=p.parse_args();a.out.mkdir(parents=True,exist_ok=False)
FULL=b'GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n'
def resources(pid):
 folder=Path(f'/proc/{pid}')
 status=dict(line.split(':',1) for line in (folder/'status').read_text().splitlines())
 return {'fds':len(list((folder/'fd').iterdir())),'rss_kib':int(status['VmRSS'].split()[0])}
async def main():
 env=dict(os.environ);env['TOKIO_WORKER_THREADS']='1';env.pop('NAGI_HTTP_REQUEST_WAIT_SECONDS',None)
 cpus=sorted(os.sched_getaffinity(0))
 with (a.out/'server.log').open('w') as log:
  server=subprocess.Popen(['taskset','-c',str(cpus[0]),str(a.binary)],stdout=log,stderr=log,env=env)
  connections=[];load=None
  try:
   for _ in range(200):
    try:
     r,w=await asyncio.open_connection('127.0.0.1',8080);w.write(FULL);await w.drain();await r.readuntil(b'\r\n\r\n');assert await r.readexactly(2)==b'ok';w.close();await w.wait_closed();break
    except (ConnectionRefusedError,OSError):await asyncio.sleep(.025)
   else:raise AssertionError('server not ready')
   baseline=resources(server.pid)
   loadfile=(a.out/'healthy.bin').open('wb')
   load=subprocess.Popen(['taskset','-c',','.join(map(str,cpus[1:3])),str(a.vegeta),'attack','-rate=5000/s','-duration=25s','-timeout=5s','-workers=32','-max-workers=32'],stdin=subprocess.PIPE,stdout=loadfile,stderr=subprocess.PIPE)
   load.stdin.write(b'GET http://127.0.0.1:8080/health\n');load.stdin.close()
   started=time.monotonic()
   groups={'silent':[],'partial':[],'idle':[]}
   async def connect(kind):
    r,w=await asyncio.open_connection('127.0.0.1',8080)
    if kind=='partial':w.write(b'GET /health HTTP/1.1\r\nHost:');await w.drain()
    elif kind=='idle':w.write(FULL);await w.drain();await r.readuntil(b'\r\n\r\n');assert await r.readexactly(2)==b'ok'
    groups[kind].append((r,w));connections.append((r,w))
   for kind in groups:
    await asyncio.gather(*(connect(kind) for _ in range(350)))
   samples=[]
   while time.monotonic()-started<27:
    samples.append({'elapsed_s':time.monotonic()-started,'server':resources(server.pid),'remaining':{kind:sum(not r.at_eof() for r,w in group) for kind,group in groups.items()}})
    await asyncio.sleep(.5)
   assert load.wait(timeout=5)==0,load.stderr.read().decode()
   loadfile.close()
   report=subprocess.check_output([str(a.vegeta),'report','-type=json',str(a.out/'healthy.bin')],text=True)
   result=json.loads(report)
   assert result['status_codes']=={'200':result['requests']} and result['requests']>=123750,result
   assert not result['errors'] and result['success']==1,result
   assert all(r.at_eof() for r,w in connections),'connection did not expire'
   record={'baseline':baseline,'groups_per_kind':350,'healthy_rate':5000,'healthy_duration_s':25,'healthy_report':result,'samples':samples,'binary_sha256':hashlib.sha256(a.binary.read_bytes()).hexdigest(),'harness_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
   (a.out/'results.json').write_text(json.dumps(record,indent=2)+'\n')
   print(json.dumps({'healthy_requests':result['requests'],'errors':result['errors'],'last':samples[-1]}),flush=True)
  finally:
   for r,w in connections:w.close()
   for r,w in connections:
    try:await w.wait_closed()
    except OSError:pass
   if load is not None and load.poll() is None:load.terminate();load.wait(timeout=5)
   server.send_signal(signal.SIGINT)
   try:server.wait(timeout=5)
   except subprocess.TimeoutExpired:server.kill();server.wait()
asyncio.run(main())
