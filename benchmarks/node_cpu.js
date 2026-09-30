// 同じ入力・明示ループ。Numberはこのデータ範囲では整数を正確に表現する。
// i64全範囲の意味は共有しない。V8のJIT/GCを含む参考比較として扱う。
const data = Array.from({length:100000}, (_,i)=>(i*17+13)%997-498);
const floats = data.map(v=>v/7);
function integerSum(xs) {let s=0;for (let i=0;i<xs.length;i++) s+=xs[i];return s;}
function mapReduce(xs) {let s=0;for (let i=0;i<xs.length;i++) s+=xs[i]*3+1;return s;}
function filterReduce(xs) {let s=0;for (let i=0;i<xs.length;i++) if(xs[i]>0)s+=xs[i];return s;}
function branch(xs) {let s=0;for (let i=0;i<xs.length;i++) {const v=xs[i];if(v%7===0)s+=v;else s-=v;}return s;}
function loop(n) {let s=0;for (let i=0;i<n;i++) s+=i%7;return s;}
for (const [name,fn,input] of [['integer_sum',integerSum,data],['map_reduce',mapReduce,data],['filter_reduce',filterReduce,data],['branch',branch,data],['float_sum',integerSum,floats],['loop',loop,100000]]) {
  for(let i=0;i<50;i++) globalThis.__nagiBenchSink=fn(input);
  const raw=[];let value;
  for(let r=0;r<7;r++) {
    const start=process.hrtime.bigint();
    for(let i=0;i<100;i++) globalThis.__nagiBenchSink=value=fn(input);
    raw.push(Number(process.hrtime.bigint()-start)/100);
  }
  const ns=[...raw].sort((a,b)=>a-b)[3];
  console.log(JSON.stringify({name:'node_'+name,ns_per_op:ns,ns_per_item:ns/100000,raw_ns:raw,checksum:value,node:process.version,warmup_calls:50,loops_per_repetition:100,allocation:'not measured; Number/Array and V8 JIT/GC',numeric_domain:'integer inputs/results within exact Number range'}));
}
