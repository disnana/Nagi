"""Native wrk、同一payload、1 server worker、CPUをserver/clientに分離する。"""
import argparse,json,os,random,signal,subprocess,time,urllib.request,statistics
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'benchmarks/results'
OUT.mkdir(exist_ok=True,parents=True)
def snapshot(pid):
    try:
        stat=Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()
        rss=0;switches=0
        for line in Path(f'/proc/{pid}/status').read_text().splitlines():
            if line.startswith('VmRSS:'):rss=int(line.split()[1])
        for task in Path(f'/proc/{pid}/task').iterdir():
            try:
                for line in (task/'status').read_text().splitlines():
                    if line.startswith(('voluntary_ctxt_switches:','nonvoluntary_ctxt_switches:')):switches+=int(line.split()[1])
            except OSError:pass
        return {'cpu_ticks':int(stat[11])+int(stat[12]),'rss_kib':rss,'context_switches':switches,'fds':len(list(Path(f'/proc/{pid}/fd').iterdir())),'threads':len(list(Path(f'/proc/{pid}/task').iterdir()))}
    except OSError:return {}
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--wrk',type=Path,required=True);ap.add_argument('--soak',type=int,default=120);ap.add_argument('--duration',type=int,default=3);a=ap.parse_args()
    target=Path(os.environ.get('CARGO_TARGET_DIR',ROOT/'target'));available=sorted(os.sched_getaffinity(0));server_core=available[0];client_cores=available[-2:]
    env=dict(os.environ,NAGI_THREADS='1',NAGI_DB=':memory:')
    configurations={'nagi':([ROOT/'native-target/release/nagi-crud'],8080),'axum':([target/'release/examples/axum_baseline'],8082),'node':(['node',ROOT/'benchmarks/node_server.js'],8081),'aiohttp':(['python3',ROOT/'benchmarks/python_server.py'],8083)}
    rows=[]
    def load(port,case,duration):
        args=['taskset','-c',','.join(map(str,client_cores)),str(a.wrk),'-t2','-c64','-d'+str(duration)+'s','--latency','-s',str(ROOT/'benchmarks/http.lua'),'http://127.0.0.1:'+str(port)+case[1],'--']+case[2]
        p=subprocess.run(args,text=True,capture_output=True,check=True)
        result=json.loads(next(x[7:] for x in p.stdout.splitlines() if x.startswith('RESULT ')))
        return result,p.stdout
    cases=[('plaintext','/health',[]),('small_json','/small',[]),('post_small','/echo',['echo','5']),('post_medium','/echo',['echo','4096'])]
    # 各frameworkは常に1 worker。測定順序は固定seedで並べ替える。
    for repeat in range(3):
        order=list(configurations);random.Random(100+repeat).shuffle(order)
        for name in order:
            cmd,port=configurations[name];log=(OUT/f'{name}-server-{repeat}.log').open('w')
            p=subprocess.Popen(['taskset','-c',str(server_core)]+[str(x) for x in cmd],env=env,stdout=log,stderr=log)
            try:
                for _ in range(100):
                    if p.poll() is not None:raise RuntimeError(f'{name} exited; see log')
                    try:
                        with urllib.request.urlopen(f'http://127.0.0.1:{port}/health',timeout=.5) as r:assert r.read()==b'ok'
                        break
                    except OSError:time.sleep(.05)
                else:raise RuntimeError('readiness')
                with urllib.request.urlopen(f'http://127.0.0.1:{port}/small') as r:assert json.load(r)=={'id':1,'name':'tp-li','age':18}
                measured_cases=list(cases)
                if name=='nagi':
                    req=urllib.request.Request(f'http://127.0.0.1:{port}/users',data=b'{"name":"tp-li","age":18}',headers={'Content-Type':'application/json'})
                    with urllib.request.urlopen(req) as r:assert json.load(r)['id']==1
                    measured_cases += [('query_parameter','/query?limit=5&name=tp-li',[]),('db_single_row','/users/1',[])]
                for case in measured_cases:
                    load(port,case,1);before=snapshot(p.pid);start=time.monotonic();r,raw=load(port,case,a.duration);wall=time.monotonic()-start;after=snapshot(p.pid)
                    r.update(framework=name,case=case[0],repeat=repeat,server_workers=1,server_affinity=[server_core],client_affinity=client_cores,before=before,after=after,cpu_seconds=(after['cpu_ticks']-before['cpu_ticks'])/os.sysconf('SC_CLK_TCK'),wall_seconds=wall)
                    rows.append(r);(OUT/f'wrk-{name}-{case[0]}-{repeat}.txt').write_text(raw)
                    (OUT/'http.json').write_text(json.dumps(rows,indent=2))
            finally:
                if p.poll() is None:p.send_signal(signal.SIGINT)
                try:p.wait(timeout=10)
                except subprocess.TimeoutExpired:p.terminate();p.wait(timeout=5)
                log.close()
    if a.soak:
        cmd,port=configurations['nagi'];log=(OUT/'soak-server.log').open('w');p=subprocess.Popen(['taskset','-c',str(server_core)]+[str(x) for x in cmd],env=env,stdout=log,stderr=log)
        try:
            for _ in range(100):
                try:urllib.request.urlopen(f'http://127.0.0.1:{port}/health',timeout=.5).close();break
                except OSError:time.sleep(.05)
            load(port,cases[2],2)
            cmd=['taskset','-c',','.join(map(str,client_cores)),str(a.wrk),'-t2','-c128','-d'+str(a.soak)+'s','--latency','-s',str(ROOT/'benchmarks/http.lua'),f'http://127.0.0.1:{port}/echo','--','echo','4096']
            start=time.monotonic();loadproc=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);samples=[]
            while loadproc.poll() is None:
                samples.append({'elapsed_s':time.monotonic()-start,**snapshot(p.pid)});time.sleep(1)
            raw,err=loadproc.communicate();assert loadproc.returncode==0,(raw,err)
            after=[]
            for _ in range(5):after.append(snapshot(p.pid));time.sleep(1)
            result=json.loads(next(x[7:] for x in raw.splitlines() if x.startswith('RESULT ')));result.update(duration_s=a.soak,resource_samples=samples,after_load=after,kind='closed_loop_finite_soak')
            (OUT/'soak.json').write_text(json.dumps(result,indent=2));(OUT/'soak-wrk.txt').write_text(raw)
        finally:
            if p.poll() is None:p.send_signal(signal.SIGINT)
            p.wait(timeout=10);log.close()
    print(json.dumps({'http_runs':len(rows),'soak_seconds':a.soak,'output':'benchmarks/results'}))
if __name__=='__main__':main()
