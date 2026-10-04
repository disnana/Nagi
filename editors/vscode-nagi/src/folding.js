'use strict';
const { context } = require('./features');

const pairs = { ')': '(', ']': '[', '}': '{' };
const functionHeader = /^\s*(?:extern\s+)?(?:async\s+)?fn\s+[A-Za-z_]\w*\s*\(/;

// Low bodies fold by braces, independently of their indentation. Parentheses
// and brackets only guard matching; they do not create separate header folds.
function ranges(text) {
  const lines = context(text, text.length).masked.split('\n');
  const stack = [], result = [];
  let header;
  for (let line = 0; line < lines.length; line++) {
    const code = lines[line], body = code.trim();
    const continuation = stack.some(item => item.kind !== '{');
    if (!continuation && functionHeader.test(code)) {
      header = { line, depth: stack.length };
    } else if (header && !continuation && body && !/^(?:\{|\)|\]|->)/.test(body)) {
      header = undefined;
    }
    for (let column = 0; column < code.length; column++) {
      const c = code[column];
      if (c === '(' || c === '[' || c === '{') {
        const start = c === '{' && header?.depth === stack.length ? header.line : line;
        stack.push({ kind: c, start });
        if (c === '{') header = undefined;
      } else if (pairs[c]) {
        if (stack.at(-1)?.kind !== pairs[c]) {
          // Do not pair braces across a crossed or stray closing delimiter.
          stack.length = 0;
          header = undefined;
          continue;
        }
        const opening = stack.pop();
        if (c === '}') {
          // Keep delimiter-only closers visible, but fold a final statement
          // sharing its line with the closing brace.
          const end = /[^\s)\]};]/.test(code.slice(0, column)) ? line : line - 1;
          if (opening.start < end) result.push({ start: opening.start, end });
        }
      } else if (c === ';') {
        header = undefined;
      }
    }
  }
  // VS Code needs one fold per starting line; prefer the enclosing body when
  // several opening braces share a line.
  return result.sort((a, b) => a.start - b.start || b.end - a.end)
    .filter((item, index, items) => !index || items[index - 1].start !== item.start);
}

module.exports = { ranges };
