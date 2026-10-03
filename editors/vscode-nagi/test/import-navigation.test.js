'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const compiler = require('../src/compiler');
const features = require('../src/features');
const root = path.resolve(__dirname, '../../..');

for (const low of [false, true]) {
  test(`real ${low ? 'Low' : 'High'} quoted imports complete class enum and function names without calls`, async t => {
    const folder = fs.mkdtempSync(path.join(os.tmpdir(), 'nagi-import-completion-'));
    t.after(() => fs.rmSync(folder, { recursive: true, force: true }));
    const extension = low ? 'low' : 'nagi', dependency = `models 😀.${extension}`;
    fs.writeFileSync(path.join(folder, dependency), low
      ? 'record Payload { value: i64; }\nenum Fault { Missing; Invalid(message: str); }\nfn make(value: i64) -> Payload { return Payload(value=value); }\n'
      : 'class Payload:\n    value: i64\nenum Fault:\n    Missing\n    Invalid(message: str)\ndef make(value: i64) -> Payload:\n    return Payload(value=value)\n');
    const file = path.join(folder, `main.${extension}`);
    const text = `# 日本語 😀\nfrom "${dependency}" import Payload, Fault as Problem, make${low ? ';' : ''}\n` +
      (low ? 'fn main() -> unit { print(make(1).value); }\n' : 'def main():\n    print(make(1).value)\n');
    fs.writeFileSync(file, text);
    const result = await new Promise(resolve => compiler.runCheck(compiler.compilerPath('', root, root),
      ['symbols', file], folder, 5000, resolve, 16 * 1024 * 1024));
    assert.equal(result.error, null, result.output);
    const index = JSON.parse(result.output);
    for (const name of ['Payload', 'Fault', 'make']) {
      const position = text.indexOf(name, text.indexOf('import ')) + 2;
      const item = features.completionCandidates(index, text, position, low, { file }).find(item => item.name === name);
      assert.ok(item, name);
      assert.equal(item.importOnly, true);
      assert.equal(features.insertion(item, ''), name);
    }
    const comma = text.indexOf(', make') + 2;
    assert.deepEqual(features.completionCandidates(index, text, comma, low, { file }).map(item => item.name), ['make']);
    const alias = text.indexOf('Problem') + 2;
    assert.deepEqual(features.completionCandidates(index, text, alias, low, { file }), [], 'aliases are user-chosen names');
    const constructor = text.lastIndexOf('make(1)');
    assert.equal(features.signatureAt(index, text, constructor + 5, { file }).item.parameters[0].name, 'value');
  });
}
