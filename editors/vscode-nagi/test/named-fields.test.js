'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const { spawnSync } = require('node:child_process');
const features = require('../src/features');

const point = { name: 'Point', kind: 'class', signature: 'class Point\n    x: f64\n    y: f64',
  fields: [{ name: 'x', type: 'f64' }, { name: 'y', type: 'f64' }] };
const index = { definitions: [point] };
const fields = (text, offset = text.length, snapshot = index, source = {}) =>
  features.completionCandidates(snapshot, text, offset, false, source).filter(item => item.namedField);

test('class signature help follows the field label instead of its argument position', () => {
  for (const [text, expected] of [['Point(y=', 1], ['Point(y=2.5, x=', 0],
    ['Point(y=env("a,b", "c"), x=', 0], ['Point(\r\n    y=2.5,\r\n    x=', 0]]) {
    assert.equal(features.signatureAt(index, text, text.length).argument, expected, text);
  }
  assert.equal(features.signatureAt(index, 'Point(unknown=', 14), undefined);
  const functions = { definitions: [{ name: 'read', kind: 'function', signature: 'def read(x: i64, y: i64)',
    parameters: point.fields }] };
  assert.equal(features.signatureAt(functions, 'read(y=2, x=', 12).argument, 1,
    'this change does not opt ordinary functions into named-call semantics');
});

test('class field labels complete with values and keep an existing equals sign', () => {
  assert.deepEqual(fields('Point(').map(item => [item.name, item.type]), [['x', 'f64'], ['y', 'f64']]);
  const [item] = fields('Point(y=2.5, x');
  assert.equal(item.name, 'x');
  assert.equal(item.signature, 'x: f64');
  assert.equal(features.insertion(item, ''), 'x=${1:x}');
  assert.equal(features.insertion(item, ' = 1.5)'), 'x');
  const commented = 'Point(y=2.5, x # label comment\n =1.5)';
  const [withEquals] = fields(commented, commented.indexOf('x #') + 1);
  assert.equal(features.insertion(withEquals, commented.slice(commented.indexOf('x #') + 1)), 'x');
  assert.deepEqual(fields('Point(x=1.5, ').map(item => item.name), ['y']);
  const text = 'print("😀"); Point(x, y=2.5)';
  assert.deepEqual(fields(text, text.indexOf('Point(x') + 7).map(item => item.name), ['x'],
    'fields already supplied after the caret also stay excluded');
  const typo = 'Point(xx=1.5, y=2.5)';
  assert.deepEqual(fields(typo, typo.indexOf('xx') + 1).map(item => item.name), ['x'],
    'a partial prefix can replace an unknown field spelling without losing its value');
});

test('unfinished call exploration is finite and fails closed beyond its scan and depth budgets', () => {
  const script = `
    const assert = require('node:assert/strict');
    const f = require(${JSON.stringify(require.resolve('../src/features'))});
    const index = { definitions: [{name:'Point',kind:'class',signature:'class Point',fields:[{name:'x',type:'f64'}]}] };
    for (const text of ['('.repeat(100000), ')'.repeat(100000), 'Point(' + ' '.repeat(100000)]) {
      assert.equal(f.signatureAt(index, text, text.length), undefined);
      assert.ok(!f.completionCandidates(index, text, text.length).some(item => item.namedField));
    }
    const forward = 'Point(' + '('.repeat(100000);
    assert.ok(!f.completionCandidates(index, forward, 6).some(item => item.namedField));
    for (const text of ['models.' + ' '.repeat(2000) + 'Point(', 'x'.repeat(2000) + 'Point(']) {
      assert.equal(f.signatureAt(index, text, text.length), undefined);
      assert.ok(!f.completionCandidates(index, text, text.length).some(item => item.namedField));
    }
  `;
  const result = spawnSync(process.execPath, ['-e', script], { encoding: 'utf8', timeout: 3000, maxBuffer: 4096 });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stdout + result.stderr);
});

test('field label completion does not replace value expressions or nested calls', () => {
  for (const text of ['Point(x=va', 'Point(x=env(', 'Point(x + ', 'Point(x == ', '# Point(', '"Point(', 'def Point(', 'fn Point(', 'case Point(']) {
    assert.deepEqual(fields(text), [], text);
  }
  assert.deepEqual(fields('print('), []);
  assert.deepEqual(fields('Point(', 6, null), []);
});

test('field labels respect current compiler shadowing and conservative saved declarations', () => {
  const text = 'Point(';
  const shadowed = { ...index, locals: [{ name: 'Point', type: 'i64',
    location: { file: '/main.nagi', line: 1, column: 1, length: 5 } }] };
  assert.deepEqual(fields(text, text.length, shadowed, { file: '/main.nagi' }), []);
  assert.deepEqual(fields('Point = callable\nPoint(', 23, index, { saved: true }), []);
  assert.deepEqual(fields('import missing\nPoint(', 21, index, { saved: true }), []);
  assert.deepEqual(fields('Point(y=2.5, x', 14, index, { saved: true }).map(item => item.name), ['x'],
    'a saved declaration remains an explicitly labelled declaration aid, never an inferred value type');
});

test('qualified class calls use the current compiler namespace and preserve class insertion', () => {
  const namespaced = { definitions: [], bindings: [{ name: 'models', file: '/main.nagi', kind: 'module', members: [point] }] };
  const text = 'models.Point(y=2.5, x';
  const source = { file: '/main.nagi' };
  assert.deepEqual(fields(text, text.length, namespaced, source).map(item => item.name), ['x']);
  assert.equal(features.signatureAt(namespaced, text + '=', text.length + 1, source).argument, 0);
  assert.deepEqual(fields(text, text.length, namespaced, { ...source, saved: true }), []);
  const pattern = 'case models.Point(';
  assert.deepEqual(fields(pattern, pattern.length, namespaced, source), []);
  assert.equal(features.insertion(point, ''), 'Point(x=${1:x}, y=${2:y})');
});
