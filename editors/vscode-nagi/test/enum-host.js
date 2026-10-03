'use strict';
// Run with VS Code --extensionTestsPath, after building the current compiler.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vscode = require('vscode');

module.exports.run = async () => {
  const root = path.resolve(__dirname, '../../..');
  const folder = path.join(root, 'build', 'vscode-enum-host');
  fs.mkdirSync(folder, { recursive: true });
  const extension = vscode.extensions.getExtension('Disnana.nagi-lang');
  assert.ok(extension);
  await extension.activate();
  let cases = 0;
  const fixtures = [
    ['nagi', 'enum AuthError:\n    InvalidCredentials\n    WeakPassword(message: str)\n',
      'import "errors.nagi" as errors\nfrom "errors.nagi" import AuthError as SavedError\ndef login(message: str) -> Result[i64, errors.AuthError]:\n    return fail(errors.AuthError.WeakPassword(message))\ndef main():\n    problem = SavedError.InvalidCredentials\n'],
    ['low', 'enum AuthError { InvalidCredentials; WeakPassword(message: str); }\n',
      'import "errors.low" as errors;\nfrom "errors.low" import AuthError as SavedError;\nfn login(message: str) -> Result[i64, errors.AuthError] { return fail(errors.AuthError.WeakPassword(message)); }\nfn main() { let problem = SavedError.InvalidCredentials; }\n'],
  ];
  const hoverText = hovers => hovers.flatMap(hover => hover.contents.map(content => content.value || String(content))).join('\n');
  const opened = [];
  for (const [suffix, dependency, text] of fixtures) {
    const entry = path.join(folder, `main.${suffix}`), dep = path.join(folder, `errors.${suffix}`);
    fs.writeFileSync(entry, text);
    fs.writeFileSync(dep, dependency);
    const doc = await vscode.workspace.openTextDocument(entry);
    await vscode.window.showTextDocument(doc);
    opened.push({ doc, dep, text, dependency });
    const variant = text.indexOf('errors.AuthError.WeakPassword') + 'errors.AuthError.'.length;
    const list = await vscode.commands.executeCommand('vscode.executeCompletionItemProvider', doc.uri, doc.positionAt(variant + 2));
    const payload = list.items.find(item => item.label === 'WeakPassword');
    const unit = list.items.find(item => item.label === 'InvalidCredentials');
    assert.ok(payload && unit, `${suffix} enum variants missing: ${list.items.map(item => item.label).join(', ')}; compilerPath=${vscode.workspace.getConfiguration('nagi', doc.uri).get('compilerPath', '(PATH)')}`);
    assert.equal(payload.kind, vscode.CompletionItemKind.EnumMember);
    assert.equal(unit.insertText.value || unit.insertText, 'InvalidCredentials');
    assert.equal(payload.insertText.value || payload.insertText, 'WeakPassword');
    cases++;
    const hover = await vscode.commands.executeCommand('vscode.executeHoverProvider', doc.uri, doc.positionAt(variant + 2));
    assert.match(hoverText(hover), /errors.AuthError.WeakPassword\(message: str\)/);
    cases++;
    const definitions = await vscode.commands.executeCommand('vscode.executeDefinitionProvider', doc.uri, doc.positionAt(variant + 2));
    assert.equal(definitions[0].uri.fsPath, dep);
    assert.equal(definitions[0].range.start.line, suffix === 'nagi' ? 2 : 0);
    cases++;
    const signature = await vscode.commands.executeCommand('vscode.executeSignatureHelpProvider', doc.uri, doc.positionAt(variant + 'WeakPassword('.length));
    assert.equal(signature.signatures[0].label, 'errors.AuthError.WeakPassword(message: str)');
    assert.equal(signature.signatures[0].parameters[0].label, 'message: str');
    cases++;
    const alias = text.lastIndexOf('SavedError.InvalidCredentials') + 'SavedError.'.length;
    const aliasHover = await vscode.commands.executeCommand('vscode.executeHoverProvider', doc.uri, doc.positionAt(alias + 2));
    assert.match(hoverText(aliasHover), /SavedError.InvalidCredentials/);
    cases++;
    const typeOffset = text.indexOf('Result[i64, errors.AuthError]') + 'Result[i64, errors.'.length + 2;
    const types = await vscode.commands.executeCommand('vscode.executeCompletionItemProvider', doc.uri, doc.positionAt(typeOffset));
    const enumType = types.items.find(item => item.label === 'AuthError');
    assert.equal(enumType.kind, vscode.CompletionItemKind.Enum);
    assert.equal(enumType.insertText.value || enumType.insertText, 'AuthError');
    assert.ok(!types.items.some(item => item.label === 'WeakPassword'));
    cases++;
  }
  const { doc, dep, text, dependency } = opened[0];
  const dependencyDoc = await vscode.workspace.openTextDocument(dep);
  const fresh = '# unsaved 😀\n' + dependency.replace('message: str', 'reason: str');
  const edit = new vscode.WorkspaceEdit();
  edit.replace(dependencyDoc.uri, new vscode.Range(dependencyDoc.positionAt(0), dependencyDoc.positionAt(dependencyDoc.getText().length)), fresh);
  assert.ok(await vscode.workspace.applyEdit(edit));
  await vscode.window.showTextDocument(doc);
  const variant = text.indexOf('errors.AuthError.WeakPassword') + 'errors.AuthError.'.length;
  const signature = await vscode.commands.executeCommand('vscode.executeSignatureHelpProvider', doc.uri, doc.positionAt(variant + 'WeakPassword('.length));
  assert.equal(signature.signatures[0].parameters[0].label, 'reason: str');
  const definitions = await vscode.commands.executeCommand('vscode.executeDefinitionProvider', doc.uri, doc.positionAt(variant + 2));
  assert.equal(definitions[0].range.start.line, 3);
  assert.equal(fs.readFileSync(dep, 'utf8'), dependency);
  cases++;
  if (process.env.NAGI_EDITOR_HOST_RESULT) fs.writeFileSync(process.env.NAGI_EDITOR_HOST_RESULT, JSON.stringify({ passed: true, cases }, null, 2));
  console.log(`PASS: ${cases} real Extension Host enum/type/variant/signature/F12/unsaved checks`);
};
