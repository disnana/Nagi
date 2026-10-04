'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const compiler = require('../src/compiler');
const features = require('../src/features');
const { fixtures } = require('./stdlib-fixtures');
const root = path.resolve(__dirname, '../../..');
const virtual = 'stdlib:std.http.server';
function fixture(t, suffix, text) {
  const base = path.join(root, 'build', 'vscode-stdlib-navigation');
  fs.mkdirSync(base, { recursive: true });
  const folder = fs.mkdtempSync(path.join(base, 'case-'));
  t.after(() => fs.rmSync(folder, { recursive: true, force: true }));
  fs.writeFileSync(path.join(folder, `main.${suffix}`), text);
  return folder;
}
async function symbols(folder, file, overlays = []) {
  const result = await new Promise(resolve => compiler.runCheck(compiler.compilerPath('', root, root),
    ['symbols', file, '--editor-input'], folder, 5000, resolve, 16 * 1024 * 1024,
    JSON.stringify({ files: overlays })));
  assert.equal(result.error, null, result.output);
  return JSON.parse(result.output);
}
function targetAt(index, text, file, offset, folder) {
  const before = text.slice(0, offset);
  return compiler.definitionAt(index, file, before.split('\n').length - 1, before.length - before.lastIndexOf('\n') - 1, folder);
}
for (const [suffix, text] of fixtures) {
  test(`real ${suffix} standard symbols keep resource identity, virtual sources and UTF-16 targets`, async t => {
    const folder = fixture(t, suffix, text), file = path.join(folder, `main.${suffix}`), source = { file };
    const index = await symbols(folder, `main.${suffix}`);
    const standard = index.standard_sources.find(item => item.file === virtual);
    assert.ok(standard);
    assert.equal(index.standard_modules[0].name, 'std.http.server');
    assert.ok(index.standard_modules[0].members.some(item => item.name === 'append_header_text'));
    assert.ok(index.standard_modules[0].members.some(item => item.name === 'is_json_content_type'));
    assert.ok(!index.files.includes(virtual), 'standard library is not a writable project file');
    assert.deepEqual(index.definitions.filter(item => item.kind === 'resource').map(item => item.name), ['Request', 'Response', 'Method', 'Status', 'Options', 'App']);
    assert.ok(!index.definitions.some(item => item.name === 'UNAUTHORIZED'));
    for (const spelling of ['std.http.server', 'Code.UNAUTHORIZED', 'http.text', 'http.is_json_content_type']) {
      const start = text.indexOf(spelling) + (spelling === 'std.http.server' ? 2 : spelling.lastIndexOf('.') + 1);
      const target = targetAt(index, text, file, start + 1, folder);
      assert.equal(target.file, virtual, spelling);
      if (target.length) assert.equal(standard.text.split('\n')[target.line - 1].slice(target.column - 1, target.column - 1 + target.length), spelling.split('.').at(-1));
    }
    const status = text.indexOf('Code.UNAUTHORIZED') + 'Code.'.length;
    assert.equal(features.hoverAt(index, text, status + 2, source).item.signature, 'Code.UNAUTHORIZED: Code');
    const call = text.indexOf('http.text') + 'http.'.length;
    assert.match(features.signatureAt(index, text, call + 'text('.length, source).item.signature, /def http.text\(status: Code, body: view\[str\]\) -> http.Response/);
    const jsonCall = text.indexOf('http.is_json_content_type') + 'http.'.length;
    assert.match(features.hoverAt(index, text, jsonCall + 2, source).item.signature,
      /def http.is_json_content_type\(request: view\[Incoming\]\) -> Result\[bool, Error\]/);
    assert.match(features.signatureAt(index, text, jsonCall + 'is_json_content_type('.length, source).item.signature,
      /def http.is_json_content_type\(request: view\[Incoming\]\) -> Result\[bool, Error\]/);
    const jsonCompletion = features.completionCandidates(index, text, jsonCall + 2, suffix === 'low', source)
      .find(item => item.name === 'is_json_content_type');
    assert.equal(jsonCompletion.kind, 'function');
    assert.equal(features.insertion(jsonCompletion, ''), 'is_json_content_type(${1:request})');
    const typeStart = text.indexOf('http.Response') + 'http.'.length;
    const types = features.completionCandidates(index, text, typeStart + 2, suffix === 'low', source);
    assert.equal(types.find(item => item.name === 'Request').kind, 'resource');
    assert.ok(!types.some(item => item.name === 'text'));
    const pathStart = text.indexOf('request.path') + 'request.'.length;
    const members = features.completionCandidates(index, text, pathStart + 2, suffix === 'low', source);
    assert.ok(members.some(item => item.name === 'is_get' && item.readonly));
    assert.ok(members.some(item => item.name === 'path' && item.type === 'view[str]'));
    const field = targetAt(index, text, file, pathStart + 1, folder);
    assert.equal(field.file, virtual);
    assert.match(standard.text.split('\n')[field.line - 1], /^    path: view\[str\]$/);
    const localStart = text.indexOf('print(token)') + 'print('.length;
    assert.equal(features.hoverAt(index, text, localStart + 1, source).item.signature, 'token: view[str]');
  });
}

test('unsaved dependency standard aliases update sources without saving or building', async t => {
  const text = 'import "helpers.nagi" as helpers\ndef main():\n    print(helpers.response_code())\n';
  const dependency = 'import std.http.server as http\ndef response_code() -> i64:\n    return http.Status.OK.value\n';
  const folder = fixture(t, 'nagi', text), dep = path.join(folder, 'helpers.nagi');
  fs.writeFileSync(dep, dependency);
  const fresh = '# unsaved 😀\n' + dependency.replace('http.Status.OK', 'http.Status.CREATED');
  const index = await symbols(folder, 'main.nagi', [{ file: dep, text: fresh }]);
  const offset = fresh.indexOf('http.Status.CREATED') + 'http.Status.'.length;
  const target = targetAt(index, fresh, dep, offset + 1, folder);
  assert.equal(target.file, virtual);
  assert.match(index.standard_sources[0].text.split('\n')[target.line - 1], /CREATED: Status/);
  assert.equal(fs.readFileSync(dep, 'utf8'), dependency);
  assert.equal(fs.existsSync(path.join(folder, 'build')), false);
});

test('an incomplete initial standard import uses the registry catalog from a saved snapshot', async t => {
  const saved = 'def main():\n    print("hello")\n';
  const folder = fixture(t, 'nagi', saved), file = path.join(folder, 'main.nagi');
  const index = await symbols(folder, 'main.nagi');
  assert.ok(!index.definitions.some(item => item.kind === 'resource'), 'catalog does not introduce an import binding');
  const prefix = 'import std.http.';
  const module = features.completionCandidates(index, prefix, prefix.length, false, { file, saved: true })[0];
  assert.equal(module.name, 'std.http.server');
  const from = 'from std.http.server import Req';
  const resource = features.completionCandidates(index, from, from.length, false, { file, saved: true })[0];
  assert.equal(resource.name, 'Request');
  assert.equal(features.insertion(resource, ''), 'Request');
});

test('shared receiver fields are read-only regardless of each field type', async t => {
  const text = 'class State:\n    count: i64\n    name: str\nclass Holder:\n    state: shared[State]\n' +
    'def read(state: shared[State]) -> i64:\n    return state.count\n' +
    'def inspect(holder: Holder):\n    print(holder)\ndef main():\n    print(0)\n';
  const folder = fixture(t, 'nagi', text), file = path.join(folder, 'main.nagi');
  const index = await symbols(folder, 'main.nagi');
  const state = index.expressions.find(item => item.location.line === 7 && item.type === 'shared[State]');
  assert.ok(state, 'shared receiver has checked type');
  assert.deepEqual(state.fields.map(item => [item.name, item.type, item.readonly]), [['count', 'i64', true], ['name', 'str', true]]);
  const holder = index.expressions.find(item => item.location.line === 9 && item.type === 'Holder');
  assert.ok(holder, 'owned record has checked type');
  assert.deepEqual(holder.fields.map(item => [item.name, item.type, item.readonly ?? false]), [['state', 'shared[State]', false]]);
});
