'use strict';
const fixtures = [
  ['nagi', 'import std.actor as actors\nimport std.http.server as http\n' +
    'from std.actor import Actor as Handle, Turn as Step, Options as ActorConfig, Event as Update\n' +
    'from std.http.server import Options as HttpConfig\n' +
    'class Counter:\n    value: i64\nenum ReplyError:\n    Negative\n' +
    'def choose(label: view[str], kind: actors.CallKind) -> bool:\n    print(label)\n    return kind == actors.CallKind.MAILBOX_FULL\n' +
    'def inspect(event: Update) -> i64:\n    print(event.child_name)\n    print(event.message)\n    return event.generation\n' +
    'async def call_once(handle: Handle[i64, i64, ReplyError]) -> Result[Result[i64, ReplyError], actors.CallError]:\n' +
    '    return await actors.call(view(handle), 1, 100, 1000)\n' +
    'def finish(state: Counter) -> Step[Counter, i64, ReplyError]:\n    return actors.turn[Counter, i64, ReplyError](state, ok(3))\n' +
    'def actor_limits() -> ActorConfig:\n    return actors.default_options()\n' +
    'def http_limits() -> HttpConfig:\n    return http.default_options()\n' +
    'def main():\n    print(choose(view("😀"), actors.CallKind.MAILBOX_FULL))\n'],
  ['low', 'import std.actor as actors;\nimport std.http.server as http;\n' +
    'from std.actor import Actor as Handle, Turn as Step, Options as ActorConfig, Event as Update;\n' +
    'from std.http.server import Options as HttpConfig;\n' +
    'record Counter { value: i64; }\nenum ReplyError { Negative; }\n' +
    'fn choose(label: view[str], kind: actors.CallKind) -> bool { print(label); return kind == actors.CallKind.MAILBOX_FULL; }\n' +
    'fn inspect(event: Update) -> i64 { print(event.child_name); print(event.message); return event.generation; }\n' +
    'async fn call_once(handle: Handle[i64, i64, ReplyError]) -> Result[Result[i64, ReplyError], actors.CallError] { return await actors.call(view(handle), 1, 100, 1000); }\n' +
    'fn finish(state: Counter) -> Step[Counter, i64, ReplyError] { return actors.turn[Counter, i64, ReplyError](state, ok(3)); }\n' +
    'fn actor_limits() -> ActorConfig { return actors.default_options(); }\n' +
    'fn http_limits() -> HttpConfig { return http.default_options(); }\n' +
    'fn main() { print(choose(view("😀"), actors.CallKind.MAILBOX_FULL)); }\n'],
];
module.exports = { fixtures };
