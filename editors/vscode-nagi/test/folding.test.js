'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const { ranges } = require('../src/folding');

test('unindented Low fn, record and enum bodies keep their closing braces visible', () => {
  const text = [
    'record Point {', 'x: f64;', 'y: f64;', '}',
    'enum Choice {', 'Cancelled;', 'Selected(id: i64);', '}',
    'fn main() {', 'print(42);', '}',
  ].join('\n');
  assert.deepEqual(ranges(text), [{ start: 0, end: 2 }, { start: 4, end: 6 }, { start: 8, end: 9 }]);
  assert.deepEqual(ranges(text.replaceAll('\n', '\r\n')), ranges(text));
});

test('nested match, case, scope and branch bodies fold at each opening line', () => {
  const text = [
    'fn main() {',
    'match value {',
    'case Choice.Selected(id) {',
    'scope {',
    'print(id);',
    '}',
    '}',
    'case Choice.Cancelled {',
    'if ready {',
    'print(0);',
    '} else {',
    'print(1);',
    '}',
    '}',
    '}',
    '}',
  ].join('\n');
  assert.deepEqual(ranges(text), [
    { start: 0, end: 14 }, { start: 1, end: 13 }, { start: 2, end: 5 }, { start: 3, end: 4 },
    { start: 7, end: 12 }, { start: 8, end: 9 }, { start: 10, end: 11 },
  ]);
});

test('multiline fn and async fn draft headers join their body in one fold', () => {
  for (const prefix of ['fn', 'async fn']) {
    const text = [
      `${prefix} calculate(`,
      '    value: i64,',
      '    callback: fn[i64, i64]',
      ') -> Result[',
      '    i64, Error',
      '] {',
      'return ok(callback(value));',
      '}',
    ].join('\n');
    assert.deepEqual(ranges(text), [{ start: 0, end: 6 }]);
  }
  assert.deepEqual(ranges('fn main()\n# body follows\n{\nprint(1);\n}'), [{ start: 0, end: 3 }]);
});

test('strings and comments do not create or close folds', () => {
  const text = [
    '# fn fake() {',
    'fn main() { # }',
    'print("😀 { ( [ # }");',
    "print('} ] )');",
    'print("escaped \\" }");',
    '# }',
    '}',
    '# }',
  ].join('\n');
  assert.deepEqual(ranges(text), [{ start: 1, end: 5 }]);
  assert.deepEqual(ranges('"{\nvalue\n"}'), []);
  assert.deepEqual(ranges('"unfinished {\nfn main() {\nprint(1);\n}'), [{ start: 1, end: 2 }]);
});

test('crossed, stray and unclosed delimiters cannot invent brace folds', () => {
  for (const text of [
    'fn main() {\nprint(1);',
    'fn main() {\nprint(\n}\n)',
    'fn main() {\nvalues = [\n)\n];\n}',
    'fn main() {\n]\n}',
    '}\nprint(1);\n}',
  ]) assert.deepEqual(ranges(text), [], text);
  assert.deepEqual(ranges('fn broken() {\n]\n}\nfn valid() {\nprint(1);\n}'), [{ start: 3, end: 4 }]);
});

test('matched call and list continuations guard bodies without separate folds', () => {
  const text = [
    'fn main() {',
    'print(',
    '    copy(',
    '        view([1, 2])',
    '    )',
    ');',
    '}',
  ].join('\n');
  assert.deepEqual(ranges(text), [{ start: 0, end: 5 }]);
  assert.deepEqual(ranges('values = [\n1,\n2\n];'), []);
});

test('completed declarations and unrelated lines do not donate a header to another body', () => {
  assert.deepEqual(ranges('extern fn native();\nrecord Point {\nx: i64;\n}'), [{ start: 1, end: 2 }]);
  assert.deepEqual(ranges('fn unfinished()\nrecord Point {\nx: i64;\n}'), [{ start: 1, end: 2 }]);
  assert.deepEqual(ranges('fn main() {\ncallback: fn[i64, i64] = calculate;\nif ready {\nprint(1);\n}\n}'), [
    { start: 0, end: 4 }, { start: 2, end: 3 },
  ]);
});

test('single-line and empty bodies are omitted and each start line has one fold', () => {
  assert.deepEqual(ranges('fn one() {}\nfn empty() {\n}'), []);
  assert.deepEqual(ranges('fn main() { if ready {\nprint(1);\n}\nprint(2);\n}'), [{ start: 0, end: 3 }]);
});

test('inline closing braces fold the final statement while delimiter-only closers stay visible', () => {
  assert.deepEqual(ranges('fn main() {\nprint(1); }'), [{ start: 0, end: 1 }]);
  assert.deepEqual(ranges('fn main() {\nprint(1);\nprint("}"); }'), [{ start: 0, end: 2 }]);
  assert.deepEqual(ranges('fn main() {\nif ready {\nprint(1);\n}}'), [
    { start: 0, end: 2 }, { start: 1, end: 2 },
  ]);
  assert.deepEqual(ranges('fn main() {\nprint(1);\n}; # done'), [{ start: 0, end: 1 }]);
});
