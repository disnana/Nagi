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
