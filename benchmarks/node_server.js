// Nagiと同じschema・compact JSON。bodyをtyped modelへ検査してから応答する。
const http = require('http');
http.createServer((req,res)=>{
  if(req.url==='/health'){res.end('ok');return;}
  if(req.url==='/small'){res.setHeader('content-type','application/json');res.end(JSON.stringify({id:1,name:'alice',age:18}));return;}
  if(req.url==='/echo'&&req.method==='POST'){
    let chunks=[],size=0;
    req.on('data',c=>{size+=c.length;if(size>1048576){req.destroy();return;}chunks.push(c);});
    req.on('end',()=>{try{const p=JSON.parse(Buffer.concat(chunks).toString('utf8'));
      if(typeof p.name!=='string'||!Number.isInteger(p.age)||p.age<-2147483648||p.age>2147483647||Object.keys(p).sort().join(',')!=='age,name')throw Error('type');
      res.setHeader('content-type','application/json');res.end(JSON.stringify({name:p.name,age:p.age}));
    }catch(e){res.statusCode=400;res.end('{}');}});return;
  }res.statusCode=404;res.end();
}).listen(8081,'127.0.0.1');
