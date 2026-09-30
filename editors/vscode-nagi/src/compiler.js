'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { execFile } = require('node:child_process');

function findRoot(file, workspace) {
  let dir = path.dirname(file);
  while (true) {
    if (fs.existsSync(path.join(dir, 'compiler', 'Cargo.toml')) &&
        fs.existsSync(path.join(dir, 'runtime', 'Cargo.toml'))) return dir;
    const parent = path.dirname(dir);
    if (parent === dir || (workspace && dir === workspace)) break;
    dir = parent;
  }
  return workspace || path.dirname(file);
}

function findProject(file) {
  let dir = path.dirname(file);
  while (true) {
    const manifest = path.join(dir, 'nagi.toml');
    if (fs.existsSync(manifest) && fs.statSync(manifest).isFile()) return manifest;
    const parent = path.dirname(dir);
    if (parent === dir) return undefined;
    dir = parent;
  }
}

function compilerPath(configured, root, workspace, platform = process.platform) {
  if (configured) return path.isAbsolute(configured) ? configured : path.resolve(workspace || root, configured);
  const name = platform === 'win32' ? 'nagic.exe' : 'nagic';
  for (const profile of ['release', 'debug']) {
    const candidate = path.join(root, 'target', profile, name);
    if (fs.existsSync(candidate)) return candidate;
  }
  return name;
}

function argumentsFor(command, file, nativeFiles, root, workspace, rustFile = '', rustDependencies = [], project) {
  const args = project ? [command, '--project', project] : [command, file];
  for (const native of nativeFiles) args.push('--native', path.resolve(workspace || root, native));
  if (rustFile) args.push('--rust', path.resolve(workspace || root, rustFile));
  for (const dependency of rustDependencies) args.push('--rust-dep', dependency);
  if (command === 'check') {
    const id = crypto.createHash('sha256').update(project || file).digest('hex').slice(0, 16);
    args.push('--out', path.join(root, 'build', 'vscode-nagi', id));
  }
  return args;
}

function normalizeFile(file, root) {
  if (file.startsWith('\\\\?\\UNC\\')) file = '\\\\' + file.slice(8);
  else if (file.startsWith('\\\\?\\')) file = file.slice(4);
  return path.resolve(root, file);
}

function parseDiagnostics(output, fallbackFile) {
  const lines = output.replace(/\x1b\[[0-9;]*m/g, '').split(/\r?\n/);
  const message = lines.find(s => /^error:/.test(s)) || lines.find(s => s.trim()) || 'Nagi check failed';
  const location = lines.map(s => s.match(/^\s*-->\s+(.+):(\d+)\s*$/)).find(Boolean);
  const line = location ? Number(location[2]) : Number((message.match(/\bline (\d+):/) || [0, 1])[1]);
  return [{ file: location ? location[1] : fallbackFile, line: Math.max(0, line - 1),
    message: message.replace(/^error:\s*/, '') }];
}

function runCheck(executable, args, cwd, timeout, callback, maxBuffer = 1024 * 1024, input) {
  const child = execFile(executable, args, { cwd, timeout, maxBuffer, windowsHide: true, shell: false },
    (error, stdout, stderr) => callback({ error, output: [stdout, stderr].filter(Boolean).join('\n') }));
  if (input !== undefined) {
    child.stdin.on('error', () => {}); // An early compiler failure is reported by execFile's callback.
    child.stdin.end(input);
  }
  return child;
}

function definitionAt(index, file, line, character, root) {
  if (index.format !== 'nagi-symbols-v1' || !Array.isArray(index.references)) throw new Error('Unsupported nagic symbols format');
  const key = name => { const normalized = normalizeFile(name, root); return process.platform === 'win32' ? normalized.toLowerCase() : normalized; };
  const valid = location => location && typeof location.file === 'string' &&
    Number.isInteger(location.line) && location.line > 0 && Number.isInteger(location.column) && location.column > 0 &&
    Number.isInteger(location.length) && location.length >= 0;
  for (const reference of index.references) {
    if (!valid(reference.location) || !valid(reference.target)) throw new Error('Invalid nagic symbol location');
    const location = reference.location;
    if (key(location.file) === key(file) && location.line === line + 1 &&
        character >= location.column - 1 && character < location.column - 1 + location.length) return reference.target;
  }
  return undefined;
}

module.exports = { findRoot, findProject, compilerPath, argumentsFor, parseDiagnostics, normalizeFile, runCheck, definitionAt };
