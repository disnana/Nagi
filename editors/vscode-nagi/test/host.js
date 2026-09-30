'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vscode = require('vscode');

async function run() {
  const root = path.resolve(__dirname, '../../..');
  const folder = path.join(root, 'build', 'vscode-host-fixtures');
  fs.mkdirSync(folder, { recursive: true });
  const file = path.join(folder, 'example.nagi');
  fs.writeFileSync(file, 'def main():\n    x: i32 = "wrong"\n');
  const extension = vscode.extensions.getExtension('nagi-local.nagi-language');
  assert.ok(extension, 'development extension is registered');
  await extension.activate();
  const document = await vscode.workspace.openTextDocument(file);
  await vscode.window.showTextDocument(document);
  assert.equal(document.languageId, 'nagi');
  await vscode.commands.executeCommand('nagi.check');
  const errors = vscode.languages.getDiagnostics(document.uri);
  assert.equal(errors.length, 1);
  assert.equal(errors[0].range.start.line, 1);
  const edit = new vscode.WorkspaceEdit();
  edit.replace(document.uri, new vscode.Range(0, 0, document.lineCount, 0), 'def main():\n    print("hello")\n');
  await vscode.workspace.applyEdit(edit);
  assert.equal(vscode.languages.getDiagnostics(document.uri).length, 0, 'stale diagnostic cleared on edit');
  await document.save();
  await vscode.commands.executeCommand('nagi.check');
  assert.equal(vscode.languages.getDiagnostics(document.uri).length, 0, 'valid source has no errors');
  const moduleFile = path.join(folder, 'bad_module.nagi');
  fs.writeFileSync(moduleFile, 'def bad() -> i32:\n    return "wrong"\n');
  const imported = new vscode.WorkspaceEdit();
  imported.replace(document.uri, new vscode.Range(0, 0, document.lineCount, 0), 'import "bad_module.nagi"\ndef main():\n    print(1)\n');
  await vscode.workspace.applyEdit(imported);
  await document.save();
  await vscode.commands.executeCommand('nagi.check');
  const importedErrors = vscode.languages.getDiagnostics(vscode.Uri.file(moduleFile));
  assert.equal(importedErrors.length, 1, 'imported error appears on its original file');
  assert.equal(importedErrors[0].range.start.line, 1);
  assert.equal(vscode.languages.getDiagnostics(document.uri).length, 0, 'entry file is not wrongly underlined');
  const lowFile = path.join(folder, 'example.low');
  fs.writeFileSync(lowFile, 'fn main() -> unit { print(1); }\n');
  const low = await vscode.workspace.openTextDocument(lowFile);
  assert.equal(low.languageId, 'nagi-low');
  console.log('PASS: VS Code Extension Host activation, High/Low, real diagnostics, edit/save refresh, imported file diagnostics');
}

module.exports = { run };
