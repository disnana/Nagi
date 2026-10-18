'use strict';
// Only the two existing navigation fixtures import this adapter. Ordinary
// extension execution continues to use src/compiler.js without an observer.
const { execFile } = require('node:child_process');
const MODES = Object.freeze(['actor_isolated', 'stdlib_isolated', 'parallel']);
const CASES = Object.freeze({
  actor_low: 'real low actor symbols separate modules and preserve three-parameter handles',
  stdlib_high: 'real nagi standard symbols keep resource identity, virtual sources and UTF-16 targets',
});
const PREFIX = 'NAGI_EDITOR_OBSERVATION ';
const LIMIT = 60000000;
function parseStages(stderr) {
  const stages = [];
  const rest = [];
  let valid = true;
  for (const line of String(stderr).split(/\r?\n/)) {
    if (line.startsWith('NAGI_EDITOR_STAGE')) {
      const match = /^NAGI_EDITOR_STAGE (0|[1-9][0-9]?) (0|[1-9][0-9]{0,7})$/.exec(line);
      if (!match) { valid = false; continue; }
      const stage = Number(match[1]), elapsed = Number(match[2]);
      if (stage !== stages.length || stage > 10 || elapsed > LIMIT || (stages.length && elapsed < stages.at(-1))) {
        valid = false;
      } else stages.push(elapsed);
    } else if (line) rest.push(line);
  }
  return { stages, valid, rest: rest.join('\n') };
}
function exitClass(error) {
  if (!error) return 'ok';
  if (error.code === 'ERR_CHILD_PROCESS_STDIO_MAXBUFFER') return 'buffer';
  if (error.killed && error.signal === 'SIGTERM') return 'timeout';
  return error.signal ? 'signal' : 'error';
}
function runCheck(id, compiler, executable, args, cwd, timeout, callback, maxBuffer, input, deps = {}) {
  if (process.env.NAGI_EDITOR_OBSERVE_MODE === undefined || id === undefined) {
    return compiler.runCheck(executable, args, cwd, timeout, callback, maxBuffer, input);
  }
  if (!MODES.includes(process.env.NAGI_EDITOR_OBSERVE_MODE) || !Object.hasOwn(CASES, id)
      || timeout !== 10000 || maxBuffer !== 16 * 1024 * 1024) throw new Error('invalid observation boundary');
  const now = deps.now || process.hrtime.bigint;
  const start = now();
  const child = (deps.execFile || execFile)(executable, args,
    { cwd, timeout, maxBuffer, windowsHide: true, shell: false,
      env: { ...process.env, NAGI_EDITOR_STAGE_DIAG: '1' } },
    (error, stdout, stderr) => {
      const parsed = parseStages(stderr);
      const exit = exitClass(error);
      const elapsed_us = Number((now() - start) / 1000n);
      const valid = parsed.valid && parsed.stages.length > 0 && (exit !== 'ok' || parsed.stages.length === 11)
        && Number.isSafeInteger(elapsed_us) && elapsed_us >= 0 && elapsed_us <= LIMIT;
      const record = { case: id, elapsed_us: Math.max(0, Math.min(LIMIT, elapsed_us)), exit, stages: parsed.stages, valid };
      (deps.emit || console.log)(PREFIX + JSON.stringify(record));
      // Preserve the original child error and JSON/assert semantics. Only the
      // opt-in fixed observer lines are removed from the old merged output.
      callback({ error, output: [stdout, parsed.rest].filter(Boolean).join('\n') });
    });
  if (input !== undefined) {
    child.stdin.on('error', () => {});
    child.stdin.end(input);
  }
  return child;
}
module.exports = { runCheck, parseStages, exitClass, CASES, MODES, PREFIX, LIMIT };
