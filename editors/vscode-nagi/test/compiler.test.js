'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const compiler = require('../src/compiler');

test('Windows diagnostics preserve the drive and convert to zero-based line', () => {
  const parsed = compiler.parseDiagnostics('error: line 8: wrong type\n --> C:\\my project\\demo.nagi:8\n 8 | x\n', 'fallback');
  assert.deepEqual(parsed, [{ file: 'C:\\my project\\demo.nagi', line: 7, message: 'line 8: wrong type' }]);
});

test('Low integration errors and process errors have usable fallback locations', () => {
  assert.equal(compiler.parseDiagnostics('line 12: moved value', '/demo.nagi')[0].line, 11);
  assert.equal(compiler.parseDiagnostics('spawn nagic ENOENT', '/demo.nagi')[0].line, 0);
});
test('Rust arguments remain separate and long Windows paths normalize', () => {
  const root = path.resolve('example root');
  const args = compiler.argumentsFor('build', '/demo.nagi', [], root, root, 'native.rs', ['serde_json=1.0']);
  assert.deepEqual(args.slice(2), ['--rust', path.join(root, 'native.rs'), '--rust-dep', 'serde_json=1.0']);
  if (process.platform === 'win32') assert.equal(compiler.normalizeFile('\\\\?\\C:\\project\\a.nagi', root), 'C:\\project\\a.nagi');
});

test('files and native paths are individual arguments with isolated check output', () => {
  const root = path.resolve('example root');
  const file = path.join(root, 'hello world.nagi');
  const args = compiler.argumentsFor('check', file, ['native math.low'], root, root);
  assert.equal(args[1], file);
  assert.deepEqual(args.slice(2, 4), ['--native', path.join(root, 'native math.low')]);
  assert.equal(args[4], '--out');
  assert.notEqual(args[5], compiler.argumentsFor('check', file + '.other', [], root, root)[3]);
});

test('configured compiler paths are workspace relative; PATH remains a fallback', () => {
  const root = path.resolve('example root');
  assert.equal(compiler.compilerPath('tools/nagic', root, root), path.join(root, 'tools', 'nagic'));
  assert.equal(compiler.compilerPath('', root, root, 'win32'), 'nagic.exe');
});

test('compiler execution does not interpret shell metacharacters', async () => {
  const literal = 'spaces & $(not-a-command)';
  const result = await new Promise(resolve => compiler.runCheck(process.execPath,
    ['-e', 'process.stdout.write(process.argv[1])', literal], process.cwd(), 5000, resolve));
  assert.equal(result.error, null);
  assert.equal(result.output, literal);
});

test('real Nagi checker diagnoses a source error', async () => {
  const root = path.resolve(__dirname, '../../..');
  const exe = compiler.compilerPath('', root, root);
  const folder = path.join(root, 'build', 'vscode-extension-tests');
  fs.mkdirSync(folder, { recursive: true });
  const file = path.join(folder, 'invalid source.nagi');
  fs.writeFileSync(file, 'def main():\n    x: i32 = "wrong"\n');
  const result = await new Promise(resolve => compiler.runCheck(exe,
    compiler.argumentsFor('check', file, [], root, root), root, 5000, resolve));
  assert.equal(result.error.code, 1);
  assert.equal(compiler.parseDiagnostics(result.output, file)[0].line, 1);
  assert.match(result.output, /expected i32/);
});

test('nearest manifest wins and project arguments delegate entry selection to nagic', () => {
  const root = path.resolve(__dirname, '../../..');
  const folder = path.join(root, 'build', 'vscode-project-tests');
  fs.mkdirSync(path.join(folder, 'nested', 'src'), { recursive: true });
  const outer = path.join(folder, 'nagi.toml');
  const inner = path.join(folder, 'nested', 'nagi.toml');
  fs.writeFileSync(outer, "entry = 'main.nagi'\n");
  fs.writeFileSync(inner, "entry = 'main.nagi'\n");
  const helper = path.join(folder, 'nested', 'src', 'helper.nagi');
  assert.equal(compiler.findProject(helper), inner);
  const args = compiler.argumentsFor('check', helper, [], path.dirname(inner), root, '', [], inner);
  assert.deepEqual(args.slice(0, 3), ['check', '--project', inner]);
  assert.equal(args.includes(helper), false);
  assert.equal(args.at(-1), compiler.argumentsFor('check', path.join(path.dirname(inner), 'main.nagi'), [], path.dirname(inner), root, '', [], inner).at(-1));
  fs.unlinkSync(inner);
  assert.equal(compiler.findProject(helper), outer);
  fs.unlinkSync(outer);
  assert.equal(compiler.findProject(helper), undefined);
});

test('real checker resolves a helper through its manifest entry rather than checking it alone', async () => {
  const root = path.resolve(__dirname, '../../..');
  const folder = path.join(root, 'build', 'vscode-entry-test');
  fs.mkdirSync(folder, { recursive: true });
  const manifest = path.join(folder, 'nagi.toml');
  const helper = path.join(folder, 'helper.nagi');
  fs.writeFileSync(manifest, "entry = 'entry.nagi'\n");
  fs.writeFileSync(path.join(folder, 'entry.nagi'), 'import "helper.nagi"\ndef answer() -> i64:\n    return 42\ndef main():\n    print(helper())\n');
  fs.writeFileSync(helper, 'def helper() -> i64:\n    return answer()\n');
  const exe = compiler.compilerPath('', root, root);
  const result = await new Promise(resolve => compiler.runCheck(exe,
    compiler.argumentsFor('check', helper, [], folder, root, '', [], manifest), folder, 5000, resolve));
  assert.equal(result.error, null, result.output);
  assert.match(result.output, /entry.nagi/);
});
