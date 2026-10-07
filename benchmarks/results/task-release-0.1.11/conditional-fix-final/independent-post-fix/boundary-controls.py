from pathlib import Path
import hashlib,json,os,re,shutil,subprocess,tomllib
out=Path('/tmp/nagi-critical-release-review/post-fix')
cli=out/'bin/nagic'
folder=out/'boundary-controls'
folder.mkdir(exist_ok=True)
high='''from std.task import TaskFailure

def take(result: Result[i64, TaskFailure]) -> bool:
    print("loop-receipt")
    match result:
        case Ok(number):
            assert_true(number == 7)
            return True
        case Err(failure):
            return False

def text(result: Result[str, TaskFailure]) -> str:
    print("env-receipt")
    match result:
        case Ok(value):
            return value
        case Err(failure):
            return "failed"

def env(key: str, value: str) -> str:
    print("user-env")
    return value

async def number() -> i64:
    return 7
async def word() -> str:
    return "payload"
async def main() -> Result[unit, Error]:
    async with scope:
        for index in range(2):
            task = spawn number()
            value = take(await task) and (index == 0 or index == 1)
            assert_true(value)
        text_task = spawn word()
        value = env("ignored", text(await text_task))
        assert_true(view(value) == "payload")
    return ok(print("loop-and-user-env-ok"))
'''
low='''from std.task import TaskFailure;
fn take(result: Result[i64, TaskFailure]) -> bool {
    print("loop-receipt");
    match result {
        case Ok(number) { assert_true(number == 7); return True; }
        case Err(failure) { return False; }
    }
}
fn text(result: Result[str, TaskFailure]) -> str {
    print("env-receipt");
    match result {
        case Ok(value) { return value; }
        case Err(failure) { return "failed"; }
    }
}
fn env(key: str, value: str) -> str { print("user-env"); return value; }
async fn number() -> i64 { return 7; }
async fn word() -> str { return "payload"; }
async fn main() -> Result[unit, Error] {
    scope {
        for index in range(2) {
            let task = spawn number();
            let value = take(await task) and (index == 0 or index == 1);
            assert_true(value);
        }
        let text_task = spawn word();
        let value = env("ignored", text(await text_task));
        assert_true(view(value) == "payload");
    }
    return ok(print("loop-and-user-env-ok"));
}
'''
(folder/'main.nagi').write_text(high)
(folder/'handwritten.low').write_text(low)
env={**os.environ,'NAGI_ROOT':'/tmp/nagi-task-conditional-consumption','NAGI_NATIVE_TARGET_DIR':'/tmp/nagi-container-flow-target','CARGO_NET_OFFLINE':'true'}
check=[str(cli),'check',str(folder/'main.nagi'),'--no-project','--out',str(folder/'generated')]
result=subprocess.run(check,cwd=out,env=env,capture_output=True,text=True)
(folder/'high-check.stdout').write_text(result.stdout)
(folder/'high-check.stderr').write_text(result.stderr)
assert result.returncode==0,result.stderr
shutil.copyfile(folder/'generated/generated.low',folder/'saved.low')
(folder/'main.nagi').rename(folder/'original-high.nagi.saved')
assert not (folder/'main.nagi').exists()
expected='loop-receipt\nloop-receipt\nenv-receipt\nuser-env\nloop-and-user-env-ok\n'
records=[]
for form,p in [('high',folder/'native-high.nagi'),('saved-low',folder/'saved.low'),('handwritten-low',folder/'handwritten.low')]:
    if form=='high':p.write_text(high)
    command=[str(cli),'run',str(p),'--no-project','--out',str(folder/(form+'-out'))]
    result=subprocess.run(command,cwd=out,env=env,capture_output=True,text=True)
    (folder/(form+'-native.stdout')).write_text(result.stdout)
    (folder/(form+'-native.stderr')).write_text(result.stderr)
    assert result.returncode==0 and result.stdout==expected,(form,result.returncode,result.stdout,result.stderr)
    manifest=tomllib.loads((folder/(form+'-out')/'Cargo.toml').read_text())
    actual=(folder/(form+'-out')/manifest['dependencies']['nagi-runtime']['path']).resolve()
    assert actual==Path('/tmp/nagi-task-conditional-consumption/runtime'),actual
    records.append({'form':form,'command':command,'returncode':result.returncode,'stdout':result.stdout,'source_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'origin_high_main_exists':(folder/'main.nagi').exists(),'runtime':str(actual)})
    print(form+': loop repeated receipt and user-defined env eager arguments preserved',flush=True)
negative_high='''from std.task import TaskFailure
def take(result: Result[i64, TaskFailure]) -> bool:
    match result:
        case Ok(value):
            return True
        case Err(failure):
            return False
async def number() -> i64:
    return 7
async def main() -> Result[unit, Error]:
    async with scope:
        for index in range(2):
            task = spawn number() # primary
            value = False and take(await task)
            print(value)
    return ok(print(0))
'''
negative_low='''from std.task import TaskFailure;
fn take(result: Result[i64, TaskFailure]) -> bool {
    match result { case Ok(value) { return True; } case Err(failure) { return False; } }
}
async fn number() -> i64 { return 7; }
async fn main() -> Result[unit, Error] {
    scope {
        for index in range(2) {
            let task = spawn number(); # primary
            let value = False and take(await task);
            print(value);
        }
    }
    return ok(print(0));
}
'''
for name,text in [('loop-skip.nagi',negative_high),('loop-skip.low',negative_low)]:
    p=folder/name
    p.write_text(text)
    line=next(i for i,s in enumerate(text.splitlines(),1) if '# primary' in s)
    command=[str(cli),'check',str(p),'--no-project','--out',str(folder/(name+'-out'))]
    result=subprocess.run(command,cwd=out,env={**env,'PATH':'','NAGI_ROOT':str(out/'missing-runtime')},capture_output=True,text=True)
    (folder/(name+'.stdout')).write_text(result.stdout)
    (folder/(name+'.stderr')).write_text(result.stderr)
    assert result.returncode==1 and '未受取Task' in result.stderr and re.search(re.escape(str(p))+':'+str(line)+r'\b',result.stderr),(name,result.stderr)
    records.append({'form':name,'command':command,'returncode':result.returncode,'expected_primary_line':line,'diagnostic_fragment':'未受取Task','native_build':False})
    print(name+': skipped receipt rejected at per-loop Task binding',flush=True)
(out/'boundary-control-results.json').write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n')
