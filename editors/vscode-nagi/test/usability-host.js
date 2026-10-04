'use strict';
// Run through --extensionTestsPath with a compiler containing the usability APIs.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vscode = require('vscode');
const fixtures = require('./usability-sources');

module.exports.run = async () => {
  const root = path.resolve(__dirname, '../../..');
  const folder = process.env.NAGI_EDITOR_HOST_FIXTURES || path.join(root, 'build', 'vscode-usability-host');
  fs.mkdirSync(folder, { recursive: true });
  const extension = vscode.extensions.getExtension('Disnana.nagi-lang');
  assert.ok(extension);
  await extension.activate();
  let cases = 0;
  const hoverText = hovers => hovers.flatMap(hover => hover.contents.map(content => content.value || String(content))).join('\n');
  async function open(family, suffix, text) {
    const file = path.join(folder, `${family}.${suffix}`);
    fs.writeFileSync(file, text);
    const doc = await vscode.workspace.openTextDocument(file);
    await vscode.window.showTextDocument(doc);
    return doc;
  }
  async function standardTarget(doc, offset, module, spelling) {
    const definitions = await vscode.commands.executeCommand('vscode.executeDefinitionProvider', doc.uri, doc.positionAt(offset));
    assert.equal(definitions[0].uri.scheme, 'nagi-stdlib');
    assert.equal(definitions[0].uri.path, `/${module.replaceAll('.', '/')}.nagi`);
    const virtual = await vscode.workspace.openTextDocument(definitions[0].uri);
    assert.ok(virtual.lineAt(definitions[0].range.start.line).text.includes(spelling));
    assert.equal(virtual.isDirty, false);
  }
  async function operation(doc, text, suffix, spelling, parameters, module) {
    const name = spelling.split('.').at(-1);
    const start = text.indexOf(spelling) + spelling.lastIndexOf('.') + 1;
    const hovers = await vscode.commands.executeCommand('vscode.executeHoverProvider', doc.uri, doc.positionAt(start + 2));
    assert.ok(hoverText(hovers).includes(`${suffix === 'low' ? 'fn' : 'def'} ${spelling}(`));
    const list = await vscode.commands.executeCommand('vscode.executeCompletionItemProvider', doc.uri, doc.positionAt(start + 2));
    const item = list.items.find(item => item.label === name);
    assert.equal(item.kind, vscode.CompletionItemKind.Function);
    assert.equal(item.insertText.value, name, 'an existing call keeps its argument list');
    const help = await vscode.commands.executeCommand('vscode.executeSignatureHelpProvider', doc.uri, doc.positionAt(start + name.length + 1));
    assert.ok(help.signatures[0].label.startsWith(`${suffix === 'low' ? 'fn' : 'def'} ${spelling}(`));
    assert.deepEqual(help.signatures[0].parameters.map(parameter => parameter.label.split(':')[0]), parameters);
    await standardTarget(doc, start + 2, module, name);
    cases++;
  }

  for (const [suffix, text] of fixtures.borrowed) {
    const doc = await open('borrowed', suffix, text);
    const start = text.indexOf('item.inner');
    const hovers = await vscode.commands.executeCommand('vscode.executeHoverProvider', doc.uri, doc.positionAt(start + 2));
    assert.match(hoverText(hovers), /item: Entry \(read-only borrow\)/);
    const list = await vscode.commands.executeCommand('vscode.executeCompletionItemProvider', doc.uri, doc.positionAt(start + 2));
    const local = list.items.find(item => item.label === 'item');
    assert.equal(local.detail, 'item: Entry (read-only borrow)');
    assert.equal(local.kind, vscode.CompletionItemKind.Variable);
    assert.equal(local.insertText.value, 'item');
    cases++;
    for (const name of ['inner', 'name']) {
      const offset = start + 'item.inner.name'.indexOf(name) + 2;
      const hover = await vscode.commands.executeCommand('vscode.executeHoverProvider', doc.uri, doc.positionAt(offset));
      assert.match(hoverText(hover), new RegExp(`${name}: ${name === 'inner' ? 'Inner' : 'str'} \\(read-only\\)`));
      const fields = await vscode.commands.executeCommand('vscode.executeCompletionItemProvider', doc.uri, doc.positionAt(offset));
      assert.equal(fields.items.find(item => item.label === name).kind, vscode.CompletionItemKind.Field);
      assert.equal(fields.items.find(item => item.label === name).insertText.value, name);
      cases++;
    }
    const owned = text.indexOf('print(name)') + 'print('.length;
    const hover = await vscode.commands.executeCommand('vscode.executeHoverProvider', doc.uri, doc.positionAt(owned + 2));
    assert.match(hoverText(hover), /name: str/);
    assert.ok(!hoverText(hover).includes('read-only borrow'));
    cases++;
  }

  for (const [suffix, text] of fixtures.result) {
    const doc = await open('result', suffix, text);
    await operation(doc, text, suffix, 'result.map_error', ['value', 'mapper'], 'std.result');
    const alias = text.lastIndexOf('convert(');
    const hover = await vscode.commands.executeCommand('vscode.executeHoverProvider', doc.uri, doc.positionAt(alias + 2));
    assert.ok(hoverText(hover).includes(`${suffix === 'low' ? 'fn' : 'def'} convert(`));
    const signature = await vscode.commands.executeCommand('vscode.executeSignatureHelpProvider', doc.uri, doc.positionAt(alias + 'convert('.length));
    assert.equal(signature.signatures[0].parameters.length, 2);
    await standardTarget(doc, alias + 2, 'std.result', 'map_error');
    cases++;
  }

  for (const [suffix, text] of fixtures.actor) {
    const doc = await open('actor', suffix, text);
    for (const [name, parameters] of [['mark_ready', ['signal']], ['task_with_ready', ['group', 'name', 'factory', 'policy']], ['next_event_timeout', ['control', 'timeout_ms']]]) {
      await operation(doc, text, suffix, `actors.${name}`, parameters, 'std.actor');
    }
    const ready = text.indexOf('signal: Signal') + 'signal: '.length;
    const types = await vscode.commands.executeCommand('vscode.executeCompletionItemProvider', doc.uri, doc.positionAt(ready + 2));
    assert.equal(types.items.find(item => item.label === 'Signal').insertText.value, 'Signal');
    await standardTarget(doc, ready + 2, 'std.actor', 'TaskReady');
    cases++;
    const timeout = text.indexOf('WaitReason.TIMEOUT') + 'WaitReason.'.length;
    const constants = await vscode.commands.executeCommand('vscode.executeCompletionItemProvider', doc.uri, doc.positionAt(timeout + 2));
    for (const name of ['TIMEOUT', 'INVALID_TIMEOUT']) {
      const item = constants.items.find(item => item.label === name);
      assert.equal(item.kind, vscode.CompletionItemKind.Constant);
      assert.equal(item.insertText.value, name);
    }
    await standardTarget(doc, timeout + 2, 'std.actor', 'TIMEOUT');
    const event = text.indexOf('actors.EventKind.READY') + 'actors.EventKind.'.length;
    await standardTarget(doc, event + 2, 'std.actor', 'READY');
    cases++;
    for (const name of ['kind', 'message']) {
      const offset = text.indexOf(`problem.${name}`) + 'problem.'.length;
      const fields = await vscode.commands.executeCommand('vscode.executeCompletionItemProvider', doc.uri, doc.positionAt(offset + 2));
      const item = fields.items.find(item => item.label === name);
      assert.equal(item.kind, vscode.CompletionItemKind.Field);
      assert.match(item.detail, /read-only/);
      if (name === 'kind') assert.match(item.detail, /WaitReason/);
      else assert.match(item.detail, /view\[str\]/);
      await standardTarget(doc, offset + 2, 'std.actor', name);
      cases++;
    }
  }

  // The initial failed edit falls back to a checked snapshot's catalog.
  for (const suffix of ['nagi', 'low']) {
    const doc = await open('imports', suffix, suffix === 'low' ? 'fn main() { print(0); }\n' : 'def main():\n    print(0)\n');
    const original = doc.getText();
    for (const [code, name] of [['import std.res', 'std.result'], ['from std.result import map_', 'map_error'], ['from std.actor import TaskR', 'TaskReady']]) {
      const edit = new vscode.WorkspaceEdit();
      edit.replace(doc.uri, new vscode.Range(doc.positionAt(0), doc.positionAt(doc.getText().length)), code);
      assert.ok(await vscode.workspace.applyEdit(edit));
      const list = await vscode.commands.executeCommand('vscode.executeCompletionItemProvider', doc.uri, doc.positionAt(code.length));
      const item = list.items.find(item => item.label === name);
      assert.ok(item, `initial catalog import ${name}`);
      assert.equal(item.insertText.value || item.insertText, name);
      cases++;
    }
    assert.equal(fs.readFileSync(doc.uri.fsPath, 'utf8'), original);
  }
  const result = process.env.NAGI_EDITOR_HOST_RESULT || path.join(root, 'build', 'vscode-usability-host-result.json');
  fs.writeFileSync(result, JSON.stringify({ passed: true, cases }, null, 2));
};
