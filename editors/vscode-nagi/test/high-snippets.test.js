'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const compiler = require('../src/compiler');
const snippets = require('../snippets/high.json');
const root = path.resolve(__dirname, '../../..');

function expand(snippet) {
  const defaults = new Map();
  return snippet.body.join('\n').replace(/\$\{(\d+)(?::([^}]*))?\}|\$(\d+)/g,
    (_match, number, value, bare) => {
      const key = number || bare;
      if (value !== undefined) defaults.set(key, value);
      return value ?? defaults.get(key) ?? '';
    });
}

for (const [name, snippet] of Object.entries(snippets)) {
  test(`High ${name} snippet defaults compile in their declared context`, t => {
    const folder = fs.mkdtempSync(path.join(os.tmpdir(), 'nagi-high-snippet-'));
    t.after(() => fs.rmSync(folder, { recursive: true, force: true }));
    let text = expand(snippet);
    if (name === 'GET handler' || name === 'POST handler') {
      text = 'class State:\n    greeting: str\n' + text;
    }
    if (['Loop', 'Result match', 'Nullable match'].includes(name)) {
      const setup = name === 'Result match' ? '    result = parse_i64("7")\n'
        : name === 'Nullable match' ? '    optional = some(7)\n' : '';
      text = 'def main():\n' + setup + text.split('\n').map(line => '    ' + line).join('\n');
    } else if (!['Main', 'Async main', 'HTTP server'].includes(name)) {
      text += '\ndef main():\n    print(7)\n';
    }
    const file = path.join(folder, 'main.nagi');
    fs.writeFileSync(file, text);
    const result = spawnSync(compiler.compilerPath('', root, root), ['check', file, '--out', path.join(folder, 'check')],
      { cwd: folder, encoding: 'utf8', timeout: 10000 });
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stdout + result.stderr);
  });
}
