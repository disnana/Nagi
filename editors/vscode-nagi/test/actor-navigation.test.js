'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const compiler = require('../src/compiler');
const features = require('../src/features');
const { fixtures } = require('./actor-fixtures');
const root = path.resolve(__dirname, '../../..');
const actorSource = 'stdlib:std.actor', httpSource = 'stdlib:std.http.server';
function fixture(t, suffix, text) {
  const base = path.join(root, 'build', 'vscode-actor-navigation');
  fs.mkdirSync(base, { recursive: true });
  const folder = fs.mkdtempSync(path.join(base, 'case-'));
  t.after(() => fs.rmSync(folder, { recursive: true, force: true }));
  fs.writeFileSync(path.join(folder, `main.${suffix}`), text);
  return folder;
}
async function symbols(folder, file, overlays = []) {
  const result = await new Promise(resolve => compiler.runCheck(compiler.compilerPath('', root, root),
    ['symbols', file, '--editor-input'], folder, 10000, resolve, 16 * 1024 * 1024,
    JSON.stringify({ files: overlays })));
  assert.equal(result.error, null, result.output);
  return JSON.parse(result.output);
}
function targetAt(index, text, file, offset, folder) {
  const before = text.slice(0, offset);
  return compiler.definitionAt(index, file, before.split('\n').length - 1, before.length - before.lastIndexOf('\n') - 1, folder);
}
for (const [suffix, text] of fixtures) {
  test(`real ${suffix} actor symbols separate modules and preserve three-parameter handles`, async t => {
    const folder = fixture(t, suffix, text), file = path.join(folder, `main.${suffix}`), source = { file };
    const index = await symbols(folder, `main.${suffix}`);
    assert.deepEqual(index.standard_sources.map(item => item.file).sort(), [actorSource, httpSource].sort());
    const actor = index.definitions.find(item => item.id?.module === actorSource && item.name === 'Actor');
    const turn = index.definitions.find(item => item.id?.module === actorSource && item.name === 'Turn');
    assert.deepEqual(actor.typeParameters, ['M', 'R', 'E']);
    assert.deepEqual(turn.typeParameters, ['S', 'R', 'E']);
    const typeOffset = text.indexOf('Handle[i64') + 2;
    const type = features.completionCandidates(index, text, typeOffset, suffix === 'low', source).find(item => item.name === 'Handle');
    assert.equal(features.insertion(type, ''), 'Handle[${1:M}, ${2:R}, ${3:E}]');
    for (const [spelling, module] of [['std.actor', actorSource], ['std.http.server', httpSource], ['actors.default_options', actorSource], ['http.default_options', httpSource]]) {
      const start = text.indexOf(spelling) + (spelling.startsWith('std.') ? 1 : spelling.lastIndexOf('.') + 1);
      const target = targetAt(index, text, file, start + 1, folder);
      assert.equal(target.file, module, spelling);
    }
    const actorCall = text.indexOf('actors.default_options(') + 'actors.'.length;
    const httpCall = text.indexOf('http.default_options(') + 'http.'.length;
    const actorHelp = features.signatureAt(index, text, actorCall + 'default_options('.length, source).item;
    const httpHelp = features.signatureAt(index, text, httpCall + 'default_options('.length, source).item;
    assert.equal(actorHelp.return_type, 'ActorConfig');
    assert.equal(httpHelp.return_type, 'HttpConfig');
    assert.equal(actorHelp.location.file, actorSource);
    assert.equal(httpHelp.location.file, httpSource);
    const call = text.indexOf('actors.call(') + 'actors.'.length;
    const callHelp = features.signatureAt(index, text, call + 'call('.length, source).item;
    assert.equal(callHelp.asynchronous, true);
    assert.match(callHelp.signature, /Future\[Result\[Result\[R, E\], actors.CallError\]\]/);
    const utf16 = text.lastIndexOf('actors.CallKind.MAILBOX_FULL') + 'actors.CallKind.'.length;
    const constant = targetAt(index, text, file, utf16 + 1, folder);
    assert.equal(constant.file, actorSource);
    assert.match(index.standard_sources.find(item => item.file === actorSource).text.split('\n')[constant.line - 1], /MAILBOX_FULL: CallKind/);
    const eventField = text.indexOf('event.child_name') + 'event.'.length;
    const fields = features.completionCandidates(index, text, eventField + 2, suffix === 'low', source);
    assert.ok(fields.some(item => item.name === 'child_name' && item.type === 'view[str]' && item.readonly));
    assert.ok(fields.some(item => item.name === 'lost_events' && item.type === 'i64' && item.readonly));
    assert.equal(targetAt(index, text, file, eventField + 1, folder).file, actorSource);
  });
}

test('unsaved actor dependency constants keep their own source without writing or building', async t => {
  const text = 'import "helpers.nagi" as helper\ndef main():\n    print(helper.missing())\n';
  const dependency = 'import std.actor as actors\ndef missing() -> actors.CallKind:\n    return actors.CallKind.NOT_READY\n';
  const folder = fixture(t, 'nagi', text), dep = path.join(folder, 'helpers.nagi');
  fs.writeFileSync(dep, dependency);
  const fresh = '# unsaved 😀\n' + dependency.replace('NOT_READY', 'STOPPED');
  const index = await symbols(folder, 'main.nagi', [{ file: dep, text: fresh }]);
  assert.deepEqual(index.standard_sources.map(item => item.file), [actorSource]);
  const offset = fresh.indexOf('CallKind.STOPPED') + 'CallKind.'.length;
  const target = targetAt(index, fresh, dep, offset + 1, folder);
  assert.equal(target.file, actorSource);
  assert.equal(fs.readFileSync(dep, 'utf8'), dependency);
  assert.equal(fs.existsSync(path.join(folder, 'build')), false);
});
