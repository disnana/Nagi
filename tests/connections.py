"""1,000/10,000本の実TCP接続を保持し、閉じた後のserver FDを確認する。"""
import asyncio,json,os,signal,subprocess,time,resource
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def stats(pid):
    s={}
    for line in Path(f'/proc/{pid}/status').read_text().splitlines():
        if line.startswith(('VmRSS:','VmHWM:')):s[line.split(':')[0]]=int(line.split()[1])
    s['fds']=len(list(Path(f'/proc/{pid}/fd').iterdir()));return s
async def main():
    p=subprocess.Popen([ROOT/'native-target/release/nagi-crud'],env=dict(os.environ,NAGI_THREADS='1'),stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,text=True)
    rows=[]
    try:
        for _ in range(100):
            if p.poll()is not None:raise RuntimeError(p.stderr.read())
            try:r,w=await asyncio.open_connection('127.0.0.1',8080);w.close();await w.wait_closed();break
            except OSError:await asyncio.sleep(.05)
        await asyncio.sleep(.1)
        for n in [1000,10000]:
            before=stats(p.pid);limit=asyncio.Semaphore(128);errors=[];connections=[];start=time.monotonic()
            async def open_one():
                async with limit:
                    try:
                        r,w=await asyncio.wait_for(asyncio.open_connection('127.0.0.1',8080),5)
                        connections.append((r,w))
                        w.write(b'GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n');await w.drain()
                        h=await asyncio.wait_for(r.readuntil(b'\r\n\r\n'),5)
                        size=int(next(x.split(b':',1)[1]for x in h.split(b'\r\n')if x.lower().startswith(b'content-length:')))
                        assert await r.readexactly(size)==b'ok'
                    except Exception as e:errors.append(type(e).__name__+':'+str(e))
            await asyncio.gather(*(open_one()for _ in range(n)));peak=stats(p.pid);held=time.monotonic()-start
            for r,w in connections:w.close()
            await asyncio.gather(*(w.wait_closed()for r,w in connections),return_exceptions=True)
            for _ in range(50):
                after=stats(p.pid)
                if after['fds']<=before['fds']+1:break
                await asyncio.sleep(.1)
            row={'requested':n,'held':len(connections),'errors':errors[:20],'error_count':len(errors),'seconds_to_open_and_reply':held,'before':before,'held_stats':peak,'after':after,'client_fd_limit':resource.getrlimit(resource.RLIMIT_NOFILE)}
            assert not errors,row
            assert after['fds']<=before['fds']+1,row
            rows.append(row)
        print(json.dumps(rows,indent=2))
    finally:
        if p.poll()is None:p.send_signal(signal.SIGINT)
        p.communicate(timeout=10)
if __name__=='__main__':asyncio.run(main())
