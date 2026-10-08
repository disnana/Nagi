"""実ソケット経由のCRUD、validation、keep-alive、bounded bytes、timeout。"""
import asyncio,json,os,signal,subprocess,sys,time
from pathlib import Path
import aiohttp
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT/'scripts'))
from native_artifacts import native_executable
async def main():
    target=Path(os.environ.get('NAGI_NATIVE_TARGET_DIR',ROOT/'native-target'))
    process=subprocess.Popen([native_executable(ROOT/'build/crud', target)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    tests=[]
    def check(name,cond):
        assert cond,name
        tests.append(name)
    try:
        async with aiohttp.ClientSession() as s:
            for _ in range(100):
                if process.poll() is not None:
                    out,err=process.communicate()
                    raise AssertionError(f'server exited: {out} {err}')
                try:
                    async with s.get('http://127.0.0.1:8080/health') as r:
                        if r.status==200:break
                except aiohttp.ClientError:await asyncio.sleep(.05)
            else:raise AssertionError('server did not start')
            async def request(method,path,**kwargs):
                async with s.request(method,'http://127.0.0.1:8080'+path,**kwargs) as r:
                    return r.status,await r.text(),dict(r.headers)
            status,body,_=await request('GET','/users');check('empty_list',status==200 and json.loads(body)==[])
            status,body,_=await request('GET','/query?limit=5&name=alice');check('query_params',status==200 and json.loads(body)['id']==5)
            status,body,_=await request('GET','/query?limit=bad&name=alice');check('query_validation',status==400)
            status,body,_=await request('GET','/users/999');check('missing_user',status==404)
            status,body,_=await request('POST','/users',json={'name':'alice','age':18});u=json.loads(body);check('create',status==200 and u=={'id':1,'name':'alice','age':18})
            status,body,h=await request('GET','/users/1');check('get',status==200 and json.loads(body)==u);check('json_content_type',h['Content-Type']=='application/json')
            status,body,_=await request('PUT','/users/1',json={'name':'変更','age':19});check('update_utf8',status==200 and json.loads(body)['name']=='変更')
            status,body,_=await request('PUT','/users/999',json={'name':'tp','age':18});check('update_missing',status==404)
            status,body,_=await request('GET','/users');check('list_after_insert',status==200 and len(json.loads(body))==1)
            for label,payload in [('wrong_age',{'name':'tp','age':'18'}),('extra_field',{'name':'tp','age':18,'extra':1}),('missing_field',{'name':'tp'}),('overflow_i32',{'name':'tp','age':2**40})]:
                status,_,_=await request('POST','/users',json=payload);check(label,status==400)
            status,_,_=await request('POST','/echo',data=b'{"name":');check('malformed_json',status==400)
            status,_,_=await request('POST','/echo',data=b'{"name":"\xff","age":18}');check('invalid_utf8',status==400)
            status,_,_=await request('POST','/echo',data=b'x'*1048577);check('body_limit',status==413)
            status,body,_=await request('POST','/echo',json={'name':'alice','age':18});check('json_echo',status==200 and json.loads(body)=={'name':'alice','age':18})
            status,body,_=await request('POST','/users',json={'name':"x'); DROP TABLE users;--",'age':18});check('sql_bound_params',status==200 and json.loads(body)['id']==2)
            status,body,h=await request('GET','/stream');check('bounded_bytes',status==200 and body=='chunk:0\nchunk:1\nchunk:2\nchunk:3\nchunk:4\n' and h['Content-Type']=='application/octet-stream')
            status,_,_=await request('GET','/ws');check('websocket_route_removed',status==404)
            status,_,_=await request('GET','/wait/2500');check('handler_timeout',status==504)
            status,_,_=await request('GET','/health');check('alive_after_timeout',status==200)
            status,body,_=await request('DELETE','/users/1');check('delete',status==200 and json.loads(body)==1)
            status,_,_=await request('GET','/users/1');check('gone_after_delete',status==404)
            status,_,_=await request('GET','/users/bad');check('path_validation',status==400)
            # ClientSessionの接続poolとは独立し、同じTCP接続へ2回送る。
            reader,writer=await asyncio.open_connection('127.0.0.1',8080)
            for _ in range(2):
                writer.write(b'GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n');await writer.drain()
                head=await reader.readuntil(b'\r\n\r\n');size=int(next(x.split(b':',1)[1] for x in head.split(b'\r\n') if x.lower().startswith(b'content-length:')))
                check('keepalive_request',head.startswith(b'HTTP/1.1 200') and await reader.readexactly(size)==b'ok')
            writer.close();await writer.wait_closed()
        print(json.dumps({'passed':len(tests),'tests':tests}))
    finally:
        if process.poll() is None:
            process.send_signal(signal.SIGINT)
            try:process.communicate(timeout=10)
            except subprocess.TimeoutExpired:process.terminate();process.communicate(timeout=5)
if __name__=='__main__':asyncio.run(main())
