'use strict';
// Run through --extensionTestsPath with the matching compiler on PATH.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vscode = require('vscode');

module.exports.run = async () => {
  const root = path.resolve(__dirname, '../../..');
  const folder = process.env.NAGI_EDITOR_HOST_FIXTURES || path.join(root, 'build', 'vscode-task-inputs-host');
  fs.mkdirSync(folder, { recursive: true });
  const extension = vscode.extensions.getExtension('Disnana.nagi-lang');
  assert.ok(extension);
  await extension.activate();
  let cases = 0;
  async function open(name, text) {
    const file = path.join(folder, name);
    fs.writeFileSync(file, text);
    return vscode.workspace.openTextDocument(file);
  }
  async function replace(document, text) {
    const edit = new vscode.WorkspaceEdit();
    edit.replace(document.uri, new vscode.Range(document.positionAt(0), document.positionAt(document.getText().length)), text);
    assert.ok(await vscode.workspace.applyEdit(edit));
  }
  async function lower(document) {
    await vscode.window.showTextDocument(document);
    let execution, listener, timer;
    const finished = new Map();
    const completion = new Promise((resolve, reject) => {
      timer = setTimeout(() => { execution?.terminate(); reject(new Error('lower task timed out')); }, 30000);
      listener = vscode.tasks.onDidEndTaskProcess(event => {
        finished.set(event.execution, event.exitCode);
        if (event.execution === execution) resolve(event.exitCode);
      });
    });
    try {
      execution = await vscode.commands.executeCommand('nagi.lower');
      assert.ok(execution, 'command launches after its input buffers are saved');
      assert.equal(finished.has(execution) ? finished.get(execution) : await completion, 0);
    } finally { clearTimeout(timer); listener.dispose(); }
  }
  for (const suffix of ['nagi', 'low']) {
    const entry = suffix === 'nagi' ? `import "helper.${suffix}" as helper\ndef main():\n    print(helper.answer())\n`
      : `import "helper.${suffix}" as helper;\nfn main() -> unit { print(helper.answer()); }\n`;
    const source = value => suffix === 'nagi' ? `def answer() -> i64:\n    return ${value}\n`
      : `fn answer() -> i64 { return ${value}; }\n`;
    const main = await open(`main.${suffix}`, entry);
    const helper = await open(`helper.${suffix}`, source(7));
    const unrelated = await open(`unrelated.${suffix}`, source(9));
    await replace(helper, source(42));
    await replace(unrelated, source(99));
    assert.ok(helper.isDirty && unrelated.isDirty);
    await lower(main);
    assert.equal(helper.isDirty, false);
    assert.equal(fs.readFileSync(helper.uri.fsPath, 'utf8'), source(42));
    assert.equal(unrelated.isDirty, true);
    assert.equal(fs.readFileSync(unrelated.uri.fsPath, 'utf8'), source(9));
    cases++;
  }
  for (const name of ['Async main', 'HTTP server']) {
    const doc = await open(`${name.replaceAll(' ', '-')}.nagi`, '');
    const editor = await vscode.window.showTextDocument(doc);
    assert.ok(await editor.insertSnippet(new vscode.SnippetString(require('../snippets/high.json')[name].body.join('\n'))));
    await vscode.commands.executeCommand('leaveSnippet');
    await vscode.commands.executeCommand('nagi.check');
    assert.equal(vscode.languages.getDiagnostics(doc.uri).length, 0);
    assert.ok(!doc.getText().includes('db_open'));
    if (name === 'HTTP server') {
      assert.match(doc.getText(), /shared\[State\]/);
      assert.match(doc.getText(), /http\.app_default\[State\]\(State\(/);
    }
    cases++;
  }
  const result = process.env.NAGI_EDITOR_HOST_RESULT || path.join(root, 'build', 'vscode-task-inputs-host-result.json');
  fs.writeFileSync(result, JSON.stringify({ passed: true, cases }, null, 2));
};
