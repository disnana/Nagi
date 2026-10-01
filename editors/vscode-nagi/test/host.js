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
  fs.appendFileSync(path.join(project, 'host-entry.nagi'), '\ndef add(left: i64, right: i64) -> i64:\n    return left + right\n');
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
  async function hovers(doc, line, column) {
    const values = await vscode.commands.executeCommand('vscode.executeHoverProvider', doc.uri, new vscode.Position(line, column));
    return values.flatMap(v => v.contents.map(c => c.value || String(c))).join('\n');
  }
  async function completions(doc, line, column) {
    return vscode.commands.executeCommand('vscode.executeCompletionItemProvider', doc.uri, new vscode.Position(line, column));
  }
  assert.match(await hovers(helper, 2, 12), /def answer\(\) -> i64/, 'hover shows a function return type across imports');
  assert.match(await hovers(entryDocument, typeLine, 18), /value: i64/, 'class hover lists fields');
  const originalHelper = helper.getText();
  await replaceHelper('import "models.nagi"\ndef helper() -> i64:\n    return Nu\n');
  const candidates = await completions(helper, 2, 13);
  const number = candidates.items.find(c => c.label === 'Number');
  assert.ok(number, 'class completion follows the project import graph');
  assert.equal(number.insertText.value, 'Number(value=${1:value})', 'class completion uses named arguments');
  await replaceHelper(originalHelper);
  async function replaceHelper(text) {
    const change = new vscode.WorkspaceEdit();
    change.replace(helper.uri, new vscode.Range(0, 0, helper.lineCount, 0), text);
    await vscode.workspace.applyEdit(change);
  }
  await replaceHelper('import "models.nagi"\ndef helper(value: i64) -> i64:\n    return value\n');
  assert.match(await hovers(entryDocument, 6, 12), /helper\(value: i64\)/, 'hover reads unsaved declarations in imported buffers');
  assert.equal(fs.readFileSync(helperFile, 'utf8'), originalHelper, 'editor queries never save source files');
  await replaceHelper('import "models.nagi"\ndef helper() -> i64:\n    return add(1, ');
  const hint = await vscode.commands.executeCommand('vscode.executeSignatureHelpProvider', helper.uri, new vscode.Position(2, 18), '(');
  assert.match(hint.signatures[0].label, /add\(left: i64, right: i64\)/, 'signature help survives incomplete syntax');
  assert.equal(hint.activeParameter, 1);
  const unfinished = await completions(helper, 2, 18);
  assert.match(unfinished.items.find(c => c.label === 'helper').detail, /保存済み/, 'fallback candidates explicitly identify saved declarations');
  await replaceHelper('import "models.nagi"\ndef helper() -> i64:\n    value: Re\n    return 0\n');
  const typed = await completions(helper, 2, 13);
  assert.ok(typed.items.some(c => c.label === 'Result'), 'type completion works while editing');
  await replaceHelper(originalHelper);
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
  assert.match(await hovers(lowEntry, 1, 27), /fn twice\(x: i64\) -> i64/, 'Low hover displays its declaration');
  await checkInferredTypes(folder, hovers, completions);
  console.log('PASS: VS Code Host diagnostics/run/F12, declaration and inferred hovers, field completion, unsaved imports, fallback, signatures, UTF-16, High/Low');
}

async function checkInferredTypes(folder, hovers, completions) {
  const project = path.join(folder, `inferred-${process.pid}`);
  fs.mkdirSync(project, { recursive: true });
  const modelsFile = path.join(project, 'models.nagi');
  const entryFile = path.join(project, 'main.nagi');
  const modelsText = 'class Point:\n    id: i32\n';
  const entryText = 'import "models.nagi"\ndef make() -> Point:\n    return Point(id=7)\nasync def inspect(argument: Point) -> Result[unit, Error]:\n    value = make()\n    print("😀"); print(value.id)\n    match ok(make()):\n        case Ok(payload):\n            print(payload.id)\n        case Err(problem):\n            print(error_kind(problem))\n    return ok(print(0))\ndef main():\n    print(0)\n';
  fs.writeFileSync(path.join(project, 'nagi.toml'), "entry = 'main.nagi'\n");
  fs.writeFileSync(modelsFile, modelsText);
  fs.writeFileSync(entryFile, entryText);
  const doc = await vscode.workspace.openTextDocument(entryFile);
  const models = await vscode.workspace.openTextDocument(modelsFile);
  await vscode.window.showTextDocument(doc);
  await new Promise(resolve => setTimeout(resolve, 500));
  const position = (document, needle, delta = 0) => {
    const offset = document.getText().indexOf(needle);
    assert.ok(offset >= 0, needle);
    return document.positionAt(offset + delta);
  };
  async function hover(document, needle, delta = 0) {
    const p = position(document, needle, delta);
    return hovers(document, p.line, p.character);
  }
  async function fields(document, needle, delta) {
    const p = position(document, needle, delta);
    const result = await completions(document, p.line, p.character);
    return result.items.filter(c => c.kind === vscode.CompletionItemKind.Field);
  }
  async function replace(document, text) {
    const edit = new vscode.WorkspaceEdit();
    edit.replace(document.uri, new vscode.Range(0, 0, document.lineCount, 0), text);
    await vscode.workspace.applyEdit(edit);
  }
  assert.match(await hover(doc, 'argument: Point', 2), /argument: Point/, 'parameter declaration has a type hover');
  assert.match(await hover(doc, 'value.id', 2), /value: Point/, 'inferred local hover uses UTF-16 after an emoji');
  assert.match(await hover(doc, 'payload.id', 2), /payload: Point/, 'Ok payload type is supplied by the checker');
  assert.match(await hover(doc, 'problem))', 2), /problem: Error/, 'Err payload hover shows Error');
  const initial = await fields(doc, 'value.id', 'value.'.length);
  assert.deepEqual(initial.map(c => c.label), ['id']);
  assert.equal(initial[0].insertText.value, 'id', 'field completion inserts only its name');
  assert.equal(initial[0].detail, 'id: i32');
  assert.equal(doc.getText(initial[0].range), 'id', 'completion replaces an existing member suffix');
  await replace(models, 'class Point:\n    id: i32\n    enabled: bool\n');
  const changed = await fields(doc, 'payload.id', 'payload.'.length);
  assert.deepEqual(changed.map(c => c.label).sort(), ['enabled', 'id'], 'unsaved imported class fields are used');
  assert.equal(fs.readFileSync(modelsFile, 'utf8'), modelsText, 'query leaves imported file untouched');
  await replace(models, modelsText);
  await replace(doc, entryText.replace('value.id)', 'value.)'));
  assert.deepEqual((await fields(doc, 'value.)', 'value.'.length)).map(c => c.label), ['id'], 'bare dot in an unsaved buffer offers fields');
  assert.equal(fs.readFileSync(entryFile, 'utf8'), entryText, 'completion never writes its analysis buffer');
  await replace(doc, entryText.replace('return ok(print(0))', 'print(payload.id)\n    return ok(print(0))'));
  const outside = doc.getText().lastIndexOf('payload.id');
  assert.ok(outside >= 0);
  const p = doc.positionAt(outside + 'payload.'.length);
  const outsideFields = (await completions(doc, p.line, p.character)).items.filter(c => c.kind === vscode.CompletionItemKind.Field);
  assert.equal(outsideFields.length, 0, 'case payload does not leak outside match');
  await replace(doc, entryText + '\ndef unfinished(:\n');
  assert.equal(await hover(doc, 'value.id', 2), '', 'saved fallback does not show a stale local type');
  assert.equal((await fields(doc, 'value.id', 'value.'.length)).length, 0, 'saved fallback does not guess receiver fields');
  assert.match(await hover(doc, 'value = make()', 'value = ma'.length), /保存済み/, 'declaration fallback stays available and is labelled');
  await replace(doc, entryText);
  await doc.save();
  await models.save();
  const lowFolder = path.join(folder, `inferred-low-${process.pid}`);
  fs.mkdirSync(lowFolder, { recursive: true });
  const lowFile = path.join(lowFolder, 'main.low');
  const lowText = 'record Point { id: i32; }\r\nfn main() { let value = Point(id=7); print("😀"); print(value.id); }\r\n';
  fs.writeFileSync(lowFile, lowText);
  const low = await vscode.workspace.openTextDocument(lowFile);
  assert.match(await hover(low, 'value.id', 2), /value: Point/, 'Low local hover has the correct same-line position');
  assert.deepEqual((await fields(low, 'value.id', 'value.'.length)).map(c => c.label), ['id'], 'Low record fields complete');
}

module.exports = { run };
