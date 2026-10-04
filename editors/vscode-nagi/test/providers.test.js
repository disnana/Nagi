'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const vm = require('node:vm');
const realCompiler = require('../src/compiler');

// Exercise the registered providers, including compiler/trust guards, without
// starting an editor or compiling a native application.
function host(t, trusted = true, code = 1, symbols, controls = {}) {
  const folder = fs.mkdtempSync(path.join(os.tmpdir(), 'nagi-provider-'));
  t.after(() => fs.rmSync(folder, { recursive: true, force: true }));
  const disposable = () => ({ dispose() {} });
  const providers = {}, selectors = {}, events = {}, commands = new Map(), diagnostics = new Map();
  const calls = [], tasks = [], taskDiagnostics = new Map();
  const uri = (file, scheme = 'file') => ({ scheme, fsPath: file, toString: () => `${scheme}:${file}` });
  class Position { constructor(line, character) { this.line = line; this.character = character; } }
  class Range {
    constructor(a, b, c, d) { this.start = typeof a === 'number' ? new Position(a, b) : a; this.end = typeof a === 'number' ? new Position(c, d) : b; }
    isEqual(other) { return this.start.line === other.start.line && this.start.character === other.start.character && this.end.line === other.end.line && this.end.character === other.end.character; }
  }
  class MarkdownString {
    constructor() { this.value = ''; }
    appendCodeblock(text, language) { this.value += text + '\n'; this.language = language; }
    appendText(text) { this.value += text; }
  }
  function collection(owner) {
    const items = owner === 'nagi-task' ? taskDiagnostics : diagnostics;
    return { clear() { items.clear(); }, set(resource, value) { items.set(resource.toString(), value); },
      delete(resource) { items.delete(resource.toString()); }, dispose() {} };
  }
  const vscode = {
    workspace: {
      isTrusted: trusted, textDocuments: [],
      getWorkspaceFolder: () => ({ uri: uri(folder) }),
      getConfiguration: () => ({ get: (name, fallback) => name === 'checkOnSave' ? controls.checkOnSave ?? fallback : fallback }),
      onDidOpenTextDocument(fn) { events.open = fn; return disposable(); }, onDidSaveTextDocument(fn) { events.save = fn; return disposable(); },
      onDidChangeTextDocument(fn) { events.change = fn; return disposable(); }, onDidCloseTextDocument(fn) { events.close = fn; return disposable(); },
      onDidGrantWorkspaceTrust: disposable, onDidChangeConfiguration: disposable,
      registerTextDocumentContentProvider(scheme, provider) { providers[scheme] = provider; return disposable(); },
      createFileSystemWatcher: () => ({ dispose() {}, onDidCreate: disposable, onDidChange: disposable, onDidDelete: disposable }),
    },
    window: { createOutputChannel: () => ({ appendLine() {}, show() {}, dispose() {} }),
      showWarningMessage: async () => undefined, setStatusBarMessage() {} },
    languages: { createDiagnosticCollection: collection, getDiagnostics: () => [...diagnostics, ...taskDiagnostics].map(([resource, items]) => [uri(resource.slice(5)), items]), registerOnTypeFormattingEditProvider: disposable },
    commands: { registerCommand(name, fn) { commands.set(name, fn); return disposable(); } },
    tasks: { async executeTask(task) { if (controls.failTask) throw new Error('task launch failed'); tasks.push(task); return { task }; },
      onDidEndTask(fn) { events.taskEnd = fn; return disposable(); } },
    Task: class { constructor(definition, scope, name, source, execution) { Object.assign(this, { definition, scope, name, source, execution }); } },
    ProcessExecution: class { constructor(executable, args, options) { Object.assign(this, { executable, args, options }); } },
    TaskScope: { Workspace: 1 }, TaskRevealKind: { Always: 1 }, TaskPanelKind: { Dedicated: 2 },
    Range, MarkdownString, Uri: { file: file => uri(file), parse: text => {
      const [scheme, pathname] = text.split(':');
      return { ...uri(pathname, scheme), path: pathname, authority: '', query: '', fragment: '' };
    } },
    FoldingRange: class { constructor(start, end) { Object.assign(this, { start, end }); } },
    Location: class { constructor(uri, range) { this.uri = uri; this.range = range; } },
    EventEmitter: class { constructor() { this.event = () => disposable(); } fire() {} dispose() {} },
    DiagnosticSeverity: { Error: 0 }, CompletionItemKind: { Function: 1, Class: 2, Field: 3, TypeParameter: 4, Keyword: 5, Enum: 6, EnumMember: 7, Module: 8, Constant: 9, Variable: 10 },
    Diagnostic: class { constructor(range, message) { this.range = range; this.message = message; } },
    Hover: class { constructor(contents, range) { this.contents = contents; this.range = range; } },
    CompletionItem: class { constructor(label, kind) { this.label = label; this.kind = kind; } },
    SnippetString: class { constructor(value) { this.value = value; } },
    SignatureInformation: class { constructor(label, documentation) { this.label = label; this.documentation = documentation; } },
    ParameterInformation: class { constructor(label) { this.label = label; } }, SignatureHelp: class {},
  };
  for (const kind of ['Definition', 'Hover', 'CompletionItem', 'SignatureHelp', 'FoldingRange']) {
    vscode.languages[`register${kind}Provider`] = (selector, provider) => {
      providers[kind] = provider; selectors[kind] = selector; return disposable();
    };
  }
  const compiler = { ...realCompiler, runCheck(executable, args, cwd, timeout, callback, maxBuffer, input) {
    assert.equal(vscode.workspace.isTrusted, true, 'compiler must never execute in an untrusted workspace');
    const call = { executable, args, cwd, callback, input, killed: false };
    calls.push(call);
    if (controls.defer) return { kill() { call.killed = true; } };
    queueMicrotask(() => callback(symbols && args[0] === 'symbols'
      ? { error: null, output: JSON.stringify(symbols(cwd)) }
      : { error: Object.assign(new Error('checker failed'), { code }), output: controls.checkOutput?.(cwd) || 'error: line 2: expected i32\n' }));
    return { kill() {} };
  } };
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../src/extension.js'), 'utf8'), {
    module, Buffer, require(name) { if (name === 'vscode') return vscode; if (name === './compiler') return compiler; return require(name.startsWith('./') ? path.join(__dirname, '../src', name) : name); },
  }, { filename: 'extension.js' });
  const context = { subscriptions: [] };
  module.exports.activate(context);
  t.after(() => context.subscriptions.forEach(item => item.dispose()));
  function document(text, scheme = 'file', languageId = 'nagi', filename) {
    const file = path.join(folder, filename || (languageId === 'nagi-low' ? 'main.low' : 'main.nagi'));
    if (scheme === 'file') fs.writeFileSync(file, text);
    const doc = { uri: uri(file, scheme), languageId, version: 1, isDirty: false, isClosed: false,
      getText: () => text, save: async () => true,
      positionAt(offset) { const before = text.slice(0, offset).split('\n'); return new Position(before.length - 1, before.at(-1).length); },
      offsetAt(position) { const lines = text.split('\n'); return lines.slice(0, position.line).reduce((sum, line) => sum + line.length + 1, 0) + position.character; },
    };
    vscode.window.activeTextEditor = { document: doc };
    vscode.workspace.textDocuments.push(doc);
    return doc;
  }
  const token = { isCancellationRequested: false, onCancellationRequested: disposable };
  return { providers, selectors, events, commands, diagnostics, taskDiagnostics, calls, tasks, document, token, vscode, folder };
}

test('unsaved canonical import diagnostics use the open alias and UTF-16 source range', async t => {
  const h = host(t, true, 1, undefined, { defer: true, checkOnSave: false });
  const main = h.document('import "alias.nagi"\ndef main():\n    print(1)\n');
  const actual = path.join(h.folder, 'actual.nagi');
  fs.writeFileSync(actual, 'def helper():\n    print(1)\n');
  const dirty = '# 日本語 😀\n\n\n\n\ndef helper():\n    print("😀"); missing()\n';
  const dep = h.document(dirty, 'file', 'nagi', 'alias.nagi');
  fs.unlinkSync(dep.uri.fsPath); fs.symlinkSync(actual, dep.uri.fsPath, 'file'); dep.isDirty = true;
  h.vscode.window.activeTextEditor = { document: main };
  const pending = h.commands.get('nagi.check')();
  h.calls[0].callback({ error: Object.assign(new Error('source error'), { code: 1 }), output: `error: line 7: unknown function\n --> ${actual}:7\n` });
  await pending;
  const [error] = h.diagnostics.get(dep.uri.toString());
  assert.equal(error.range.start.line, 6);
  assert.equal(error.range.end.character, dirty.split('\n')[6].length, 'range uses UTF-16 characters from the overlay');
  assert.equal(h.diagnostics.has(`file:${actual}`), false, 'Problems does not open a second saved copy');
});

test('closing a checked import overlay refreshes its owners and keeps unrelated errors', async t => {
  const h = host(t, true, 1, undefined, { defer: true, checkOnSave: false });
  const main = h.document('def main():\n    print(1)\n');
  const dep = h.document('def helper():\n    missing()\n', 'file', 'nagi', 'helper.nagi'); dep.isDirty = true;
  const other = h.document('def other():\n    missing()\n', 'file', 'nagi', 'other.nagi');
  h.vscode.window.activeTextEditor = { document: main };
  const first = h.commands.get('nagi.check')();
  h.calls[0].callback({ error: Object.assign(new Error('source error'), { code: 1 }), output: `error: line 2: imported error\n --> ${dep.uri.fsPath}:2\n` }); await first;
  h.vscode.window.activeTextEditor = { document: other };
  const second = h.commands.get('nagi.check')();
  const otherFailure = { error: Object.assign(new Error('source error'), { code: 1 }), output: `error: line 2: separate error\n --> ${other.uri.fsPath}:2\n` };
  h.calls[1].callback(otherFailure); await second;
  dep.isClosed = true; h.vscode.workspace.textDocuments = h.vscode.workspace.textDocuments.filter(d => d !== dep); h.events.close(dep);
  assert.equal(h.diagnostics.has(dep.uri.toString()), false, 'discarded overlay diagnostic is removed');
  assert.equal(h.diagnostics.get(other.uri.toString())[0].message, 'line 2: separate error', 'separate marker remains during owner refresh');
  const refreshedMain = h.calls.slice(2).find(c => c.args.includes(main.uri.fsPath));
  const refreshedOther = h.calls.slice(2).find(c => c.args.includes(other.uri.fsPath));
  assert.equal(refreshedMain.input, undefined, 'closed overlay is not sent again');
  refreshedMain.callback({ error: null, output: 'check OK' }); refreshedOther.callback(otherFailure);
  await Promise.resolve();
  assert.equal(h.diagnostics.has(dep.uri.toString()), false);
  assert.equal(h.diagnostics.get(other.uri.toString())[0].message, 'line 2: separate error');
});

test('closing an in-flight import overlay rejects its stale result', async t => {
  const h = host(t, true, 1, undefined, { defer: true, checkOnSave: false });
  const main = h.document('def main():\n    print(1)\n');
  const dep = h.document('def helper():\n    missing()\n', 'file', 'nagi', 'helper.nagi'); dep.isDirty = true;
  h.vscode.window.activeTextEditor = { document: main };
  const first = h.commands.get('nagi.check')();
  const old = h.calls[0];
  dep.isClosed = true; h.vscode.workspace.textDocuments = h.vscode.workspace.textDocuments.filter(d => d !== dep); h.events.close(dep);
  assert.equal(old.killed, true);
  old.callback({ error: Object.assign(new Error('source error'), { code: 1 }), output: `error: line 2: stale error\n --> ${dep.uri.fsPath}:2\n` }); await first;
  assert.equal(h.diagnostics.size, 0);
  assert.equal(h.calls.length, 2, 'owner check restarts without the discarded import');
  h.calls[1].callback({ error: null, output: 'check OK' });
  await Promise.resolve(); assert.equal(h.diagnostics.size, 0);
});

test('lower build and run tasks use the registered source diagnostic matcher', async t => {
  const h = host(t, true, 1, undefined, { checkOnSave: false });
  h.document('def main():\n    print(1)\n');
  for (const name of ['lower', 'build', 'run']) await h.commands.get(`nagi.${name}`)();
  assert.deepEqual(h.tasks.map(task => Array.from(task.problemMatchers)), [['$nagi'], ['$nagi'], ['$nagi']]);
});

test('tasks suppress only the matching automatic scope and checks resume when the task ends', async t => {
  const h = host(t, true, 1, undefined, { defer: true, checkOnSave: true });
  const main = h.document('def main():\n    missing()\n');
  const other = h.document('def other():\n    missing()\n', 'file', 'nagi', 'other.nagi');
  const fail = file => ({ error: Object.assign(new Error('source error'), { code: 1 }), output: `error: line 2: unknown function\n --> ${file}:2\n` });
  h.vscode.window.activeTextEditor = { document: main }; const first = h.commands.get('nagi.check')(); h.calls[0].callback(fail(main.uri.fsPath)); await first;
  h.vscode.window.activeTextEditor = { document: other }; const second = h.commands.get('nagi.check')(); h.calls[1].callback(fail(other.uri.fsPath)); await second;
  h.vscode.window.activeTextEditor = { document: main };
  const saveTriggered = h.commands.get('nagi.check')(); const stale = h.calls[2];
  const execution = await h.commands.get('nagi.build')();
  assert.equal(stale.killed, true, 'save-triggered pending check is cancelled');
  stale.callback(fail(main.uri.fsPath)); await saveTriggered;
  assert.equal(h.diagnostics.has(main.uri.toString()), false);
  assert.equal(h.diagnostics.get(other.uri.toString()).length, 1, 'unrelated source diagnostic stays visible');
  h.events.open(main); assert.equal(h.calls.length, 3, 'matching automatic check is suppressed during task execution');
  h.events.taskEnd({ execution });
  const resumed = h.commands.get('nagi.check')(); assert.equal(h.calls.length, 4);
  h.calls[3].callback({ error: null, output: 'check OK' }); await resumed;
  assert.equal(h.diagnostics.get(other.uri.toString()).length, 1);
});

test('task launch failure releases live check suppression', async t => {
  const h = host(t, true, 1, undefined, { defer: true, checkOnSave: false, failTask: true });
  h.document('def main():\n    print(1)\n');
  await assert.rejects(h.commands.get('nagi.build')(), /task launch failed/);
  const resumed = h.commands.get('nagi.check')(); assert.equal(h.calls.length, 1);
  h.calls[0].callback({ error: null, output: 'check OK' }); await resumed;
});

test('project task clears checks from its helper while preserving another project', async t => {
  const h = host(t, true, 1, undefined, { defer: true, checkOnSave: true });
  for (const name of ['app', 'other']) { fs.mkdirSync(path.join(h.folder, name)); fs.writeFileSync(path.join(h.folder, name, 'nagi.toml'), "entry = 'main.nagi'\n"); }
  const entry = h.document('def main():\n    print(1)\n', 'file', 'nagi', 'app/main.nagi');
  const helper = h.document('def helper():\n    missing()\n', 'file', 'nagi', 'app/helper.nagi');
  const other = h.document('def main():\n    missing()\n', 'file', 'nagi', 'other/main.nagi');
  const fail = file => ({ error: Object.assign(new Error('source error'), { code: 1 }), output: `error: line 2: unknown function\n --> ${file}:2\n` });
  h.vscode.window.activeTextEditor = { document: helper }; const first = h.commands.get('nagi.check')(); h.calls[0].callback(fail(helper.uri.fsPath)); await first;
  h.vscode.window.activeTextEditor = { document: other }; const second = h.commands.get('nagi.check')(); h.calls[1].callback(fail(other.uri.fsPath)); await second;
  h.vscode.window.activeTextEditor = { document: entry }; const execution = await h.commands.get('nagi.lower')();
  assert.equal(h.diagnostics.has(helper.uri.toString()), false);
  assert.equal(h.diagnostics.get(other.uri.toString()).length, 1);
  h.vscode.window.activeTextEditor = { document: helper }; h.events.open(helper); assert.equal(h.calls.length, 2);
  h.events.taskEnd({ execution });
  const resumed = h.commands.get('nagi.check')(); assert.equal(h.calls.length, 3);
  h.calls[2].callback(fail(helper.uri.fsPath)); await resumed;
  assert.equal(h.diagnostics.get(helper.uri.toString()).length, 1);
});

test('a long task permits checks of new source without reacting to other project edits', async t => {
  const h = host(t, true, 1, undefined, { defer: true, checkOnSave: true });
  for (const name of ['app', 'other']) { fs.mkdirSync(path.join(h.folder, name)); fs.writeFileSync(path.join(h.folder, name, 'nagi.toml'), "entry = 'main.nagi'\n"); }
  const main = h.document('def main():\n    print(1)\n', 'file', 'nagi', 'app/main.nagi');
  const other = h.document('def main():\n    print(2)\n', 'file', 'nagi', 'other/main.nagi');
  h.vscode.window.activeTextEditor = { document: main }; const execution = await h.commands.get('nagi.run')();
  other.version++; other.isDirty = true; h.events.change({ document: other });
  h.events.open(main); assert.equal(h.calls.length, 0, 'another project cannot lift same-source automatic suppression');
  main.version++; main.isDirty = true; h.events.change({ document: main });
  const edited = h.commands.get('nagi.check')(); assert.equal(h.calls.length, 1, 'new source can be checked while the app keeps running');
  h.calls[0].callback({ error: null, output: 'check OK' }); await edited;
  h.events.taskEnd({ execution });
});

test('failed task diagnostics are replaced by checks and removed by source edits', async t => {
  const h = host(t, true, 1, undefined, { defer: true, checkOnSave: true });
  const main = h.document('def main():\n    missing()\n');
  const other = h.document('def other():\n    missing()\n', 'file', 'nagi', 'other.nagi');
  const execution = await h.commands.get('nagi.build')();
  h.events.taskEnd({ execution });
  const marker = new h.vscode.Diagnostic(new h.vscode.Range(1, 0, 1, 1), 'line 2: unknown function');
  marker.source = 'nagic';
  h.taskDiagnostics.set(main.uri.toString(), [marker]);
  h.taskDiagnostics.set(other.uri.toString(), [marker]);
  const count = doc => (h.taskDiagnostics.get(doc.uri.toString()) || []).length + (h.diagnostics.get(doc.uri.toString()) || []).length;
  assert.equal(count(main), 1);
  h.vscode.window.activeTextEditor = { document: main };
  const checked = h.commands.get('nagi.check')();
  h.calls[0].callback({ error: Object.assign(new Error('source error'), { code: 1 }), output: `error: line 2: unknown function\n --> ${main.uri.fsPath}:2\n` }); await checked;
  assert.equal(count(main), 1, 'live check replaces the previous task error');
  assert.equal(count(other), 1, 'another file keeps its task error');
  // A successful frontend check does not invalidate a backend-only task error.
  h.taskDiagnostics.set(main.uri.toString(), [marker]);
  const success = h.commands.get('nagi.check')(); h.calls[1].callback({ error: null, output: 'check OK' }); await success;
  assert.equal(count(main), 1);
  main.version++; main.isDirty = true; h.events.change({ document: main });
  h.events.save(main); h.calls[2].callback({ error: null, output: 'check OK' }); await Promise.resolve();
  assert.equal(count(main), 0, 'editing and checking the fixed source clears the old task marker');
  assert.equal(count(other), 1);
});

test('manual checks inspect dirty imports outside the running task project', async t => {
  const h = host(t, true, 1, undefined, { defer: true, checkOnSave: false });
  for (const name of ['app', 'shared']) fs.mkdirSync(path.join(h.folder, name));
  fs.writeFileSync(path.join(h.folder, 'app', 'nagi.toml'), "entry = 'main.nagi'\n");
  const main = h.document('import "../shared/helper.nagi" as shared\ndef main():\n    print(shared.answer())\n', 'file', 'nagi', 'app/main.nagi');
  const helper = h.document('def answer() -> i32:\n    return "wrong"\n', 'file', 'nagi', 'shared/helper.nagi');
  h.vscode.window.activeTextEditor = { document: main };
  const execution = await h.commands.get('nagi.run')();
  helper.version++; helper.isDirty = true; h.events.change({ document: helper });
  const checked = h.commands.get('nagi.check')();
  assert.equal(h.calls.length, 1, 'explicit checks always run while the task remains active');
  assert.equal(JSON.parse(h.calls[0].input).files[0].file, helper.uri.fsPath);
  h.calls[0].callback({ error: Object.assign(new Error('source error'), { code: 1 }), output: `error: line 2: expected i32\n --> ${helper.uri.fsPath}:2\n` }); await checked;
  assert.equal(h.diagnostics.get(helper.uri.toString())[0].range.start.line, 1);
  assert.equal(h.tasks.length, 1);
  h.events.taskEnd({ execution });
});

test('editing and closing imported aliases remove only their canonical task markers', t => {
  const h = host(t, true, 1, undefined, { checkOnSave: false });
  const actual = h.document('def helper():\n    print(1)\n', 'file', 'nagi', 'actual.nagi');
  const alias = h.document(actual.getText(), 'file', 'nagi', 'alias.nagi');
  fs.unlinkSync(alias.uri.fsPath); fs.symlinkSync(actual.uri.fsPath, alias.uri.fsPath, 'file');
  const other = h.document('def other():\n    print(1)\n', 'file', 'nagi', 'other.nagi');
  const marker = new h.vscode.Diagnostic(new h.vscode.Range(1, 0, 1, 1), 'backend error');
  marker.source = 'nagic';
  h.taskDiagnostics.set(actual.uri.toString(), [marker]); h.taskDiagnostics.set(other.uri.toString(), [marker]);
  alias.version++; h.events.change({ document: alias });
  assert.equal(h.taskDiagnostics.has(actual.uri.toString()), false);
  assert.equal(h.taskDiagnostics.get(other.uri.toString()).length, 1);
  h.taskDiagnostics.set(actual.uri.toString(), [marker]); alias.isClosed = true; h.events.close(alias);
  assert.equal(h.taskDiagnostics.has(actual.uri.toString()), false);
  assert.equal(h.taskDiagnostics.get(other.uri.toString()).length, 1);
});

test('typing invalidates snapshots without launching save-only checks per keystroke', async t => {
  const h = host(t);
  const doc = h.document('def main():\n    print(7)\n');
  for (let i = 0; i < 20; i++) {
    doc.version++; doc.isDirty = true; h.events.change({ document: doc });
  }
  await Promise.resolve();
  assert.equal(h.calls.length, 0);
  h.events.save(doc);
  await Promise.resolve();
  assert.equal(h.calls.length, 1);
  assert.equal(h.calls[0].args[0], 'check');
});

function separateProjects(h) {
  for (const name of ['app', 'other']) {
    fs.mkdirSync(path.join(h.folder, name));
    fs.writeFileSync(path.join(h.folder, name, 'nagi.toml'), "entry = 'main.nagi'\n");
  }
  return ['app', 'other'].map(name => h.document('def main():\n    print(1)\n', 'file', 'nagi', `${name}/main.nagi`));
}
const fileIndex = cwd => ({ format: 'nagi-symbols-v1', definitions: [], files: [path.join(cwd, 'main.nagi')] });
const projectError = cwd => `error: line 2: expected i32\n --> ${path.join(cwd, 'main.nagi')}:2\n`;
async function loadDependencyIndex(h, document) {
  return h.providers.Hover.provideHover(document, document.positionAt(document.getText().indexOf('print') + 2), h.token);
}

test('known unrelated diagnostics survive repeated edits while unknown and project inputs stay conservative', async t => {
  const h = host(t, true, 1, fileIndex, { checkOnSave: false, checkOutput: projectError });
  const [app, other] = separateProjects(h);
  h.vscode.window.activeTextEditor = { document: other };
  await h.commands.get('nagi.check')();
  app.version++; app.isDirty = true; h.events.change({ document: app });
  assert.equal(h.diagnostics.has(other.uri.toString()), false, 'unknown dependency closures remain conservative');
  app.isDirty = false;
  await h.commands.get('nagi.check')();
  await loadDependencyIndex(h, other);
  const calls = h.calls.length;
  for (let i = 0; i < 3; i++) {
    app.version++; app.isDirty = true; h.events.change({ document: app });
    assert.equal(h.diagnostics.get(other.uri.toString()).length, 1, 'a known independent project keeps its error');
  }
  assert.equal(h.calls.length, calls, 'dependency preservation starts no additional compiler processes');
  const manifest = h.document("entry = 'main.nagi'\n", 'file', 'toml', 'app/nagi.toml');
  manifest.version++; manifest.isDirty = true; h.events.change({ document: manifest });
  assert.equal(h.diagnostics.size, 0, 'manifest edits invalidate all dependency proofs');
});

test('known unrelated pending checks finish during edits and same-scope changes reject stale callbacks', async t => {
  const h = host(t, true, 1, fileIndex, { checkOnSave: false, defer: true });
  const [app, other] = separateProjects(h);
  const hover = loadDependencyIndex(h, other);
  h.calls[0].callback({ error: null, output: JSON.stringify(fileIndex(path.dirname(other.uri.fsPath))) });
  await hover;
  h.vscode.window.activeTextEditor = { document: other };
  const pending = h.commands.get('nagi.check')(), call = h.calls[1];
  for (let i = 0; i < 3; i++) { app.version++; app.isDirty = true; h.events.change({ document: app }); }
  assert.equal(call.killed, false, 'typing in another project cannot starve a known independent check');
  call.callback({ error: Object.assign(new Error('source error'), { code: 1 }), output: `error: line 2: other error\n --> ${other.uri.fsPath}:2\n` });
  await pending;
  assert.equal(h.diagnostics.get(other.uri.toString())[0].message, 'line 2: other error');
  const next = h.commands.get('nagi.check')(), stale = h.calls[2];
  other.version++; other.isDirty = true; h.events.change({ document: other });
  assert.equal(stale.killed, true);
  stale.callback({ error: Object.assign(new Error('source error'), { code: 1 }), output: `error: line 2: stale error\n --> ${other.uri.fsPath}:2\n` });
  await next;
  assert.equal(h.diagnostics.size, 0, 'the edited source cannot receive the old result');
});

test('shared imports and stale dependency snapshots still invalidate diagnosed projects', async t => {
  let shared;
  const h = host(t, true, 1, cwd => ({ ...fileIndex(cwd), files: [path.join(cwd, 'main.nagi'), shared.uri.fsPath] }), { checkOnSave: false, checkOutput: projectError });
  const [app, other] = separateProjects(h);
  shared = h.document('def helper():\n    print(1)\n', 'file', 'nagi', 'shared.nagi');
  h.vscode.window.activeTextEditor = { document: other };
  await h.commands.get('nagi.check')(); await loadDependencyIndex(h, other);
  shared.version++; shared.isDirty = true; h.events.change({ document: shared });
  assert.equal(h.diagnostics.size, 0, 'a cross-project dependency is not independent');
  shared.isDirty = false;
  await h.commands.get('nagi.check')(); await loadDependencyIndex(h, other);
  fs.appendFileSync(shared.uri.fsPath, '# changed on disk\n');
  app.version++; app.isDirty = true; h.events.change({ document: app });
  assert.equal(h.diagnostics.size, 0, 'stale filesystem stamps cannot prove independence');
});

test('failed symbols queries still supply static builtin help without erasing source diagnostics', async t => {
  const h = host(t);
  const doc = h.document('def main():\n    print(');
  await h.commands.get('nagi.check')();
  const errors = h.diagnostics.get(doc.uri.toString());
  assert.equal(errors.length, 1);
  const hover = await h.providers.Hover.provideHover(doc, doc.positionAt(doc.getText().indexOf('print') + 2), h.token);
  assert.match(hover.contents.value, /def print\(value\) -> unit/);
  assert.match(hover.contents.value, /組み込み関数/);
  const signature = await h.providers.SignatureHelp.provideSignatureHelp(doc, doc.positionAt(doc.getText().length), h.token);
  assert.match(signature.signatures[0].label, /def print/);
  assert.equal(h.diagnostics.get(doc.uri.toString()), errors, 'assistance does not publish or clear diagnostics');
  assert.equal((await h.providers.Definition.provideDefinition(doc, doc.positionAt(0), h.token)).length, 0);
});

test('missing compiler keeps prefix completion, hover and signatures available', async t => {
  const h = host(t, true, 'ENOENT');
  const doc = h.document('print(');
  assert.ok(await h.providers.Hover.provideHover(doc, doc.positionAt(2), h.token));
  assert.ok(await h.providers.SignatureHelp.provideSignatureHelp(doc, doc.positionAt(6), h.token));
  const prefix = h.document('pri');
  const items = await h.providers.CompletionItem.provideCompletionItems(prefix, prefix.positionAt(3), h.token);
  assert.ok(items.some(x => x.label === 'print'));
  assert.ok(items.some(x => x.label === 'scope'));
  assert.equal(h.diagnostics.size, 0);
});

test('borrowed local providers annotate hover and detail while keeping variable kind and insertion', async t => {
  const text = 'print(item)';
  const h = host(t, true, 1, folder => ({ format: 'nagi-symbols-v1', definitions: [], bindings: [], references: [], files: [], expressions: [],
    locals: ['main.nagi', 'main.low'].map(name => ({ name: 'item', type: 'Entry', borrowed: true, readonly: true,
      location: { file: path.join(folder, name), line: 1, column: 7, length: 4 } })),
  }));
  for (const language of ['nagi', 'nagi-low']) {
    const doc = h.document(text, 'file', language);
    const hover = await h.providers.Hover.provideHover(doc, doc.positionAt(8), h.token);
    const expected = `item: Entry (read-only borrow)\n\nmain.${language === 'nagi-low' ? 'low' : 'nagi'}:1`;
    assert.equal(hover.contents.value, expected);
    assert.equal(hover.contents.language, language);
    const items = await h.providers.CompletionItem.provideCompletionItems(doc, doc.positionAt(8), h.token);
    const item = items.find(item => item.label === 'item');
    assert.equal(item.detail, 'item: Entry (read-only borrow)');
    assert.equal(item.documentation.value, expected);
    assert.equal(item.kind, h.vscode.CompletionItemKind.Variable);
    assert.equal(item.insertText.value, 'item');
  }
});

test('untrusted and untitled assistance executes no compiler and retains High/Low selectors', async t => {
  for (const trusted of [true, false]) {
    const h = host(t, trusted);
    for (const languageId of ['nagi', 'nagi-low']) {
      const doc = h.document('print(', 'untitled', languageId);
      assert.ok(await h.providers.Hover.provideHover(doc, doc.positionAt(2), h.token));
      assert.ok(await h.providers.SignatureHelp.provideSignatureHelp(doc, doc.positionAt(6), h.token));
      const items = await h.providers.CompletionItem.provideCompletionItems(doc, doc.positionAt(6), h.token);
      assert.ok(items.some(x => x.label === (languageId === 'nagi' ? 'class' : 'record')));
      assert.ok(items.some(x => x.label === 'spawn'));
      for (const kind of ['Hover', 'CompletionItem', 'SignatureHelp']) assert.ok(h.selectors[kind].some(x => x.language === languageId && x.scheme === 'untitled'));
    }
    if (!trusted) {
      const doc = h.document('print(');
      assert.ok(await h.providers.Hover.provideHover(doc, doc.positionAt(2), h.token));
      await h.commands.get('nagi.check')();
    }
    assert.equal(h.calls.length, 0);
  }
});

test('Low builtin help uses fn and record in hover, completion and signatures without a compiler', async t => {
  const h = host(t, false);
  for (const language of ['nagi', 'nagi-low']) {
    const doc = h.document('fail(', 'untitled', language);
    const keyword = language === 'nagi-low' ? 'fn' : 'def';
    const typeKeyword = language === 'nagi-low' ? 'record' : 'class';
    const hover = await h.providers.Hover.provideHover(doc, doc.positionAt(2), h.token);
    assert.ok(hover.contents.value.startsWith(`${keyword} fail(problem: E) -> Result[T, E]`));
    assert.ok(hover.contents.value.includes(`独自${typeKeyword}・enum`));
    assert.equal(hover.contents.language, language);
    const help = await h.providers.SignatureHelp.provideSignatureHelp(doc, doc.positionAt(5), h.token);
    assert.ok(help.signatures[0].label.startsWith(`${keyword} fail`));
    const items = await h.providers.CompletionItem.provideCompletionItems(doc, doc.positionAt(5), h.token);
    const fail = items.find(item => item.label === 'fail');
    assert.ok(fail.detail.startsWith(`${keyword} fail`));
    assert.equal(fail.insertText.value, 'fail(${1:problem})');
  }
  assert.equal(h.calls.length, 0);
  assert.equal(h.diagnostics.size, 0);
});

test('Low declaration and standard-operation help displays braces while retaining compiler kinds and call shapes', async t => {
  const h = host(t, true, 1, folder => {
    const item = { name: 'Item', kind: 'class', signature: 'class Item\n    value: i64', fields: [{ name: 'value', type: 'i64' }] };
    const fault = { name: 'Fault', kind: 'enum', signature: 'enum Fault\n    Fault.Missing\n    Fault.Invalid(message: str)', variants: [] };
    const make = { name: 'make', kind: 'function', signature: 'async def make(value: i64) -> Result[Item, Error]', parameters: [{ name: 'value', type: 'i64' }], return_type: 'Result[Item, Error]', asynchronous: true };
    return { format: 'nagi-symbols-v1', definitions: [item, fault, make], bindings: [{ file: path.join(folder, 'main.low'), name: 'models', kind: 'module', members: [item, fault, make] }], references: [], files: [], locals: [], expressions: [] };
  });
  const doc = h.document('models.make(', 'file', 'nagi-low');
  const hover = await h.providers.Hover.provideHover(doc, doc.positionAt(9), h.token);
  assert.match(hover.contents.value, /^async fn make\(value: i64\)/);
  const help = await h.providers.SignatureHelp.provideSignatureHelp(doc, doc.positionAt(12), h.token);
  assert.match(help.signatures[0].label, /^async fn make/);
  assert.equal(help.signatures[0].parameters[0].label, 'value: i64');
  const prefix = h.document('models.', 'file', 'nagi-low');
  const items = await h.providers.CompletionItem.provideCompletionItems(prefix, prefix.positionAt(7), h.token);
  const item = items.find(item => item.label === 'Item');
  assert.equal(item.detail, 'record Item {\n    value: i64;\n}');
  assert.equal(item.kind, h.vscode.CompletionItemKind.Class);
  assert.equal(item.insertText.value, 'Item(value=${1:value})');
  assert.equal(items.find(item => item.label === 'Fault').detail, 'enum Fault {\n    Missing;\n    Invalid(message: str);\n}');
  assert.ok(items.find(item => item.label === 'make').detail.startsWith('async fn make'));
});

test('Low enum declaration help removes namespace prefixes only from variant names', async t => {
  const raw = 'enum models.Fault\n    models.Fault.Missing\n    models.Fault.Invalid(problem: models.Error)';
  const expected = 'enum models.Fault {\n    Missing;\n    Invalid(problem: models.Error);\n}';
  const h = host(t, true, 1, folder => {
    const fault = { name: 'Fault', kind: 'enum', signature: raw, variants: [
      { name: 'Missing', kind: 'enum_member', signature: 'models.Fault.Missing', parameters: [] },
      { name: 'Invalid', kind: 'enum_member', signature: 'models.Fault.Invalid(problem: models.Error)', parameters: [{ name: 'problem', type: 'models.Error' }] },
    ] };
    return { format: 'nagi-symbols-v1', definitions: [fault], bindings: ['main.low', 'main.nagi'].map(name => ({
      file: path.join(folder, name), name: 'models', kind: 'module', members: [fault],
    })), references: [], files: [], locals: [], expressions: [] };
  });
  for (const language of ['nagi-low', 'nagi']) {
    const doc = h.document('models.Fault', 'file', language);
    const hover = await h.providers.Hover.provideHover(doc, doc.positionAt(9), h.token);
    assert.ok(hover.contents.value.startsWith(language === 'nagi-low' ? expected : raw));
    const prefix = h.document('models.', 'file', language);
    const items = await h.providers.CompletionItem.provideCompletionItems(prefix, prefix.positionAt(7), h.token);
    const item = items.find(item => item.label === 'Fault');
    assert.equal(item.detail, language === 'nagi-low' ? expected : raw);
    assert.equal(item.kind, h.vscode.CompletionItemKind.Enum);
    const call = h.document('models.Fault.Invalid(', 'file', language);
    const help = await h.providers.SignatureHelp.provideSignatureHelp(call, call.positionAt(call.getText().length), h.token);
    assert.equal(help.signatures[0].label, 'models.Fault.Invalid(problem: models.Error)');
    assert.equal(help.signatures[0].parameters[0].label, 'problem: models.Error');
  }
});

test('Low folding is available without trust or a compiler and honors cancellation', t => {
  const h = host(t, false);
  const doc = h.document('fn main() {\nprint(42);\n}', 'untitled', 'nagi-low');
  assert.deepEqual(Array.from(h.selectors.FoldingRange, item => item.language), ['nagi-low']);
  assert.deepEqual(Array.from(h.providers.FoldingRange.provideFoldingRanges(doc, {}, h.token), item => [item.start, item.end]), [[0, 1]]);
  h.token.isCancellationRequested = true;
  assert.equal(h.providers.FoldingRange.provideFoldingRanges(doc, {}, h.token).length, 0);
  assert.equal(h.calls.length, 0);
});

test('cancelled, closed and changed documents cannot receive static fallback from failed queries', async t => {
  const h = host(t);
  for (const mutate of [doc => { doc.version++; }, doc => { doc.isClosed = true; }, () => { h.token.isCancellationRequested = true; }]) {
    h.token.isCancellationRequested = false;
    const doc = h.document('print(');
    const pending = h.providers.Hover.provideHover(doc, doc.positionAt(2), h.token);
    mutate(doc);
    assert.equal(await pending, undefined);
  }
});

test('registered providers render enum/member kinds and payload signature parameters', async t => {
  const h = host(t, true, 1, folder => {
    const enumeration = { name: 'AuthError', kind: 'enum', signature: 'enum AuthError\n    Missing\n    Invalid(message: str)', variants: [
      { name: 'Missing', kind: 'enum_member', signature: 'AuthError.Missing', parameters: [], return_type: 'AuthError' },
      { name: 'Invalid', kind: 'enum_member', signature: 'AuthError.Invalid(message: str)', parameters: [{ name: 'message', type: 'str' }], return_type: 'AuthError' },
    ] };
    return { format: 'nagi-symbols-v1', definitions: [enumeration], bindings: [{ file: path.join(folder, 'main.nagi'), name: 'AuthError', kind: 'enum', definition: enumeration }], references: [], files: [], locals: [], expressions: [] };
  });
  const type = h.document('value: AuthError');
  const types = await h.providers.CompletionItem.provideCompletionItems(type, type.positionAt(type.getText().length), h.token);
  const enumeration = types.find(item => item.label === 'AuthError');
  assert.equal(enumeration.kind, 6);
  assert.equal(enumeration.insertText.value, 'AuthError');
  const doc = h.document('AuthError.Invalid(');
  const offset = doc.getText().indexOf('Invalid') + 2;
  const variants = await h.providers.CompletionItem.provideCompletionItems(doc, doc.positionAt(offset), h.token);
  assert.equal(variants.find(item => item.label === 'Missing').kind, 7);
  assert.equal(variants.find(item => item.label === 'Missing').insertText.value, 'Missing');
  assert.equal(variants.find(item => item.label === 'Invalid').insertText.value, 'Invalid', 'existing call parenthesis is preserved');
  const hover = await h.providers.Hover.provideHover(doc, doc.positionAt(offset), h.token);
  assert.match(hover.contents.value, /AuthError.Invalid\(message: str\)/);
  const signature = await h.providers.SignatureHelp.provideSignatureHelp(doc, doc.positionAt(doc.getText().length), h.token);
  assert.equal(signature.signatures[0].label, 'AuthError.Invalid(message: str)');
  assert.equal(signature.signatures[0].parameters[0].label, 'message: str');
});

test('standard F12 targets open compiler registry text through a read-only virtual document', async t => {
  const text = 'http.text(Code.OK, view("hello"))';
  const source = 'stdlib:std.http.server';
  const h = host(t, true, 1, folder => {
    const location = { file: source, line: 2, column: 5, length: 4 };
    return { format: 'nagi-symbols-v1', definitions: [], bindings: [], files: [],
      standard_sources: [{ file: source, text: '# Standard HTTP\ndef text(status: Status, body: view[str]) -> Response\n' }],
      references: [{ location: { file: path.join(folder, 'main.nagi'), line: 1, column: 6, length: 4 }, target: location }],
      expressions: [], locals: [] };
  });
  const doc = h.document(text);
  const targets = await h.providers.Definition.provideDefinition(doc, doc.positionAt(7), h.token);
  assert.equal(targets[0].uri.scheme, 'nagi-stdlib');
  assert.equal(targets[0].uri.path, '/std/http/server.nagi');
  assert.equal(targets[0].range.start.line, 1);
  assert.match(h.providers['nagi-stdlib'].provideTextDocumentContent(targets[0].uri), /def text/);
  assert.equal(h.providers['nagi-stdlib'].provideTextDocumentContent({ scheme: 'nagi-stdlib', path: '/std/../private.nagi' }), '');
});
