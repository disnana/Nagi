'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const features = require('../src/features');
const compiler = require('../src/compiler');
const stdlib = require('../src/stdlib');
const { catalogIndex, checkedIndex, modules } = require('./usability-fixtures');

for (const [suffix, low] of [['nagi', false], ['low', true]]) {
  const file = `/tmp/nagi-usability-editor/main.${suffix}`;
  const source = { file };
  const index = checkedIndex(file);

  test(`${suffix} saved snapshots complete new modules and from-import names before imports are checked`, () => {
    const saved = { file, saved: true };
    const prefix = low ? 'fn helper() {} ' : '';
    for (const [text, expected] of [['import std.res', 'std.result'], ['from std.ac', 'std.actor'], ['import std.http.', 'std.http.server']]) {
      const code = prefix + text;
      const [candidate] = features.completionCandidates(catalogIndex, code, code.length, low, saved);
      assert.equal(candidate.name, expected);
      assert.equal(candidate.kind, 'module');
      assert.equal(candidate.replaceStart, prefix.length + text.indexOf(' ') + 1);
      assert.equal(candidate.replaceEnd, code.length);
      assert.equal(features.insertion(candidate, ''), expected);
    }
    const middle = prefix + 'import std.result as results';
    const [module] = features.completionCandidates(catalogIndex, middle, prefix.length + 'import std.res'.length, low, saved);
    assert.equal(module.replaceEnd, middle.indexOf(' as '));
    for (const [text, expected, kind] of [
      ['from std.result import map_', 'map_error', 'function'],
      ['from std.actor import TaskReady as Ready, WaitK', 'WaitKind', 'resource'],
      ['from std.actor import mark_', 'mark_ready', 'function'],
      ['from std.actor import task_with_', 'task_with_ready', 'function'],
      ['from std.actor import next_event_t', 'next_event_timeout', 'function'],
      ['from std.http.server import is_json_', 'is_json_content_type', 'function'],
    ]) {
      const code = prefix + text;
      const [candidate] = features.completionCandidates(catalogIndex, code, code.length, low, saved);
      assert.equal(candidate.name, expected, code);
      assert.equal(candidate.kind, kind);
      assert.equal(features.insertion(candidate, ''), expected, 'imports insert names without calls or type arguments');
    }
    const alreadyImported = prefix + 'from std.actor import TaskReady as Ready, ';
    const imports = features.completionCandidates(catalogIndex, alreadyImported, alreadyImported.length, low, saved);
    assert.ok(!imports.some(item => item.name === 'TaskReady'));
    for (const text of ['from std.result import Task', 'from std.http.server import Wait']) {
      const code = prefix + text;
      assert.deepEqual(features.completionCandidates(catalogIndex, code, code.length, low, saved), []);
    }
    assert.ok(!features.completionCandidates(catalogIndex, '', 0, low, saved).some(item => item.name === 'TaskReady'));
  });

  test(`${suffix} readiness and wait resources preserve namespaces and do not offer constructors`, () => {
    const types = low ? 'fn inspect(signal: actors.' : 'def inspect(signal: actors.';
    const resources = features.completionCandidates(index, types, types.length, low, source);
    assert.ok(resources.every(item => item.kind === 'resource'));
    for (const name of ['TaskReady', 'WaitError', 'WaitKind', 'EventKind']) {
      const item = resources.find(item => item.name === name);
      assert.ok(item, name);
      assert.equal(item.id.module, 'stdlib:std.actor');
      assert.equal(item.id.kind, 'Resource');
      assert.equal(features.insertion(item, ''), name);
      const call = `actors.${name}(`;
      assert.equal(features.signatureAt(index, call, call.length, source), undefined);
    }
    for (const name of ['Ready', 'WaitFailure']) {
      const call = `${name}(`;
      assert.equal(features.signatureAt(index, call, call.length, source), undefined);
      const item = features.completionCandidates(index, name, name.length, low, source).find(item => item.name === name);
      assert.equal(features.insertion(item, ''), name);
      assert.equal(item.id.name, name === 'Ready' ? 'TaskReady' : 'WaitError');
    }
    for (const namespace of ['actors.TaskReady.', 'Ready.', 'actors.WaitError.', 'WaitFailure.']) {
      assert.deepEqual(features.completionCandidates(index, namespace, namespace.length, low, source), []);
    }
    for (const namespace of ['actors.WaitKind.', 'WaitReason.']) {
      const constants = features.completionCandidates(index, namespace, namespace.length, low, source);
      assert.deepEqual(constants.map(item => [item.name, item.kind]), [['TIMEOUT', 'constant'], ['INVALID_TIMEOUT', 'constant']]);
      for (const item of constants) {
        assert.equal(features.insertion(item, ''), item.name);
        const call = namespace + item.name + '(';
        assert.equal(features.signatureAt(index, call, call.length, source), undefined);
      }
    }
    for (const namespace of ['actors.EventKind.', 'EventType.']) {
      const constants = features.completionCandidates(index, namespace, namespace.length, low, source);
      for (const name of ['STARTING', 'STARTED', 'READY', 'FAILED', 'PANICKED', 'RESTART_SCHEDULED',
        'STOPPED', 'INTENSITY_EXCEEDED', 'SHUTDOWN', 'LAGGED']) assert.ok(constants.some(item => item.name === name), name);
    }
    const timeout = 'WaitReason.INVALID_TIMEOUT';
    assert.equal(features.hoverAt(index, timeout, timeout.indexOf('INVALID') + 2, source).item.signature,
      'WaitReason.INVALID_TIMEOUT: WaitReason');
    const ready = 'actors.EventKind.READY';
    assert.equal(features.hoverAt(index, ready, ready.lastIndexOf('READY') + 2, source).item.return_type, 'actors.EventKind');
    assert.ok(!features.completionCandidates(index, 'actors.', 7, low, source).some(item => item.kind === 'constant'));
    assert.ok(!features.completionCandidates(index, '', 0, low, source).some(item => item.name === 'TIMEOUT' || item.name === 'READY'));
  });

  test(`${suffix} WaitError value fields keep borrowed payload types and read-only status`, () => {
    const waitError = index.bindings.find(binding => binding.name === 'WaitFailure').definition;
    const fields = { ...index, expressions: [{ location: { file, line: 1, column: 1, length: 7 },
      end_line: 1, end_column: 8, type: 'view[WaitFailure]', fields: waitError.fields }] };
    const text = 'problem.message';
    const items = features.completionCandidates(fields, text, text.length, low, source);
    assert.deepEqual(items.map(item => [item.name, item.type, item.readonly]),
      [['kind', 'WaitReason', true], ['message', 'view[str]', true]]);
    assert.equal(features.hoverAt(fields, text, text.indexOf('message') + 2, source).item.signature, 'message: view[str] (read-only)');
    assert.equal(features.insertion(items[1], ''), 'message');
    assert.ok(!items.some(item => item.name === 'TIMEOUT' || item.name === 'INVALID_TIMEOUT'));
    assert.deepEqual(features.completionCandidates(fields, text, text.length, low, { file, saved: true }), []);
  });

  test(`${suffix} inferred standard operations insert positional calls and retain exact callback and Future contracts`, () => {
    const cases = [
      ['results.map_error', ['value', 'mapper'], ['Result[T, E]', 'fn[E, F]'], 'Result[T, F]', false,
        'map_error(${1:value}, ${2:mapper})', ['read_result(view("input,a"), limits)', 'remap']],
      ['actors.task_with_ready', ['group', 'name', 'factory', 'policy'],
        ['view[actors.Supervisor[C]]', 'view[str]', 'fn[shared[C], Ready, Future[Result[unit, Error]]]', 'actors.RestartPolicy'],
        'Result[unit, Error]', false, 'task_with_ready(${1:group}, ${2:name}, ${3:factory}, ${4:policy})',
        ['view(group)', 'view("worker,ready")', 'make_worker', 'actors.RestartPolicy.TRANSIENT']],
      ['actors.mark_ready', ['signal'], ['view[Ready]'], 'Result[unit, Error]', false, 'mark_ready(${1:signal})', ['view(signal)']],
      ['actors.next_event_timeout', ['control', 'timeout_ms'], ['view[actors.Control]', 'i64'],
        'Future[Result[Option[actors.Event], WaitFailure]]', true, 'next_event_timeout(${1:control}, ${2:timeout_ms})', ['view(control)', '1000']],
      ['http.is_json_content_type', ['request'], ['view[Incoming]'], 'Result[bool, Error]', false,
        'is_json_content_type(${1:request})', ['view(request)']],
    ];
    for (const [qualified, names, types, result, asynchronous, insertion, arguments_] of cases) {
      const [candidate] = features.completionCandidates(index, qualified, qualified.length, low, source)
        .filter(item => item.name === qualified.split('.').at(-1));
      assert.ok(candidate, qualified);
      assert.equal(candidate.kind, 'function');
      assert.equal(candidate.id.kind, 'Function');
      assert.deepEqual(candidate.typeParameters, [], 'inferred variables do not request explicit type arguments');
      assert.deepEqual(candidate.parameters.map(item => item.name), names);
      assert.deepEqual(candidate.parameters.map(item => item.type), types);
      assert.equal(candidate.return_type, result);
      assert.equal(candidate.asynchronous, asynchronous);
      assert.equal(features.insertion(candidate, ''), insertion);
      assert.equal(features.insertion(candidate, '('), candidate.name);
      for (let argument = 0; argument < names.length; argument++) {
        const call = qualified + '(' + arguments_.slice(0, argument).join(', ') + (argument ? ', ' : '');
        const help = features.signatureAt(index, call, call.length, source);
        assert.equal(help.argument, argument, call);
        assert.equal(help.item.parameters[argument].name, names[argument]);
        assert.equal(help.item.parameters[argument].type, types[argument]);
        assert.equal(help.item.return_type, result);
        assert.equal(help.item.callName, qualified);
      }
      const call = qualified + '(' + arguments_.join(', ') + ')';
      const hover = features.hoverAt(index, call, qualified.lastIndexOf('.') + 2, source).item;
      assert.equal(hover.id.module, candidate.id.module);
      assert.equal(hover.signature, candidate.signature);
    }
    const alias = 'remap_error(value, ';
    const help = features.signatureAt(index, alias, alias.length, source);
    assert.equal(help.argument, 1);
    assert.equal(help.item.parameters[1].type, 'fn[E, F]');
    assert.deepEqual(help.item.id, { module: 'stdlib:std.result', kind: 'Function', name: 'map_error' });
    assert.equal(features.insertion(help.item, ''), 'remap_error(${1:value}, ${2:mapper})');
  });
}

test('new operations and constants navigate to compiler standard sources with UTF-16 locations', () => {
  const file = '/tmp/nagi-usability-editor/main.nagi';
  const index = checkedIndex(file);
  assert.deepEqual(stdlib.sources(index).map(item => item.file), ['stdlib:std.result', 'stdlib:std.actor', 'stdlib:std.http.server']);
  for (const module of modules) {
    const uri = stdlib.sourceUri(module.id);
    assert.equal(uri, `nagi-stdlib:/${module.name.replaceAll('.', '/')}.nagi`);
    assert.equal(stdlib.sourceFile({ scheme: 'nagi-stdlib', path: uri.slice('nagi-stdlib:'.length) }), module.id);
    const declarations = module.members.flatMap(member => [member, ...(member.constants || [])]);
    for (const declaration of declarations) {
      const text = `print("😀"); ${declaration.name}`;
      const start = text.lastIndexOf(declaration.name);
      const references = [{ location: { file, line: 1, column: start + 1, length: declaration.name.length }, target: declaration.location }];
      const target = compiler.definitionAt({ ...index, references }, file, 0, start + 1, '/tmp/nagi-usability-editor');
      assert.equal(target.file, module.id);
      assert.equal(stdlib.sourceUri(target.file), uri);
      const source = stdlib.sources(index).find(item => item.file === target.file);
      assert.equal(source.text.split('\n')[target.line - 1].slice(target.column - 1, target.column - 1 + target.length), declaration.name);
    }
  }
});
