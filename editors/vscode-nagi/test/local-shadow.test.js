'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const compiler = require('../src/compiler');
const features = require('../src/features');
const root = path.resolve(__dirname, '../../..');
const source = { file: path.resolve('shadow-main.nagi') };
const payload = { name: 'Payload', kind: 'class', signature: 'class Payload\n    value: i64',
  location: { file: source.file, line: 1, column: 7, length: 7 }, fields: [{ name: 'value', type: 'i64' }] };
function location(text, start, name) {
  const before = text.slice(0, start);
  return { file: source.file, line: before.split('\n').length,
    column: before.length - before.lastIndexOf('\n'), length: name.length };
}
function candidate(index, text, offset, name, saved = false) {
  return features.completionCandidates(index, text, offset, false, { ...source, saved }).find(item => item.name === name);
}

test('checked local values replace same-name class and builtin call insertions', () => {
  for (const [name, global] of [['Payload', payload], ['print', undefined]]) {
    const text = `def show(${name}: i64) -> i64:\r\n    # 日本語 😀\r\n    return ${name}\r\n`;
    const start = text.lastIndexOf(name), at = location(text, start, name);
    const index = { definitions: global ? [global] : [], locals: [{ name, type: 'i64', location: at }] };
    const item = candidate(index, text, start + 2, name);
    assert.equal(item.kind, 'variable');
    assert.equal(item.signature, `${name}: i64`);
    assert.equal(features.insertion(item, ''), name);
    assert.equal(features.hoverAt(index, text, start + 2, source).item.signature, item.signature);
  }
});

test('lexical references without local types never insert an unrelated global call', () => {
  const text = 'def show(Payload: i64):\n    return Payload';
  const start = text.lastIndexOf('Payload');
  const index = { definitions: [payload], locals: [], references: [{
    location: location(text, start, 'Payload'), target: location(text, text.indexOf('Payload'), 'Payload'),
  }] };
  const item = candidate(index, text, start + 2, 'Payload');
  assert.equal(item.kind, 'variable');
  assert.equal(features.insertion(item, ''), 'Payload');
  assert.equal(item.type, undefined, 'a lexical reference does not invent a type');
});

test('partial and saved occurrences keep global names without guessing local scopes', () => {
  const text = 'class Payload:\n    value: i64\ndef show(Payload: i64) -> i64:\n    return Pay';
  const index = { definitions: [payload], locals: [{ name: 'Payload', type: 'i64',
    location: location(text, text.indexOf('Payload: i64'), 'Payload') }], references: [] };
  for (const saved of [false, true]) {
    const item = candidate(index, text, text.length, 'Payload', saved);
    assert.equal(item.kind, 'class', 'an unresolved occurrence must not claim a local variable is in scope');
    assert.equal(item.type, undefined);
    assert.equal(features.insertion(item, ''), 'Payload');
  }
  const type = text + '\ndef other(item: Pay';
  const item = candidate(index, type, type.length, 'Payload');
  assert.equal(item.kind, 'class');
  assert.equal(item.typeOnly, true, 'a value binding does not shadow the annotation namespace');
  const declaration = 'class Payload:\n    value: i64\ndef main():\n    Pay';
  assert.equal(features.insertion(candidate({ definitions: [payload] }, declaration, declaration.length, 'Payload'), ''),
    'Payload(value=${1:value})', 'a type declaration alone is not a local value binding');
});

test('a resolved global constructor remains available outside the shadowing function', () => {
  const text = 'def show(Payload: i64) -> i64:\n    return Payload\ndef main():\n    return Payload';
  const local = text.indexOf('return Payload') + 7, global = text.lastIndexOf('Payload');
  const index = { definitions: [payload], locals: [{ name: 'Payload', type: 'i64', location: location(text, local, 'Payload') }],
    references: [{ location: location(text, global, 'Payload'), target: payload.location }] };
  assert.equal(features.insertion(candidate(index, text, global + 2, 'Payload'), ''), 'Payload(value=${1:value})');
  assert.equal(candidate(index, text, global + 2, 'Payload').kind, 'class');
  assert.equal(candidate(index, text, global + 2, 'Payload', true).kind, 'class');
});

test('binding hints distinguish declaration tokens, masked text, and real value binders', () => {
  const index = { definitions: [payload] };
  for (const declaration of [
    'class\tPayload:\r\n    value: i64\r\n',
    'enum Payload:\n    Empty\n',
    'record Payload { value: i64; }\n',
    'def Payload(value: i64) -> i64:\n    return value\n',
    'fn Payload(value: i64) -> i64 { return value; }\n',
  ]) {
    const text = declaration + '# 日本語 😀 Payload: i64 = 1\nprint("Payload = 2; case Some(Payload)")\nPay';
    const item = candidate(index, text, text.length, 'Payload');
    assert.equal(item.nameOnly, undefined, declaration);
    assert.equal(features.insertion(item, ''), 'Payload(value=${1:value})', declaration);
  }
  for (const binding of [
    'Payload:\ti64 = 1', 'let Payload = 1;', 'Payload += 1',
    'for Payload in values:', 'case Some(Payload):', 'case Domain.Item(Payload):',
  ]) {
    const text = '# class Payload:\n' + binding + '\nPay';
    assert.equal(features.insertion(candidate(index, text, text.length, 'Payload'), ''), 'Payload', binding);
  }
});

async function query(folder, file, text, command = 'symbols') {
  const result = await new Promise(resolve => compiler.runCheck(compiler.compilerPath('', root, root),
    [command, file, '--editor-input'], folder, 5000, resolve, 16 * 1024 * 1024,
    JSON.stringify({ files: [{ file, text }] })));
  assert.equal(result.error, null, result.output);
  return command === 'symbols' ? JSON.parse(result.output) : result;
}
for (const low of [false, true]) {
  test(`real ${low ? 'Low' : 'High'} local and parameter completions stay bare while types and constructors remain usable`, async t => {
    const folder = fs.mkdtempSync(path.join(os.tmpdir(), 'nagi-local-completion-'));
    t.after(() => fs.rmSync(folder, { recursive: true, force: true }));
    const file = path.join(folder, low ? 'main.low' : 'main.nagi');
    const text = low
      ? 'record Payload { value: i64; }\nfn transform(value: i64) -> i64 { return value * 2; }\nfn display(Payload: i64, transform: i64, print: i64) -> i64 { return Payload + transform + print; }\nfn local() -> i64 { let Payload = 7; return Payload; }\nfn construct() -> Payload { return Payload(value=1); }\nfn main() -> unit { print(display(1, 2, 3)); print(local()); print(construct().value); }\n'
      : 'class Payload:\n    value: i64\ndef transform(value: i64) -> i64:\n    return value * 2\ndef display(Payload: i64, transform: i64, print: i64) -> i64:\n    return Payload + transform + print\ndef local() -> i64:\n    Payload = 7\n    return Payload\ndef construct() -> Payload:\n    return Payload(value=1)\ndef main():\n    print(display(1, 2, 3))\n    print(local())\n    print(construct().value)\n';
    fs.writeFileSync(file, text);
    const index = await query(folder, file, text), here = { file };
    const start = text.indexOf('return Payload +') + 7;
    for (const name of ['Payload', 'transform', 'print']) {
      const offset = text.indexOf(name, start) + 2;
      const item = features.completionCandidates(index, text, offset, low, here).find(item => item.name === name);
      assert.equal(item.kind, 'variable', name);
      const inserted = features.insertion(item, '');
      assert.equal(inserted, name);
      const word = features.wordAt(text, offset);
      await query(folder, file, text.slice(0, word.start) + inserted + text.slice(word.end), 'check');
    }
    const local = text.indexOf('return Payload;', start) >= 0 ? text.indexOf('return Payload;', start) + 9 : text.indexOf('return Payload\n', start) + 9;
    assert.equal(features.completionCandidates(index, text, local, low, here).find(item => item.name === 'Payload').kind, 'variable');
    const constructor = text.indexOf('return Payload(value=') + 9;
    assert.equal(features.completionCandidates(index, text, constructor, low, here).find(item => item.name === 'Payload').kind, 'class');
    const annotation = text.indexOf('-> Payload') + 5;
    const type = features.completionCandidates(index, text, annotation, low, here).find(item => item.name === 'Payload');
    assert.equal(type.kind, 'class'); assert.equal(type.typeOnly, true);
    const partial = text.slice(0, start) + 'Pay' + text.slice(start + 'Payload'.length);
    const partialIndex = await query(folder, file, partial);
    const item = features.completionCandidates(partialIndex, partial, start + 3, low, here).find(item => item.name === 'Payload');
    assert.equal(features.insertion(item, ''), 'Payload');
    await query(folder, file, partial.slice(0, start) + features.insertion(item, '') + partial.slice(start + 3), 'check');
  });
}
