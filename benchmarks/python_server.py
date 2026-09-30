"""aiohttp+dataclass。Nagiと同じ成功payload、型とunknown-fieldの検査を行う。"""
import json
from dataclasses import dataclass
from aiohttp import web
@dataclass(slots=True)
class CreateUser:
    name:str
    age:int
async def health(request):return web.Response(text='ok')
async def small(request):return web.Response(body=json.dumps({'id':1,'name':'tp-li','age':18},separators=(',',':')).encode('utf-8'),content_type='application/json')
async def echo(request):
    try:
        p=json.loads(await request.read())
        if not isinstance(p,dict) or set(p)!={'name','age'}:raise ValueError('fields')
        u=CreateUser(**p)
        if type(u.name)is not str or type(u.age)is not int or not -(2**31)<=u.age<2**31:raise ValueError('type')
        return web.Response(body=json.dumps({'name':u.name,'age':u.age},ensure_ascii=False,separators=(',',':')).encode('utf-8'),content_type='application/json')
    except (ValueError,TypeError):return web.Response(status=400,text='{}')
app=web.Application(client_max_size=1048576)
app.add_routes([web.get('/health',health),web.get('/small',small),web.post('/echo',echo)])
if __name__=='__main__':web.run_app(app,host='127.0.0.1',port=8083,print=None)
