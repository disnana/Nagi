'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const features = require('../src/features');
const stdlib = require('../src/stdlib');
const file = '/tmp/nagi-actor-editor/main.nagi';
const actorSource = 'stdlib:std.actor', httpSource = 'stdlib:std.http.server';
const location = module => ({ file: module, line: 2, column: 10, length: 7 });
const handle = { name: 'Actor', kind: 'resource', signature: 'resource actors.Actor[M, R, E]', typeParameters: ['M', 'R', 'E'], location: location(actorSource), constants: [] };
const turn = { name: 'Turn', kind: 'resource', signature: 'resource actors.Turn[S, R, E]', typeParameters: ['S', 'R', 'E'], location: location(actorSource), constants: [] };
const actorOptions = { name: 'Options', kind: 'resource', signature: 'resource actors.Options', location: location(actorSource), constants: [] };
const httpOptions = { ...actorOptions, signature: 'resource http.Options', location: location(httpSource) };
const callKind = { name: 'CallKind', kind: 'resource', signature: 'resource actors.CallKind', location: location(actorSource), constants: [
  { name: 'MAILBOX_FULL', kind: 'constant', signature: 'actors.CallKind.MAILBOX_FULL: actors.CallKind', location: { ...location(actorSource), line: 3 }, return_type: 'actors.CallKind' },
] };
const actorDefault = { name: 'default_options', kind: 'function', signature: 'def actors.default_options() -> ActorConfig', parameters: [], return_type: 'ActorConfig', location: { ...location(actorSource), line: 4 } };
const httpDefault = { ...actorDefault, signature: 'def http.default_options() -> HttpConfig', return_type: 'HttpConfig', location: { ...location(httpSource), line: 4 } };
const call = { name: 'call', kind: 'function', signature: 'def actors.call[M, R, E](actor: view[actors.Actor[M, R, E]], message: M, mailbox_ms: i64, reply_ms: i64) -> Future[Result[Result[R, E], actors.CallError]]',
  typeParameters: ['M', 'R', 'E'], asynchronous: true, parameters: [{ name: 'actor', type: 'view[actors.Actor[M, R, E]]' }, { name: 'message', type: 'M' }, { name: 'mailbox_ms', type: 'i64' }, { name: 'reply_ms', type: 'i64' }], location: { ...location(actorSource), line: 5 } };
const members = [handle, turn, actorOptions, callKind, actorDefault, call];
const index = { definitions: members.concat(httpOptions, httpDefault), bindings: [
  { file, name: 'actors', kind: 'module', target: location(actorSource), members },
  { file, name: 'http', kind: 'module', target: location(httpSource), members: [httpOptions, httpDefault] },
  { file, name: 'Handle', kind: 'resource', definition: { ...handle, name: 'Handle' } },
], standard_modules: [{ name: 'std.http.server', id: httpSource, members: [httpOptions, httpDefault] }, { name: 'std.actor', id: actorSource, members }], references: [], expressions: [], locals: [] };
const source = { file };

test('Actor and Turn snippets preserve all three registered type parameters', () => {
  const code = 'def read(value: actors.):';
  const offset = code.indexOf('actors.') + 'actors.'.length;
  const types = features.completionCandidates(index, code, offset, false, source);
  assert.equal(features.insertion(types.find(item => item.name === 'Actor'), ''), 'Actor[${1:M}, ${2:R}, ${3:E}]');
  assert.equal(features.insertion(types.find(item => item.name === 'Turn'), ''), 'Turn[${1:S}, ${2:R}, ${3:E}]');
  const alias = features.completionCandidates(index, 'value: Han', 10, false, source).find(item => item.name === 'Handle');
  assert.equal(features.insertion(alias, ''), 'Handle[${1:M}, ${2:R}, ${3:E}]');
  assert.equal(features.insertion(alias, '[i64, i64, Error]'), 'Handle');
});

test('same-name standard options retain their owning namespace and return type', () => {
  for (const [code, expected, module] of [['actors.default_options(', 'ActorConfig', actorSource], ['http.default_options(', 'HttpConfig', httpSource]]) {
    const help = features.signatureAt(index, code, code.length, source);
    assert.equal(help.item.return_type, expected);
    assert.equal(help.item.location.file, module);
  }
  const constant = features.hoverAt(index, 'actors.CallKind.MAILBOX_FULL', 20, source).item;
  assert.equal(constant.location.file, actorSource);
  assert.equal(features.insertion(constant, ''), 'MAILBOX_FULL');
  assert.equal(stdlib.sourceUri(actorSource), 'nagi-stdlib:/std/actor.nagi');
});

test('both module catalogs complete imports without merging their members', () => {
  const prefix = 'import std.';
  assert.deepEqual(features.completionCandidates(index, prefix, prefix.length, false, { file, saved: true }).map(item => item.name), ['std.http.server', 'std.actor']);
  const actor = 'from std.actor import Actor as Handle, Tur';
  assert.deepEqual(features.completionCandidates(index, actor, actor.length, false, source).map(item => item.name), ['Turn']);
  const http = 'from std.http.server import Act';
  assert.deepEqual(features.completionCandidates(index, http, http.length, false, source), []);
  const call = 'actors.call[';
  const types = features.completionCandidates(index, call, call.length, false, source);
  assert.ok(types.some(item => item.name === 'Handle' && item.typeOnly));
  assert.ok(!types.some(item => item.name === 'call'));
});
