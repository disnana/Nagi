'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const compiler = require('../src/compiler');
const features = require('../src/features');
const root = path.resolve(__dirname, '../../..');
function fixture(t, files) {
  const base = path.join(root, 'build', 'vscode-enum-navigation');
  fs.mkdirSync(base, { recursive: true });
  const folder = fs.mkdtempSync(path.join(base, 'case-'));
  t.after(() => fs.rmSync(folder, { recursive: true, force: true }));
  for (const [name, text] of Object.entries(files)) fs.writeFileSync(path.join(folder, name), text);
  return folder;
}
async function symbols(folder, file, overlays = []) {
  const result = await new Promise(resolve => compiler.runCheck(compiler.compilerPath('', root, root),
    ['symbols', file, '--editor-input'], folder, 5000, resolve, 16 * 1024 * 1024,
    JSON.stringify({ files: overlays })));
  assert.equal(result.error, null, result.output);
  return JSON.parse(result.output);
}
const variants = ['InvalidCredentials', 'WeakPassword'];
const fixtures = [
  ['nagi', 'enum AuthError:\n    InvalidCredentials\n    WeakPassword(message: str)\n',
    'import "errors.nagi" as errors\nfrom "errors.nagi" import AuthError as SavedError\ndef classify(problem: SavedError) -> str:\n    match problem:\n        case SavedError.InvalidCredentials:\n            return "invalid"\n        case SavedError.WeakPassword(message):\n            return message\ndef login(message: str) -> Result[i64, errors.AuthError]:\n    return fail(errors.AuthError.WeakPassword(message))\ndef main():\n    problem = SavedError.InvalidCredentials\n    print(classify(problem))\n'],
  ['low', 'enum AuthError { InvalidCredentials; WeakPassword(message: str); }\n',
    'import "errors.low" as errors;\nfrom "errors.low" import AuthError as SavedError;\nfn classify(problem: SavedError) -> str { match problem { case SavedError.InvalidCredentials { return "invalid"; } case SavedError.WeakPassword(message) { return message; } } }\nfn login(message: str) -> Result[i64, errors.AuthError] { return fail(errors.AuthError.WeakPassword(message)); }\nfn main() { let problem = SavedError.InvalidCredentials; print(classify(problem)); }\n'],
];

for (const [extension, dependency, text] of fixtures) {
  test(`real ${extension} enum symbols preserve aliases, payload help and declaration targets`, async t => {
    const entry = `main.${extension}`, dep = `errors.${extension}`;
    const folder = fixture(t, { [entry]: text, [dep]: dependency });
    const file = path.join(folder, entry), index = await symbols(folder, entry), source = { file };
    const declaration = index.definitions.find(item => item.kind === 'enum');
    assert.deepEqual(declaration.variants.map(item => item.name), variants);
    assert.ok(!index.definitions.some(item => item.kind === 'enum_member'), 'variant names remain inside their enum');
    const variant = text.indexOf('errors.AuthError.WeakPassword') + 'errors.AuthError.'.length;
    const hover = features.hoverAt(index, text, variant + 2, source);
    assert.equal(hover.item.kind, 'enum_member');
    assert.equal(hover.item.signature, 'errors.AuthError.WeakPassword(message: str)');
    assert.equal(features.signatureAt(index, text, variant + 'WeakPassword('.length, source).item.signature, hover.item.signature);
    const before = text.slice(0, variant), line = before.split('\n').length - 1, column = before.length - before.lastIndexOf('\n') - 1;
    const target = compiler.definitionAt(index, file, line, column + 1, folder);
    assert.equal(compiler.normalizeFile(target.file, folder), fs.realpathSync(path.join(folder, dep)));
    assert.equal(target.line, extension === 'nagi' ? 3 : 1);
    assert.equal(dependency.split('\n')[target.line - 1].slice(target.column - 1, target.column - 1 + target.length), 'WeakPassword');
    const alias = text.indexOf('SavedError.WeakPassword') + 'SavedError.'.length;
    assert.equal(features.hoverAt(index, text, alias + 2, source).item.signature, 'SavedError.WeakPassword(message: str)');
    const type = text.indexOf('Result[i64, errors.AuthError]') + 'Result[i64, errors.'.length;
    const items = features.completionCandidates(index, text, type + 2, extension === 'low', source);
    assert.deepEqual(items.filter(item => item.kind === 'enum').map(item => item.name), ['AuthError']);
    assert.equal(features.insertion(items.find(item => item.kind === 'enum'), ''), 'AuthError');
    const prefix = text.indexOf('SavedError.InvalidCredentials', text.indexOf('main')) + 'SavedError.'.length;
    assert.deepEqual(features.completionCandidates(index, text, prefix + 2, extension === 'low', source).map(item => item.name), variants);
  });
}

test('unsaved enum changes update payloads and F12 without writing source or building', async t => {
  const [, dependency, text] = fixtures[0];
  const folder = fixture(t, { 'main.nagi': text, 'errors.nagi': dependency });
  const freshDependency = '# unsaved 😀\n' + dependency.replace('WeakPassword(message: str)', 'WeakPassword(reason: str)');
  const index = await symbols(folder, 'main.nagi', [{ file: 'errors.nagi', text: freshDependency }]);
  const start = text.indexOf('errors.AuthError.WeakPassword') + 'errors.AuthError.'.length;
  const source = { file: path.join(folder, 'main.nagi') };
  assert.equal(features.hoverAt(index, text, start + 2, source).item.parameters[0].name, 'reason');
  const before = text.slice(0, start);
  const target = compiler.definitionAt(index, source.file, before.split('\n').length - 1, before.length - before.lastIndexOf('\n'), folder);
  assert.equal(target.line, 4);
  assert.equal(fs.readFileSync(path.join(folder, 'errors.nagi'), 'utf8'), dependency);
  assert.equal(fs.existsSync(path.join(folder, 'build')), false);
});
