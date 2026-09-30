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
  const navigation = new Set();
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
    const project = compiler.findProject(document.uri.fsPath);
    return { root: project ? path.dirname(project) : root, workspace, config, project,
      executable: compiler.compilerPath(config.get('compilerPath', ''), root, workspace) };
  }

  function argsFor(name, document, options) {
    const { root, workspace, config, project } = options;
    return compiler.argumentsFor(name, document.uri.fsPath, config.get('nativeFiles', []), root, workspace,
      config.get('rustFile', ''), config.get('rustDependencies', []), project);
  }

  function projectDirty(project) {
    return project && vscode.workspace.textDocuments.some(d => d.isDirty && d.uri.scheme === 'file' &&
      compiler.findProject(d.uri.fsPath) === project);
  }

  async function saveProject(project) {
    for (const d of vscode.workspace.textDocuments) {
      if (d.isDirty && d.uri.scheme === 'file' && compiler.findProject(d.uri.fsPath) === project && !await d.save()) return false;
    }
    return true;
  }

  function invalidate() {
    epoch++;
    for (const d of vscode.workspace.textDocuments) cancel(d);
    results.clear();
    diagnostics.clear();
    for (const child of navigation) child.kill();
    navigation.clear();
  }

  function recheckOpen() {
    for (const d of vscode.workspace.textDocuments) if (optionsForAuto(d)) check(d);
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
    const settings = options(document);
    const { root, config, executable, project } = settings;
    if (projectDirty(project)) return Promise.resolve();
    const version = document.version;
    const startEpoch = epoch;
    const job = {};
    pending.set(key, job);
    return new Promise(resolve => {
      job.child = compiler.runCheck(executable,
        argsFor('check', document, settings),
        root, config.get('checkTimeoutMs', 15000), result => {
          if (pending.get(key) !== job || document.isClosed || document.version !== version || document.isDirty || startEpoch !== epoch) return resolve();
          pending.delete(key);
          output.appendLine(`[check] ${document.uri.fsPath}\n${result.output || result.error?.message || ''}`);
          if (!result.error) {
            results.delete(key);
            if (manual) vscode.window.setStatusBarMessage('Nagi: 型検査 OK', 3000);
          } else {
            const text = result.output || result.error.message;
            const parsed = compiler.parseDiagnostics(text, project || document.uri.fsPath)[0];
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
    const settings = options(document);
    // Imported files and the manifest must be saved before a project-wide command.
    if (projectDirty(settings.project) && !await saveProject(settings.project)) return;
    if (name === 'check') return check(document, true);
    const { root, executable, project } = settings;
    const task = new vscode.Task({ type: 'nagi', command: name, file: document.uri.fsPath },
      vscode.workspace.getWorkspaceFolder(document.uri) || vscode.TaskScope.Workspace,
      `Nagi ${name}: ${project ? path.basename(root) : path.basename(document.uri.fsPath)}`, 'Nagi',
      new vscode.ProcessExecution(executable,
        argsFor(name, document, settings), { cwd: root }));
    task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, panel: vscode.TaskPanelKind.Dedicated };
    return vscode.tasks.executeTask(task);
  }

  function provideDefinition(document, position, token) {
    if (!isNagi(document) || !vscode.workspace.isTrusted || token.isCancellationRequested) return [];
    const settings = options(document);
    if (document.isDirty || projectDirty(settings.project)) {
      vscode.window.setStatusBarMessage('Nagi: 定義ジャンプの前にプロジェクトの変更を保存してください', 3000);
      return [];
    }
    const version = document.version;
    const startEpoch = epoch;
    return new Promise(resolve => {
      let cancellation;
      const child = compiler.runCheck(settings.executable, argsFor('symbols', document, settings), settings.root,
        settings.config.get('checkTimeoutMs', 15000), result => {
          navigation.delete(child);
          cancellation?.dispose();
          if (token.isCancellationRequested || startEpoch !== epoch || document.isClosed || document.version !== version || document.isDirty) return resolve([]);
          try {
            if (result.error) throw new Error(result.output || result.error.message);
            const target = compiler.definitionAt(JSON.parse(result.output), document.uri.fsPath, position.line, position.character, settings.root);
            if (!target) return resolve([]);
            const range = new vscode.Range(target.line - 1, target.column - 1, target.line - 1, target.column - 1 + target.length);
            resolve([new vscode.Location(vscode.Uri.file(compiler.normalizeFile(target.file, settings.root)), range)]);
          } catch (error) { output.appendLine(`[symbols] ${error.message}`); resolve([]); }
        }, 16 * 1024 * 1024);
      cancellation = token.onCancellationRequested(() => child.kill());
      navigation.add(child);
    });
  }

  context.subscriptions.push(output, diagnostics,
    vscode.languages.registerDefinitionProvider([{ language: 'nagi', scheme: 'file' }, { language: 'nagi-low', scheme: 'file' }], { provideDefinition }),
    { dispose() { for (const job of pending.values()) job.child?.kill(); pending.clear(); for (const child of navigation) child.kill(); navigation.clear(); } },
    vscode.workspace.onDidOpenTextDocument(d => { if (optionsForAuto(d)) check(d); }),
    vscode.workspace.onDidSaveTextDocument(d => {
      if (isNagi(d) || compiler.findProject(d.uri.fsPath)) { invalidate(); recheckOpen(); }
    }),
    vscode.workspace.onDidChangeTextDocument(e => {
      if (isNagi(e.document) || compiler.findProject(e.document.uri.fsPath)) invalidate();
    }),
    vscode.workspace.onDidCloseTextDocument(d => { cancel(d); results.delete(d.uri.toString()); publishDiagnostics(); }),
    vscode.workspace.onDidGrantWorkspaceTrust(() => { for (const d of vscode.workspace.textDocuments) if (optionsForAuto(d)) check(d); }),
    vscode.workspace.onDidChangeConfiguration(e => { if (e.affectsConfiguration('nagi')) { invalidate(); recheckOpen(); } }),
    vscode.commands.registerCommand('nagi.showOutput', () => output.show(true)));
  for (const name of ['check', 'lower', 'build', 'run']) {
    context.subscriptions.push(vscode.commands.registerCommand(`nagi.${name}`, () => command(name)));
  }
  function optionsForAuto(d) { return isNagi(d) && vscode.workspace.getConfiguration('nagi', d.uri).get('checkOnSave', true); }
  for (const d of vscode.workspace.textDocuments) if (optionsForAuto(d)) check(d);
  const watcher = vscode.workspace.createFileSystemWatcher('**/nagi.toml');
  context.subscriptions.push(watcher, watcher.onDidCreate(() => { invalidate(); recheckOpen(); }),
    watcher.onDidChange(() => { invalidate(); recheckOpen(); }), watcher.onDidDelete(() => { invalidate(); recheckOpen(); }));
}

module.exports = { activate };
