'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const compiler = require('../src/compiler');
const features = require('../src/features');
const { borrowed, result, actor } = require('./usability-sources');
const root = path.resolve(__dirname, '../../..');

async function checked(t, suffix, text) {
  const folder = fs.mkdtempSync(path.join(os.tmpdir(), 'nagi-usability-navigation-'));
  t.after(() => fs.rmSync(folder, { recursive: true, force: true }));
  const file = path.join(folder, `main.${suffix}`);
  fs.writeFileSync(file, text);
  const response = await new Promise(resolve => compiler.runCheck(compiler.compilerPath('', root, root),
    ['symbols', file, '--editor-input'], folder, 10000, resolve, 16 * 1024 * 1024, JSON.stringify({ files: [] })));
  assert.equal(response.error, null, response.output);
  return { index: JSON.parse(response.output), file, folder, source: { file } };
}

function target(index, text, file, folder, offset) {
  const before = text.slice(0, offset);
  return compiler.definitionAt(index, file, before.split('\n').length - 1, before.length - before.lastIndexOf('\n') - 1, folder);
}

function operation(index, text, source, low, spelling, names, standard) {
  const start = text.indexOf(spelling) + spelling.lastIndexOf('.') + 1;
  const item = features.hoverAt(index, text, start + 2, source).item;
  assert.equal(item.kind, 'function');
  assert.deepEqual(item.typeParameters || [], []);
  assert.deepEqual(item.parameters.map(parameter => parameter.name), names);
  assert.equal(item.location.file, standard);
  assert.deepEqual(features.signatureAt(index, text, start + spelling.split('.').at(-1).length + 1, source).item.parameters, item.parameters);
  const completion = features.completionCandidates(index, text, start + 2, low, source).find(candidate => candidate.name === spelling.split('.').at(-1));
  assert.equal(completion.kind, 'function');
  const expected = `${completion.name}(${names.map((name, index) => `\$\{${index + 1}:${name}\}`).join(', ')})`;
  assert.equal(features.insertion(completion, ''), expected);
  return start;
}

for (const [suffix, text] of borrowed) {
  test(`real ${suffix} borrowed list locals and nested fields keep read-only metadata in editor assistance`, async t => {
    const { index, source } = await checked(t, suffix, text);
    const start = text.indexOf('item.inner');
    const local = features.hoverAt(index, text, start + 2, source).item;
    assert.equal(local.signature, 'item: Entry (read-only borrow)');
    assert.equal(local.type, 'Entry');
    assert.equal(local.borrowed, true);
    assert.equal(local.readonly, true);
    const completion = features.completionCandidates(index, text, start + 2, suffix === 'low', source).find(item => item.name === 'item');
    assert.equal(completion.kind, 'variable');
    assert.equal(features.insertion(completion, ''), 'item');
    for (const field of ['inner', 'name']) {
      const offset = start + 'item.inner.name'.indexOf(field) + 2;
      const hover = features.hoverAt(index, text, offset, source).item;
      assert.equal(hover.signature, `${field}: ${field === 'inner' ? 'Inner' : 'str'} (read-only)`);
      assert.equal(hover.readonly, true);
    }
    const owned = text.indexOf('print(name)') + 'print('.length;
    assert.equal(features.hoverAt(index, text, owned + 2, source).item.signature, 'name: str');
    assert.equal(index.locals.find(item => item.location.line === text.slice(0, owned).split('\n').length && item.name === 'name').borrowed, undefined);
  });
}

for (const [suffix, text] of result) {
  test(`real ${suffix} map_error aliases retain inferred generics and standard virtual navigation`, async t => {
    const { index, source, file, folder } = await checked(t, suffix, text);
    const virtual = 'stdlib:std.result';
    const standard = index.standard_sources.find(item => item.file === virtual);
    assert.ok(standard);
    const start = operation(index, text, source, suffix === 'low', 'result.map_error', ['value', 'mapper'], virtual);
    const destination = target(index, text, file, folder, start + 2);
    assert.equal(destination.file, virtual);
    assert.match(standard.text.split('\n')[destination.line - 1], /def map_error\(/);
    const alias = text.lastIndexOf('convert(');
    assert.match(features.hoverAt(index, text, alias + 2, source).item.signature, /^def convert\(/);
    assert.deepEqual(features.signatureAt(index, text, alias + 'convert('.length, source).item.typeParameters || [], []);
    assert.equal(target(index, text, file, folder, alias + 2).file, virtual);
    assert.ok(index.standard_modules.find(item => item.name === 'std.result').members.some(item => item.name === 'map_error'));
  });
}

for (const [suffix, text] of actor) {
  test(`real ${suffix} readiness and deadline API assistance keeps resource aliases and read-only fields`, async t => {
    const { index, source, file, folder } = await checked(t, suffix, text);
    const virtual = 'stdlib:std.actor';
    for (const [name, names] of [['mark_ready', ['signal']], ['task_with_ready', ['group', 'name', 'factory', 'policy']], ['next_event_timeout', ['control', 'timeout_ms']]]) {
      const start = operation(index, text, source, suffix === 'low', `actors.${name}`, names, virtual);
      assert.equal(target(index, text, file, folder, start + 2).file, virtual);
    }
    const ready = index.bindings.find(item => item.name === 'Signal').definition;
    assert.equal(ready.kind, 'resource');
    assert.equal(ready.id.name, 'TaskReady');
    assert.equal(features.insertion(ready, ''), 'Signal');
    assert.equal(features.signatureAt(index, 'Signal(', 7, source), undefined);
    const wait = text.indexOf('WaitReason.TIMEOUT') + 'WaitReason.'.length;
    const constants = features.completionCandidates(index, text, wait + 2, suffix === 'low', source);
    assert.deepEqual(constants.map(item => [item.name, item.kind]), [['TIMEOUT', 'constant'], ['INVALID_TIMEOUT', 'constant']]);
    assert.equal(target(index, text, file, folder, wait + 2).file, virtual);
    const event = text.indexOf('actors.EventKind.READY') + 'actors.EventKind.'.length;
    assert.equal(features.hoverAt(index, text, event + 2, source).item.kind, 'constant');
    assert.equal(target(index, text, file, folder, event + 2).file, virtual);
    for (const name of ['kind', 'message']) {
      const offset = text.indexOf(`problem.${name}`) + 'problem.'.length;
      const field = features.hoverAt(index, text, offset + 2, source).item;
      assert.equal(field.kind, 'field');
      assert.equal(field.readonly, true);
      assert.equal(field.type, name === 'kind' ? 'WaitReason' : 'view[str]');
      assert.equal(target(index, text, file, folder, offset + 2).file, virtual);
    }
    const prefix = 'from std.actor import TaskR';
    const resource = features.completionCandidates(index, prefix, prefix.length, suffix === 'low', { ...source, saved: true })[0];
    assert.equal(resource.kind, 'resource');
    assert.equal(features.insertion(resource, ''), 'TaskReady');
  });
}
