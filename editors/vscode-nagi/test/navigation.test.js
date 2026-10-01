'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const compiler = require('../src/compiler');

const root = path.resolve(__dirname, '../../..');
function fixture(t, files) {
  const folder = fs.mkdtempSync(path.join(os.tmpdir(), 'nagi-navigation-'));
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
function target(index, folder, file, text, needle, delta = 0) {
  const offset = text.indexOf(needle) + delta;
  assert.ok(text.indexOf(needle) >= 0, needle);
  const prefix = text.slice(0, offset);
  const line = prefix.split('\n').length - 1;
  const column = prefix.length - prefix.lastIndexOf('\n') - 1;
  return compiler.definitionAt(index, path.join(folder, file), line, column, folder);
}

test('real compiler navigation uses edited source and target positions across imports', async t => {
  const savedHelper = 'def helper(argument: i64) -> i64:\n    value = argument\n    return value\n';
  const savedMain = 'import "helper.nagi"\ndef main():\n    result = helper(21)\n    print(result)\n';
  const folder = fixture(t, { 'main.nagi': savedMain, 'helper.nagi': savedHelper });
  const helper = '# edited import\n' + savedHelper;
  const main = '# edited entry\n' + savedMain.replace('print(result)', 'print("😀"); print(result)');
  const index = await symbols(folder, 'main.nagi', [{ file: 'main.nagi', text: main }, { file: 'helper.nagi', text: helper }]);
  const result = target(index, folder, 'main.nagi', main, 'print(result)', 6);
  assert.equal(result.file, fs.realpathSync(path.join(folder, 'main.nagi')));
  assert.equal(result.line, 4);
  assert.equal(result.column, 5);
  const call = target(index, folder, 'main.nagi', main, 'helper(21)');
  assert.equal(call.file, fs.realpathSync(path.join(folder, 'helper.nagi')));
  assert.equal(call.line, 2, 'target follows the unsaved imported line');
  const argument = target(index, folder, 'helper.nagi', helper, 'value = argument', 8);
  assert.equal(argument.line, 2);
  assert.equal(argument.column, 12);
  const local = target(index, folder, 'helper.nagi', helper, 'return value', 7);
  assert.equal(local.line, 3);
  assert.equal(local.column, 5);
  assert.equal(fs.readFileSync(path.join(folder, 'main.nagi'), 'utf8'), savedMain);
  assert.equal(fs.readFileSync(path.join(folder, 'helper.nagi'), 'utf8'), savedHelper);
  assert.equal(fs.existsSync(path.join(folder, 'build')), false);
});

test('real Low navigation keeps reassignments at their original binding and restores loop shadowing', async t => {
  const source = 'fn main() { let value = 7; value += 1; for value in [value] { print(value); } print("😀"); print(value); }\r\n';
  const folder = fixture(t, { 'main.low': source });
  const index = await symbols(folder, 'main.low');
  const first = target(index, folder, 'main.low', source, 'value = 7');
  assert.deepEqual(target(index, folder, 'main.low', source, 'value += 1'), first);
  assert.deepEqual(target(index, folder, 'main.low', source, '[value]', 1), first);
  const loop = target(index, folder, 'main.low', source, 'for value', 4);
  assert.notDeepEqual(loop, first);
  assert.deepEqual(target(index, folder, 'main.low', source, 'print(value); }', 6), loop);
  assert.deepEqual(target(index, folder, 'main.low', source, 'print(value); }\r', 6), first);
  assert.equal(target(index, folder, 'main.low', source, 'print("😀")', 8), undefined);
});
