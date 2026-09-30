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

  const project = path.join(folder, `project-${process.pid}`);
  fs.mkdirSync(project, { recursive: true });
  const manifestFile = path.join(project, 'nagi.toml');
  const helperFile = path.join(project, 'helper.nagi');
  fs.writeFileSync(manifestFile, "entry = 'host-entry.nagi'\n[rust]\nfile = 'native.rs'\n[rust.dependencies]\nserde_json = '1.0'\n");
  fs.writeFileSync(path.join(project, 'host-entry.nagi'), 'import "helper.nagi"\n@rust("native::record")\nextern def record()\ndef answer() -> i64:\n    return 42\ndef main():\n    print(helper())\n    record()\n');
  fs.appendFileSync(path.join(project, 'host-entry.nagi'), '\ndef tagged() -> Number:\n    return Number(value=42)\n');
  const modelsFile = path.join(project, 'models.nagi');
  fs.writeFileSync(modelsFile, 'class Number:\n    value: i64\n');
  fs.writeFileSync(helperFile, 'import "models.nagi"\ndef helper() -> i64:\n    return answer()\n');
  fs.writeFileSync(path.join(project, 'native.rs'), 'pub fn record() { std::fs::write("host-run.json", serde_json::to_string(&42).unwrap()).unwrap(); }\n');
  const witness = path.join(project, 'host-run.json');
  if (fs.existsSync(witness)) fs.unlinkSync(witness);
  const helper = await vscode.workspace.openTextDocument(helperFile);
  await vscode.window.showTextDocument(helper);
  // Let the manifest file watcher settle before manual commands.
  await new Promise(resolve => setTimeout(resolve, 500));
  await vscode.commands.executeCommand('nagi.check');
  assert.equal(vscode.languages.getDiagnostics(helper.uri).length, 0, 'helper resolves symbols defined by the entry');
  async function definitions(doc, line, column) {
    return vscode.commands.executeCommand('vscode.executeDefinitionProvider', doc.uri, new vscode.Position(line, column));
  }
  const entryFile = path.join(project, 'host-entry.nagi');
  const answerDefinition = await definitions(helper, 2, 12);
  assert.equal(answerDefinition.length, 1, 'F12 on a helper resolves an entry-defined function');
  assert.equal(answerDefinition[0].uri.toString(), vscode.Uri.file(entryFile).toString());
  assert.equal(answerDefinition[0].range.start.line, 3);
  const importDefinition = await definitions(helper, 0, 12);
  assert.equal(importDefinition[0].uri.toString(), vscode.Uri.file(modelsFile).toString(), 'F12 on import opens the file');
  const entryDocument = await vscode.workspace.openTextDocument(entryFile);
  const helperDefinition = await definitions(entryDocument, 6, 12);
  assert.equal(helperDefinition[0].uri.toString(), helper.uri.toString());
  assert.equal(helperDefinition[0].range.start.line, 1);
  const typeLine = entryDocument.getText().split(/\r?\n/).findIndex(line => line.startsWith('def tagged'));
  const typeDefinition = await definitions(entryDocument, typeLine, 18);
  assert.equal(typeDefinition[0].uri.toString(), vscode.Uri.file(modelsFile).toString(), 'F12 on a class type resolves its import');
  const commentEdit = new vscode.WorkspaceEdit();
  commentEdit.insert(helper.uri, new vscode.Position(helper.lineCount, 0), '# answer()\n');
  await vscode.workspace.applyEdit(commentEdit);
  assert.equal((await definitions(helper, 2, 12)).length, 0, 'unsaved source never uses stale definitions');
  await helper.save();
  assert.equal((await definitions(helper, 3, 3)).length, 0, 'comments never resolve as calls');
  let execution;
  const finished = new Map();
  let finish;
  const completion = new Promise((resolve, reject) => {
    const timer = setTimeout(() => { listener.dispose(); reject(new Error('project run timed out')); }, 60000);
    finish = code => { clearTimeout(timer); listener.dispose(); resolve(code); };
  });
  const listener = vscode.tasks.onDidEndTaskProcess(e => {
    finished.set(e.execution, e.exitCode);
    if (e.execution === execution) finish(e.exitCode);
  });
  execution = await vscode.commands.executeCommand('nagi.run');
  if (finished.has(execution)) finish(finished.get(execution));
  assert.equal(await completion, 0, 'run on a helper builds and runs the project entry with configured Rust dependencies');
  assert.equal(fs.readFileSync(witness, 'utf8'), '42', 'project cwd is used by the running app');

  const manifest = await vscode.workspace.openTextDocument(manifestFile);
  const invalidConfig = new vscode.WorkspaceEdit();
  invalidConfig.replace(manifest.uri, new vscode.Range(0, 0, manifest.lineCount, 0), "entry = 'host-entry.nagi'\nunknown = true\n");
  await vscode.workspace.applyEdit(invalidConfig);
  assert.equal(vscode.languages.getDiagnostics(helper.uri).length, 0, 'manifest edits invalidate diagnostics');
  await manifest.save();
  const deadline = Date.now() + 10000;
  while (!vscode.languages.getDiagnostics(manifest.uri).length && Date.now() < deadline) {
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  assert.equal(vscode.languages.getDiagnostics(manifest.uri).length, 1, 'manifest save rechecks the project automatically');
  assert.equal(vscode.languages.getDiagnostics(manifest.uri)[0].range.start.line, 1, 'manifest diagnostic uses TOML source line');
  const validConfig = new vscode.WorkspaceEdit();
  validConfig.replace(manifest.uri, new vscode.Range(0, 0, manifest.lineCount, 0), "entry = 'host-entry.nagi'\n[rust]\nfile = 'native.rs'\n[rust.dependencies]\nserde_json = '1.0'\n");
  await vscode.workspace.applyEdit(validConfig);
  await manifest.save();
  await new Promise(resolve => setTimeout(resolve, 500));
  await vscode.commands.executeCommand('nagi.check');
  assert.equal(vscode.languages.getDiagnostics(manifest.uri).length, 0, 'fixed manifest clears the error');
  const lowFolder = path.join(project, '..', `low-navigation-${process.pid}`);
  fs.mkdirSync(lowFolder, { recursive: true });
  fs.writeFileSync(path.join(lowFolder, 'math.low'), 'fn twice(x: i64) -> i64 { return x * 2; }\n');
  fs.writeFileSync(path.join(lowFolder, 'main.low'), 'import "math.low";\nfn main() -> unit { print(twice(21)); }\n');
  const lowEntry = await vscode.workspace.openTextDocument(path.join(lowFolder, 'main.low'));
  const lowDefinition = await definitions(lowEntry, 1, 27);
  assert.equal(lowDefinition[0].uri.toString(), vscode.Uri.file(path.join(lowFolder, 'math.low')).toString(), 'Low import definition jump works');
  console.log('PASS: VS Code Extension Host diagnostics, project run, Rust dependencies, manifest refresh, F12 functions/classes/imports/Low, unsaved and comment guards');
}

module.exports = { run };
