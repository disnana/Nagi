'use strict';
const vscode = require('vscode');
const path = require('node:path');
const fs = require('node:fs');
const compiler = require('./compiler');

function activate(context) {
  const output = vscode.window.createOutputChannel('Nagi');
  const diagnostics = vscode.languages.createDiagnosticCollection('nagi');
  const pending = new Map();
  const results = new Map();
  let epoch = 0;
  const isNagi = d => d && ['nagi', 'nagi-low'].includes(d.languageId) && d.uri.scheme === 'file';
  function publishDiagnostics() {
    diagnostics.clear();
    const grouped = new Map();
    for (const result of results.values()) {
      const uri = result.uri.toString();
      if (!grouped.has(uri)) grouped.set(uri, { uri: result.uri, items: [] });
      const items = grouped.get(uri).items;
      if (!items.some(item => item.message === result.item.message && item.range.isEqual(result.item.range))) items.push(result.item);
    }
    for (const group of grouped.values()) diagnostics.set(group.uri, group.items);
  }

  function options(document) {
    const workspace = vscode.workspace.getWorkspaceFolder(document.uri)?.uri.fsPath;
    const root = compiler.findRoot(document.uri.fsPath, workspace);
    const config = vscode.workspace.getConfiguration('nagi', document.uri);
    return { root, workspace, config, executable: compiler.compilerPath(config.get('compilerPath', ''), root, workspace) };
  }

  function cancel(document) {
    const key = document.uri.toString();
    const job = pending.get(key);
    pending.delete(key);
    job?.child?.kill();
  }

  function check(document, manual = false) {
    if (!isNagi(document) || !vscode.workspace.isTrusted || document.isDirty) return Promise.resolve();
    cancel(document);
    const key = document.uri.toString();
    const { root, workspace, config, executable } = options(document);
    const version = document.version;
    const startEpoch = epoch;
    const job = {};
    pending.set(key, job);
    return new Promise(resolve => {
      job.child = compiler.runCheck(executable,
        compiler.argumentsFor('check', document.uri.fsPath, config.get('nativeFiles', []), root, workspace, config.get('rustFile', ''), config.get('rustDependencies', [])),
        root, config.get('checkTimeoutMs', 15000), result => {
          if (pending.get(key) !== job || document.isClosed || document.version !== version || document.isDirty || startEpoch !== epoch) return resolve();
          pending.delete(key);
          output.appendLine(`[check] ${document.uri.fsPath}\n${result.output || result.error?.message || ''}`);
          if (!result.error) {
            results.delete(key);
            if (manual) vscode.window.setStatusBarMessage('Nagi: 型検査 OK', 3000);
          } else {
            const text = result.output || result.error.message;
            const parsed = compiler.parseDiagnostics(text, document.uri.fsPath)[0];
            const target = vscode.Uri.file(compiler.normalizeFile(parsed.file, root));
            let sourceLines;
            try { sourceLines = fs.readFileSync(target.fsPath, 'utf8').split(/\r?\n/); } catch { sourceLines = ['']; }
            const line = Math.min(parsed.line, sourceLines.length - 1);
            const textLine = sourceLines[line];
            const start = textLine.search(/\S/);
            const item = new vscode.Diagnostic(new vscode.Range(line, Math.max(0, start),
              line, Math.max(Math.max(0, start) + 1, textLine.length)),
              parsed.message, vscode.DiagnosticSeverity.Error);
            item.source = 'nagic';
            results.set(key, { uri: target, item });
            if (manual) output.show(true);
          }
          publishDiagnostics();
          resolve();
        });
    });
  }

  async function command(name) {
    if (!vscode.workspace.isTrusted) {
      vscode.window.showWarningMessage('Nagiのコンパイラ実行にはワークスペースの信頼が必要です。');
      return;
    }
    const document = vscode.window.activeTextEditor?.document;
    if (!isNagi(document)) return vscode.window.showInformationMessage('.nagi または .low ファイルを開いてください。');
    if (!await document.save()) return;
    if (name === 'check') return check(document, true);
    const { root, workspace, config, executable } = options(document);
    const task = new vscode.Task({ type: 'nagi', command: name, file: document.uri.fsPath },
      vscode.workspace.getWorkspaceFolder(document.uri) || vscode.TaskScope.Workspace,
      `Nagi ${name}: ${path.basename(document.uri.fsPath)}`, 'Nagi',
      new vscode.ProcessExecution(executable,
        compiler.argumentsFor(name, document.uri.fsPath, config.get('nativeFiles', []), root, workspace, config.get('rustFile', ''), config.get('rustDependencies', [])), { cwd: root }));
    task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, panel: vscode.TaskPanelKind.Dedicated };
    return vscode.tasks.executeTask(task);
  }

  context.subscriptions.push(output, diagnostics,
    { dispose() { for (const job of pending.values()) job.child?.kill(); pending.clear(); } },
    vscode.workspace.onDidOpenTextDocument(d => { if (optionsForAuto(d)) check(d); }),
    vscode.workspace.onDidSaveTextDocument(d => { if (optionsForAuto(d)) check(d); }),
    vscode.workspace.onDidChangeTextDocument(e => { if (isNagi(e.document)) { epoch++; for (const d of vscode.workspace.textDocuments) cancel(d); results.clear(); diagnostics.clear(); } }),
    vscode.workspace.onDidCloseTextDocument(d => { cancel(d); results.delete(d.uri.toString()); publishDiagnostics(); }),
    vscode.workspace.onDidGrantWorkspaceTrust(() => { for (const d of vscode.workspace.textDocuments) if (optionsForAuto(d)) check(d); }),
    vscode.commands.registerCommand('nagi.showOutput', () => output.show(true)));
  for (const name of ['check', 'lower', 'build', 'run']) {
    context.subscriptions.push(vscode.commands.registerCommand(`nagi.${name}`, () => command(name)));
  }
  function optionsForAuto(d) { return isNagi(d) && vscode.workspace.getConfiguration('nagi', d.uri).get('checkOnSave', true); }
  for (const d of vscode.workspace.textDocuments) if (optionsForAuto(d)) check(d);
}

module.exports = { activate };
