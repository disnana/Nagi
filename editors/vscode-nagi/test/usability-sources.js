'use strict';

// Checked by the real compiler tests and shared with the VS Code Host suite.
const borrowed = [
  ['nagi', 'class Inner:\n    name: str\nclass Entry:\n    inner: Inner\n    score: i64\n' +
    'def main():\n    entries = [Entry(inner=Inner(name="Nagi"), score=10)]\n' +
    '    for item in entries:\n        print(item.inner.name)\n        print(item.score)\n' +
    '        name = copy(view(item.inner.name))\n        print(name)\n' +
    '    append(entries, Entry(inner=Inner(name="凪"), score=20))\n'],
  ['low', 'record Inner { name: str; }\nrecord Entry { inner: Inner; score: i64; }\n' +
    'fn main() {\n    let entries = [Entry(inner=Inner(name="Nagi"), score=10)];\n' +
    '    for item in entries {\n        print(item.inner.name);\n        print(item.score);\n' +
    '        let name = copy(view(item.inner.name));\n        print(name);\n    }\n' +
    '    append(entries, Entry(inner=Inner(name="凪"), score=20));\n}\n'],
];

const result = [
  ['nagi', 'import std.result as result\nfrom std.result import map_error as convert\n' +
    'enum Problem:\n    Missing\nenum Failure:\n    Invalid\n' +
    'def explain(problem: Problem) -> Failure:\n    print(problem)\n    return Failure.Invalid\n' +
    'def translate(value: Result[i64, Problem]) -> Result[i64, Failure]:\n    return result.map_error(value, explain)\n' +
    'def aliased(value: Result[i64, Problem]) -> Result[i64, Failure]:\n    mapper = explain\n    return convert(value, mapper)\n' +
    'def main():\n    print(0)\n'],
  ['low', 'import std.result as result;\nfrom std.result import map_error as convert;\n' +
    'enum Problem { Missing; }\nenum Failure { Invalid; }\n' +
    'fn explain(problem: Problem) -> Failure { print(problem); return Failure.Invalid; }\n' +
    'fn translate(value: Result[i64, Problem]) -> Result[i64, Failure] { return result.map_error(value, explain); }\n' +
    'fn aliased(value: Result[i64, Problem]) -> Result[i64, Failure] { let mapper = explain; return convert(value, mapper); }\n' +
    'fn main() { print(0); }\n'],
];

const actor = [
  ['nagi', 'import std.actor as actors\nfrom std.actor import TaskReady as Signal, WaitError as WaitProblem, WaitKind as WaitReason\n' +
    'class State:\n    active: bool\n' +
    'async def worker(state: shared[State], signal: Signal) -> Result[unit, Error]:\n' +
    '    print(state.active)\n    return actors.mark_ready(view(signal))\n' +
    'def install(group: view[actors.Supervisor[State]]) -> Result[unit, Error]:\n' +
    '    return actors.task_with_ready(group, view("worker"), worker, actors.RestartPolicy.TEMPORARY)\n' +
    'async def next(control: view[actors.Control]) -> Result[Option[actors.Event], WaitProblem]:\n' +
    '    return await actors.next_event_timeout(control, 1000)\n' +
    'def inspect(problem: WaitProblem) -> bool:\n    print(problem.message)\n    return problem.kind == WaitReason.TIMEOUT\n' +
    'def ready(event: actors.Event) -> bool:\n    return event.kind == actors.EventKind.READY\n' +
    'def main():\n    print(WaitReason.INVALID_TIMEOUT)\n'],
  ['low', 'import std.actor as actors;\nfrom std.actor import TaskReady as Signal, WaitError as WaitProblem, WaitKind as WaitReason;\n' +
    'record State { active: bool; }\n' +
    'async fn worker(state: shared[State], signal: Signal) -> Result[unit, Error] { print(state.active); return actors.mark_ready(view(signal)); }\n' +
    'fn install(group: view[actors.Supervisor[State]]) -> Result[unit, Error] { return actors.task_with_ready(group, view("worker"), worker, actors.RestartPolicy.TEMPORARY); }\n' +
    'async fn next(control: view[actors.Control]) -> Result[Option[actors.Event], WaitProblem] { return await actors.next_event_timeout(control, 1000); }\n' +
    'fn inspect(problem: WaitProblem) -> bool { print(problem.message); return problem.kind == WaitReason.TIMEOUT; }\n' +
    'fn ready(event: actors.Event) -> bool { return event.kind == actors.EventKind.READY; }\n' +
    'fn main() { print(WaitReason.INVALID_TIMEOUT); }\n'],
];

module.exports = { borrowed, result, actor };
