"""Compare two local CRUD binaries with alternating order and the same load."""
import argparse, hashlib, json, os, subprocess, sys, time
from pathlib import Path
from types import SimpleNamespace

p = argparse.ArgumentParser()
p.add_argument('--repository',type=Path,required=True)
p.add_argument('--baseline',type=Path,required=True)
p.add_argument('--candidate',type=Path,required=True)
p.add_argument('--wrk',type=Path,required=True)
p.add_argument('--out',type=Path,required=True)
a=p.parse_args()
a.out.mkdir(parents=True,exist_ok=False)
sys.path.insert(0,str(a.repository/'scripts'))
from http_capacity import server_for,proc,save
cpus=sorted(os.sched_getaffinity(0))
client_cpus=','.join(map(str,cpus[1:3]))
args=SimpleNamespace(binary=a.baseline,server_cpus=str(cpus[0]))
save(a.out/'environment.json',{'order':'AB,BA,AB,BA,AB per endpoint','duration_s':10,'repetitions':5,'connections':128,'server_cpus':str(cpus[0]),'client_cpus':client_cpus,'server_workers':1,'client_threads':2,'cpu_quota':Path('/sys/fs/cgroup/cpu.max').read_text().strip(),'binary_sha256':{k:hashlib.sha256(v.read_bytes()).hexdigest() for k,v in [('baseline',a.baseline),('candidate',a.candidate)]},'harness_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()})
rows=[]
for case,path,extras in [('health','/health',[]),('small','/small',[]),('json_4k','/echo',['echo','4096'])]:
 for repeat in range(5):
  labels=['baseline','candidate'] if repeat%2==0 else ['candidate','baseline']
  for label in labels:
   args.binary=getattr(a,label)
   folder=a.out/f'{case}-{repeat}-{label}';folder.mkdir()
   base=['taskset','-c',client_cpus,str(a.wrk),'-t2']
   tail=['--timeout','5s','--latency','-s',str(a.repository/'benchmarks/http.lua'),'http://127.0.0.1:8080'+path,'--',*extras]
   with server_for(args,folder) as server:
    subprocess.run([*base,'-c32','-d1s',*tail],stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,check=True,timeout=15)
    before=proc(server.pid)
    r=subprocess.run([*base,'-c128','-d10s',*tail],capture_output=True,text=True,check=True,timeout=30)
    (folder/'wrk.txt').write_text(r.stdout+r.stderr)
    metrics=json.loads(next(line[7:] for line in r.stdout.splitlines() if line.startswith('RESULT ')))
    rows.append({'label':label,'case':case,'repeat':repeat,'metrics':metrics,'server_before':before,'server_after':proc(server.pid)})
   save(a.out/'results.json',rows)
   print(json.dumps({'label':label,'case':case,'repeat':repeat,'rps':metrics['requests_s']}),flush=True)
