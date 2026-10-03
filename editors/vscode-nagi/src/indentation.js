'use strict';
const { context } = require('./features');

const header = /^(?:(?:async\s+)?def\b|class\b|enum\b|if\b|else\b|while\b|for\b|match\b|case\b|scope\b|async\s+with\b)[\s\S]*:\s*$/;
const pairs = { ')': '(', ']': '[', '}': '{' };

function width(space, size) {
  let column = 0;
  for (const c of space) column = c === '\t' ? column + size - column % size : column + 1;
  return column;
}

function branchParent(blocks, kind, indent, size) {
  const target = kind === 'else' ? 'if' : 'match';
  const extra = kind === 'case' ? size : 0;
  return [...blocks].reverse().find(block => block.kind === target && block.indent + extra <= indent);
}

// Return edits to leading whitespace only. Strings and comments keep their text.
function edits(text, line, character, trigger, options, language = 'nagi') {
  const size = Math.max(1, Math.trunc(options.tabSize) || 4);
  const rawLines = text.split('\n');
  const lines = rawLines.map(value => value.replace(/\r$/, ''));
  if (line < 0 || line >= lines.length) return [];
  // Nagi strings end on their source line. An unfinished quote elsewhere must
  // not suppress indentation while another line is being edited.
  const state = context(lines[line], character);
  if (!state.allowed) return [];
  const code = lines.map((value, index) => index === line ? state.masked : context(value, value.length).masked);
  const brackets = [];
  const blocks = [];
  let statement = '', statementIndent = 0, statementStart = 0, lastIndent = 0, lastHeader = false, headerIndent;
  for (let i = 0; i < line; i++) {
    const body = code[i].trim();
    const raw = lines[i].trim();
    if (!body && (!raw || raw.startsWith('#'))) continue;
    const indent = width(lines[i].match(/^[ \t]*/)[0], size);
    if (!brackets.length) { statement = ''; statementIndent = indent; statementStart = i; }
    statement += body + ' ';
    for (const c of code[i]) {
      if (c === '(' || c === '[' || language === 'nagi-low' && c === '{') brackets.push({ kind: c, indent });
      else if (pairs[c] && brackets.at(-1)?.kind === pairs[c]) brackets.pop();
    }
    lastIndent = statementIndent;
    lastHeader = language === 'nagi' && !brackets.length && header.test(statement.trim());
    headerIndent = undefined;
    const branch = lastHeader && /^(else|case)\b/.exec(statement.trim())?.[1];
    if (branch) {
      const parent = branchParent(blocks, branch, statementIndent, size);
      if (parent) headerIndent = parent.indent + (branch === 'case' ? size : 0);
    }
    if (!brackets.length && language === 'nagi') {
      while (blocks.length && blocks.at(-1).indent >= statementIndent) blocks.pop();
      if (lastHeader) blocks.push({ kind: statement.trim().split(/\s/)[0], indent: statementIndent });
    }
  }
  const result = [];
  function replace(target, columns) {
    const previous = lines[target].match(/^[ \t]*/)[0];
    const next = options.insertSpaces ? ' '.repeat(columns) : '\t'.repeat(Math.floor(columns / size)) + ' '.repeat(columns % size);
    if (previous !== next) result.push({ line: target, length: previous.length, text: next });
  }
  function alignHeader(start, end, indent, desired) {
    const delta = desired - indent;
    for (let i = start; i <= end; i++) {
      if (lines[i].trim()) replace(i, Math.max(0, width(lines[i].match(/^[ \t]*/)[0], size) + delta));
    }
  }
  const current = code[line].trim();
  const first = lines[line].trim()[0];
  if (trigger === ':') {
    if (language !== 'nagi' || !current.endsWith(':')) return [];
    const remaining = [...brackets];
    for (const c of code[line]) {
      if (c === '(' || c === '[') remaining.push({ kind: c });
      else if (pairs[c] && remaining.at(-1)?.kind === pairs[c]) remaining.pop();
    }
    if (remaining.length) return [];
    const kind = /^(else|case)\b/.exec(brackets.length ? statement + current : current)?.[1];
    const start = brackets.length ? statementStart : line;
    const indent = width(lines[start].match(/^[ \t]*/)[0], size);
    const parent = kind && branchParent(blocks, kind, indent, size);
    if (parent) alignHeader(start, line, indent, parent.indent + (kind === 'case' ? size : 0));
  } else if (pairs[trigger]) {
    if (first === trigger && brackets.at(-1)?.kind === pairs[trigger]) replace(line, brackets.at(-1).indent);
  } else if (trigger === '\n') {
    if (brackets.length) {
      const opening = brackets.at(-1);
      replace(line, first && pairs[first] === opening.kind ? opening.indent : opening.indent + size);
      // Enter between an automatically inserted pair also moves the closing
      // delimiter to its own line. Align that line with the opening statement.
      if (!current && line + 1 < lines.length && pairs[lines[line + 1].trim()[0]] === opening.kind) replace(line + 1, opening.indent);
    } else if (line > 0 && code[line - 1].trim()) {
      // Typing Enter immediately after ':' can cancel VS Code's colon edit.
      // Resolve the branch again so both lines still use the matching block.
      if (headerIndent !== undefined) alignHeader(statementStart, line - 1, statementIndent, headerIndent);
      replace(line, (headerIndent ?? lastIndent) + (lastHeader ? size : 0));
    }
  }
  return result;
}

module.exports = { edits };
