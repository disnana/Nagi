'use strict';
const { normalizeFile } = require('./compiler');

// These signatures describe the supported builtins, rather than inferred overloads.
const builtinRows = [
  ['print', 'value', 'unit', '1つの値を改行付きで表示します。'],
  ['write', 'value', 'unit', '改行なしで表示します。'],
  ['len', 'value', 'i64', '文字列はbyte数、配列は要素数です。'],
  ['view', 'value', 'view[T]', '元の値を読み取り用に借ります。'],
  ['copy', 'borrowed', 'T', 'viewから独立した所有コピーを作ります。'],
  ['append', 'list, value', 'unit', '配列に同じ型の値を追加します。'],
  ['parse_i64', 'text: str | view[str]', 'Result[i64, Error]', '整数に変換します。tryまたはmatchで失敗を処理します。'],
  ['parse_f64', 'text: str | view[str]', 'Result[f64, Error]', '小数に変換します。'],
  ['ok', 'value: T', 'Result[T, Error]', '成功値を返します。'],
  ['error', 'message: str', 'Result[T, Error]', '入力エラーを作ります。HTTPでは400です。'],
  ['not_found', 'message: str', 'Result[T, Error]', '対象なしを返します。HTTPでは404です。'],
  ['internal_error', 'message: str', 'Result[T, Error]', '内部エラーを返します。HTTPでは詳細を伏せた500です。'],
  ['fail', 'problem: Error', 'Result[T, Error]', 'Errorをmoveして種類を保ったまま返します。'],
  ['error_kind', 'problem: Error', 'str', 'Errorを消費せず、種類の名前を返します。'],
  ['error_message', 'problem: Error', 'str', 'Errorを消費せず、messageをコピーします。'],
  ['some', 'value: T', 'T?', 'nullableの値ありを作ります。'],
  ['env', 'name: str, fallback: str', 'str', '環境変数がなければ既定値を返します。'],
  ['assert_true', 'condition: bool', 'unit', 'Falseならpanicします。'],
  ['range', 'end: i64', 'Range', 'forで0から終端未満まで走査します。'],
  ['html', 'text: str', 'Html', '所有文字列をHTML応答にします。'],
  ['include_text', 'path: str', 'str', '文字列リテラルの相対パスをコンパイル時に埋め込みます。'],
  ['read_line', '', 'Result[str, Error]', 'コンソールから同期的に1行読みます。'],
  ['sleep', 'milliseconds: i64', 'unit', 'ミリ秒単位で待ちます。', true],
  ['db_open', 'path: str | view[str]', 'Result[Db, Error]', 'SQLiteを開きます。', true],
  ['serve', 'db: Db, port: i64', 'Result[unit, Error]', 'loopbackでHTTPサーバーを起動します。', true],
  ['db_exec', 'db: Db, sql: str | view[str]', 'Result[i64, Error]', 'SQLを実行して影響した行数を返します。', true],
  ['db_all', 'db: Db, sql: str | view[str]', 'Result[List[T], Error]', '複数行を指定classへ読みます。', true],
  ['db_query', 'db: Db, sql: str | view[str], id: i64', 'Result[T?, Error]', '1行を読みます。値がなければNoneです。', true],
  ['db_write', 'db: Db, sql: str | view[str], id: i64', 'Result[i64, Error]', 'bind引数1つでSQLを実行します。', true],
  ['json_decode', 'input: str | bytes | view[str] | view[bytes]', 'Result[T, Error]', 'JSONを指定classへ読みます。'],
  ['json_encode', 'value: T', 'Result[str, Error]', '値をJSON文字列にします。'],
  ['slice', 'borrowed: view[T], start: i64, end: i64', 'Result[view[T], Error]', '終端を含まない区間を借ります。範囲とUTF-8境界を検査します。'],
];
const builtins = builtinRows.map(([name, args, result, description, asynchronous = false]) => ({
  name, kind: 'function', signature: `${asynchronous ? 'async ' : ''}def ${name}${['db_all', 'db_query', 'json_decode'].includes(name) ? '[T]' : ''}(${args}) -> ${result}`,
  typeParameters: ['db_all', 'db_query', 'json_decode'].includes(name) ? ['T'] : [],
  parameters: args ? args.split(', ').map(arg => { const [name, type] = arg.split(': '); return { name, type: type || 'T' }; }) : [],
  return_type: result, asynchronous, description, builtin: true,
}));
const types = ['i8', 'i16', 'i32', 'i64', 'u8', 'u16', 'u32', 'u64', 'f32', 'f64', 'bool', 'str', 'bytes', 'unit', 'Error', 'Db', 'Html', 'UUID', 'timestamp', 'List', 'Result', 'view', 'shared'];

// Preserve UTF-16 offsets while masking strings/comments, including unfinished strings.
function context(text, offset) {
  let quote, escaped = false, comment = false;
  let masked = '';
  let allowed = true;
  let inComment = false;
  for (let i = 0; i < text.length; i++) {
    if (i === offset) { allowed = !quote && !comment; inComment = comment; }
    const c = text[i];
    if (c === '\n') { comment = false; masked += '\n'; continue; }
    if (comment) { masked += ' '; continue; }
    if (quote) {
      masked += ' ';
      if (c === quote && !escaped) quote = undefined;
      escaped = c === '\\' && !escaped;
    } else if (c === '#') { comment = true; masked += ' '; }
    else if (c === '"' || c === "'") { quote = c; escaped = false; masked += ' '; }
    else masked += c;
  }
  if (offset === text.length) { allowed = !quote && !comment; inComment = comment; }
  return { masked, allowed, inComment };
}

function wordAt(text, offset) {
  let start = offset, end = offset;
  while (start > 0 && /[A-Za-z0-9_]/.test(text[start - 1])) start--;
  while (end < text.length && /[A-Za-z0-9_]/.test(text[end])) end++;
  return { start, end, name: text.slice(start, end) };
}

function declarations(index) {
  const result = new Map();
  for (const item of index?.definitions || []) {
    if (typeof item.name === 'string' && typeof item.signature === 'string' && item.signature && !result.has(item.name)) result.set(item.name, item);
  }
  for (const item of builtins) if (!result.has(item.name)) result.set(item.name, item);
  return result;
}

function inTypeContext(before) {
  const line = before.slice(before.lastIndexOf('\n') + 1);
  if (/(?:->|\b[A-Za-z_]\w*\s*:)\s*[\w\[\], ?]*$/.test(line)) return true;
  const brackets = [];
  for (let i = 0; i < before.length; i++) { if (before[i] === '[') brackets.push(i); else if (before[i] === ']') brackets.pop(); }
  return brackets.some(i => /\b(?:Result|List|view|shared|json_decode|db_query|db_all)\s*$/.test(before.slice(0, i)));
}

function fileMatches(a, b) {
  if (typeof a !== 'string' || typeof b !== 'string') return false;
  const key = file => {
    const name = normalizeFile(file, '.');
    return process.platform === 'win32' ? name.toLowerCase() : name;
  };
  return key(a) === key(b);
}

function sourceOffsets(text) {
  const starts = [0];
  for (let i = text.indexOf('\n'); i >= 0; i = text.indexOf('\n', i + 1)) {
    starts.push(i + 1);
  }
  return (line, column) => {
    if (!Number.isInteger(line) || line < 1 || !Number.isInteger(column) || column < 1) return undefined;
    const start = starts[line - 1];
    if (start === undefined) return undefined;
    let end = starts[line] === undefined ? text.length : starts[line] - 1;
    if (text[end - 1] === '\r') end--;
    return column - 1 <= end - start ? start + column - 1 : undefined;
  };
}

// Only the member being edited is masked. The compiler determines the receiver's
// type from this in-memory source; no type is inferred from its spelling here.
function memberContext(text, offset) {
  const state = context(text, offset);
  if (!state.allowed) return undefined;
  const word = wordAt(state.masked, offset);
  let dot = word.start - 1;
  while (dot >= 0 && /[ \t\r]/.test(state.masked[dot])) dot--;
  if (state.masked[dot] !== '.') return undefined;
  let receiverEnd = dot;
  while (receiverEnd > 0 && /\s/.test(state.masked[receiverEnd - 1])) receiverEnd--;
  return { dot, receiverEnd, start: word.start, end: word.end,
    text: text.slice(0, dot) + text.slice(dot, word.end).replace(/[^\r\n]/g, ' ') + text.slice(word.end) };
}

function fieldCandidates(index, text, member, source) {
  if (!source.file || source.saved) return [];
  const offsetAt = sourceOffsets(text);
  const candidates = (index?.expressions || []).filter(e => fileMatches(e.location?.file, source.file) &&
    offsetAt(e.end_line, e.end_column) === member.receiverEnd);
  // A trailing dot binds to the innermost postfix expression: `try fetch().`
  // accesses the Result, while `(try fetch()).` accesses its payload.
  candidates.sort((a, b) => (offsetAt(b.location.line, b.location.column) ?? -1) -
    (offsetAt(a.location.line, a.location.column) ?? -1));
  const receiver = candidates[0];
  return (receiver?.fields || []).filter(f => typeof f.name === 'string' && typeof f.type === 'string')
    .map(f => ({ name: f.name, kind: 'field', signature: `${f.name}: ${f.type}`, type: f.type }));
}

function hoverAt(index, text, offset, source = {}) {
  const state = context(text, offset);
  if (!state.allowed) return undefined;
  const word = wordAt(state.masked, offset);
  if (!word.name || memberContext(text, offset)) return undefined;
  if (source.file && !source.saved) {
    const offsetAt = sourceOffsets(text);
    const local = (index?.locals || []).find(item => item.name === word.name && typeof item.type === 'string' &&
      fileMatches(item.location?.file, source.file) && offsetAt(item.location.line, item.location.column) === word.start &&
      item.location.length === word.end - word.start);
    if (local) return { item: { ...local, kind: 'variable', signature: `${local.name}: ${local.type}` }, start: word.start, end: word.end };
  }
  const item = declarations(index).get(word.name);
  if (!item) return undefined;
  const before = state.masked.slice(0, word.start);
  const after = state.masked.slice(word.end);
  const declaration = /\b(?:def|fn|class|record)\s*$/.test(before);
  const call = /^\s*\(/.test(after) || /^\s*\[[\w\[\], ?]+\]\s*\(/.test(after);
  if (!declaration && !call && !(item.kind === 'class' && inTypeContext(before))) return undefined;
  return { item, start: word.start, end: word.end };
}

function completionCandidates(index, text, offset, low = false, source = {}) {
  const state = context(text, offset);
  if (!state.allowed) return [];
  const word = wordAt(state.masked, offset);
  const member = memberContext(text, offset);
  if (member) return fieldCandidates(index, text, member, source);
  const before = state.masked.slice(0, word.start);
  const all = [...declarations(index).values()];
  if (inTypeContext(before)) return [...all.filter(x => x.kind === 'class'), ...types.map(name => ({ name, kind: 'type', signature: name }))];
  return [...all, ...(low ? ['fn', 'let', 'return', 'if', 'else', 'while', 'for', 'match', 'case', 'async', 'await', 'try'] : ['def', 'class', 'return', 'if', 'else', 'while', 'for', 'match', 'case', 'async', 'await', 'try']).map(name => ({ name, kind: 'keyword', signature: name }))];
}

function insertion(item, following) {
  if (item.kind === 'type') {
    return { List: 'List[${1:T}]', Result: 'Result[${1:T}, Error]', view: 'view[${1:str}]', shared: 'shared[${1:T}]' }[item.name] || item.name;
  }
  if (!['function', 'class'].includes(item.kind) || /^\s*\(/.test(following)) return item.name;
  const args = item.kind === 'class' ? item.fields || [] : item.parameters || [];
  const types = item.typeParameters || [];
  const generic = types.length ? `[${types.map((name, i) => `\$\{${i + 1}:${name}\}`).join(', ')}]` : '';
  return `${item.name}${generic}(${args.map((arg, i) => `${item.kind === 'class' ? arg.name + '=' : ''}\$\{${i + 1 + types.length}:${arg.name}\}`).join(', ')})`;
}

function activeCall(text, offset) {
  const state = context(text, offset);
  // Signature help can stay visible inside a string argument.
  if (state.inComment) return undefined;
  const code = state.masked.slice(0, offset);
  let parens = 0, brackets = 0, braces = 0, argument = 0;
  for (let i = code.length - 1; i >= 0; i--) {
    const c = code[i];
    if (c === ')') parens++;
    else if (c === '(') {
      if (parens) { parens--; continue; }
      let prefix = code.slice(0, i).trimEnd();
      if (prefix.endsWith(']')) {
        let depth = 1, j = prefix.length - 2;
        for (; j >= 0 && depth; j--) { if (prefix[j] === ']') depth++; else if (prefix[j] === '[') depth--; }
        prefix = prefix.slice(0, j + 1).trimEnd();
      }
      const match = prefix.match(/([A-Za-z_]\w*)$/);
      if (match && prefix[prefix.length - match[1].length - 1] !== '.') return { name: match[1], argument };
    } else if (c === ']') brackets++;
    else if (c === '[') brackets--;
    else if (c === '}') braces++;
    else if (c === '{') braces--;
    else if (c === ',' && !parens && !brackets && !braces) argument++;
  }
  return undefined;
}

module.exports = { context, wordAt, declarations, hoverAt, completionCandidates, insertion, activeCall, memberContext };
