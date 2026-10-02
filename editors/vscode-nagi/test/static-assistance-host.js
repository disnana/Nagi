'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vscode = require('vscode');

const root = path.resolve(__dirname, '../../..');
const report = path.join(root, 'build/vscode-static-assistance-result.json');
const cases = [];
function save(status, error) {
  fs.mkdirSync(path.dirname(report), { recursive: true });
  fs.writeFileSync(report, JSON.stringify({ status, workspaceTrusted: vscode.workspace.isTrusted, cases, error }, null, 2));
}

async function run() {
  save('running');
  const manifest = require('../package.json');
  await vscode.extensions.getExtension(`${manifest.publisher}.${manifest.name}`).activate();
  if (process.env.NAGI_TEST_UNTRUSTED === '1') assert.equal(vscode.workspace.isTrusted, false);
  async function untitled(text, language = 'nagi') {
    const doc = await vscode.workspace.openTextDocument({ language, content: text });
    await vscode.window.showTextDocument(doc);
    return doc;
  }
  async function hover(doc, start) {
    const values = await vscode.commands.executeCommand('vscode.executeHoverProvider', doc.uri, doc.positionAt(start));
    return values.flatMap(value => value.contents.map(content => content.value || String(content))).join('\n');
  }
  async function signature(doc) {
    return vscode.commands.executeCommand('vscode.executeSignatureHelpProvider', doc.uri, doc.positionAt(doc.getText().length), ',');
  }
  async function completions(doc) {
    const values = await vscode.commands.executeCommand('vscode.executeCompletionItemProvider', doc.uri, doc.positionAt(doc.getText().length));
    return values.items;
  }
  async function close() { await vscode.commands.executeCommand('workbench.action.revertAndCloseActiveEditor'); }

  for (const language of ['nagi', 'nagi-low']) {
    const doc = await untitled('env("😀", ', language);
    assert.match(await hover(doc, 1), /def env\(name: str, fallback: str\) -> str/);
    const hint = await signature(doc);
    assert.match(hint.signatures[0].label, /def env/);
    assert.equal(hint.activeParameter, 1);
    cases.push(`${language}: untitled builtin hover/signature with UTF-16 string argument`);
    await close();

    const prefix = await untitled('sco', language);
    const candidates = await completions(prefix);
    assert.ok(candidates.some(item => item.label === 'scope' && item.kind === vscode.CompletionItemKind.Keyword));
    assert.ok(candidates.some(item => item.label === (language === 'nagi' ? 'class' : 'record')));
    assert.ok(candidates.some(item => item.label === 'None'));
    cases.push(`${language}: keyword prefix completion`);
    await close();
  }

  for (const text of [
    'def main(print: fn(i64) -> unit):\n    print(',
    'print = custom\nprint(',
    'import "missing.nagi"\ndef main():\n    print(',
    '# print(',
    'text = "print(',
  ]) {
    const doc = await untitled(text);
    assert.doesNotMatch(await hover(doc, text.lastIndexOf('print') + 2), /def print/);
    const hint = await signature(doc);
    assert.ok(!hint || hint.signatures.length === 0, text);
    assert.ok(!(await completions(doc)).some(item => item.label === 'print' && item.kind === vscode.CompletionItemKind.Function), text);
    cases.push(`no guessed builtin: ${text}`);
    await close();
  }

  const brokenString = await untitled('text = "😀 \\\r\nenv(');
  const found = await hover(brokenString, brokenString.getText().indexOf('env') + 1);
  assert.match(found, /def env/);
  assert.match((await signature(brokenString)).signatures[0].label, /def env/);
  cases.push('unfinished line string recovers for the following CRLF line');
  await close();

  const folder = path.join(root, 'build/vscode-static-fixtures');
  fs.mkdirSync(folder, { recursive: true });
  const file = path.join(folder, `missing-compiler-${process.pid}.nagi`);
  fs.writeFileSync(file, 'def main():\n    env(');
  const config = vscode.workspace.getConfiguration('nagi');
  const previous = config.inspect('compilerPath').globalValue;
  try {
    await config.update('compilerPath', path.join(folder, 'missing-nagic'), vscode.ConfigurationTarget.Global);
    const doc = await vscode.workspace.openTextDocument(file);
    await vscode.window.showTextDocument(doc);
    assert.match(await hover(doc, doc.getText().indexOf('env') + 1), /def env/);
    assert.match((await signature(doc)).signatures[0].label, /def env/);
    assert.ok((await completions(doc)).some(item => item.label === 'env'));
    assert.equal(vscode.languages.getDiagnostics(doc.uri).length, 0, 'missing compiler is not a source error');
    cases.push('saved file with unavailable compiler retains builtin assistance');
    await close();
  } finally {
    await config.update('compilerPath', previous, vscode.ConfigurationTarget.Global);
    fs.rmSync(file, { force: true });
  }
  save('passed');
  console.log(`PASS: ${cases.length} static assistance cases, workspace trusted=${vscode.workspace.isTrusted}`);
}

module.exports = { async run() { try { await run(); } catch (error) { save('failed', error.stack); throw error; } } };
