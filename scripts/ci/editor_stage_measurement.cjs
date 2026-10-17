'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { validResult, successfulResult } = require('./editor_stage_reporter.cjs');
const { CASES, MODES } = require('../../editors/vscode-nagi/test/editor-stage-observation');
const EXPECTED_FILES = Object.freeze(["actor-features.test.js", "actor-navigation.test.js", "borrowed-locals.test.js", "compiler.test.js", "enum-features.test.js", "enum-navigation.test.js", "features.test.js", "folding.test.js", "high-snippets.test.js", "import-navigation.test.js", "indentation.test.js", "inferred-types.test.js", "local-shadow.test.js", "low-snippets.test.js", "module-features.test.js", "navigation.test.js", "providers.test.js", "static-assistance.test.js", "stdlib-features.test.js", "stdlib-navigation.test.js", "usability-catalog.test.js", "usability-navigation.test.js"]);
function plans(root, readDirectory = fs.readdirSync) {
  const directory = 'editors/vscode-nagi/test';
  const names = readDirectory(path.join(root, directory)).filter(name => name.endsWith('.test.js')).sort();
  if (JSON.stringify(names) !== JSON.stringify(EXPECTED_FILES)) throw new Error('editor file inventory mismatch');
  const files = names.map(name => `${directory}/${name}`);
  const base = ['--test', '--test-reporter=./scripts/ci/editor_stage_reporter.cjs'];
  return [
    { mode: MODES[0], args: [...base, `--test-name-pattern=^${CASES.actor_low}$`, `${directory}/actor-navigation.test.js`] },
    { mode: MODES[1], args: [...base, `--test-name-pattern=^${CASES.stdlib_high}$`, `${directory}/stdlib-navigation.test.js`] },
    // Same glob plus trailing graph file as the Intel/package step; retain Node's default
    // process isolation/file concurrency. No test-concurrency override.
    { mode: MODES[2], args: [...base, ...files, 'scripts/test_graph_html_viewport.mjs'] },
  ];
}
function run(root, spawn = spawnSync, emit = console.log, readDirectory = fs.readdirSync) {
  const executionPlans = plans(root, readDirectory);
  const build = spawn('cargo', ['build', '--release', '--locked', '-p', 'nagic', '--bin', 'nagic'],
    { cwd: root, encoding: 'utf8', maxBuffer: 2 * 1024 * 1024,
      env: { ...process.env, RUSTFLAGS: `${process.env.RUSTFLAGS || ''} --cfg nagi_editor_stage_diag` } });
  const built = !build.error && build.status === 0 && !build.signal;
  emit(JSON.stringify({ stage: 'build', exit: built ? 'ok' : 'error' }));
  if (!built) return 1;
  let passed = true;
  for (const plan of executionPlans) {
    const result = spawn(process.execPath, plan.args, { cwd: root, encoding: 'utf8', maxBuffer: 65536,
      env: { ...process.env, NAGI_EDITOR_OBSERVE_MODE: plan.mode } });
    let record;
    try {
      const lines = String(result.stdout).trim().split('\n');
      if (lines.length !== 1) throw new Error('closed output');
      record = JSON.parse(lines[0]);
      if (!validResult(record) || record.mode !== plan.mode) throw new Error('closed result');
    } catch { record = { mode: plan.mode, status: 'invalid', counts: { tests: 0, pass: 0, fail: 0, cancelled: 0, skipped: 0, todo: 0 }, cases: [] }; }
    // The reporter is part of the source-bound trusted harness. Never publish
    // raw stdout/stderr, errors, signal strings, paths or exception messages.
    const ok = !result.error && result.status === 0 && !result.signal && successfulResult(record);
    if (!ok && record.status === 'passed') record.status = 'invalid';
    emit(JSON.stringify(record));
    passed &&= ok;
  }
  return passed ? 0 : 1;
}
if (require.main === module) {
  try { process.exitCode = run(path.resolve(__dirname, '../..')); }
  catch { console.log(JSON.stringify({ stage: 'harness', exit: 'error' })); process.exitCode = 1; }
}
module.exports = { plans, run, EXPECTED_FILES };
