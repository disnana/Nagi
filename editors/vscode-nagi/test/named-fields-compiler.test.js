'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const fs = require('node:fs');
const path = require('node:path');
const compiler = require('../src/compiler');
const features = require('../src/features');

for (const low of [false, true]) test(`real ${low ? 'Low' : 'High'} symbols retain named constructor fields and explicit saved fallback`, async t => {
  const root = path.resolve(__dirname, '../../..');
  const base = path.join(root, 'build', 'vscode-named-fields');
  fs.mkdirSync(base, { recursive: true });
  const folder = fs.mkdtempSync(path.join(base, 'case-'));
  t.after(() => fs.rmSync(folder, { recursive: true, force: true }));
  const file = path.join(folder, low ? 'main.low' : 'main.nagi');
  const text = low ? 'record Point { x: f64; y: f64; }\r\nfn main() { print("😀"); let p = Point(y=2.5, x=1.5); print(p.x); }\r\n'
    : 'class Point:\r\n    x: f64\r\n    y: f64\r\ndef main():\r\n    print("😀"); p = Point(y=2.5, x=1.5)\r\n    print(p.x)\r\n';
  fs.writeFileSync(file, text);
  const executable = compiler.compilerPath(process.env.NAGI_TEST_COMPILER || '', root, root);
  const query = async overlay => new Promise(resolve => compiler.runCheck(executable,
    ['symbols', file, ...(overlay ? ['--editor-input'] : [])], folder, 10000, resolve, 16 * 1024 * 1024,
    overlay ? JSON.stringify({ files: [{ file, text: overlay }] }) : undefined));
  const checked = await query(text);
  assert.equal(checked.error, null, checked.output);
  const index = JSON.parse(checked.output);
  const offset = text.lastIndexOf('x=') + 1;
  const source = { file };
  const candidates = features.completionCandidates(index, text, offset, low, source);
  assert.deepEqual(candidates.map(item => [item.name, item.type, item.namedField]), [['x', 'f64', true]]);
  assert.equal(features.signatureAt(index, text, offset + 1, source).argument, 0);
  const commented = text.replace('Point(y=2.5, x=1.5)', 'Point(\r\n y=2.5,\r\n x # label comment 😀\r\n =1.5\r\n)');
  const checkedComment = await new Promise(resolve => compiler.runCheck(executable,
    ['check', file, '--editor-input'], folder, 10000, resolve, 16 * 1024 * 1024,
    JSON.stringify({ files: [{ file, text: commented }] })));
  assert.equal(checkedComment.error, null, checkedComment.output);
  const commentIndex = await query(commented);
  assert.equal(commentIndex.error, null, commentIndex.output);
  const commentOffset = commented.indexOf('x #') + 1;
  const [commentField] = features.completionCandidates(JSON.parse(commentIndex.output), commented, commentOffset, low, source);
  assert.equal(commentField.name, 'x');
  assert.equal(features.insertion(commentField, commented.slice(commentOffset)), 'x');
  const partial = text.replace('x=1.5', 'x');
  const failed = await query(partial);
  assert.ok(failed.error, 'the compiler still rejects the original incomplete argument');
  const saved = await query();
  assert.equal(saved.error, null, saved.output);
  const partialOffset = partial.indexOf('Point(y=2.5, x') + 'Point(y=2.5, x'.length;
  assert.deepEqual(features.completionCandidates(JSON.parse(saved.output), partial, partialOffset, low,
    { file, saved: true }).filter(item => item.namedField).map(item => item.name), ['x']);
  assert.equal(fs.readFileSync(file, 'utf8'), text, 'assistance never writes the incomplete buffer to disk');
});
