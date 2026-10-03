'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const features = require('../src/features');
const compiler = require('../src/compiler');
const file = '/tmp/nagi-enums-editor/main.nagi';
const source = { file };
const enumLocation = { file: '/tmp/nagi-enums-editor/errors.nagi', line: 1, column: 6, length: 9 };
const unitLocation = { ...enumLocation, line: 2, column: 5, length: 18 };
const payloadLocation = { ...enumLocation, line: 3, column: 5, length: 12 };
function enumeration(name, qualified = name) {
  return { name, kind: 'enum', location: enumLocation, signature: `enum ${qualified}\n    InvalidCredentials\n    WeakPassword(message: str)`, variants: [
    { name: 'InvalidCredentials', kind: 'enum_member', location: unitLocation, signature: `${qualified}.InvalidCredentials`, parameters: [], return_type: qualified },
    { name: 'WeakPassword', kind: 'enum_member', location: payloadLocation, signature: `${qualified}.WeakPassword(message: str)`, parameters: [{ name: 'message', type: 'str' }], return_type: qualified },
  ] };
}
const direct = enumeration('AuthError');
const imported = enumeration('AuthError', 'errors.AuthError');
const renamed = enumeration('SavedError');
const record = { name: 'Detail', kind: 'class', signature: 'class errors.Detail\n    message: str', fields: [{ name: 'message', type: 'str' }] };
const moduleLocation = { file, line: 1, column: 25, length: 6 };
const index = { format: 'nagi-symbols-v1', definitions: [direct, record], bindings: [
  { file, name: 'AuthError', kind: 'enum', definition: direct },
  { file, name: 'SavedError', kind: 'enum', definition: renamed },
  { file, name: 'errors', kind: 'module', location: moduleLocation, target: { ...enumLocation, line: 1, column: 1, length: 0 }, members: [imported, record] },
], references: [], expressions: [], locals: [] };

test('enum completion includes own/imported type namespaces and never bare variants', () => {
  for (const low of [false, true]) {
    for (const prefix of ['AuthError.', 'SavedError.', 'errors.AuthError.', 'errors . AuthError .']) {
      const items = features.completionCandidates(index, prefix, prefix.length, low, source);
      assert.deepEqual(items.map(item => [item.name, item.kind]), [['InvalidCredentials', 'enum_member'], ['WeakPassword', 'enum_member']], prefix);
      assert.equal(features.insertion(items[0], ''), 'InvalidCredentials');
      assert.equal(features.insertion(items[1], ''), 'WeakPassword(${1:message})');
      assert.equal(features.insertion(items[1], '(value)'), 'WeakPassword');
    }
    const plain = features.completionCandidates(index, '', 0, low, source);
    for (const name of ['AuthError', 'SavedError', 'errors']) assert.ok(plain.some(item => item.name === name), name);
    assert.ok(!plain.some(item => ['InvalidCredentials', 'WeakPassword'].includes(item.name)));
    assert.equal(features.insertion(plain.find(item => item.name === 'AuthError'), ''), 'AuthError');
  }
});

test('enum type positions offer enum types and exclude variant values', () => {
  for (const text of ['def check(error: ', 'fn check() -> ', 'Result[i64, ', 'Result[\n    i64, ']) {
    const items = features.completionCandidates(index, text, text.length, text.startsWith('fn'), source);
    for (const name of ['AuthError', 'SavedError']) {
      const item = items.find(item => item.name === name);
      assert.equal(item.kind, 'enum');
      assert.equal(item.typeOnly, true);
      assert.equal(features.insertion(item, ''), name);
    }
    assert.ok(!items.some(item => item.kind === 'enum_member' || item.name === 'fail'));
  }
  const moduleTypes = 'def check(error: errors.';
  assert.deepEqual(features.completionCandidates(index, moduleTypes, moduleTypes.length, false, source).map(item => item.name), ['AuthError', 'Detail']);
  for (const text of ['def check(error: AuthError.', 'Result[i64, errors.AuthError.']) {
    assert.deepEqual(features.completionCandidates(index, text, text.length, false, source), []);
  }
});

test('unit/payload variant hover and payload signatures preserve alias-qualified spelling', () => {
  for (const prefix of ['AuthError', 'SavedError', 'errors.AuthError', 'errors . AuthError']) {
    const unit = `${prefix}.InvalidCredentials`;
    const payload = `${prefix}.WeakPassword(`;
    const variant = features.hoverAt(index, unit, unit.indexOf('InvalidCredentials') + 2, source);
    assert.equal(variant.item.kind, 'enum_member');
    const hover = features.hoverAt(index, payload, payload.indexOf('WeakPassword') + 2, source);
    const signature = features.signatureAt(index, payload, payload.length, source);
    assert.equal(signature.item.signature, hover.item.signature);
    assert.equal(signature.item.callName, `${prefix.replace(/\s*\.\s*/g, '.')}.WeakPassword`);
    assert.equal(signature.argument, 0);
    assert.equal(features.signatureAt(index, `${unit}(`, unit.length + 1, source), undefined, 'unit variant is a value, not an empty constructor');
  }
  const type = 'def handle() -> Result[i64, SavedError]:';
  assert.equal(features.hoverAt(index, type, type.indexOf('SavedError') + 2, source).item.signature, renamed.signature);
  const nested = 'fail(errors.AuthError.WeakPassword("reason"';
  assert.equal(features.signatureAt(index, nested, nested.length, source).item.return_type, 'errors.AuthError');
  assert.equal(features.signatureAt(index, 'AuthError(', 10, source), undefined, 'enum itself is not constructible');
});

test('nested variant declaration references resolve hover and F12 without exporting names', () => {
  const text = 'enum AuthError:\n    InvalidCredentials\n    WeakPassword(message: str)\n';
  const file = enumLocation.file;
  const checked = { ...index, references: [
    { location: unitLocation, target: unitLocation }, { location: payloadLocation, target: payloadLocation },
  ] };
  assert.equal(features.hoverAt(checked, text, text.indexOf('InvalidCredentials') + 2, { file }).item.signature, direct.variants[0].signature);
  assert.equal(features.hoverAt(checked, text, text.indexOf('WeakPassword') + 2, { file }).item.signature, direct.variants[1].signature);
  const main = 'errors.AuthError.WeakPassword("small")';
  const start = main.indexOf('WeakPassword');
  const navigation = { ...checked, references: [{ location: { file: source.file, line: 1, column: start + 1, length: 12 }, target: payloadLocation }] };
  const target = compiler.definitionAt(navigation, source.file, 0, start + 2, '/tmp/nagi-enums-editor');
  assert.equal(target.file, payloadLocation.file);
  assert.equal(target.line, payloadLocation.line);
  assert.equal(target.column, payloadLocation.column);
  const builtinName = 'enum Problems:\n    print(message: str)\n';
  const location = { file, line: 2, column: 5, length: 5 };
  const printVariant = { name: 'print', kind: 'enum_member', signature: 'Problems.print(message: str)', location, parameters: [{ name: 'message', type: 'str' }] };
  const colliding = { definitions: [{ name: 'Problems', kind: 'enum', signature: 'enum Problems', variants: [printVariant] }], references: [{ location, target: location }] };
  assert.equal(features.hoverAt(colliding, builtinName, builtinName.indexOf('print') + 2, { file }).item.signature, printVariant.signature);
});

test('lexical values that shadow enum/module roots suppress type namespaces', () => {
  for (const text of ['AuthError.', 'SavedError.', 'errors.AuthError.']) {
    const root = text.split('.')[0];
    const rootLocation = { file, line: 1, column: 1, length: root.length };
    const localTarget = { file, line: 4, column: 10, length: root.length };
    for (const extra of [
      { locals: [{ name: root, type: 'AuthError', location: rootLocation }] },
      { references: [{ location: rootLocation, target: localTarget }] },
    ]) {
      const shadowed = { ...index, ...extra };
      assert.deepEqual(features.completionCandidates(shadowed, text, text.length, false, source), [], text);
      const call = `${text}WeakPassword(`;
      assert.equal(features.signatureAt(shadowed, call, call.length, source), undefined, call);
      assert.equal(features.hoverAt(shadowed, call, call.indexOf('WeakPassword') + 1, source), undefined, call);
    }
  }
  for (const text of ['problem.', 'thing.AuthError.', 'thing. errors.AuthError.', '(AuthError).', 'AuthError.InvalidCredentials.', 'errors.Detail.']) {
    assert.deepEqual(features.completionCandidates(index, text, text.length, false, source), [], text);
    const call = `${text}WeakPassword(`;
    assert.equal(features.signatureAt(index, call, call.length, source), undefined, call);
  }
  for (const text of ['AuthError.', 'errors.AuthError.']) {
    assert.deepEqual(features.completionCandidates(index, text, text.length, false, { ...source, saved: true }), []);
  }
});

test('typed failure help exposes an error value without requiring builtin Error', () => {
  const fail = features.declarations().get('fail');
  assert.equal(fail.signature, 'def fail(problem: E) -> Result[T, E]');
  assert.equal(features.declarations().get('ok').return_type, 'Result[T, E]');
  assert.equal(features.insertion(fail, ''), 'fail(${1:problem})');
  const types = features.completionCandidates(undefined, 'value: Res', 10);
  assert.equal(features.insertion(types.find(item => item.name === 'Result'), ''), 'Result[${1:T}, ${2:Error}]');
});

test('Result patterns have completion hover and argument help only after case', () => {
  for (const low of [false, true]) {
    const text = '    case ';
    const items = features.completionCandidates(undefined, text, text.length, low);
    for (const [name, argument] of [['Ok', 'value'], ['Err', 'problem']]) {
      const item = items.find(item => item.name === name);
      assert.equal(item.kind, 'pattern');
      assert.equal(features.insertion(item, ''), `${name}(\${1:${argument}})`);
      assert.equal(features.insertion(item, '('), name);
      const pattern = `case ${name}(`;
      assert.equal(features.signatureAt(undefined, pattern, pattern.length).item.parameters[0].name, argument);
      assert.equal(features.hoverAt(undefined, pattern, 6).item.signature, `case ${name}(${argument})`);
      assert.equal(features.signatureAt(undefined, `${name}(`, name.length + 1), undefined);
      assert.ok(!features.completionCandidates(undefined, '', 0, low).some(item => item.name === name));
    }
  }
  const custom = { name: 'Ok', kind: 'function', signature: 'def Ok(value: i64) -> i64', parameters: [{ name: 'value', type: 'i64' }] };
  assert.equal(features.signatureAt({ definitions: [custom] }, 'Ok(', 3).item.signature, custom.signature, 'ordinary same-name functions keep their help');
});
