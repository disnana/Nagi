'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const features = require('../src/features');
const file = '/tmp/nagi-modules-editor/main.nagi';
const location = { file, line: 1, column: 25, length: 6 };
const target = { file: '/tmp/nagi-modules-editor/orders.nagi', line: 2, column: 7, length: 5 };
const order = { id: { module: target.file, kind: 'Class', name: 'Order' }, name: 'Order', kind: 'class', location: target,
  signature: 'class orders.Order\n    value: i32', fields: [{ name: 'value', type: 'i32' }] };
const make = { name: 'make', kind: 'function', location: { ...target, line: 4, length: 4 }, signature: 'def orders.make(value: i32) -> orders.Order',
  parameters: [{ name: 'value', type: 'i32' }], return_type: 'orders.Order' };
const alias = { ...order, name: 'SavedOrder', signature: 'class SavedOrder\n    value: i32' };
const index = { definitions: [order, make], bindings: [
  { file, name: 'orders', kind: 'module', location, target: { ...target, line: 1, column: 1, length: 0 }, members: [order, make] },
  { file, name: 'SavedOrder', kind: 'class', definition_id: order.id, definition: alias },
], references: [], expressions: [], locals: [] };
const source = { file };

test('module member completion exposes functions/classes and type positions insert only classes', () => {
  for (const text of ['orders.', 'orders.ma', 'def inspect(value: orders.):']) {
    const offset = text.indexOf('orders.') + 'orders.'.length + (text.includes('orders.ma') ? 2 : 0);
    const items = features.completionCandidates(index, text, offset, false, source);
    assert.deepEqual(items.map(item => item.name), text.startsWith('def ') ? ['Order'] : ['Order', 'make']);
    if (text.startsWith('def ')) assert.equal(features.insertion(items[0], ''), 'Order');
  }
  const plain = features.completionCandidates(index, '', 0, false, source);
  assert.ok(plain.some(item => item.name === 'orders'));
  assert.ok(plain.some(item => item.name === 'SavedOrder'));
  assert.ok(!plain.some(item => item.name === 'Order' || item.name === 'make'));
});

test('qualified and renamed declarations retain structured hover/signature help', () => {
  const text = 'orders.make(7, ';
  const hover = features.hoverAt(index, text, 9, source);
  assert.equal(hover.item.signature, make.signature);
  const signature = features.signatureAt(index, text, text.length, source);
  assert.equal(signature.item.signature, make.signature);
  assert.equal(signature.argument, 1);
  assert.ok(features.activeCall(text, text.length, true));
  assert.equal(features.hoverAt(index, 'SavedOrder(value=7)', 4, source).item.name, 'SavedOrder');
  assert.equal(features.signatureAt(index, 'SavedOrder(', 11, source).item.name, 'SavedOrder');
  assert.equal(features.hoverAt(index, 'other.make(', 8, source), undefined);
  assert.equal(features.signatureAt(index, 'other.make(', 11, source), undefined);
});

test('lexical alias shadowing switches completion to fields and prevents module help', () => {
  const text = 'orders.local';
  const local = { file, line: 1, column: 1, length: 6 };
  const shadowed = { ...index, references: [{ location: local, target: { ...local, line: 3 } }],
    expressions: [{ location: local, end_line: 1, end_column: 7, type: 'Holder', fields: [{ name: 'local', type: 'bool' }] }] };
  assert.deepEqual(features.completionCandidates(shadowed, text, text.length, false, source).map(item => item.name), ['local']);
  assert.equal(features.hoverAt(shadowed, 'orders.make(', 9, source), undefined);
  assert.equal(features.signatureAt(shadowed, 'orders.make(', 12, source), undefined);
  assert.deepEqual(features.completionCandidates(index, 'orders.', 7, false, { file, saved: true }), []);
});

test('spaced field chains do not borrow a module namespace', () => {
  for (const text of ['object.orders.', 'object. orders.', 'object.\n orders.']) {
    assert.deepEqual(features.completionCandidates(index, text, text.length, false, source), []);
    const call = `${text}make(`;
    assert.equal(features.signatureAt(index, call, call.length, source), undefined);
  }
  assert.equal(features.signatureAt(index, 'orders.Order(', 13, source).item.callName, 'orders.Order');
});

test('imported class and module binding tokens keep their resolved hover', () => {
  const text = 'from "orders.nagi" import Order as SavedOrder';
  const original = text.indexOf('Order');
  const renamed = text.lastIndexOf('SavedOrder');
  const imported = { ...index, references: [original, renamed].map(start => ({
    location: { file, line: 1, column: start + 1, length: start === original ? 5 : 10 }, target,
  })) };
  assert.equal(features.hoverAt(imported, text, original + 1, source).item.kind, 'class');
  assert.equal(features.hoverAt(imported, text, renamed + 1, source).item.kind, 'class');
  const moduleText = 'import "orders.nagi" as orders';
  const start = moduleText.lastIndexOf('orders');
  const binding = { ...index.bindings[0], location: { file, line: 1, column: start + 1, length: 6 } };
  const namespace = { ...index, bindings: [binding], references: [{ location: binding.location, target: binding.target }] };
  assert.equal(features.hoverAt(namespace, moduleText, start + 1, source).item.kind, 'module');
});

test('quoted from imports insert names only and preserve aliases and comma-separated lists', () => {
  const enumeration = { name: 'Issue', kind: 'enum', location: { ...target, line: 6 }, signature: 'enum Issue', variants: [] };
  for (const low of [false, true]) {
    for (const [prefix, expected] of [
      ['Or', ['Order']], ['ma', ['make']], ['Is', ['Issue']],
      ['Order as SavedOrder, ', ['make', 'Issue']], ['Order, ma', ['make']], ['Order as Sav', []],
    ]) {
      const text = `from "orders.nagi" import ${prefix}`;
      const quoted = text.indexOf('"orders.nagi"');
      const imports = { ...index, definitions: [...index.definitions, enumeration], references: [{
        location: { file, line: 1, column: quoted + 1, length: '"orders.nagi"'.length },
        target: { file: target.file, line: 1, column: 1, length: 0 },
      }] };
      const items = features.completionCandidates(imports, text, text.length, low, source);
      assert.deepEqual(items.map(item => item.name), expected, text);
      for (const item of items) {
        assert.equal(item.importOnly, true);
        assert.equal(features.insertion(item, ''), item.name, 'imports never instantiate types or call functions');
      }
    }
  }
  assert.equal(features.insertion(order, ''), 'Order(value=${1:value})', 'expression constructors are unchanged');
  assert.equal(features.insertion(make, ''), 'make(${1:value})');
});

test('unresolved and changed fallback import targets never offer expression completions', () => {
  for (const text of ['from "missing.nagi" import Or', 'from "orders.nagi" import Or']) {
    assert.deepEqual(features.completionCandidates(index, text, text.length, false, source), []);
  }
  const text = 'from "orders.nagi" import Or';
  const imported = { ...index, references: [{ location: { file, line: 1, column: 6, length: 13 }, target: { file: target.file, line: 1, column: 1, length: 0 } }] };
  assert.deepEqual(features.completionCandidates(imported, text, text.length, false, { ...source, saved: true }).map(item => item.name), ['Order']);
  const changed = text.replace('orders.nagi', 'absent.nagi');
  assert.deepEqual(features.completionCandidates(imported, changed, changed.length, false, { ...source, saved: true }), []);
});

test('Low quoted imports after a same-line declaration or semicolon retain name-only insertion', () => {
  for (const prefix of ['fn helper() {} ', 'import "other.low" as other; ', 'fn helper() { print("};"); } ']) {
    const text = prefix + 'from "orders.nagi" import ma';
    const start = text.indexOf('"orders.nagi"');
    const imported = { ...index, references: [{ location: { file, line: 1, column: start + 1, length: 13 },
      target: { file: target.file, line: 1, column: 1, length: 0 } }] };
    const items = features.completionCandidates(imported, text, text.length, true, source);
    assert.deepEqual(items.map(item => item.name), ['make'], prefix);
    assert.equal(features.insertion(items[0], ''), 'make');
  }
});
