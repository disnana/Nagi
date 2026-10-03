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
function host(t, trusted = true, code = 1, symbols) {
  const folder = fs.mkdtempSync(path.join(os.tmpdir(), 'nagi-provider-'));
  t.after(() => fs.rmSync(folder, { recursive: true, force: true }));
  const disposable = () => ({ dispose() {} });
  const providers = {}, selectors = {}, events = {}, commands = new Map(), diagnostics = new Map();
  const calls = [];
  const uri = (file, scheme = 'file') => ({ scheme, fsPath: file, toString: () => `${scheme}:${file}` });
  class Position { constructor(line, character) { this.line = line; this.character = character; } }
  class Range {
    constructor(a, b, c, d) { this.start = typeof a === 'number' ? new Position(a, b) : a; this.end = typeof a === 'number' ? new Position(c, d) : b; }
    isEqual(other) { return this.start.line === other.start.line && this.start.character === other.start.character && this.end.line === other.end.line && this.end.character === other.end.character; }
  }
  class MarkdownString {
    constructor() { this.value = ''; }
    appendCodeblock(text) { this.value += text + '\n'; }
    appendText(text) { this.value += text; }
  }
  const collection = { clear() { diagnostics.clear(); }, set(resource, items) { diagnostics.set(resource.toString(), items); }, dispose() {} };
  const vscode = {
    workspace: {
      isTrusted: trusted, textDocuments: [],
      getWorkspaceFolder: () => ({ uri: uri(folder) }),
      getConfiguration: () => ({ get: (_, fallback) => fallback }),
      onDidOpenTextDocument: disposable, onDidSaveTextDocument(fn) { events.save = fn; return disposable(); },
      onDidChangeTextDocument(fn) { events.change = fn; return disposable(); }, onDidCloseTextDocument: disposable,
      onDidGrantWorkspaceTrust: disposable, onDidChangeConfiguration: disposable,
      registerTextDocumentContentProvider(scheme, provider) { providers[scheme] = provider; return disposable(); },
      createFileSystemWatcher: () => ({ dispose() {}, onDidCreate: disposable, onDidChange: disposable, onDidDelete: disposable }),
    },
    window: { createOutputChannel: () => ({ appendLine() {}, show() {}, dispose() {} }),
      showWarningMessage: async () => undefined, setStatusBarMessage() {} },
    languages: { createDiagnosticCollection: () => collection, registerOnTypeFormattingEditProvider: disposable },
    commands: { registerCommand(name, fn) { commands.set(name, fn); return disposable(); } },
    Range, MarkdownString, Uri: { file: file => uri(file), parse: text => {
      const [scheme, pathname] = text.split(':');
      return { ...uri(pathname, scheme), path: pathname, authority: '', query: '', fragment: '' };
    } },
    Location: class { constructor(uri, range) { this.uri = uri; this.range = range; } },
    EventEmitter: class { constructor() { this.event = () => disposable(); } fire() {} dispose() {} },
    DiagnosticSeverity: { Error: 0 }, CompletionItemKind: { Function: 1, Class: 2, Field: 3, TypeParameter: 4, Keyword: 5, Enum: 6, EnumMember: 7, Module: 8, Constant: 9 },
    Diagnostic: class { constructor(range, message) { this.range = range; this.message = message; } },
    Hover: class { constructor(contents, range) { this.contents = contents; this.range = range; } },
    CompletionItem: class { constructor(label, kind) { this.label = label; this.kind = kind; } },
    SnippetString: class { constructor(value) { this.value = value; } },
    SignatureInformation: class { constructor(label, documentation) { this.label = label; this.documentation = documentation; } },
    ParameterInformation: class { constructor(label) { this.label = label; } }, SignatureHelp: class {},
  };
  for (const kind of ['Definition', 'Hover', 'CompletionItem', 'SignatureHelp']) {
    vscode.languages[`register${kind}Provider`] = (selector, provider) => {
      providers[kind] = provider; selectors[kind] = selector; return disposable();
    };
  }
  const compiler = { ...realCompiler, runCheck(executable, args, cwd, timeout, callback) {
    assert.equal(vscode.workspace.isTrusted, true, 'compiler must never execute in an untrusted workspace');
    calls.push({ executable, args });
    queueMicrotask(() => callback(symbols && args[0] === 'symbols'
      ? { error: null, output: JSON.stringify(symbols(cwd)) }
      : { error: Object.assign(new Error('checker failed'), { code }), output: 'error: line 2: expected i32\n' }));
    return { kill() {} };
  } };
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../src/extension.js'), 'utf8'), {
    module, Buffer, require(name) { if (name === 'vscode') return vscode; if (name === './compiler') return compiler; return require(name.startsWith('./') ? path.join(__dirname, '../src', name) : name); },
  }, { filename: 'extension.js' });
  const context = { subscriptions: [] };
  module.exports.activate(context);
  t.after(() => context.subscriptions.forEach(item => item.dispose()));
  function document(text, scheme = 'file', languageId = 'nagi') {
    const file = path.join(folder, languageId === 'nagi-low' ? 'main.low' : 'main.nagi');
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
  return { providers, selectors, events, commands, diagnostics, calls, document, token };
}

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
