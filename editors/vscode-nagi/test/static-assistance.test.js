'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const f = require('../src/features');
const index = { definitions: [] };

test('keyword completion covers existing syntax in High and Low without leaking into types or members', () => {
  const shared = ['scope', 'spawn', 'import', 'extern', 'in', 'with', 'and', 'or', 'not', 'True', 'False', 'None', 'true', 'false', 'null'];
  for (const low of [false, true]) {
    const names = f.completionCandidates(undefined, 'sco', 3, low).filter(x => x.kind === 'keyword').map(x => x.name);
    for (const name of [...shared, ...(low ? ['fn', 'record', 'let'] : ['def', 'class'])]) assert.ok(names.includes(name), name);
    for (const name of ['elif', 'break', 'continue', 'pass', ...(low ? ['def', 'class'] : ['fn', 'record'])]) assert.ok(!names.includes(name), name);
  }
  for (const text of ['value: sco', 'value.sco', '# sco', '"sco']) {
    assert.ok(!f.completionCandidates(undefined, text, text.length).some(x => x.kind === 'keyword'), text);
  }
});

test('static builtin help works with incomplete syntax without an index', () => {
  const text = 'def main():\n    print("😀", ';
  assert.equal(f.hoverAt(undefined, text, text.indexOf('print') + 2).item.builtin, true);
  assert.equal(f.signatureAt(undefined, text, text.length).item.name, 'print');
  assert.equal(f.signatureAt(undefined, text, text.length).argument, 1);
  assert.ok(f.completionCandidates(undefined, 'def main():\n    pri', 19).some(x => x.name === 'print'));
  for (const text of ['# print(', 'object.print(', 'print(1) # print(']) assert.equal(f.signatureAt(undefined, text, text.length), undefined, text);
});

test('static builtin fallback avoids declarations, parameters and local bindings that shadow the name', () => {
  for (const binding of [
    'def print(value: str):', 'extern async def print()', 'fn print(value: str) {', 'class print:', 'record print {',
    'def other(print: fn(i64) -> unit):', 'print = custom', 'let print: i64 = 1;', 'let print', 'print += 1',
    'for print in items:', 'for print', 'case Ok(print):', 'case Err(print):',
  ]) {
    const text = binding + '\n    print(';
    assert.equal(f.hoverAt(undefined, text, text.lastIndexOf('print') + 2), undefined, binding);
    assert.equal(f.signatureAt(undefined, text, text.length), undefined, binding);
    assert.ok(!f.completionCandidates(undefined, text, text.length).some(x => x.name === 'print'), binding);
    assert.ok(f.completionCandidates(undefined, text, text.length).some(x => x.name === 'len'), binding);
  }
  for (const text of ['# print = custom\nprint(', 'write("print = custom")\nprint(', 'if print == custom:\n    print(']) {
    assert.equal(f.signatureAt(undefined, text, text.length).item.name, 'print', text);
  }
});

test('unresolved imports suppress guessed builtins but retain syntax and types; checked declarations win', () => {
  const text = 'import "missing.nagi"\ndef main():\n    print(';
  for (const snapshot of [undefined, index]) {
    const source = { saved: true };
    assert.equal(f.hoverAt(snapshot, text, text.indexOf('print') + 2, source), undefined);
    assert.equal(f.signatureAt(snapshot, text, text.length, source), undefined);
    assert.ok(f.completionCandidates(snapshot, text, text.length, false, source).some(x => x.name === 'scope'));
    const typed = text + '\n    value: Res';
    assert.ok(f.completionCandidates(snapshot, typed, typed.length, false, source).some(x => x.name === 'Result'));
  }
  const declared = { definitions: [{ name: 'print', kind: 'function', signature: 'def print() -> i64', parameters: [], return_type: 'i64' }] };
  assert.equal(f.signatureAt(declared, text, text.length).item.return_type, 'i64');
  assert.equal(f.signatureAt(index, text, text.length).item.name, 'print');
  const low = 'fn helper() {} import "missing.low"; fn main() { print(';
  assert.equal(f.signatureAt(undefined, low, low.length), undefined, 'Low imports can follow a declaration on the same line');
  assert.ok(!f.completionCandidates(undefined, low, low.length, true).some(x => x.builtin));
});

test('signature help does not replace a compiler-resolved local function with builtin metadata', () => {
  const text = 'write("😀"); print(\r\n    1';
  const start = text.indexOf('print');
  const checked = { definitions: [], locals: [{ name: 'print', type: 'fn(i64) -> unit',
    location: { file: '/main.nagi', line: 1, column: start + 1, length: 5 } }] };
  assert.equal(f.signatureAt(checked, text, text.length, { file: '/main.nagi' }), undefined);
  assert.equal(f.hoverAt(checked, text, start + 2, { file: '/main.nagi' }).item.signature, 'print: fn(i64) -> unit');
  assert.equal(f.signatureAt(checked, text, text.length, { file: '/other.nagi' }).item.name, 'print');
});

test('lexical references prevent guessed signatures when a type error removes local types', () => {
  const text = 'def main(print: i64):\r\n    print(1)';
  const source = { file: '/main.nagi' };
  const call = text.lastIndexOf('print');
  const snapshot = { definitions: [], locals: [], references: [{
    location: { file: '/main.nagi', line: 2, column: 5, length: 5 },
    target: { file: '/main.nagi', line: 1, column: 10, length: 5 },
  }] };
  assert.equal(f.hoverAt(snapshot, text, call + 2, source), undefined);
  assert.equal(f.signatureAt(snapshot, text, call + 6, source), undefined);
  const global = { name: 'print', kind: 'function', signature: 'def print(value: i64)',
    location: { file: '/lib.nagi', line: 3, column: 5, length: 5 } };
  snapshot.definitions = [global];
  assert.equal(f.signatureAt(snapshot, text, call + 6, source), undefined);
  snapshot.references[0].target = global.location;
  assert.equal(f.signatureAt(snapshot, text, call + 6, source).item, global);
});

test('unfinished line strings do not hide later assistance and preserve UTF-16 CRLF ranges', () => {
  const text = 'broken = "😀 \\\r\nprint(';
  const start = text.indexOf('print');
  const state = f.context(text, text.length);
  assert.equal(state.masked.length, text.length);
  assert.equal(state.allowed, true);
  assert.deepEqual(f.hoverAt(undefined, text, start + 2), { item: f.declarations().get('print'), start, end: start + 5 });
  assert.equal(f.signatureAt(undefined, text, text.length).item.name, 'print');
  assert.ok(f.completionCandidates(undefined, text, text.length).some(x => x.name === 'scope'));
  const unfinished = '"😀 \\"print';
  assert.equal(f.completionCandidates(undefined, unfinished, unfinished.length).length, 0);
});
