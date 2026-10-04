'use strict';
const vscode = require('vscode');
const path = require('node:path');
const fs = require('node:fs');
const crypto = require('node:crypto');
const compiler = require('./compiler');
const features = require('./features');
const indentation = require('./indentation');
const stdlib = require('./stdlib');

function activate(context) {
  const output = vscode.window.createOutputChannel('Nagi');
  const diagnostics = vscode.languages.createDiagnosticCollection('nagi');
  const taskDiagnostics = vscode.languages.createDiagnosticCollection('nagi-task');
  const pending = new Map();
  const results = new Map();
  const navigation = new Set();
  const symbolCache = new Map();
  const notifiedFailures = new Set();
  const standardSources = new Map();
  const taskScopes = new Map();
  const runningTasks = new Map();
  const endedTasks = new WeakSet();
  const standardSourceChanges = new vscode.EventEmitter();
  let epoch = 0;
  let symbolsEpoch = 0;
  const isNagi = d => d && ['nagi', 'nagi-low'].includes(d.languageId) && d.uri.scheme === 'file';
  const assistanceSelector = ['file', 'untitled'].flatMap(scheme => ['nagi', 'nagi-low'].map(language => ({ language, scheme })));
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

  function clearTaskFile(uri) {
    if (uri.scheme !== 'file') return;
    const root = path.dirname(uri.fsPath);
    const key = compiler.fileKey(uri.fsPath, root);
    const targets = new Map([[uri.toString(), uri]]);
    // Task markers may use a canonical path while the editor displays an alias.
    for (const [resource, items] of vscode.languages.getDiagnostics()) {
      if (resource.scheme === 'file' && items.some(item => item.source === 'nagic') &&
          compiler.fileKey(resource.fsPath, root) === key) targets.set(resource.toString(), resource);
    }
    for (const resource of targets.values()) taskDiagnostics.delete(resource);
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

  function symbolKey(executable, args, input) {
    return JSON.stringify([executable, args, crypto.createHash('sha256').update(input).digest('hex')]);
  }

  function stamp(file) {
    try { const s = fs.statSync(file); return `${s.mtimeMs}:${s.size}`; } catch { return ''; }
  }

  function dependencySnapshot(owner, stampFor, keyFor) {
    let snapshot = owner.dependencies;
    if (!snapshot) {
      const cached = symbolCache.get(owner.symbolsKey);
      if (!cached || cached.value.saved || cached.scope !== owner.scope || !Array.isArray(cached.value.index.files) ||
          cached.value.index.files.some(file => typeof file !== 'string')) return;
      const files = cached.value.index.files.filter(file => !stdlib.sourceUri(file));
      if (!files.length) return;
      snapshot = { files, stamps: cached.stamps };
    }
    const ownerKey = keyFor(owner.ownerFile, owner.root);
    if (!snapshot.stamps.every(([file, time]) => stampFor(file) === time) ||
        !snapshot.files.some(file => keyFor(file, owner.root) === ownerKey)) return;
    owner.dependencies = snapshot;
    return snapshot;
  }

  function checkScope(document, settings) {
    return `${settings.project ? 'project' : 'source'}:${compiler.fileKey(settings.project || document.uri.fsPath, settings.root)}`;
  }

  function taskSnapshot(document, settings) {
    const scope = checkScope(document, settings);
    const projectKey = settings.project && compiler.fileKey(settings.project, settings.root);
    const directory = path.dirname(document.uri.fsPath);
    const includes = d => {
      if (d.uri.scheme !== 'file' || !isNagi(d) && d.uri.fsPath !== settings.project) return false;
      const project = compiler.findProject(d.uri.fsPath);
      if (projectKey) return project && compiler.fileKey(project, settings.root) === projectKey;
      const relative = path.relative(directory, d.uri.fsPath);
      return !project && relative !== '..' && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
    };
    const versions = new Map(vscode.workspace.textDocuments.filter(includes).map(d => [d, d.version]));
    const configuration = JSON.stringify([settings.executable, argsFor('check', document, settings), settings.config.get('checkTimeoutMs', 15000)]);
    return { scope, includes, versions, configuration };
  }

  function unchangedTask(snapshot, document, settings) {
    const configuration = JSON.stringify([settings.executable, argsFor('check', document, settings), settings.config.get('checkTimeoutMs', 15000)]);
    return configuration === snapshot.configuration && [...snapshot.versions].every(([d, version]) => !d.isClosed && d.version === version) &&
      !vscode.workspace.textDocuments.some(d => d.isDirty && snapshot.includes(d) && !snapshot.versions.has(d));
  }

  function releaseTaskScope(snapshot) {
    const active = taskScopes.get(snapshot.scope);
    active?.delete(snapshot);
    if (!active?.size) taskScopes.delete(snapshot.scope);
  }

  function clearLiveScope(scope) {
    for (const [owner, job] of pending) {
      if (job.scope === scope) { pending.delete(owner); job.child?.kill(); }
    }
    for (const [owner, result] of results) if (result.scope === scope) results.delete(owner);
    publishDiagnostics();
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

  function invalidate(document) {
    epoch++;
    // Several owners can share the same checked dependency graph. Resolve each
    // path only once for this event, without retaining filesystem identities.
    const stamps = new Map(), keys = new Map();
    const stampFor = file => {
      if (!stamps.has(file)) stamps.set(file, stamp(file));
      return stamps.get(file);
    };
    const keyFor = (file, root) => {
      const normalized = compiler.normalizeFile(file, root);
      if (!keys.has(normalized)) keys.set(normalized, compiler.fileKey(normalized, root));
      return keys.get(normalized);
    };
    const changed = isNagi(document) ? {
      key: keyFor(document.uri.fsPath, '.'), scope: checkScope(document, options(document)),
    } : undefined;
    const unrelated = owner => {
      if (!changed || owner.scope === changed.scope) return false;
      const dependencies = dependencySnapshot(owner, stampFor, keyFor);
      return dependencies && !dependencies.files.some(file => keyFor(file, owner.root) === changed.key);
    };
    for (const [key, job] of pending) {
      if (unrelated(job)) job.epoch = epoch;
      else { pending.delete(key); job.child?.kill(); }
    }
    for (const [key, result] of results) if (!unrelated(result)) results.delete(key);
    publishDiagnostics();
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

  function overlays() {
    return vscode.workspace.textDocuments.filter(d => isNagi(d) && !d.isClosed && d.isDirty && fs.existsSync(d.uri.fsPath))
      .map(d => ({ file: d.uri.fsPath, text: d.getText() }));
  }

  function check(document, manual = false) {
    if (!isNagi(document) || !vscode.workspace.isTrusted) return Promise.resolve();
    cancel(document);
    const key = document.uri.toString();
    const settings = options(document);
    const { root, config, executable, project } = settings;
    const scope = checkScope(document, settings);
    if (!manual && [...(taskScopes.get(scope) || [])].some(snapshot => unchangedTask(snapshot, document, settings))) return Promise.resolve();
    if (vscode.workspace.textDocuments.some(d => d.isDirty && d.uri.fsPath === project)) return Promise.resolve();
    const files = overlays();
    const input = files.length ? JSON.stringify({ files }) : undefined;
    if (input && Buffer.byteLength(input) > 16 * 1000 * 1000) return Promise.resolve();
    const version = document.version;
    const overlayKeys = files.map(file => compiler.fileKey(file.file, root));
    const job = { overlayKeys, root, scope, epoch, ownerFile: document.uri.fsPath,
      symbolsKey: symbolKey(executable, argsFor('symbols', document, settings), JSON.stringify({ files })) };
    pending.set(key, job);
    return new Promise(resolve => {
      job.child = compiler.runCheck(executable,
        [...argsFor('check', document, settings), ...(input ? ['--editor-input'] : [])],
        root, config.get('checkTimeoutMs', 15000), result => {
          if (pending.get(key) !== job || document.isClosed || document.version !== version || job.epoch !== epoch) return resolve();
          pending.delete(key);
          output.appendLine(`[check] ${document.uri.fsPath}\n${result.output || result.error?.message || ''}`);
          const failure = compiler.processFailure(result.error);
          if (failure) {
            results.delete(key);
            const failureKey = JSON.stringify([root, executable, result.error.code, result.error.signal]);
            if (manual || !notifiedFailures.has(failureKey)) {
              notifiedFailures.add(failureKey);
              vscode.window.showWarningMessage(failure, 'Nagiの出力を開く', 'コンパイラの設定を開く').then(choice => {
                if (choice === 'Nagiの出力を開く') output.show(true);
                if (choice === 'コンパイラの設定を開く') vscode.commands.executeCommand('workbench.action.openSettings', 'nagi.compilerPath');
              });
            }
            if (manual) output.show(true);
          } else if (!result.error) {
            results.delete(key);
            for (const failureKey of notifiedFailures) {
              const [failureRoot, failureExecutable] = JSON.parse(failureKey);
              if (failureRoot === root && failureExecutable === executable) notifiedFailures.delete(failureKey);
            }
            if (manual) vscode.window.setStatusBarMessage('Nagi: 型検査 OK', 3000);
          } else {
            const text = result.output || result.error.message;
            const parsed = compiler.parseDiagnostics(text, project || document.uri.fsPath)[0];
            const targetKey = compiler.fileKey(parsed.file, root);
            const overlay = files.find(file => compiler.fileKey(file.file, root) === targetKey);
            const sourceDocument = vscode.workspace.textDocuments.find(d => isNagi(d) && !d.isClosed &&
              compiler.fileKey(d.uri.fsPath, root) === targetKey && (!overlay || d.uri.fsPath === overlay.file));
            const target = sourceDocument?.uri || vscode.Uri.file(compiler.normalizeFile(parsed.file, root));
            let sourceLines;
            try { sourceLines = (overlay?.text ?? sourceDocument?.getText() ?? fs.readFileSync(target.fsPath, 'utf8')).split(/\r?\n/); } catch { sourceLines = ['']; }
            const line = Math.min(parsed.line, sourceLines.length - 1);
            const textLine = sourceLines[line];
            const start = textLine.search(/\S/);
            const item = new vscode.Diagnostic(new vscode.Range(line, Math.max(0, start),
              line, Math.max(Math.max(0, start) + 1, textLine.length)),
              parsed.message, vscode.DiagnosticSeverity.Error);
            item.source = 'nagic';
            clearTaskFile(target);
            results.set(key, { ...job, child: undefined, uri: target, item });
            if (manual) output.show(true);
          }
          publishDiagnostics();
          resolve();
        }, 1024 * 1024, input);
    });
  }

  async function command(name) {
    if (!vscode.workspace.isTrusted) {
      vscode.window.showWarningMessage('Nagiのコンパイラ実行にはワークスペースの信頼が必要です。');
      return;
    }
    const document = vscode.window.activeTextEditor?.document;
    if (!isNagi(document)) return vscode.window.showInformationMessage('.nagi または .low ファイルを開いてください。');
    if (name === 'check') return check(document, true);
    if (!await document.save()) return;
    const settings = options(document);
    // Imported files and the manifest must be saved before a project-wide command.
    if (projectDirty(settings.project) && !await saveProject(settings.project)) return;
    const { root, executable, project } = settings;
    const task = new vscode.Task({ type: 'nagi', command: name, file: document.uri.fsPath },
      vscode.workspace.getWorkspaceFolder(document.uri) || vscode.TaskScope.Workspace,
      `Nagi ${name}: ${project ? path.basename(root) : path.basename(document.uri.fsPath)}`, 'Nagi',
      new vscode.ProcessExecution(executable,
        argsFor(name, document, settings), { cwd: root }));
    task.problemMatchers = ['$nagi'];
    task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, panel: vscode.TaskPanelKind.Dedicated };
    const snapshot = taskSnapshot(document, settings);
    const scope = snapshot.scope;
    if (!taskScopes.has(scope)) taskScopes.set(scope, new Set());
    taskScopes.get(scope).add(snapshot);
    clearLiveScope(scope);
    let handedOff = false;
    try {
      const execution = await vscode.tasks.executeTask(task);
      if (!endedTasks.has(execution)) { runningTasks.set(execution, snapshot); handedOff = true; }
      return execution;
    } finally {
      if (!handedOff) releaseTaskScope(snapshot);
    }
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
      const standard = stdlib.sourceUri(target.file);
      if (target.file.startsWith('stdlib:') && (!standard || !standardSources.has(target.file))) return [];
      const uri = standard ? vscode.Uri.parse(standard) : vscode.Uri.file(compiler.normalizeFile(target.file, root));
      return [new vscode.Location(uri, range)];
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
    const key = symbolKey(settings.executable, args, input);
    const cached = symbolCache.get(key);
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
              for (const source of stdlib.sources(index)) {
                if (standardSources.get(source.file) !== source.text) {
                  standardSources.set(source.file, source.text);
                  standardSourceChanges.fire(vscode.Uri.parse(stdlib.sourceUri(source.file)));
                }
              }
              while (standardSources.size > 128) standardSources.delete(standardSources.keys().next().value);
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
        const paths = [...(value.index.files || []).filter(file => !stdlib.sourceUri(file)).map(file => compiler.normalizeFile(file, settings.root)), settings.project, settings.executable].filter(Boolean);
        symbolCache.set(key, { value, scope: checkScope(document, settings), created: Date.now(), stamps: paths.map(file => [file, stamp(file)]) });
        if (symbolCache.size > 16) symbolCache.delete(symbolCache.keys().next().value);
      }
      return value;
    })();
  }

  function documentation(item, snapshot) {
    const text = new vscode.MarkdownString();
    text.appendCodeblock(item.signature, 'nagi');
    if (item.description) text.appendText(item.description);
    if (item.location) text.appendText(`\n${stdlib.sourceUri(item.location.file) ? item.location.file.slice('stdlib:'.length) : path.basename(compiler.normalizeFile(item.location.file, '.'))}:${item.location.line}`);
    if (snapshot?.saved && !item.builtin) text.appendText('\n書きかけの構文を解析できないため、保存済みの宣言を表示しています。');
    if (item.builtin && (!snapshot || snapshot.saved)) text.appendText(item.kind === 'pattern' ? '\nmatchの分岐' : '\n組み込み関数');
    return text;
  }

  async function provideHover(document, position, token) {
    const version = document.version;
    const snapshot = await querySymbols(document, token);
    if (token.isCancellationRequested || document.isClosed || document.version !== version) return;
    const found = features.hoverAt(snapshot?.index, document.getText(), document.offsetAt(position), { file: document.uri.fsPath, saved: snapshot?.saved });
    if (!found) return;
    return new vscode.Hover(documentation(found.item, snapshot), new vscode.Range(document.positionAt(found.start), document.positionAt(found.end)));
  }

  async function provideCompletionItems(document, position, token) {
    const version = document.version;
    const member = features.memberContext(document.getText(), document.offsetAt(position));
    const snapshot = await querySymbols(document, token, member);
    if (token.isCancellationRequested || document.isClosed || document.version !== version) return [];
    const text = document.getText();
    const offset = document.offsetAt(position);
    const word = features.wordAt(text, offset);
    return features.completionCandidates(snapshot?.index, text, offset, document.languageId === 'nagi-low', { file: document.uri.fsPath, saved: snapshot?.saved }).map(item => {
      const kind = { function: vscode.CompletionItemKind.Function, variable: vscode.CompletionItemKind.Variable, pattern: vscode.CompletionItemKind.Keyword, class: vscode.CompletionItemKind.Class, resource: vscode.CompletionItemKind.Class, constant: vscode.CompletionItemKind.Constant, enum: vscode.CompletionItemKind.Enum, enum_member: vscode.CompletionItemKind.EnumMember, field: vscode.CompletionItemKind.Field, module: vscode.CompletionItemKind.Module, type: vscode.CompletionItemKind.TypeParameter, keyword: vscode.CompletionItemKind.Keyword }[item.kind];
      const completion = new vscode.CompletionItem(item.name, kind);
      completion.detail = item.signature + (snapshot?.saved && !item.builtin ? ' （保存済み）' : '');
      completion.documentation = documentation(item, snapshot);
      completion.insertText = new vscode.SnippetString(features.insertion(item, text.slice(word.end)));
      completion.range = new vscode.Range(document.positionAt(item.replaceStart ?? word.start), document.positionAt(item.replaceEnd ?? word.end));
      completion.sortText = `${item.builtin ? '1' : item.kind === 'keyword' ? '2' : '0'}${item.name}`;
      return completion;
    });
  }

  async function provideSignatureHelp(document, position, token) {
    const version = document.version;
    if (!features.activeCall(document.getText(), document.offsetAt(position), true)) return;
    const snapshot = await querySymbols(document, token);
    if (token.isCancellationRequested || document.isClosed || document.version !== version) return;
    const found = features.signatureAt(snapshot?.index, document.getText(), document.offsetAt(position), { file: document.uri.fsPath, saved: snapshot?.saved });
    if (!found) return;
    const { item } = found;
    const parameters = item.kind === 'class' ? item.fields || [] : item.parameters || [];
    const constructor = item.callName || item.name;
    const label = item.kind === 'class' ? `${constructor}(${parameters.map(p => `${p.name}: ${p.type}`).join(', ')}) -> ${constructor}` : item.signature;
    const signature = new vscode.SignatureInformation(label, documentation(item, snapshot));
    signature.parameters = parameters.map(p => {
      const typed = `${p.name}: ${p.type}`;
      return new vscode.ParameterInformation(label.includes(typed) ? typed : p.name);
    });
    const help = new vscode.SignatureHelp();
    help.signatures = [signature];
    help.activeSignature = 0;
    help.activeParameter = Math.min(found.argument, Math.max(0, parameters.length - 1));
    return help;
  }

  context.subscriptions.push(output, diagnostics, taskDiagnostics, standardSourceChanges,
    vscode.workspace.registerTextDocumentContentProvider('nagi-stdlib', {
      onDidChange: standardSourceChanges.event,
      provideTextDocumentContent(uri) { return standardSources.get(stdlib.sourceFile(uri)) || ''; },
    }),
    vscode.languages.registerOnTypeFormattingEditProvider([{ language: 'nagi' }, { language: 'nagi-low' }], {
      provideOnTypeFormattingEdits(document, position, ch, options, token) {
        if (token.isCancellationRequested) return [];
        return indentation.edits(document.getText(), position.line, position.character, ch, options, document.languageId)
          .map(edit => vscode.TextEdit.replace(new vscode.Range(edit.line, 0, edit.line, edit.length), edit.text));
      },
    }, '\n', ':', ')', ']', '}'),
    vscode.languages.registerDefinitionProvider([{ language: 'nagi', scheme: 'file' }, { language: 'nagi-low', scheme: 'file' }], { provideDefinition }),
    vscode.languages.registerHoverProvider(assistanceSelector, { provideHover }),
    vscode.languages.registerCompletionItemProvider(assistanceSelector, { provideCompletionItems }, '.'),
    vscode.languages.registerSignatureHelpProvider(assistanceSelector, { provideSignatureHelp }, '(', ','),
    { dispose() { for (const job of pending.values()) job.child?.kill(); pending.clear(); for (const child of navigation) child.kill(); navigation.clear(); } },
    vscode.workspace.onDidOpenTextDocument(d => { if (optionsForAuto(d)) check(d); }),
    vscode.workspace.onDidSaveTextDocument(d => {
      if (isNagi(d) || compiler.findProject(d.uri.fsPath)) { invalidate(d); recheckOpen(); }
    }),
    vscode.workspace.onDidChangeTextDocument(e => {
      if (isNagi(e.document) || compiler.findProject(e.document.uri.fsPath)) { clearTaskFile(e.document.uri); invalidate(e.document); }
    }),
    vscode.workspace.onDidCloseTextDocument(d => {
      if (isNagi(d)) clearTaskFile(d.uri);
      cancel(d);
      if (isNagi(d) || d.uri.scheme === 'file' && compiler.findProject(d.uri.fsPath)) invalidateSymbols();
      const refresh = new Set();
      if (d.uri.scheme === 'file') {
        for (const [owner, job] of pending) {
          if (job.overlayKeys?.includes(compiler.fileKey(d.uri.fsPath, job.root))) {
            pending.delete(owner);
            job.child?.kill();
            refresh.add(owner);
          }
        }
        for (const [owner, result] of results) {
          const closedKey = compiler.fileKey(d.uri.fsPath, result.root);
          if (result.overlayKeys?.includes(closedKey)) {
            refresh.add(owner);
            if (compiler.fileKey(result.uri.fsPath, result.root) === closedKey) results.delete(owner);
          }
        }
      }
      results.delete(d.uri.toString());
      publishDiagnostics();
      for (const document of vscode.workspace.textDocuments) {
        if (!document.isClosed && isNagi(document) && refresh.has(document.uri.toString())) check(document);
      }
    }),
    vscode.workspace.onDidGrantWorkspaceTrust(() => { for (const d of vscode.workspace.textDocuments) if (optionsForAuto(d)) check(d); }),
    vscode.workspace.onDidChangeConfiguration(e => { if (e.affectsConfiguration('nagi')) { invalidate(); recheckOpen(); } }),
    vscode.tasks.onDidEndTask(event => {
      const snapshot = runningTasks.get(event.execution);
      if (snapshot) { runningTasks.delete(event.execution); releaseTaskScope(snapshot); }
      else if (event.execution.task.definition.type === 'nagi') endedTasks.add(event.execution);
    }),
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
