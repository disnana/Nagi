'use strict';
// Run in a real VS Code Extension Host with nagi.compilerPath configured.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vscode = require('vscode');

const report = process.env.NAGI_EDITOR_HOST_RESULT || path.resolve(__dirname, '../../../build/vscode-low-authoring-result.json');
const cases = [];
function save(status, error) {
  fs.mkdirSync(path.dirname(report), { recursive: true });
  fs.writeFileSync(report, JSON.stringify({ status, workspaceTrusted: vscode.workspace.isTrusted, cases, error }, null, 2));
}
const hoverText = values => values.flatMap(value => value.contents.map(content => content.value || String(content))).join('\n');

async function run() {
  save('running');
  await vscode.extensions.getExtension('Disnana.nagi-lang').activate();
  const folder = path.join(path.dirname(report), 'low-authoring-fixtures');
  fs.mkdirSync(folder, { recursive: true });
  async function open(name, text) {
    const file = path.join(folder, name);
    fs.writeFileSync(file, text);
    const doc = await vscode.workspace.openTextDocument(file);
    await vscode.window.showTextDocument(doc);
    return doc;
  }
  async function completions(doc, offset) {
    return (await vscode.commands.executeCommand('vscode.executeCompletionItemProvider', doc.uri, doc.positionAt(offset))).items;
  }
  async function hover(doc, offset) {
    return hoverText(await vscode.commands.executeCommand('vscode.executeHoverProvider', doc.uri, doc.positionAt(offset)));
  }

  const models = await open('models.low', 'record Point {\nx: i64;\n}\nenum Fault {\nMissing;\nInvalid(message: str);\n}\nfn make(value: i64) -> Point { return Point(x=value); }\nfn foldable() -> unit {\nprint(1); }\n');
  const text = 'fn before() {} from "models.low" import Point as SavedPoint, Fault, make;\n' +
    'import std.http.server as http;\n' +
    'fn standard() -> http.Response { return http.text(http.Status.OK, view("ok")); }\n' +
    'fn consume(problem: Fault) -> unit { match problem { case Fault.Missing { print(0); } case Fault.Invalid(message) { print(message); } } }\n' +
    'fn main() -> unit { let point = make(7); print(point.x); consume(Fault.Invalid("bad")); }\n';
  const main = await open('main.low', text);
  const imported = text.indexOf(', make') + 2;
  const importedMake = (await completions(main, imported + 2)).find(item => item.label === 'make');
  assert.ok(importedMake, 'quoted from import after a declaration offers the imported function');
  assert.equal(importedMake.insertText.value || importedMake.insertText, 'make');
  assert.match(importedMake.detail, /^fn make/);
  cases.push('same-line quoted imports insert declaration names without calls');

  const standardImport = text.indexOf('std.http.server');
  const module = (await completions(main, standardImport + 'std.http.'.length)).find(item => item.label === 'std.http.server');
  assert.ok(module);
  assert.equal(module.range.start.character, 'import '.length);
  assert.equal(module.range.end.character, 'import std.http.server'.length);
  const operation = text.indexOf('http.text(') + 'http.'.length;
  assert.match(await hover(main, operation + 1), /fn http.text\(status: http.Status, body: view\[str\]\)/);
  const help = await vscode.commands.executeCommand('vscode.executeSignatureHelpProvider', main.uri, main.positionAt(operation + 'text('.length));
  assert.match(help.signatures[0].label, /^fn http.text/);
  assert.equal(help.signatures[0].parameters[1].label, 'body: view[str]');
  cases.push('standard module path replacement and fn operation hover/signature');

  const record = text.indexOf('SavedPoint');
  assert.match(await hover(main, record + 1), /record SavedPoint \{\n    x: i64;\n\}/);
  const recordJump = await vscode.commands.executeCommand('vscode.executeDefinitionProvider', main.uri, main.positionAt(record + 1));
  assert.equal(recordJump[0].uri.toString(), models.uri.toString());
  const variant = text.lastIndexOf('Fault.Invalid') + 'Fault.'.length;
  assert.match(await hover(main, variant + 1), /Fault.Invalid\(message: str\)/);
  const variantJump = await vscode.commands.executeCommand('vscode.executeDefinitionProvider', main.uri, main.positionAt(variant + 1));
  assert.equal(variantJump[0].uri.toString(), models.uri.toString());
  assert.equal(variantJump[0].range.start.line, 5);
  cases.push('record aliases and enum payloads retain hover and F12 targets');

  const folds = await vscode.commands.executeCommand('vscode.executeFoldingRangeProvider', models.uri);
  assert.deepEqual(folds.map(range => [range.start, range.end]), [[0, 1], [3, 5], [8, 9]]);
  cases.push('valid unindented record, enum and inline-closing fn bodies fold by braces');

  await vscode.window.showTextDocument(main);
  await vscode.commands.executeCommand('nagi.check');
  assert.equal(vscode.languages.getDiagnostics(main.uri).length, 0);
  const bad = await open('invalid.low', 'fn main() -> unit {\n    let x: i64 = "bad";\n}\n');
  await vscode.commands.executeCommand('nagi.check');
  const diagnostics = vscode.languages.getDiagnostics(bad.uri);
  assert.ok(diagnostics.some(item => item.severity === vscode.DiagnosticSeverity.Error && item.range.start.line === 1));
  const editor = await vscode.window.showTextDocument(bad);
  await editor.edit(edit => edit.replace(new vscode.Range(1, 0, 1, bad.lineAt(1).text.length), '    let x: i64 = 7;'));
  await vscode.commands.executeCommand('nagi.check');
  assert.equal(vscode.languages.getDiagnostics(bad.uri).length, 0, 'corrected in-memory Low source clears its diagnostic');
  cases.push('Low check diagnostics point to the source line and refresh after an in-memory fix');

  const untitled = await vscode.workspace.openTextDocument({ language: 'nagi-low', content: 'env("😀", ' });
  await vscode.window.showTextDocument(untitled);
  assert.match(await hover(untitled, 1), /fn env\(name: str, fallback: str\)/);
  const builtin = await vscode.commands.executeCommand('vscode.executeSignatureHelpProvider', untitled.uri, untitled.positionAt(untitled.getText().length));
  assert.match(builtin.signatures[0].label, /^fn env/);
  assert.equal(builtin.activeParameter, 1);
  cases.push('untitled builtin assistance uses Low syntax without a compiler query');

  const snippet = await vscode.workspace.openTextDocument({ language: 'nagi-low', content: '' });
  const snippetEditor = await vscode.window.showTextDocument(snippet);
  snippetEditor.options = { insertSpaces: true, tabSize: 4 };
  await snippetEditor.insertSnippet(new vscode.SnippetString(require('../snippets/low.json').Main.body.join('\n')));
  assert.equal(snippet.getText(), 'fn main() -> unit {\n    print("Hello, Nagi!");\n}');
  cases.push('Low main snippet inserts a braced entry function');
  save('passed');
  console.log(`PASS: ${cases.length} Low authoring cases in real VS Code`);
}

module.exports = { async run() { try { await run(); } catch (error) { save('failed', error.stack); throw error; } } };
