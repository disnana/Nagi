'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const f = require('../src/features');
const index = { definitions: [
  { name: 'read_item', kind: 'function', signature: 'async def read_item(db: Db, id: i64) -> Result[Item?, Error]', parameters: [{ name: 'db', type: 'Db' }, { name: 'id', type: 'i64' }], return_type: 'Result[Item?, Error]', asynchronous: true },
  { name: 'Item', kind: 'class', signature: 'class Item\n    id: i64\n    name: str', fields: [{ name: 'id', type: 'i64' }, { name: 'name', type: 'str' }] },
] };

test('hover preserves declared async/nullable types and class fields but skips locals, members and comments', () => {
  const call = '    result = await read_item(db, 1)';
  assert.equal(f.hoverAt(index, call, call.indexOf('read_item') + 2).item.return_type, 'Result[Item?, Error]');
  const type = 'def find() -> Result[Item?, Error]:';
  assert.equal(f.hoverAt(index, type, type.indexOf('Item') + 2).item.fields[1].type, 'str');
  for (const text of ['# read_item(db, 1)', 'print("read_item(db, 1)")', 'thing.read_item(db, 1)', 'read_item = 1', 'print(read_item)', 'print(read_item[0])']) {
    assert.equal(f.hoverAt(index, text, text.indexOf('read_item') + 2), undefined, text);
  }
});

test('completion works on incomplete prefixes and inserts positional calls or named class fields', () => {
  const items = f.completionCandidates(index, '    rea', 7);
  const read = items.find(x => x.name === 'read_item');
  assert.equal(f.insertion(read, ''), 'read_item(${1:db}, ${2:id})');
  assert.equal(f.insertion(read, '('), 'read_item');
  assert.equal(f.insertion(items.find(x => x.name === 'Item'), ''), 'Item(id=${1:id}, name=${2:name})');
  assert.equal(f.insertion(items.find(x => x.name === 'read_line'), ''), 'read_line()');
  assert.equal(f.insertion(items.find(x => x.name === 'db_query'), ''), 'db_query[${1:T}](${2:db}, ${3:sql}, ${4:id})');
  assert.equal(f.completionCandidates(index, 'thing.', 6).length, 0);
  for (const text of ['# rea', '"rea', "'rea", 'print("😀 rea']) assert.equal(f.completionCandidates(index, text, text.length).length, 0);
});

test('type positions offer classes and type constructors rather than value calls', () => {
  for (const text of ['    value: It', 'def find() -> Res', 'def find(id: i64, data: view[', 'def find() -> Result[\n    It', 'json_decode[\n    It']) {
    const items = f.completionCandidates(index, text, text.length);
    assert.ok(items.some(x => x.name === 'Item'));
    assert.ok(items.some(x => x.name === 'Result'));
    assert.ok(!items.some(x => x.name === 'read_item'));
  }
  const low = f.completionCandidates(index, 'f', 1, true);
  assert.ok(low.some(x => x.name === 'fn'));
  assert.ok(!low.some(x => x.name === 'def'));
});

test('signature argument tracking ignores nested calls, strings, arrays and generic arguments', () => {
  for (const [text, expected] of [
    ['read_item(db, ', { name: 'read_item', argument: 1 }],
    ['read_item(other(1, 2), ', { name: 'read_item', argument: 1 }],
    ['read_item([1, 2], ', { name: 'read_item', argument: 1 }],
    ['read_item("a,b", ', { name: 'read_item', argument: 1 }],
    ['read_item("a,b', { name: 'read_item', argument: 0 }],
    ['db_query[Item](db, "sql", ', { name: 'db_query', argument: 2 }],
    ['read_item(db, # comment', undefined],
    ['other.read_item(', undefined],
  ]) assert.deepEqual(f.activeCall(text, text.length), expected, text);
});

test('local hovers use the exact UTF-16 occurrence and file rather than a shared name', () => {
  const text = 'print("😀"); value = 7; print(value)\r\nprint(value)';
  const declaration = text.indexOf('value');
  const usage = text.indexOf('value', declaration + 1);
  const locals = { ...index, locals: [
    { name: 'value', type: 'i32', location: { file: '/project/main.nagi', line: 1, column: declaration + 1, length: 5 } },
    { name: 'value', type: 'bool', location: { file: '/project/main.nagi', line: 1, column: usage + 1, length: 5 } },
    { name: 'value', type: 'str', location: { file: '/project/other.nagi', line: 2, column: 7, length: 5 } },
  ] };
  const source = { file: '/project/main.nagi' };
  assert.equal(f.hoverAt(locals, text, declaration + 1, source).item.signature, 'value: i32');
  assert.equal(f.hoverAt(locals, text, usage + 1, source).item.signature, 'value: bool');
  assert.equal(f.hoverAt(locals, text, text.lastIndexOf('value') + 1, source), undefined);
  assert.equal(f.hoverAt(locals, text, usage + 1, { ...source, saved: true }), undefined);
  assert.equal(f.hoverAt(index, 'thing. read_item(', 12), undefined);
});

test('member analysis masks just the edited member and preserves UTF-16 and CRLF positions', () => {
  const text = 'print("😀"); item . naMore\r\nprint(1)';
  const offset = text.indexOf('naMore') + 2;
  const member = f.memberContext(text, offset);
  assert.equal(member.start, text.indexOf('naMore'));
  assert.equal(member.end, text.indexOf('naMore') + 6);
  assert.equal(member.receiverEnd, text.indexOf('item') + 4);
  assert.equal(member.text.length, text.length);
  assert.equal(member.text, 'print("😀"); item         \r\nprint(1)');
  for (const text of ['# item.', 'print("😀 item.', 'print("item.")', 'item # .', 'item']) {
    assert.equal(f.memberContext(text, text.length), undefined, text);
  }
});

test('field candidates come from the receiver occurrence and never from saved or other-file types', () => {
  const text = 'item.na';
  const expression = { location: { file: '/project/main.low', line: 1, column: 1 }, end_line: 1, end_column: 5,
    type: 'Item', fields: [{ name: 'name', type: 'str' }] };
  const fields = { ...index, expressions: [expression] };
  const source = { file: '/project/main.low' };
  const items = f.completionCandidates(fields, text, text.length, true, source);
  assert.deepEqual(items, [{ name: 'name', kind: 'field', signature: 'name: str', type: 'str' }]);
  assert.equal(f.insertion(items[0], '('), 'name');
  for (const context of [{ file: '/project/other.low' }, { ...source, saved: true }, {}]) {
    assert.deepEqual(f.completionCandidates(fields, text, text.length, true, context), []);
  }
  assert.deepEqual(f.completionCandidates(index, text, text.length, true, source), []);
  const missing = { ...fields, expressions: [{ ...expression, fields: [] }] };
  assert.deepEqual(f.completionCandidates(missing, text, text.length, true, source), []);
});

test('a trailing member binds inside try/await unless the receiver is parenthesized', () => {
  const text = 'try fetch().';
  const end_column = text.length;
  const fields = { expressions: [
    { location: { file: '/main.nagi', line: 1, column: 1 }, end_line: 1, end_column, type: 'Item', fields: [{ name: 'id', type: 'i64' }] },
    { location: { file: '/main.nagi', line: 1, column: 5 }, end_line: 1, end_column, type: 'Result[Item, Error]', fields: [] },
  ] };
  assert.deepEqual(f.completionCandidates(fields, text, text.length, false, { file: '/main.nagi' }), []);
});

test('builtin completion catalog matches compiler arities and required type arguments', () => {
  const source = require('node:fs').readFileSync(require('node:path').resolve(__dirname, '../../../compiler/src/check.rs'), 'utf8');
  const arityStart = source.indexOf('let arity = match n {');
  const arityTable = source.slice(arityStart, source.indexOf('};', arityStart));
  const expected = new Map();
  for (const row of arityTable.matchAll(/((?:"[a-z][a-z0-9_]*"\s*\|\s*)*"[a-z][a-z0-9_]*")\s*=>\s*(\d+)/g)) {
    for (const name of row[1].matchAll(/"([a-z][a-z0-9_]*)"/g)) expected.set(name[1], Number(row[2]));
  }
  assert.ok(expected.size > 0, 'read the compiler builtin arity table');
  const builtins = f.declarations();
  assert.deepEqual([...builtins.keys()].sort(), [...expected.keys()].sort());
  for (const [name, arity] of expected) assert.equal(builtins.get(name).parameters.length, arity, name);
  const genericStart = source.indexOf('let generic = matches!(');
  const genericTable = source.slice(genericStart, source.indexOf(');', genericStart));
  const generic = new Set([...genericTable.matchAll(/"([a-z][a-z0-9_]*)"/g)].map(row => row[1]));
  assert.ok(generic.size > 0, 'read the compiler generic builtin table');
  for (const [name, item] of builtins) assert.deepEqual(item.typeParameters, generic.has(name) ? ['T'] : [], name);
});

test('builtin help preserves conversion, SQLite, async and callback contracts', () => {
  const builtins = f.declarations();
  assert.equal(builtins.get('i64').signature, 'def i64(value: i8 | i16 | i32 | u8 | u16 | u32) -> i64');
  assert.equal(builtins.get('i32').signature, 'def i32(value: i64) -> Result[i32, Error]');
  assert.equal(builtins.get('uuid_parse').signature, 'def uuid_parse(text: str | view[str]) -> Result[UUID, Error]');
  assert.equal(builtins.get('uuid_format').signature, 'def uuid_format(value: UUID) -> str');
  assert.equal(builtins.get('share').return_type, 'shared[T]');
  assert.equal(builtins.get('clone_shared').parameters[0].type, 'shared[T]');
  assert.equal(builtins.get('size_of').signature, 'def size_of[T]() -> i64');
  assert.equal(builtins.get('clock_ns').signature, 'def clock_ns() -> i64');
  assert.equal(builtins.get('make_ints').return_type, 'List[i64]');
  assert.equal(builtins.get('db_insert').signature, 'async def db_insert[T](db: Db, sql: str | view[str], text: str, number: i32) -> Result[T, Error]');
  assert.equal(builtins.get('db_update').signature, 'async def db_update[T](db: Db, sql: str | view[str], id: i64, text: str, number: i32) -> Result[T, Error]');
  for (const [name, kernel] of [['bench_i64', 'fn[view[i64], i64]'], ['bench_f64', 'fn[view[f64], f64]'], ['bench_scalar', 'fn[i64, i64]']]) {
    assert.deepEqual(builtins.get(name).parameters, [{ name: 'name', type: 'str' }, { name: 'count', type: 'i64' }, { name: 'kernel', type: kernel }]);
    assert.equal(f.insertion(builtins.get(name), ''), `${name}(\${1:name}, \${2:count}, \${3:kernel})`);
  }
  for (const name of ['actor_demo', 'actor_pair_demo', 'queue_demo', 'task_demo', 'cpu_sum', 'supervisor_demo']) {
    assert.equal(builtins.get(name).asynchronous, true, name);
    assert.equal(builtins.get(name).return_type, 'Result[i64, Error]', name);
  }
  const text = 'db_insert[Item](db, sql, text, number)';
  assert.equal(f.hoverAt({}, text, 3).item.signature, builtins.get('db_insert').signature);
  const override = { name: 'share', kind: 'function', signature: 'def share(value: i64) -> i64', parameters: [{ name: 'value', type: 'i64' }] };
  assert.equal(f.declarations({ definitions: [override] }).get('share'), override);
});

test('function and nullable types complete inside all supported generic builtin contexts', () => {
  for (const text of ['def apply(callback: f', 'value: Opt', 'value: Option[\n    ', 'def apply(callback: fn[\n    ', 'db_insert[\n    ', 'db_update[\n    ', 'size_of[\n    ']) {
    const items = f.completionCandidates(index, text, text.length);
    for (const name of ['Item', 'fn', 'Option', 'i64']) assert.ok(items.some(item => item.name === name), `${text}: ${name}`);
    assert.ok(!items.some(item => item.name === 'print'), text);
  }
  assert.ok(f.completionCandidates(index, 'values[\n    ', 12).some(item => item.name === 'print'), 'ordinary indexing remains a value context');
});

test('completion preserves existing generic arguments and call parentheses', () => {
  const builtins = f.declarations();
  const query = builtins.get('db_query');
  for (const following of ['[Item](db, sql, id)', ' [Item]', '[']) assert.equal(f.insertion(query, following), 'db_query', following);
  assert.equal(f.insertion(query, '('), 'db_query[${1:T}]');
  assert.equal(f.insertion(builtins.get('size_of'), '('), 'size_of[${1:T}]');
  assert.equal(f.insertion(builtins.get('uuid_parse'), '(text)'), 'uuid_parse');
  assert.equal(f.insertion(builtins.get('make_ints'), '[0]'), 'make_ints(${1:count})');
  const types = f.completionCandidates(index, 'value: ', 7);
  for (const name of ['List', 'Result', 'Option', 'fn']) {
    assert.equal(f.insertion(types.find(item => item.name === name), '[i64]'), name);
  }
  assert.equal(f.insertion(types.find(item => item.name === 'Option'), ''), 'Option[${1:T}]');
  assert.equal(f.insertion(types.find(item => item.name === 'fn'), ''), 'fn[${1:i64}, ${2:i64}]');
});
