"""CPythonの同じデータと明示ループ。builtin sumの参考値も別に出す。"""
import gc,json,statistics,time,platform
data=[(i*17+13)%997-498 for i in range(100000)]
floats=[v/7.0 for v in data]
def integer_sum(xs):
    total=0
    for v in xs: total+=v
    return total
def integer_map(xs):
    total=0
    for v in xs: total+=v*3+1
    return total
def integer_filter(xs):
    total=0
    for v in xs:
        if v>0: total+=v
    return total
def branch(xs):
    total=0
    for v in xs:
        if v%7==0: total+=v
        else: total-=v
    return total
def loop(n):
    total=0
    for i in range(n):total+=i%7
    return total
for name,fn,arg in [('integer_sum',integer_sum,data),('map_reduce',integer_map,data),('filter_reduce',integer_filter,data),('branch',branch,data),('float_sum',integer_sum,floats),('loop',loop,100000),('builtin_sum_reference',sum,data)]:
    for _ in range(5):fn(arg)
    raw=[]
    for _ in range(7):
        t=time.perf_counter_ns()
        for _ in range(25):value=fn(arg)
        raw.append((time.perf_counter_ns()-t)/25)
    print(json.dumps({'name':'python_'+name,'ns_per_op':statistics.median(raw),'ns_per_item':statistics.median(raw)/100000,'raw_ns':raw,'checksum':value,'python':platform.python_version(),'gc_enabled':gc.isenabled(),'allocation':'not measured; not comparable with Rust allocator'}))
