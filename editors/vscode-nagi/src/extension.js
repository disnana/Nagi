'use strict';
const vscode = require('vscode');
const path = require('node:path');
const fs = require('node:fs');
const crypto = require('node:crypto');
const compiler = require('./compiler');
const features = require('./features');

function activate(context) {
  const output = vscode.window.createOutputChannel('Nagi');
  const diagnostics = vscode.languages.createDiagnosticCollection('nagi');
  const pending = new Map();
  const results = new Map();
  const navigation = new Set();
  const symbolCache = new Map();
  let epoch = 0;
  let symbolsEpoch = 0;
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

  function invalidateSymbols() {
    symbolsEpoch++;
    for (const child of navigation) child.kill();
    navigation.clear();
    symbolCache.clear();
  }

  function invalidate() {
    epoch++;
    for (const d of vscode.workspace.textDocuments) cancel(d);
    results.clear();
    diagnostics.clear();
    invalidateSymbols();
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

  async function provideDefinition(document, position, token) {
    if (!isNagi(document) || !vscode.workspace.isTrusted || token.isCancellationRequested) return [];
    const version = document.version;
    const startEpoch = symbolsEpoch;
    const snapshot = await querySymbols(document, token);
    if (!snapshot || snapshot.saved || token.isCancellationRequested || startEpoch !== symbolsEpoch ||
        document.isClosed || document.version !== version) return [];
    try {
      const root = options(document).root;
      const target = compiler.definitionAt(snapshot.index, document.uri.fsPath, position.line, position.character, root);
      if (!target) return [];
      const range = new vscode.Range(target.line - 1, target.column - 1, target.line - 1, target.column - 1 + target.length);
      return [new vscode.Location(vscode.Uri.file(compiler.normalizeFile(target.file, root)), range)];
    } catch (error) { output.appendLine(`[symbols] ${error.message}`); return []; }
  }

  function querySymbols(document, token, member) {
    if (!isNagi(document) || !vscode.workspace.isTrusted || token.isCancellationRequested) return Promise.resolve();
    const settings = options(document);
    // Unsaved manifests cannot be passed as a source overlay.
    if (vscode.workspace.textDocuments.some(d => d.isDirty && d.uri.fsPath === settings.project)) return Promise.resolve();
    if (member && !fs.existsSync(document.uri.fsPath)) return Promise.resolve();
    const files = vscode.workspace.textDocuments.filter(d => isNagi(d) && (d.isDirty || member && d === document) && fs.existsSync(d.uri.fsPath))
      .map(d => ({ file: d.uri.fsPath, text: member && d === document ? member.text : d.getText() }));
    const input = JSON.stringify({ files });
    if (Buffer.byteLength(input) > 16 * 1000 * 1000) return Promise.resolve();
    const args = argsFor('symbols', document, settings);
    const key = JSON.stringify([settings.executable, args, crypto.createHash('sha256').update(input).digest('hex')]);
    const cached = symbolCache.get(key);
    const stamp = file => { try { const s = fs.statSync(file); return `${s.mtimeMs}:${s.size}`; } catch { return ''; } };
    if (cached && Date.now() - cached.created < 2000 && cached.stamps.every(([file, time]) => stamp(file) === time)) return Promise.resolve(cached.value);
    const startEpoch = symbolsEpoch;
    const version = document.version;
    function run(editorInput) {
      return new Promise(resolve => {
        let cancellation;
        const child = compiler.runCheck(settings.executable, editorInput ? [...args, '--editor-input'] : args,
          settings.root, settings.config.get('checkTimeoutMs', 15000), result => {
            navigation.delete(child);
            cancellation?.dispose();
            if (token.isCancellationRequested || startEpoch !== symbolsEpoch || document.isClosed || document.version !== version) return resolve();
            if (result.error) return resolve();
            try {
              const index = JSON.parse(result.output);
              if (index.format !== 'nagi-symbols-v1' || !Array.isArray(index.definitions)) throw new Error('Unsupported symbols format');
              resolve({ index, saved: !editorInput && files.length > 0 });
            } catch (error) { output.appendLine(`[symbols] ${error.message}`); resolve(); }
          }, 16 * 1024 * 1024, editorInput ? input : undefined);
        cancellation = token.onCancellationRequested(() => child.kill());
        navigation.add(child);
      });
    }
    return (async () => {
      let value = await run(files.length > 0);
      if (!value && files.length > 0 && startEpoch === symbolsEpoch && !token.isCancellationRequested) value = await run(false);
      if (value) {
        const paths = [...(value.index.files || []).map(file => compiler.normalizeFile(file, settings.root)), settings.project, settings.executable].filter(Boolean);
        symbolCache.set(key, { value, created: Date.now(), stamps: paths.map(file => [file, stamp(file)]) });
        if (symbolCache.size > 16) symbolCache.delete(symbolCache.keys().next().value);
      }
      return value;
    })();
  }

  function documentation(item, snapshot) {
    const text = new vscode.MarkdownString();
    text.appendCodeblock(item.signature, 'nagi');
    if (item.description) text.appendText(item.description);
    if (item.location) text.appendText(`\n${path.basename(compiler.normalizeFile(item.location.file, '.'))}:${item.location.line}`);
    if (snapshot?.saved && !item.builtin) text.appendText('\n書きかけの構文を解析できないため、保存済みの宣言を表示しています。');
    return text;
  }

  async function provideHover(document, position, token) {
    const version = document.version;
    const snapshot = await querySymbols(document, token);
    if (!snapshot || token.isCancellationRequested || document.isClosed || document.version !== version) return;
    const found = features.hoverAt(snapshot.index, document.getText(), document.offsetAt(position), { file: document.uri.fsPath, saved: snapshot.saved });
    if (!found) return;
    return new vscode.Hover(documentation(found.item, snapshot), new vscode.Range(document.positionAt(found.start), document.positionAt(found.end)));
  }

  async function provideCompletionItems(document, position, token) {
    const version = document.version;
    const member = features.memberContext(document.getText(), document.offsetAt(position));
    const snapshot = await querySymbols(document, token, member);
    if (token.isCancellationRequested || !vscode.workspace.isTrusted || document.isClosed || document.version !== version) return [];
    const text = document.getText();
    const offset = document.offsetAt(position);
    const word = features.wordAt(text, offset);
    return features.completionCandidates(snapshot?.index, text, offset, document.languageId === 'nagi-low', { file: document.uri.fsPath, saved: snapshot?.saved }).map(item => {
      const kind = { function: vscode.CompletionItemKind.Function, class: vscode.CompletionItemKind.Class, field: vscode.CompletionItemKind.Field, type: vscode.CompletionItemKind.TypeParameter, keyword: vscode.CompletionItemKind.Keyword }[item.kind];
      const completion = new vscode.CompletionItem(item.name, kind);
      completion.detail = item.signature + (snapshot?.saved && !item.builtin ? ' （保存済み）' : '');
      completion.documentation = documentation(item, snapshot);
      completion.insertText = new vscode.SnippetString(features.insertion(item, text.slice(word.end)));
      completion.range = new vscode.Range(document.positionAt(word.start), document.positionAt(word.end));
      completion.sortText = `${item.builtin ? '1' : item.kind === 'keyword' ? '2' : '0'}${item.name}`;
      return completion;
    });
  }

  async function provideSignatureHelp(document, position, token) {
    const version = document.version;
    const call = features.activeCall(document.getText(), document.offsetAt(position));
    if (!call) return;
    const snapshot = await querySymbols(document, token);
    if (!snapshot || token.isCancellationRequested || document.isClosed || document.version !== version) return;
    const item = features.declarations(snapshot.index).get(call.name);
    if (!item) return;
    const parameters = item.kind === 'class' ? item.fields || [] : item.parameters || [];
    const label = item.kind === 'class' ? `${item.name}(${parameters.map(p => `${p.name}: ${p.type}`).join(', ')}) -> ${item.name}` : item.signature;
    const signature = new vscode.SignatureInformation(label, documentation(item, snapshot));
    signature.parameters = parameters.map(p => {
      const typed = `${p.name}: ${p.type}`;
      return new vscode.ParameterInformation(label.includes(typed) ? typed : p.name);
    });
    const help = new vscode.SignatureHelp();
    help.signatures = [signature];
    help.activeSignature = 0;
    help.activeParameter = Math.min(call.argument, Math.max(0, parameters.length - 1));
    return help;
  }

  context.subscriptions.push(output, diagnostics,
    vscode.languages.registerDefinitionProvider([{ language: 'nagi', scheme: 'file' }, { language: 'nagi-low', scheme: 'file' }], { provideDefinition }),
    vscode.languages.registerHoverProvider([{ language: 'nagi', scheme: 'file' }, { language: 'nagi-low', scheme: 'file' }], { provideHover }),
    vscode.languages.registerCompletionItemProvider([{ language: 'nagi', scheme: 'file' }, { language: 'nagi-low', scheme: 'file' }], { provideCompletionItems }, '.'),
    vscode.languages.registerSignatureHelpProvider([{ language: 'nagi', scheme: 'file' }, { language: 'nagi-low', scheme: 'file' }], { provideSignatureHelp }, '(', ','),
    { dispose() { for (const job of pending.values()) job.child?.kill(); pending.clear(); for (const child of navigation) child.kill(); navigation.clear(); } },
    vscode.workspace.onDidOpenTextDocument(d => { if (optionsForAuto(d)) check(d); }),
    vscode.workspace.onDidSaveTextDocument(d => {
      if (isNagi(d) || compiler.findProject(d.uri.fsPath)) { invalidate(); recheckOpen(); }
    }),
    vscode.workspace.onDidChangeTextDocument(e => {
      if (isNagi(e.document) || compiler.findProject(e.document.uri.fsPath)) invalidate();
    }),
    vscode.workspace.onDidCloseTextDocument(d => {
      cancel(d);
      if (isNagi(d) || d.uri.scheme === 'file' && compiler.findProject(d.uri.fsPath)) invalidateSymbols();
      results.delete(d.uri.toString());
      publishDiagnostics();
    }),
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
