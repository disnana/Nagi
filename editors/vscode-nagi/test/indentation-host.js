'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vscode = require('vscode');

const report = path.resolve(__dirname, '../../../build/vscode-typing-result.json');
const cases = [];
function save(status, error) {
  fs.mkdirSync(path.dirname(report), { recursive: true });
  const manifest = require('../package.json');
  const extension = vscode.extensions.getExtension(`${manifest.publisher}.${manifest.name}`);
  fs.writeFileSync(report, JSON.stringify({ status, workspaceTrusted: vscode.workspace.isTrusted,
    extensionPath: extension?.extensionPath, cases, error }, null, 2));
}

async function run() {
  save('running');
  const manifest = require('../package.json');
  await vscode.extensions.getExtension(`${manifest.publisher}.${manifest.name}`).activate();
  if (process.env.NAGI_TEST_UNTRUSTED === '1') assert.equal(vscode.workspace.isTrusted, false, 'workspace stays untrusted');
  let count = 0;
  async function typing(before, input, expected, language = 'nagi', options = {}, undo = false) {
    const doc = await vscode.workspace.openTextDocument({ language, content: before });
    const editor = await vscode.window.showTextDocument(doc);
    editor.options = { insertSpaces: true, tabSize: 4, ...options };
    const end = doc.positionAt(before.length);
    editor.selection = new vscode.Selection(end, end);
    for (const char of input) await vscode.commands.executeCommand('type', { text: char });
    const deadline = Date.now() + 3000;
    while (doc.getText() !== expected && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 20));
    assert.equal(doc.getText(), expected, `typed ${JSON.stringify(input)} in ${language}: ${JSON.stringify(before)}`);
    if (undo) {
      await vscode.commands.executeCommand('undo');
      assert.equal(doc.getText(), before + input, 'Undo restores the indentation before the automatic edit');
      await vscode.commands.executeCommand('undo');
      assert.equal(doc.getText(), before, 'a second Undo restores the keystroke');
    }
    count++;
    cases.push({ language, before, input, expected });
    save('running');
    await vscode.commands.executeCommand('workbench.action.revertAndCloseActiveEditor');
  }
  assert.equal(vscode.workspace.getConfiguration('editor', { languageId: 'nagi' }).get('formatOnType'), true);
  await typing('def main():', '\n', 'def main():\n    ');
  await typing('async def main() -> Result[unit, Error]: # 凪', '\n', 'async def main() -> Result[unit, Error]: # 凪\n    ');
  await typing('def main():\n    if True:\n        print(1)\n        else', ':\n', 'def main():\n    if True:\n        print(1)\n    else:\n        ');
  await typing('match result:\n    case Ok(value)', ':\n', 'match result:\n    case Ok(value):\n        ');
  await typing('match result:\n    case Ok(value):\n        print(value)\n        case Err(problem)', ':\n', 'match result:\n    case Ok(value):\n        print(value)\n    case Err(problem):\n        ');
  await typing('def main():\n    if outer:\n        if inner:\n            print(1)\n    else', ':\n', 'def main():\n    if outer:\n        if inner:\n            print(1)\n    else:\n        ');
  await typing('match first:\n    case Ok(value):\n        match second:\n            case Ok(other):\n                print(other)\n    case Err(problem)', ':\n', 'match first:\n    case Ok(value):\n        match second:\n            case Ok(other):\n                print(other)\n    case Err(problem):\n        ');
  await typing('match result:\n    case Ok(value):\n        print(value)\n        case Err(\n            problem\n        )', ':\n', 'match result:\n    case Ok(value):\n        print(value)\n    case Err(\n        problem\n    ):\n        ');
  await typing('if True:\n    print(1)\n    else', ':', 'if True:\n    print(1)\nelse:', 'nagi', {}, true);
  await typing('    print', '(\n', '    print(\n        \n    )');
  await typing('    values = ', '[\n', '    values = [\n        \n    ]');
  await typing('    print(', '\n', '    print(\n        ');
  await typing('    values = [', '\n', '    values = [\n        ');
  await typing('    print(\n        42,\n        ', ')', '    print(\n        42,\n    )');
  await typing('    print(\n        "hello"', ')', '    print(\n        "hello")');
  await typing('    values = [\n        "hello"', ']', '    values = [\n        "hello"]');
  await typing('def main(\n    value: i64,\n) -> i64:', '\n', 'def main(\n    value: i64,\n) -> i64:\n    ');
  await typing('    print("😀 凪 # [ :")', '\n', '    print("😀 凪 # [ :")\n    ');
  await typing('if ready # comment:', '\n', 'if ready # comment:\n');
  await typing('broken = "unfinished [\n\ndef main():', '\n', 'broken = "unfinished [\n\ndef main():\n    ');
  await typing('match result:\n    # case Ok(value)', ':', 'match result:\n    # case Ok(value):');
  await typing('# comment ', '(', '# comment (');
  await typing('    print("text ', '[', '    print("text [');
  await typing('scope:', '\n', 'scope:\n    ');
  await typing('async with scope:', '\n', 'async with scope:\n    ');
  await typing('def main():', '\n', 'def main():\n  ', 'nagi', { tabSize: 2 });
  await typing('\tif True:', '\n', '\tif True:\n\t\t', 'nagi', { insertSpaces: false });
  await typing('fn main() {', '\n', 'fn main() {\n    ', 'nagi-low');
  await typing('fn main() {\n    print(\n        42,\n        ', ')', 'fn main() {\n    print(\n        42,\n    )', 'nagi-low');
  await typing('fn main() {\n    print(42);\n    ', '}', 'fn main() {\n    print(42);\n}', 'nagi-low');
  await typing('fn main() ', '{\n', 'fn main() {\n    \n}', 'nagi-low');
  const config = vscode.workspace.getConfiguration('editor', { languageId: 'nagi' });
  const oldFormatOnType = config.inspect('formatOnType').globalLanguageValue;
  try {
    await config.update('formatOnType', false, vscode.ConfigurationTarget.Global, true);
    await typing('if True:\n    print(1)\n    else', ':', 'if True:\n    print(1)\n    else:');
  } finally {
    await config.update('formatOnType', oldFormatOnType, vscode.ConfigurationTarget.Global, true);
  }
  const snippetDoc = await vscode.workspace.openTextDocument({ language: 'nagi', content: '' });
  const snippetEditor = await vscode.window.showTextDocument(snippetDoc);
  await snippetEditor.insertSnippet(new vscode.SnippetString(require('../snippets/high.json').Main.body.join('\n')));
  assert.equal(snippetDoc.getText(), 'def main():\n    print("Hello, Nagi!")');
  cases.push({ snippet: 'main', expected: snippetDoc.getText() });
  count++;
  await vscode.commands.executeCommand('workbench.action.revertAndCloseActiveEditor');
  console.log(`PASS: ${count} real typing cases, High/Low, untitled documents, without compiler`);
  save('passed');
}

module.exports = { async run() { try { await run(); } catch (error) { save('failed', error.stack); throw error; } } };
