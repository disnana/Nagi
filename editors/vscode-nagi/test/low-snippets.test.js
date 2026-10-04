'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const compiler = require('../src/compiler');
const snippets = require('../snippets/low.json');
const root = path.resolve(__dirname, '../../..');

function expand(snippet) {
  return snippet.body.join('\n').replace(/\$\{\d+(?::([^}]*))?\}|\$\d+/g, (_match, value) => value || '');
}

for (const [name, snippet] of Object.entries(snippets)) {
  test(`Low ${name} snippet defaults compile in their declared context`, t => {
    const folder = fs.mkdtempSync(path.join(os.tmpdir(), 'nagi-low-snippet-'));
    t.after(() => fs.rmSync(folder, { recursive: true, force: true }));
    fs.writeFileSync(path.join(folder, 'helpers.low'), 'fn helper() -> i64 { return 7; }\n');
    let text = expand(snippet), file = path.join(folder, 'main.low'), native;
    if (name === 'Replace') {
      native = path.join(folder, 'native.low');
      fs.writeFileSync(native, text);
      file = path.join(folder, 'main.nagi');
      text = 'def name(x: i64) -> i64:\n    return x\ndef main():\n    print(name(7))\n';
    } else if (name === 'Result match' || name === 'Nullable match') {
      const setup = name === 'Result match' ? 'let result = parse_i64("7");' : 'let optional = some(7);';
      text = `fn main() -> unit {\n    ${setup}\n${text.split('\n').map(line => '    ' + line).join('\n')}\n}\n`;
    } else if (name !== 'Main') {
      text += '\nfn main() -> unit { print(7); }\n';
    }
    fs.writeFileSync(file, text);
    const args = ['check', file, '--out', path.join(folder, 'check')];
    if (native) args.push('--native', native);
    const result = spawnSync(compiler.compilerPath('', root, root), args, { cwd: folder, encoding: 'utf8', timeout: 10000 });
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stdout + result.stderr);
  });
}
