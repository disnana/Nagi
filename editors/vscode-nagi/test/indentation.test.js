'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const { edits } = require('../src/indentation');
const spaces = { tabSize: 4, insertSpaces: true };

function cursor(source, replacement = '') {
  const offset = source.indexOf('|');
  assert.ok(offset >= 0, 'fixture needs a cursor marker');
  assert.equal(source.lastIndexOf('|'), offset, 'fixture needs exactly one cursor marker');
  return { offset, text: source.slice(0, offset) + replacement + source.slice(offset + 1) };
}

function format(source, trigger = '\n', options = spaces, language = 'nagi') {
  const { offset, text } = cursor(source);
  const before = source.slice(0, offset);
  const line = before.split('\n').length - 1;
  const character = before.slice(before.lastIndexOf('\n') + 1).length;
  const lines = text.split('\n');
  for (const edit of edits(text, line, character, trigger, options, language)) {
    lines[edit.line] = edit.text + lines[edit.line].slice(edit.length);
  }
  return lines.join('\n');
}

test('High block headers indent one level, including main, async, match and case', () => {
  for (const header of ['def main():', 'async def main() -> Result[unit, Error]:', 'class Item:', 'if ready:', 'else:', 'for n in range(3):', 'while ready:', 'match value:', 'case Ok(value):', 'scope:', 'async with scope:']) {
    assert.equal(format('    ' + header + '\n|'), '    ' + header + '\n        ');
  }
  assert.equal(format('def main(): # 😀 凪\r\n|'), 'def main(): # 😀 凪\r\n    ');
  assert.equal(format('def main(\n    value: i64,\n) -> i64:\n|'), 'def main(\n    value: i64,\n) -> i64:\n    ');
});

test('else uses the matching if indentation after nested bodies and comments', () => {
  assert.equal(format('def main():\n    if a:\n        if b:\n            print(1)\n            else:|', ':'), 'def main():\n    if a:\n        if b:\n            print(1)\n        else:');
  assert.equal(format('def main():\n    if a:\n        if b:\n            print(1)\n        else:\n            print(2)\n\n        # explanation\n            else:|', ':'), 'def main():\n    if a:\n        if b:\n            print(1)\n        else:\n            print(2)\n\n        # explanation\n    else:');
  assert.equal(format('if a:\n    print(1)\nprint(2)\n    else:|', ':'), 'if a:\n    print(1)\nprint(2)\n    else:');
});

test('first and following cases align under their nearest match', () => {
  assert.equal(format('def main():\n    match result:\n        case Ok(value):|', ':'), 'def main():\n    match result:\n        case Ok(value):');
  assert.equal(format('def main():\n    match result:\n        case Ok(value):\n            print(value)\n            case Err(problem):|', ':'), 'def main():\n    match result:\n        case Ok(value):\n            print(value)\n        case Err(problem):');
  assert.equal(format('match first:\n    case Ok(value):\n        match second:\n            case Ok(other):\n                print(other)\n                case Err(problem):|', ':'), 'match first:\n    case Ok(value):\n        match second:\n            case Ok(other):\n                print(other)\n            case Err(problem):');
});

test('immediate Enter after else/case aligns the branch and its new body together', () => {
  assert.equal(format('if ready:\n    print(1)\n    else:\n        |'), 'if ready:\n    print(1)\nelse:\n    ');
  assert.equal(format('match result:\n    case Ok(value):\n        print(value)\n        case Err(problem):\n            |'), 'match result:\n    case Ok(value):\n        print(value)\n    case Err(problem):\n        ');
});

test('explicitly dedented outer branches keep their matching outer block', () => {
  const outer = 'def main():\n    if a:\n        if b:\n            print(1)\n    else:';
  assert.equal(format(outer + '|', ':'), outer);
  assert.equal(format(outer + '\n|'), outer + '\n        ');
  const match = 'match first:\n    case Ok(value):\n        match second:\n            case Ok(other):\n                print(other)\n    case Err(problem):';
  assert.equal(format(match + '|', ':'), match);
  assert.equal(format(match + '\n|'), match + '\n        ');
  assert.equal(format('if a:\n    print(1)\n"done"\n    else:|', ':'), 'if a:\n    print(1)\n"done"\n    else:');
});

test('multiline case headers move together when colon is typed or Enter cancels it', () => {
  const before = 'match result:\n    case Ok(value):\n        print(value)\n        case Err(\n            problem\n        ):';
  const expected = 'match result:\n    case Ok(value):\n        print(value)\n    case Err(\n        problem\n    ):';
  assert.equal(format(before + '|', ':'), expected);
  assert.equal(format(before + '\n            |'), expected + '\n        ');
});

test('multiline calls, lists and nested delimiters indent and align closing lines', () => {
  assert.equal(format('def main():\n    print(\n|\n    )'), 'def main():\n    print(\n        \n    )');
  assert.equal(format('def main():\n    values = [\n|\n        ]'), 'def main():\n    values = [\n        \n    ]');
  assert.equal(format('    print(\n        call(\n|'), '    print(\n        call(\n            ');
  assert.equal(format('    print(\n        42,\n        )|', ')'), '    print(\n        42,\n    )');
  assert.equal(format('    values = [\n        42,\n        ]|', ']'), '    values = [\n        42,\n    ]');
  assert.equal(format('    print(\n        42,\n    )\n|'), '    print(\n        42,\n    )\n    ');
});

test('strings, escapes and comments never create fake blocks or brackets', () => {
  assert.equal(format('if ready # this is not a header:\n    |'), 'if ready # this is not a header:\n');
  for (const text of ['    print("match x: [")\n|', '    print("escaped \\" [")\n|', '    print("😀 凪 # ( :")\n|']) {
    assert.equal(format(text), cursor(text, '    ').text);
  }
  for (const text of ['if ready:\n    # else:|', 'match result:\n    print("case Ok(x):|', '    print("[ ]|', '    # )|']) {
    assert.equal(format(text, text.includes(')|') ? ')' : ':'), cursor(text).text);
  }
  assert.equal(format('    print(\n        "hello")|', ')'), '    print(\n        "hello")');
  assert.equal(format('    values = [\n        "hello"]|', ']'), '    values = [\n        "hello"]');
  assert.equal(format('    print(\n|"hello")'), '    print(\n        "hello")');
});

test('an unfinished string on an earlier line does not disable later indentation', () => {
  for (const quote of ['"', "'"]) {
    const earlier = 'def broken():\n    text = ' + quote + 'unfinished # [ :\n\n';
    assert.equal(format(earlier + 'def main():\n|'), earlier + 'def main():\n    ');
    const branch = 'match value:\n    case Ok(x):\n        print(x)\n        case Err(e):';
    assert.equal(format(earlier + branch + '|', ':'), earlier + branch.replace('        case Err', '    case Err'));
  }
  assert.equal(format('    print("unfinished )|', ')'), '    print("unfinished )');
});

test('indentation respects two spaces, tabs, CRLF and Low brace blocks', () => {
  assert.equal(format('def main():\n|', '\n', { tabSize: 2, insertSpaces: true }), 'def main():\n  ');
  assert.equal(format('\tif ready:\n|', '\n', { tabSize: 4, insertSpaces: false }), '\tif ready:\n\t\t');
  assert.equal(format('match result:\r\n    case Ok(value):\r\n        print(value)\r\n        case Err(problem):|', ':'), 'match result:\r\n    case Ok(value):\r\n        print(value)\r\n    case Err(problem):');
  assert.equal(format('fn main() {\n|\n}', '\n', spaces, 'nagi-low'), 'fn main() {\n    \n}');
  assert.equal(format('fn main() {\n    print(\n|\n    )\n}', '\n', spaces, 'nagi-low'), 'fn main() {\n    print(\n        \n    )\n}');
  assert.equal(format('fn main() {\n    print(42);\n    }|', '}', spaces, 'nagi-low'), 'fn main() {\n    print(42);\n}');
});
