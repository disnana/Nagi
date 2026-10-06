from pathlib import Path
import json,subprocess
root=Path('/tmp/nagi-s1-final-review/inputs');root.mkdir(exist_ok=True)
pairs={}
for name,operation in [('discard-await','discard(task)\n        received = await task'),('task-copy','copied = copy(task)'),('task-share','shared = share(task)'),('task-view','borrowed = view(task)'),('task-print','print(task)')]:
    h='from std.task import discard\nasync def work() -> i64:\n    return 7\nasync def main() -> Result[unit, Error]:\n    async with scope:\n        task = spawn work()\n        '+operation+'  # primary\n    return ok(print(0))\n'
    lines=operation.splitlines()
    l='from std.task import discard;\nasync fn work() -> i64 { return 7; }\nasync fn main() -> Result[unit, Error] {\n    scope {\n        let task = spawn work();\n'
    for i,line in enumerate(lines):
        line=line.strip()
        if ' = ' in line: line='let '+line
        l+='        '+line+';'+(' # primary' if i==len(lines)-1 else '')+'\n'
    l+='    }\n    return ok(print(0));\n}\n'
    pairs[name]=(h,l)
pairs['match-partial-consume']=('from std.task import discard\nasync def work() -> i64:\n    return 7\nasync def main() -> Result[unit, Error]:\n    async with scope:\n        task = spawn work()  # primary\n        choice: Option[i64] = some(1)\n        match choice:\n            case Some(number):\n                discard(task)\n            case None:\n                print(0)\n    return ok(print(0))\n','from std.task import discard;\nasync fn work() -> i64 { return 7; }\nasync fn main() -> Result[unit, Error] {\n    scope {\n        let task = spawn work(); # primary\n        let choice: Option[i64] = some(1);\n        match choice {\n            case Some(number) { discard(task); }\n            case None { print(0); }\n        }\n    }\n    return ok(print(0));\n}\n')
pairs['while-multiple-consume']=('async def work() -> i64:\n    return 7\nasync def main() -> Result[unit, Error]:\n    async with scope:\n        task = spawn work()\n        number = 0\n        while number < 2:\n            received = await task  # primary\n            number = number + 1\n    return ok(print(0))\n','async fn work() -> i64 { return 7; }\nasync fn main() -> Result[unit, Error] {\n    scope {\n        let task = spawn work();\n        let number = 0;\n        while number < 2 {\n            let received = await task; # primary\n            number = number + 1;\n        }\n    }\n    return ok(print(0));\n}\n')
pairs['task-field']=('from std.task import Task\nclass Holder:\n    child: Task[i64]  # primary\n','from std.task import Task;\nrecord Holder {\n    child: Task[i64]; # primary\n}\n')
pairs['task-enum-payload']=('from std.task import Task\nenum Holder:\n    Child(child: Task[i64])  # primary\n','from std.task import Task;\nenum Holder {\n    Child(child: Task[i64]); # primary\n}\n')
pairs['match-while-consumed']=('from std.task import discard\nasync def work() -> i64:\n    return 7\nasync def exercise(flag: bool) -> Result[unit, Error]:\n    choice: Option[i64] = None\n    if flag:\n        choice = some(1)\n    async with scope:\n        task = spawn work()\n        match choice:\n            case Some(number):\n                received = await task\n                match received:\n                    case Ok(value):\n                        assert_true(value == 7)\n                    case Err(failure):\n                        assert_true(False)\n            case None:\n                discard(task)\n        number = 0\n        while number < 2:\n            task = spawn work()\n            received = await task\n            match received:\n                case Ok(value):\n                    assert_true(value == 7)\n                case Err(failure):\n                    assert_true(False)\n            number = number + 1\n    return ok(print(0))\n','from std.task import discard;\nasync fn work() -> i64 { return 7; }\nasync fn exercise(flag: bool) -> Result[unit, Error] {\n    let choice: Option[i64] = None;\n    if flag { choice = some(1); }\n    scope {\n        let task = spawn work();\n        match choice {\n            case Some(number) {\n                let received = await task;\n                match received { case Ok(value) { assert_true(value == 7); } case Err(failure) { assert_true(False); } }\n            }\n            case None { discard(task); }\n        }\n        let number = 0;\n        while number < 2 {\n            task = spawn work();\n            let received = await task;\n            match received { case Ok(value) { assert_true(value == 7); } case Err(failure) { assert_true(False); } }\n            number = number + 1;\n        }\n    }\n    return ok(print(0));\n}\n')
obs=[]
for name,(h,l) in pairs.items():
    for ext,text in [('nagi',h),('low',l)]:
        path=root/(name+'.'+ext);path.write_text(text)
        p=subprocess.run(['/tmp/nagi-container-flow-target/debug/nagic','check',str(path),'--no-project'],text=True,capture_output=True)
        row=dict(source=path.name,exit=p.returncode,diagnostic=p.stderr,primary_line=next((i+1 for i,x in enumerate(text.splitlines()) if '# primary' in x),0))
        obs.append(row);print(row)
Path('/tmp/nagi-s1-final-review/probes.json').write_text(json.dumps(obs,ensure_ascii=False,indent=2)+'\n')
