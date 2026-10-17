'use strict';
// Small injected source/harness tests; this never launches Cargo or nagic.
const assert = require('node:assert/strict');
const observation = require('../../editors/vscode-nagi/test/editor-stage-observation');
const reporter = require('../ci/editor_stage_reporter.cjs');
const measurement = require('../ci/editor_stage_measurement.cjs');
const root = require('node:path').resolve(__dirname, '../..');
const stages = Array.from({ length: 11 }, (_, index) => index * 10);
const stageText = stages.map((value, index) => `NAGI_EDITOR_STAGE ${index} ${value}\n`).join('');
const record = (id = 'actor_low') => ({ case: id, elapsed_us: 1000, exit: 'ok', stages, valid: true });
const marker = value => observation.PREFIX + JSON.stringify(value) + '\n';
const event = (type, data) => ({ type, data });
function events(mode = 'actor_isolated') {
  const ids = mode === 'parallel' ? ['actor_low', 'stdlib_high'] : [mode === 'actor_isolated' ? 'actor_low' : 'stdlib_high'];
  const output = ids.flatMap(id => [event('test:stdout', { file: id, message: marker(record(id)) }),
    event('test:pass', { name: observation.CASES[id] })]);
  const counts = { tests: mode === 'parallel' ? 201 : 1, pass: mode === 'parallel' ? 201 : 1, fail: 0, cancelled: 0, skipped: 0, todo: 0 };
  return [...output, ...Object.entries(counts).map(([key, value]) => event('test:diagnostic', { nesting: 0, message: `${key} ${value}` }))];
}
async function* stream(values) { yield* values; }
async function collect(values, mode = 'actor_isolated') { return reporter.collect(stream(values), mode); }
function adapter({ error = null, stderr = stageText, stdout = '{"format":"ok"}' } = {}) {
  let options, ended, originalError, result;
  const output = [];
  const ticks = [0n, 1000000n];
  const deps = {
    now: () => ticks.shift(), emit: line => output.push(line),
    execFile: (executable, args, opts, callback) => {
      options = opts; originalError = error; callback(error, stdout, stderr);
      return { stdin: { on: (name, fn) => { assert.equal(name, 'error'); assert.equal(typeof fn, 'function'); }, end: value => { ended = value; } } };
    },
  };
  const parentFlag = process.env.NAGI_EDITOR_STAGE_DIAG;
  observation.runCheck('actor_low', {}, 'fake', ['symbols', 'main.low', '--editor-input'], 'private-cwd', 10000,
    value => { result = value; }, 16 * 1024 * 1024, '{"files":[]}', deps);
  assert.equal(process.env.NAGI_EDITOR_STAGE_DIAG, parentFlag);
  assert.equal(result.error, originalError);
  assert.equal(options.timeout, 10000); assert.equal(options.maxBuffer, 16 * 1024 * 1024);
  assert.equal(options.shell, false); assert.equal(options.windowsHide, true);
  assert.equal(options.env.NAGI_EDITOR_STAGE_DIAG, '1'); assert.equal(ended, '{"files":[]}');
  return { result, diagnostic: JSON.parse(output[0].slice(observation.PREFIX.length)) };
}
let count = 0;
async function check(fn) { await fn(); count++; }
async function main() {
  await check(() => assert.deepEqual(observation.parseStages(stageText), { stages, valid: true, rest: '' }));
  for (const bad of [
    'NAGI_EDITOR_STAGE 00 0\n', 'NAGI_EDITOR_STAGE 0 -1\n', 'NAGI_EDITOR_STAGE 0 60000001\n',
    'NAGI_EDITOR_STAGE 0 0\nNAGI_EDITOR_STAGE 0 0\n', 'NAGI_EDITOR_STAGE 0 0\nNAGI_EDITOR_STAGE 2 1\n',
    'NAGI_EDITOR_STAGE 0 2\nNAGI_EDITOR_STAGE 1 1\n', stageText + 'NAGI_EDITOR_STAGE 11 200\n',
  ]) await check(() => assert.equal(observation.parseStages(bad).valid, false));
  const previous = process.env.NAGI_EDITOR_OBSERVE_MODE;
  try {
    delete process.env.NAGI_EDITOR_OBSERVE_MODE;
    await check(() => {
      let called = 0; const args = ['binary', [], 'cwd', 10000, () => {}, 16 * 1024 * 1024, 'input'];
      const compiler = { runCheck: (...actual) => { assert.deepEqual(actual, args); called++; return 7; } };
      assert.equal(observation.runCheck('actor_low', compiler, ...args), 7); assert.equal(called, 1);
    });
    process.env.NAGI_EDITOR_OBSERVE_MODE = 'parallel';
    await check(() => { let calls = 0; observation.runCheck(undefined, { runCheck: () => calls++ }); assert.equal(calls, 1); });
    await check(() => { const value = adapter(); assert.equal(value.result.output, '{"format":"ok"}'); assert.equal(value.diagnostic.valid, true); });
    await check(() => { const value = adapter({ stderr: '' }); assert.equal(value.diagnostic.valid, false); });
    await check(() => { const value = adapter({ stderr: stageText.slice(0, stageText.indexOf('NAGI_EDITOR_STAGE 3')), error: { killed: true, signal: 'SIGTERM' } }); assert.equal(value.diagnostic.exit, 'timeout'); assert.equal(value.diagnostic.valid, true); assert.equal(value.diagnostic.stages.length, 3); });
    await check(() => { const value = adapter({ stderr: 'PRIVATE_RAW_SECRET\n' }); assert.equal(value.result.output, '{"format":"ok"}\nPRIVATE_RAW_SECRET'); assert.equal(JSON.stringify(value.diagnostic).includes('PRIVATE_RAW_SECRET'), false); });
    await check(() => assert.equal(adapter({ error: { code: 'ERR_CHILD_PROCESS_STDIO_MAXBUFFER', killed: true, signal: 'SIGTERM' } }).diagnostic.exit, 'buffer'));
    await check(() => assert.throws(() => observation.runCheck('actor_low', {}, '', [], '', 10001, () => {}, 16 * 1024 * 1024, '')));
    process.env.NAGI_EDITOR_OBSERVE_MODE = 'unknown';
    await check(() => assert.throws(() => observation.runCheck('actor_low', {}, '', [], '', 10000, () => {}, 16 * 1024 * 1024, '')));
  } finally {
    if (previous === undefined) delete process.env.NAGI_EDITOR_OBSERVE_MODE;
    else process.env.NAGI_EDITOR_OBSERVE_MODE = previous;
  }
  for (const mode of observation.MODES) await check(async () => assert.equal((await collect(events(mode), mode)).status, 'passed'));
  await check(async () => assert.equal((await collect([])).status, 'invalid'));
  await check(async () => assert.equal((await collect(events().filter(item => item.type !== 'test:pass'))).status, 'invalid'));
  await check(async () => assert.equal((await collect([...events(), events()[0]])).status, 'invalid'));
  await check(async () => assert.equal((await collect([...events(), events()[1]])).status, 'invalid'));
  await check(async () => { const input = events(); input[1].data.skip = true; assert.equal((await collect(input)).status, 'failed'); });
  await check(async () => { const input = events(); input[1].type = 'test:fail'; input[1].data.details = { error: 'PRIVATE_RAW_SECRET' }; const output = await collect(input); assert.equal(output.status, 'failed'); assert.equal(JSON.stringify(output).includes('PRIVATE_RAW_SECRET'), false); });
  await check(async () => { const input = events(); input[0].data.message = marker({ ...record(), exit: 'timeout', stages: stages.slice(0, 3) }); input[1].type = 'test:fail'; assert.equal((await collect(input)).status, 'failed'); });
  await check(async () => { const input = events(); input[0].data.message = marker({ ...record(), path: 'PRIVATE_RAW_SECRET' }); const output = await collect(input); assert.equal(output.status, 'invalid'); assert.equal(JSON.stringify(output).includes('PRIVATE_RAW_SECRET'), false); });
  await check(async () => { const input = events(); input[0].data.message = marker(record('stdlib_high')); assert.equal((await collect(input)).status, 'invalid'); });
  await check(async () => { const input = events(); input[0].data.message = observation.PREFIX + '{invalid}\n'; assert.equal((await collect(input)).status, 'invalid'); });
  await check(async () => { const input = events(); input[0].data.message = observation.PREFIX + 'x'.repeat(5000); assert.equal((await collect(input)).status, 'invalid'); });
  await check(async () => { const input = events(); input.push(event('test:stderr', { message: 'PRIVATE_RAW_SECRET /private/path' })); assert.equal(JSON.stringify(await collect(input)).includes('PRIVATE_RAW_SECRET'), false); });
  await check(async () => { const input = events(); const first = input.shift(); const message = first.data.message; input.unshift(event('test:stdout', { file: 'x', message: message.slice(0, 10) }), event('test:stdout', { file: 'x', message: message.slice(10) })); assert.equal((await collect(input)).status, 'passed'); });
  await check(async () => assert.equal((await collect(events(), 'unrecognized')).status, 'invalid'));
  await check(async () => { const input = events('parallel'); input.find(item => item.data.message === 'tests 201').data.message = 'tests 200'; assert.equal((await collect(input, 'parallel')).status, 'invalid'); });
  await check(async () => { const input = events(); input.push(event('test:diagnostic', { nesting: 0, message: 'tests 1' })); assert.equal((await collect(input)).status, 'invalid'); });
  await check(async () => { const input = events(); input.splice(2); assert.equal((await collect(input)).status, 'invalid'); });
  await check(() => {
    const plans = measurement.plans(root, () => measurement.EXPECTED_FILES);
    assert.equal(plans.length, 3);
    assert.deepEqual(plans.map(item => item.mode), observation.MODES);
    assert.equal(plans[0].args.at(-1), 'editors/vscode-nagi/test/actor-navigation.test.js');
    assert.equal(plans[1].args.at(-1), 'editors/vscode-nagi/test/stdlib-navigation.test.js');
    assert.equal(plans[2].args.slice(2).length, 23);
    assert.equal(plans[2].args.at(-1), 'scripts/test_graph_html_viewport.mjs');
    assert.ok(plans.every(item => !item.args.some(value => value.startsWith('--test-concurrency'))));
  });
  await check(() => assert.throws(() => measurement.plans(root, () => [])));
  await check(() => assert.throws(() => measurement.plans(root, () => measurement.EXPECTED_FILES.slice(1))));
  await check(() => assert.throws(() => measurement.plans(root, () => [...measurement.EXPECTED_FILES, 'extra.test.js'])));
  await check(() => assert.throws(() => measurement.plans(root, () => [...measurement.EXPECTED_FILES, measurement.EXPECTED_FILES[0]])));
  const results = await Promise.all(observation.MODES.map(mode => collect(events(mode), mode)));
  function fakeRun(mutate = value => value) {
    let calls = 0; const published = [];
    const code = measurement.run(root, (executable, args, options) => {
      const index = calls++;
      if (index === 0) { assert.equal(executable, 'cargo'); assert.deepEqual(args, ['build', '--release', '--locked', '-p', 'nagic', '--bin', 'nagic']); assert.match(options.env.RUSTFLAGS, /--cfg nagi_editor_stage_diag/); return mutate({ status: 0, stdout: 'PRIVATE_RAW_BUILD', stderr: 'PRIVATE_RAW_BUILD' }, index); }
      assert.equal(options.env.NAGI_EDITOR_OBSERVE_MODE, observation.MODES[index - 1]);
      return mutate({ status: 0, stdout: JSON.stringify(results[index - 1]), stderr: 'PRIVATE_RAW_STDERR' }, index);
    }, line => published.push(line), () => measurement.EXPECTED_FILES);
    assert.equal(published.join('').includes('PRIVATE_RAW'), false);
    return { code, calls, published };
  }
  await check(() => { const result = fakeRun(); assert.equal(result.code, 0); assert.equal(result.calls, 4); });
  await check(() => { const result = fakeRun((value, index) => index === 0 ? { ...value, error: { code: 'EPERM' } } : value); assert.equal(result.code, 1); assert.equal(result.calls, 1); });
  await check(() => { const result = fakeRun((value, index) => index === 1 ? { ...value, error: { code: 'EPERM' } } : value); assert.equal(result.code, 1); assert.equal(result.calls, 4); });
  await check(() => { const result = fakeRun((value, index) => index === 1 ? { ...value, status: 1 } : value); assert.equal(result.code, 1); assert.equal(result.calls, 4); });
  await check(() => { const result = fakeRun((value, index) => index === 1 ? { ...value, stdout: 'PRIVATE_RAW_STDOUT' } : value); assert.equal(result.code, 1); assert.equal(result.calls, 4); });
  await check(() => { const result = fakeRun((value, index) => index === 1 ? { ...value, stdout: JSON.stringify({ ...results[0], cases: [{ ...results[0].cases[0], path: 'PRIVATE_RAW_PATH' }] }) } : value); assert.equal(result.code, 1); });
  await check(() => { const result = fakeRun((value, index) => index === 1 ? { ...value, stdout: JSON.stringify({ ...results[0], cases: [] }) } : value); assert.equal(result.code, 1); });
  await check(() => { const result = fakeRun((value, index) => index === 1 ? { ...value, stdout: JSON.stringify({ ...results[0], counts: { ...results[0].counts, tests: 0, pass: 0 } }) } : value); assert.equal(result.code, 1); });
  console.log(JSON.stringify({ source_fake_pass: count, native_cases_executed: 0 }));
}
main().catch(() => { console.log(JSON.stringify({ source_fake_failed: true, native_cases_executed: 0 })); process.exitCode = 1; });
