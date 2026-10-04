'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const features = require('../src/features');
const stdlib = require('../src/stdlib');
const file = '/tmp/nagi-stdlib-editor/main.nagi';
const virtual = 'stdlib:std.http.server';
const location = { file: virtual, line: 2, column: 10, length: 6 };
const status = { name: 'Status', kind: 'resource', signature: 'resource http.Status', fields: [
  { name: 'value', type: 'i64', readonly: true }, { name: 'phrase', type: 'view[str]', readonly: true },
], location, constants: [
  { name: 'OK', kind: 'constant', signature: 'http.Status.OK: http.Status', location: { ...location, line: 3, length: 2 }, return_type: 'http.Status' },
  { name: 'UNAUTHORIZED', kind: 'constant', signature: 'http.Status.UNAUTHORIZED: http.Status', location: { ...location, line: 4, length: 12 }, return_type: 'http.Status' },
] };
const request = { name: 'Request', kind: 'resource', signature: 'resource http.Request', fields: [], constants: [], location: { ...location, line: 5, length: 7 } };
const app = { name: 'App', kind: 'resource', signature: 'resource http.App[S, E]', typeParameters: ['S', 'E'], location, constants: [] };
const text = { name: 'text', kind: 'function', signature: 'def http.text(status: http.Status, body: view[str]) -> http.Response', parameters: [
  { name: 'status', type: 'http.Status' }, { name: 'body', type: 'view[str]' },
], return_type: 'http.Response', location: { ...location, line: 6, length: 4 } };
const index = { definitions: [status, request, app, text], bindings: [
  { file, name: 'http', kind: 'module', target: { ...location, line: 1, column: 1, length: 0 }, members: [status, request, app, text] },
  { file, name: 'Code', kind: 'resource', definition: { ...status, name: 'Code', signature: 'resource Code', constants: status.constants.map(item => ({ ...item, signature: item.signature.replaceAll('http.Status', 'Code') })) } },
], standard_modules: [{ name: 'std.http.server', id: virtual, members: [status, request, app, text] }], references: [], expressions: [], locals: [] };
const source = { file };

test('registered resources and constants keep namespaces and never create constructor snippets', () => {
  const all = features.completionCandidates(index, 'http.', 5, false, source);
  assert.deepEqual(all.map(item => item.name), ['Status', 'Request', 'App', 'text']);
  for (const name of ['http.Status.', 'Code.']) {
    const items = features.completionCandidates(index, name, name.length, false, source);
    assert.deepEqual(items.map(item => item.name), ['OK', 'UNAUTHORIZED']);
    assert.equal(features.insertion(items[0], ''), 'OK');
  }
  const typeText = 'def handle(request: http.):';
  const types = features.completionCandidates(index, typeText, typeText.indexOf('http.') + 5, false, source);
  assert.deepEqual(types.map(item => item.name), ['Status', 'Request', 'App']);
  assert.equal(features.insertion(types.find(item => item.name === 'Request'), ''), 'Request');
  assert.equal(features.insertion(types.find(item => item.name === 'App'), ''), 'App[${1:S}, ${2:E}]');
  assert.equal(features.insertion(types.find(item => item.name === 'App'), '[State, Error]'), 'App');
  assert.ok(!features.completionCandidates(index, '', 0, false, source).some(item => item.name === 'OK'));
  assert.equal(features.signatureAt(index, 'Code(', 5, source), undefined);
});

test('standard operation and aliased constants preserve hover and argument help', () => {
  const call = 'http.text(http.Status.OK, ';
  assert.match(features.hoverAt(index, call, 7, source).item.signature, /def http.text/);
  const help = features.signatureAt(index, call, call.length, source);
  assert.equal(help.argument, 1);
  assert.equal(help.item.parameters[1].name, 'body');
  assert.equal(features.hoverAt(index, 'Code.UNAUTHORIZED', 8, source).item.signature, 'Code.UNAUTHORIZED: Code');
  assert.equal(features.signatureAt(index, 'Code.UNAUTHORIZED(', 18, source), undefined);
});

test('typed resource fields are read-only and do not expose type constants on values', () => {
  const value = 'code.phrase';
  const local = { file, line: 1, column: 1, length: 4 };
  const typed = { ...index, expressions: [{ location: local, end_line: 1, end_column: 5, type: 'Code', fields: status.fields }] };
  const items = features.completionCandidates(typed, value, value.length, false, source);
  assert.deepEqual(items.map(item => item.name), ['value', 'phrase']);
  assert.equal(items.find(item => item.name === 'phrase').signature, 'phrase: view[str] (read-only)');
  assert.match(features.hoverAt(typed, value, 7, source).item.signature, /read-only/);
  assert.ok(!items.some(item => item.name === 'OK'));
  const shadowed = { ...index, references: [{ location: local, target: { ...local, line: 4 } }],
    expressions: [{ location: local, end_line: 1, end_column: 5, type: 'Holder', fields: [{ name: 'local', type: 'bool' }] }],
    bindings: [...index.bindings, { ...index.bindings[1], name: 'code' }] };
  assert.deepEqual(features.completionCandidates(shadowed, 'code.', 5, false, source).map(item => item.name), ['local']);
});

test('virtual source identifiers cannot navigate to project paths or URLs', () => {
  assert.equal(stdlib.sourceUri(virtual), 'nagi-stdlib:/std/http/server.nagi');
  assert.equal(stdlib.sourceFile({ scheme: 'nagi-stdlib', path: '/std/http/server.nagi' }), virtual);
  for (const file of ['stdlib:../secret', 'stdlib:std.http/../secret', 'stdlib:https://example.com', '/std/http/server.nagi', 'stdlib:std.http.server#x']) {
    assert.equal(stdlib.sourceUri(file), undefined);
  }
  for (const uri of [{ scheme: 'file', path: '/std/http/server.nagi' }, { scheme: 'nagi-stdlib', path: '/std/../server.nagi' },
    { scheme: 'nagi-stdlib', path: '/std/http/server.nagi', authority: 'remote' }]) assert.equal(stdlib.sourceFile(uri), undefined);
  assert.deepEqual(stdlib.sources({ standard_sources: [{ file: virtual, text: 'resource Status' }, { file: 'stdlib:../x', text: 'x' }, { file: virtual, text: 7 }] }),
    [{ file: virtual, text: 'resource Status' }]);
});

test('Some and None are pattern assistance, not general value constructors', () => {
  for (const code of ['    case So', '    case No']) {
    const candidates = features.completionCandidates(undefined, code, code.length);
    assert.ok(candidates.some(item => item.name === 'Some'));
    assert.ok(candidates.some(item => item.name === 'None'));
    assert.equal(features.insertion(candidates.find(item => item.name === 'Some'), ''), 'Some(${1:value})');
    assert.equal(features.insertion(candidates.find(item => item.name === 'None'), ''), 'None');
  }
  assert.equal(features.hoverAt(undefined, 'case Some(value):', 7).item.signature, 'case Some(value)');
  assert.equal(features.signatureAt(undefined, 'case Some(', 10).item.parameters[0].name, 'value');
  assert.equal(features.signatureAt(undefined, 'Some(', 5), undefined);
  assert.ok(!features.completionCandidates(undefined, '', 0).some(item => item.name === 'Some'));
  assert.equal(features.signatureAt(undefined, 'case None(', 10), undefined);
});

test('standard import completion uses the compiler catalog and replaces the full dotted path', () => {
  const code = 'import std.http.';
  const candidate = features.completionCandidates(index, code, code.length, false, { file, saved: true })[0];
  assert.equal(candidate.name, 'std.http.server');
  assert.equal(candidate.replaceStart, 'import '.length);
  assert.equal(candidate.replaceEnd, code.length);
  const mid = 'import std.http.server as http';
  const middle = features.completionCandidates(index, mid, 'import std.ht'.length, false, source)[0];
  assert.equal(middle.replaceEnd, mid.indexOf(' as '));
  const imported = 'from std.http.server import Request as Incoming, St';
  const imports = features.completionCandidates(index, imported, imported.length, false, { file, saved: true });
  assert.deepEqual(imports.map(item => item.name), ['Status']);
  assert.equal(features.insertion(imports[0], ''), 'Status');
  const functionCode = 'from std.http.server import te';
  const operation = features.completionCandidates(index, functionCode, functionCode.length, false, source)[0];
  assert.equal(features.insertion(operation, ''), 'text');
  assert.ok(!features.completionCandidates(index, 'import "models.nagi" ', 21, false, source).some(item => item.name === 'std.http.server'));
});

test('Low standard imports can follow another declaration or import on the same line', () => {
  for (const prefix of ['fn helper() {} ', 'import "models.low" as models; ']) {
    const text = prefix + 'import std.http.';
    const [item] = features.completionCandidates(index, text, text.length, true, source);
    assert.equal(item.name, 'std.http.server');
    assert.equal(item.replaceStart, prefix.length + 'import '.length);
    assert.equal(item.replaceEnd, text.length);
    const names = prefix + 'from std.http.server import te';
    const [operation] = features.completionCandidates(index, names, names.length, true, source);
    assert.equal(operation.name, 'text');
    assert.equal(features.insertion(operation, ''), 'text');
  }
  const nested = 'fn main() { print(1); import std.http.';
  assert.deepEqual(features.completionCandidates(index, nested, nested.length, true, source), []);
});
