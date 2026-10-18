'use strict';
const { CASES, MODES, PREFIX, LIMIT } = require('../../editors/vscode-nagi/test/editor-stage-observation');
const COUNTS = ['tests', 'pass', 'fail', 'cancelled', 'skipped', 'todo'];
const EXITS = ['ok', 'timeout', 'buffer', 'signal', 'error'];
function expected(mode) {
  return mode === 'actor_isolated' ? ['actor_low'] : mode === 'stdlib_isolated' ? ['stdlib_high'] : ['actor_low', 'stdlib_high'];
}
function validRecord(record) {
  return record && Object.keys(record).sort().join(',') === 'case,elapsed_us,exit,stages,valid'
    && Object.hasOwn(CASES, record.case) && typeof record.valid === 'boolean'
    && Number.isSafeInteger(record.elapsed_us) && record.elapsed_us >= 0 && record.elapsed_us <= LIMIT
    && EXITS.includes(record.exit) && Array.isArray(record.stages) && record.stages.length <= 11
    && record.stages.every((value, index) => Number.isSafeInteger(value) && value >= 0 && value <= LIMIT
      && (index === 0 || value >= record.stages[index - 1]));
}
function validResult(result) {
  return result && Object.keys(result).sort().join(',') === 'cases,counts,mode,status'
    && MODES.includes(result.mode) && ['passed', 'failed', 'invalid'].includes(result.status)
    && result.counts && Object.keys(result.counts).sort().join(',') === [...COUNTS].sort().join(',')
    && COUNTS.every(key => Number.isSafeInteger(result.counts[key]) && result.counts[key] >= 0 && result.counts[key] <= 999999)
    && Array.isArray(result.cases) && result.cases.length <= 2
    && result.cases.every(item => {
      const { outcome, ...record } = item;
      return ['passed', 'failed', 'missing'].includes(outcome) && validRecord(record);
    });
}
function successfulResult(result) {
  if (!validResult(result) || result.status !== 'passed') return false;
  const wanted = expected(result.mode), counts = result.counts;
  return result.cases.length === wanted.length && new Set(result.cases.map(item => item.case)).size === wanted.length
    && result.cases.every(item => wanted.includes(item.case) && item.valid && item.outcome === 'passed'
      && item.exit === 'ok' && item.stages.length === 11)
    && counts.tests > 0 && counts.fail === 0 && counts.cancelled === 0 && counts.todo === 0
    && counts.tests === counts.pass + counts.skipped
    && (result.mode === 'parallel' ? counts.tests >= 2 && counts.skipped === 0 : counts.pass === 1);
}
async function collect(source, mode) {
  let valid = MODES.includes(mode);
  const wanted = expected(mode), observations = new Map(), outcomes = new Map(), counts = {};
  const pending = new Map();
  for await (const event of source) {
    const data = event.data || {};
    if (event.type === 'test:stdout') {
      // File identities remain internal; neither arbitrary output nor paths are
      // projected into the closed result. Bound partial lines and stream count.
      const key = data.file || '';
      if (!pending.has(key) && pending.size >= 128) { valid = false; continue; }
      let buffer = (pending.get(key) || '') + String(data.message || '');
      if (buffer.length > 4096) { valid = false; pending.set(key, ''); continue; }
      const lines = buffer.split('\n');
      pending.set(key, lines.pop());
      for (const raw of lines) {
        const line = raw.replace(/\r$/, '');
        if (!line.startsWith(PREFIX)) continue;
        let record;
        try { record = JSON.parse(line.slice(PREFIX.length)); } catch { valid = false; continue; }
        if (!validRecord(record) || !wanted.includes(record.case) || observations.has(record.case)) { valid = false; continue; }
        observations.set(record.case, record);
      }
    } else if (event.type === 'test:pass' || event.type === 'test:fail') {
      const id = Object.keys(CASES).find(key => CASES[key] === data.name);
      if (id && wanted.includes(id)) {
        if (outcomes.has(id)) valid = false;
        outcomes.set(id, event.type === 'test:pass' && !data.skip && !data.todo ? 'passed' : 'failed');
      }
    } else if (event.type === 'test:diagnostic' && data.nesting === 0) {
      const match = /^(tests|pass|fail|cancelled|skipped|todo) (0|[1-9][0-9]{0,5})$/.exec(String(data.message));
      if (match) {
        if (Object.hasOwn(counts, match[1])) valid = false;
        counts[match[1]] = Number(match[2]);
      }
    }
  }
  if ([...pending.values()].some(value => value.startsWith(PREFIX))) valid = false;
  valid &&= COUNTS.every(key => Object.hasOwn(counts, key)) && counts.tests > 0
    && counts.tests === counts.pass + counts.fail + counts.cancelled + counts.skipped + counts.todo
    && wanted.every(id => observations.has(id) && outcomes.has(id) && observations.get(id).valid);
  if (mode === 'parallel') valid &&= counts.tests >= 2;
  const passed = valid && counts.fail === 0 && counts.cancelled === 0 && counts.todo === 0
    && (mode === 'parallel' ? counts.skipped === 0 : counts.pass === wanted.length)
    && wanted.every(id => outcomes.get(id) === 'passed' && observations.get(id).exit === 'ok');
  return { mode: MODES.includes(mode) ? mode : 'invalid', status: !valid ? 'invalid' : passed ? 'passed' : 'failed',
    counts: Object.fromEntries(COUNTS.map(key => [key, counts[key] ?? 0])),
    cases: wanted.filter(id => observations.has(id)).map(id => ({ ...observations.get(id), outcome: outcomes.get(id) || 'missing' })) };
}
module.exports = async function* reporter(source) {
  const result = await collect(source, process.env.NAGI_EDITOR_OBSERVE_MODE);
  if (result.status !== 'passed') process.exitCode = 1;
  yield JSON.stringify(result) + '\n';
};
module.exports.collect = collect;
module.exports.validRecord = validRecord;
module.exports.validResult = validResult;
module.exports.successfulResult = successfulResult;
