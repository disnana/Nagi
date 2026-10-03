'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const compiler = require('../src/compiler');
const features = require('../src/features');
const root = path.resolve(__dirname, '../../..');

for (const low of [false, true]) {
  for (const aliased of [false, true]) {
  test(`real ${low ? 'Low' : 'High'} quoted imports complete class enum and function names without calls${aliased ? ' through a directory alias' : ''}`, async t => {
    const base = fs.mkdtempSync(path.join(os.tmpdir(), 'nagi-import-completion-'));
    t.after(() => fs.rmSync(base, { recursive: true, force: true }));
    const folder = aliased ? path.join(base, 'alias') : base;
    if (aliased) {
      const actual = path.join(base, 'actual');
      fs.mkdirSync(actual);
      fs.symlinkSync(actual, folder, process.platform === 'win32' ? 'junction' : 'dir');
    }
    const extension = low ? 'low' : 'nagi', dependency = `models 😀.${extension}`;
    fs.writeFileSync(path.join(folder, dependency), low
      ? 'record Payload { value: i64; }\nenum Fault { Missing; Invalid(message: str); }\nfn make(value: i64) -> Payload { return Payload(value=value); }\n'
      : 'class Payload:\n    value: i64\nenum Fault:\n    Missing\n    Invalid(message: str)\ndef make(value: i64) -> Payload:\n    return Payload(value=value)\n');
    const file = path.join(folder, `main.${extension}`);
    const text = `# 日本語 😀\nfrom "${dependency}" import Payload, Fault as Problem, make${low ? ';' : ''}\n` +
      (low ? 'fn main() -> unit { print(make(1).value); }\n' : 'def main():\n    print(make(1).value)\n');
    fs.writeFileSync(file, text);
    const result = await new Promise(resolve => compiler.runCheck(compiler.compilerPath('', root, root),
      ['symbols', file], folder, 5000, resolve, 16 * 1024 * 1024));
    assert.equal(result.error, null, result.output);
    const index = JSON.parse(result.output);
    for (const name of ['Payload', 'Fault', 'make']) {
      const position = text.indexOf(name, text.indexOf('import ')) + 2;
      const item = features.completionCandidates(index, text, position, low, { file }).find(item => item.name === name);
      assert.ok(item, name);
      assert.equal(item.importOnly, true);
      assert.equal(features.insertion(item, ''), name);
      const before = text.slice(0, position);
      const target = compiler.definitionAt(index, file, before.split('\n').length - 1,
        before.length - before.lastIndexOf('\n') - 1, folder);
      assert.ok(target, `${name} definition`);
      assert.equal(compiler.fileKey(target.file, folder), compiler.fileKey(path.join(folder, dependency), folder));
    }
    const comma = text.indexOf(', make') + 2;
    assert.deepEqual(features.completionCandidates(index, text, comma, low, { file }).map(item => item.name), ['make']);
    const alias = text.indexOf('Problem') + 2;
    assert.deepEqual(features.completionCandidates(index, text, alias, low, { file }), [], 'aliases are user-chosen names');
    const constructor = text.lastIndexOf('make(1)');
    assert.equal(features.signatureAt(index, text, constructor + 5, { file }).item.parameters[0].name, 'value');
  });
  }
}

test('canonical source aliases share one identity per request and refresh after a symlink changes', t => {
  const base = fs.mkdtempSync(path.join(os.tmpdir(), 'nagi-import-identity-'));
  t.after(() => fs.rmSync(base, { recursive: true, force: true }));
  const actual = path.join(base, 'actual'), replacement = path.join(base, 'replacement'), alias = path.join(base, 'alias');
  for (const folder of [actual, replacement]) {
    fs.mkdirSync(folder);
    fs.writeFileSync(path.join(folder, 'main.nagi'), '');
    fs.writeFileSync(path.join(folder, 'models.nagi'), '');
  }
  const linkType = process.platform === 'win32' ? 'junction' : 'dir';
  fs.symlinkSync(actual, alias, linkType);
  const file = path.join(alias, 'main.nagi'), canonical = fs.realpathSync.native(path.join(actual, 'main.nagi'));
  const targetFile = fs.realpathSync.native(path.join(actual, 'models.nagi'));
  const text = 'from "models.nagi" import Pay', source = { file };
  const target = { file: targetFile, line: 1, column: 1, length: 0 };
  const payload = { name: 'Payload', kind: 'class', signature: 'class Payload',
    location: { ...target, line: 2, column: 7, length: 7 } };
  const reference = { location: { file: canonical, line: 1, column: 6, length: 13 }, target };
  const index = { format: 'nagi-symbols-v1', definitions: [payload], bindings: [],
    references: [...Array.from({ length: 100 }, () => ({ ...reference, location: { ...reference.location, column: 1 } })), reference] };
  const realpath = fs.realpathSync.native;
  let reads = 0;
  fs.realpathSync.native = (...args) => { reads++; return realpath(...args); };
  t.after(() => { fs.realpathSync.native = realpath; });
  assert.deepEqual(features.completionCandidates(index, text, text.length, false, source).map(item => item.name), ['Payload']);
  assert.equal(reads, 2, 'canonical and displayed paths are resolved once despite repeated references');
  fs.unlinkSync(alias);
  fs.symlinkSync(replacement, alias, linkType);
  assert.deepEqual(features.completionCandidates(index, text, text.length, false, source), [], 'another request must resolve a retargeted alias again');
  assert.equal(compiler.definitionAt(index, file, 0, 6, base), undefined, 'F12 must not use the old alias target');
});

test('standard source identities remain logical IDs without filesystem lookups', t => {
  const realpath = fs.realpathSync.native;
  let reads = 0;
  fs.realpathSync.native = (...args) => { reads++; return realpath(...args); };
  t.after(() => { fs.realpathSync.native = realpath; });
  const file = 'stdlib:std.actor', target = { file, line: 2, column: 1, length: 5 };
  const actor = { name: 'Actor', kind: 'resource', signature: 'resource Actor' };
  const index = { format: 'nagi-symbols-v1', definitions: [], bindings: [{ file, definition: actor }], references: [{
    location: { file, line: 1, column: 1, length: 5 }, target,
  }] };
  assert.ok(features.completionCandidates(index, '', 0, false, { file }).some(item => item.name === 'Actor'));
  assert.ok(!features.completionCandidates(index, '', 0, false, { file: 'stdlib:std.http.server' }).some(item => item.name === 'Actor'));
  assert.deepEqual(compiler.definitionAt(index, file, 0, 1, '.'), target);
  assert.equal(compiler.definitionAt(index, 'stdlib:std.http.server', 0, 1, '.'), undefined);
  assert.equal(reads, 0, 'virtual sources must not be resolved on disk');
});
