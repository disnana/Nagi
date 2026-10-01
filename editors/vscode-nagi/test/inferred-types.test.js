'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const compiler = require('../src/compiler');
const features = require('../src/features');

const root = path.resolve(__dirname, '../../..');
const prelude = `class Point:
    x: i32
class Box:
    point: Point
def make() -> Point:
    return Point(x=7)
def boxed() -> Box:
    return Box(point=make())
def fetch() -> Result[Point, Error]:
    return ok(make())
def nullable() -> Point?:
    return some(make())
async def later() -> Point:
    return make()
async def inspect(argument: Point) -> Result[unit, Error]:
    value = make()
    points = [make()]
`;

async function fixture(t, files) {
  const base = path.join(root, 'build', 'vscode-inferred-types');
  fs.mkdirSync(base, { recursive: true });
  const folder = fs.mkdtempSync(path.join(base, 'case-'));
  t.after(() => fs.rmSync(folder, { recursive: true, force: true }));
  for (const [name, text] of Object.entries(files)) fs.writeFileSync(path.join(folder, name), text);
  return folder;
}

async function symbols(folder, entry, overlays) {
  const args = compiler.argumentsFor('symbols', entry, [], folder, root, '', [], compiler.findProject(entry));
  const result = await new Promise(resolve => compiler.runCheck(compiler.compilerPath('', root, root),
    [...args, '--editor-input'], folder, 10000, resolve, 16 * 1024 * 1024, JSON.stringify({ files: overlays })));
  assert.equal(result.error, null, result.output);
  return JSON.parse(result.output);
}

test('real checker completes arguments, inferred locals, chains, calls, index receivers and match payloads', async t => {
  const folder = await fixture(t, { 'main.nagi': prelude + '    return ok(print(0))\n' });
  const file = path.join(folder, 'main.nagi');
  const snippets = ['value.', 'argument.x', '(value).', 'boxed().point.', 'points[0].', '(try fetch()).', '(await later()).',
    'for item in points:\n        item.', 'match fetch():\n        case Ok(point):\n            point.\n        case Err(problem):\n            print(error_kind(problem))'];
  for (const snippet of snippets) {
    const text = prelude + '    ' + snippet + '\n    return ok(print(0))\n';
    const offset = text.lastIndexOf('.') + 1;
    const member = features.memberContext(text, offset);
    assert.ok(member, snippet);
    const index = await symbols(folder, file, [{ file, text: member.text }]);
    const candidates = features.completionCandidates(index, text, offset, false, { file });
    assert.deepEqual(candidates.map(c => [c.name, c.type]), [['x', 'i32']], snippet);
    assert.equal(features.insertion(candidates[0], ''), 'x');
  }
  assert.equal(fs.readFileSync(file, 'utf8'), prelude + '    return ok(print(0))\n');
});

test('real checker suppresses fields for unknown, moved, out-of-scope and wrapped values', async t => {
  const folder = await fixture(t, { 'main.nagi': prelude + '    return ok(print(0))\n' });
  const file = path.join(folder, 'main.nagi');
  for (const snippet of ['missing.', 'fetch().', 'nullable().', 'try fetch().', 'await later().',
    'if True:\n        hidden = make()\n    hidden.', 'match fetch():\n        case Ok(point):\n            print(point.x)\n        case Err(problem):\n            print(error_kind(problem))\n    point.',
    'text = "example"\n    other = text\n    text.']) {
    const text = prelude + '    ' + snippet + '\n    return ok(print(0))\n';
    const offset = text.lastIndexOf('.') + 1;
    const member = features.memberContext(text, offset);
    const index = await symbols(folder, file, [{ file, text: member.text }]);
    assert.deepEqual(features.completionCandidates(index, text, offset, false, { file }), [], snippet);
  }
});

test('real unsaved High/Low imports supply local hovers and fields with UTF-16 positions', async t => {
  const saved = 'import "models.nagi"\ndef main():\n    print("😀"); point = make(); print(point.x)\n';
  const folder = await fixture(t, { 'main.nagi': saved, 'models.nagi': 'class Point:\n    x: i64\n',
    'math.low': 'fn make() -> Point { return Point(x=7); }\n', 'nagi.toml': "entry = 'main.nagi'\nnative = ['math.low']\n" });
  const file = path.join(folder, 'main.nagi');
  const lowFile = path.join(folder, 'math.low');
  const overlays = [{ file: path.join(folder, 'models.nagi'), text: 'class Point:\n    x: i32\n    enabled: bool\n' },
    { file: lowFile, text: 'fn make() -> Point { let point = Point(x=7, enabled=True); return point; }\n' }];
  const index = await symbols(folder, file, overlays);
  const occurrence = saved.lastIndexOf('point') + 2;
  assert.equal(features.hoverAt(index, saved, occurrence, { file }).item.signature, 'point: Point');
  assert.equal(features.hoverAt(index, overlays[1].text, overlays[1].text.indexOf('point') + 2, { file: lowFile }).item.signature, 'point: Point');
  const text = saved.replace('point.x)', 'point.en)');
  const offset = text.indexOf('point.en') + 'point.en'.length;
  const member = features.memberContext(text, offset);
  const fields = await symbols(folder, file, [...overlays, { file, text: member.text }]);
  assert.deepEqual(features.completionCandidates(fields, text, offset, false, { file }).map(c => [c.name, c.type]), [['x', 'i32'], ['enabled', 'bool']]);
  const target = compiler.definitionAt(index, file, 2, saved.split('\n')[2].indexOf('make'), root);
  assert.equal(compiler.normalizeFile(target.file, root), lowFile);
  assert.equal(fs.readFileSync(file, 'utf8'), saved);
  assert.equal(fs.readFileSync(path.join(folder, 'models.nagi'), 'utf8'), 'class Point:\n    x: i64\n');
});

test('real Low completion isolates same-line scopes, restores outer loop variables and handles CRLF', async t => {
  const saved = 'record Point { x: i32; }\r\nfn main() { let value = Point(x=7); print("😀"); print(value.x); }\r\n';
  const folder = await fixture(t, { 'main.low': saved });
  const file = path.join(folder, 'main.low');
  const index = await symbols(folder, file, []);
  assert.equal(features.hoverAt(index, saved, saved.lastIndexOf('value') + 1, { file }).item.signature, 'value: Point');
  for (const [text, expected] of [
    ['record Point { x: i32; }\nfn main() { let value = Point(x=7); print("😀"); value. }\n', ['x']],
    ['record Point { x: i32; }\nfn main() { if True { let value = Point(x=7); } value. }\n', []],
    ['record Point { x: i32; }\nfn main() { let value = Point(x=7); for value in [True] { value. } }\n', []],
    ['record Point { x: i32; }\nfn main() { let value = Point(x=7); for value in [True] { print(value); } value. }\n', ['x']],
  ]) {
    const offset = text.lastIndexOf('value.') + 'value.'.length;
    const member = features.memberContext(text, offset);
    const parsed = await symbols(folder, file, [{ file, text: member.text }]);
    assert.deepEqual(features.completionCandidates(parsed, text, offset, true, { file }).map(c => c.name), expected, text);
  }
});
