import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { stage, businessConfig, trustEnv, FAILURE_STAGES, checkedFailure } from './browser_stage.mjs';

test('text identity suffix is bounded static WebKit failure metadata only', () => {
  const digest = 'a'.repeat(64);
  for (const role of ['ca', 'host']) {
    const base = `engine_wrong_${role}_one_shot_console_malformed`;
    const value = `${base}_events_GRMNXJKO_0_text_${digest}`;
    assert.deepEqual(checkedFailure(value, 'webkit'), { schema_version: 1, stage: value, engine: 'webkit' });
    for (const engine of ['none', 'chromium', 'firefox']) assert.throws(() => checkedFailure(value, engine));
    for (const invalid of [value + '\n', value + '_extra', value.replace('GRMNXJKO', 'GRMNJKO'),
      value.replace('console_malformed', 'accepted'), value.replace('console_malformed', 'deadline'),
      value.replace(digest, 'A'.repeat(64)), value.replace(digest, 'a'.repeat(63)),
      value.replace(digest, 'PRIVATE_SENTINEL'), value.replace('GRMNXJKO_0', 'X_1')])
      assert.throws(() => checkedFailure(invalid, 'webkit'));
  }
});

test('bounded one-shot event suffix is a strict failure-only WebKit vocabulary', () => {
  for (const role of ['ca', 'host']) {
    const base = `engine_wrong_${role}_one_shot_deadline`;
    for (const trace of ['_0', 'GRMNSFWDJHO_0', 'GRMQNSFWDJHO_0', 'GRMNSFWXJKO_0', 'E'.repeat(32) + '_1']) {
      const stage = `${base}_events_${trace}`;
      assert.deepEqual(checkedFailure(stage, 'webkit'), { schema_version: 1, stage, engine: 'webkit' });
      for (const engine of ['none', 'chromium', 'firefox']) assert.throws(() => checkedFailure(stage, engine));
    }
    for (const trace of ['PRIVATE_SENTINEL_0', 'https://private.invalid/_0', 'E'.repeat(33) + '_0',
      'E_1', '_1', 'E_2', 'E_00', 'E_0\n', 'E_0_suffix'])
      assert.throws(() => checkedFailure(`${base}_events_${trace}`, 'webkit'));
    for (const invalid of [`engine_wrong_${role}_one_shot_accepted_events_E_0`,
      `engine_wrong_${role}_one_shot_peer_code_sha256_${'a'.repeat(64)}_events_E_0`,
      `engine_wrong_${role}_events_E_0`]) assert.throws(() => checkedFailure(invalid, 'webkit'));
  }
});
const IMAGE = 'sha256:bdfda2a8fdf48707c5e50a49bd3e06d8d0c759153dc8ee057d8a81c3450bb3e9';
const home = '/run/nagi-sf08/trust-unit/home';
const ca = Buffer.from('synthetic-public-main-ca');
function input() {
  return { work: '/run/nagi-sf08/run-unit', test_dir: '/owned/tests', node_modules: '/owned/node_modules',
    playwright_module: '/owned/playwright/index.mjs', playwright_cli: '/owned/playwright/cli.js',
    ready: { schema_version: 1, app: 'https://localhost:11001', ui: 'https://app.example.invalid:11002',
      denied: 'https://127.0.0.1:11003', ca: '/run/nagi-sf08/run-unit/owned-unit/ca.pem', control: '/run/nagi-sf08/run-unit/control.sock' },
    ca_pem: ca.toString('base64'), negative: { ca_origin: 'https://localhost:11004',
      host_origin: 'https://sf08-wronghost.invalid:11001', ca_pem: Buffer.from('synthetic-other-ca').toString('base64') },
    provisioned: { schema_version: 1, stores_provisioned: true, browser_trust_verified: false,
      home, firefox_policy: '/run/nagi-sf08/trust-unit/firefox-policy.json', ca: '/run/nagi-sf08/trust-unit/ca.pem',
      ca_sha256: createHash('sha256').update(ca).digest('hex'), declared_image: IMAGE } };
}
const receipt = () => ({ schema_version: 1, transport_preflight_passed: true,
  engines: ['chromium','firefox','webkit'].map(engine => ({ engine, positive_sans: 3, wrong_ca_refused: true, wrong_host_refused: true })) });
const devices = { 'Desktop Chrome': { browserName: 'chromium' }, 'Desktop Firefox': { browserName: 'firefox' },
  'Desktop Safari': { browserName: 'webkit', userAgent: 'unit-desktop-safari' } };
function fake(overrides = {}) {
  const calls = [];
  return { calls, deps: { browsers: { devices }, nativeNegative: async () => calls.push('native'),
    preflight: async (_b, provisioned) => { calls.push('preflight'); assert.equal(provisioned.home, home); return receipt(); },
    write: (_file, data) => { calls.push('write'); calls.config = JSON.parse(data.slice(15, -2)); },
    child: async (_node, cli, config, env) => { calls.push('business'); calls.child = { cli, config, env }; }, ...overrides } };
}
test('same trust context and original deadlines/devices pass from preflight to business', async () => {
  const f = fake(); await stage(input(), f.deps);
  assert.deepEqual([...f.calls], ['native','preflight','write','business']);
  assert.equal(f.calls.child.env.HOME, home);
  assert.equal(f.calls.child.env.XDG_DATA_HOME, home + '/.local/share');
  assert.equal(f.calls.child.env.SF08_CONTROL_SOCKET, input().ready.control);
  assert.equal(f.calls.child.env.SF08_OBSERVATION_FILE, input().work + '/observations.json');
  assert.equal(f.calls.child.env.SF08_PREFLIGHT_MODULE, new URL('./preflight.mjs', import.meta.url).href);
  const c = f.calls.config;
  assert.equal(c.timeout, 45000); assert.equal(c.retries, 0); assert.equal(c.workers, 1);
  assert.equal(c.use.ignoreHTTPSErrors, false); assert.equal(c.use.launchOptions.env.HOME, home);
  assert.equal(c.projects[1].use.launchOptions.env.HOME, home);
  assert.equal(c.projects[1].use.launchOptions.env.PLAYWRIGHT_FIREFOX_POLICIES_JSON, input().provisioned.firefox_policy);
  assert.equal(Object.hasOwn(c.projects[1].use.launchOptions, 'firefoxUserPrefs'), false);
  assert.equal(Object.hasOwn(c.projects[0].use, 'launchOptions'), false);
  assert.equal(Object.hasOwn(c.projects[2].use, 'launchOptions'), false);
  assert.equal(c.projects[2].use.userAgent, 'unit-desktop-safari');
  assert.deepEqual(c.reporter, [['json', { outputFile: input().work + '/browser-report.json' }]]);
});

test('Firefox project startup env overwrites caller input without changing parent or worker env', async () => {
  const key = 'PLAYWRIGHT_FIREFOX_POLICIES_JSON';
  const previous = process.env[key];
  const inherited = '/caller/untrusted-policies.json';
  process.env[key] = inherited;
  try {
    const f = fake();
    await stage(input(), f.deps);
    const c = f.calls.config;
    assert.equal(process.env[key], inherited);
    assert.equal(f.calls.child.env[key], inherited);
    assert.equal(c.use.launchOptions.env[key], inherited);
    assert.equal(c.projects[1].use.launchOptions.env[key], input().provisioned.firefox_policy);
    assert.equal(Object.hasOwn(c.projects[1].use.launchOptions, 'firefoxUserPrefs'), false);
    assert.equal(Object.hasOwn(c.projects[0].use, 'launchOptions'), false);
    assert.equal(Object.hasOwn(c.projects[2].use, 'launchOptions'), false);
    delete process.env[key];
    const noInherited = businessConfig({ home, policy: input().provisioned.firefox_policy,
      origins: ['https://localhost:11001'] }, '/run/nagi-sf08/run-unit', '/owned/tests', devices);
    assert.equal(Object.hasOwn(noInherited.use.launchOptions.env, key), false);
    assert.equal(noInherited.projects[1].use.launchOptions.env[key], input().provisioned.firefox_policy);
    assert.equal(Object.hasOwn(noInherited.projects[0].use, 'launchOptions'), false);
    assert.equal(Object.hasOwn(noInherited.projects[2].use, 'launchOptions'), false);
  } finally {
    if (previous === undefined) delete process.env[key];
    else process.env[key] = previous;
  }
});

test('business helper import is fixed adjacent bound source, overriding inherited/caller input', async () => {
  const previous = process.env.SF08_PREFLIGHT_MODULE;
  process.env.SF08_PREFLIGHT_MODULE = 'data:text/javascript,throw%20new%20Error()';
  try {
    const value = input(); value.preflight_module = 'file:///unowned/caller.mjs';
    const f = fake(); await stage(value, f.deps);
    assert.equal(f.calls.child.env.SF08_PREFLIGHT_MODULE, new URL('./preflight.mjs', import.meta.url).href);
  } finally {
    if (previous === undefined) delete process.env.SF08_PREFLIGHT_MODULE;
    else process.env.SF08_PREFLIGHT_MODULE = previous;
  }
});

test('business beforeAll awaits readiness on its supplied worker Browser before per-test reset', { timeout: 1000 }, async () => {
  const source = readFileSync(new URL('./business/security-foundation.browser.spec.ts', import.meta.url), 'utf8');
  const start = source.indexOf('test.beforeAll(');
  const end = source.indexOf('\n\nfunction expectManaged', start);
  assert.ok(start >= 0 && end > start && end < source.indexOf('test.beforeEach('));
  let hook;
  Function('test', 'process', 'appOrigin', source.slice(start, end))(
    { beforeAll(fn) { hook = fn; } },
    { env: { SF08_PREFLIGHT_MODULE: new URL('./preflight.mjs', import.meta.url).href } }, input().ready.app);
  const calls = [];
  let arrived, release;
  const entered = new Promise(resolve => { arrived = resolve; });
  const held = new Promise(resolve => { release = resolve; });
  const browser = { async newContext(options) {
    assert.equal(this, browser);
    assert.deepEqual(options, { ignoreHTTPSErrors: false, serviceWorkers: 'block' });
    calls.push('context');
    return { async newPage() { calls.push('page'); return {
      async goto(url) { assert.equal(url, input().ready.app + '/'); calls.push('goto'); arrived(); await held; return { status: () => 200 }; },
      async close() { calls.push('page-close'); },
    }; }, async close() { calls.push('context-close'); } };
  } };
  let completed = false;
  const operation = hook({ browser, browserName: 'firefox' }).then(() => { completed = true; });
  await entered;
  assert.equal(completed, false);
  release(); await operation;
  assert.deepEqual(calls, ['context', 'page', 'goto', 'page-close', 'context-close']);
  calls.length = 0;
  await hook({ browser, browserName: 'chromium' });
  assert.deepEqual(calls, []);
});
test('failed native wrong CA blocks preflight and business', async () => {
  const f = fake({ nativeNegative: async () => { throw new Error('unit'); } });
  await assert.rejects(stage(input(), f.deps)); assert.equal(f.calls.length, 0);
});
test('failed three-engine preflight never writes config or starts business', async () => {
  const f = fake({ preflight: async () => { throw new Error('unit'); } });
  await assert.rejects(stage(input(), f.deps)); assert.deepEqual([...f.calls], ['native']);
});
test('incomplete/malformed transport receipt never starts business', async () => {
  for (const bad of [{}, { ...receipt(), transport_preflight_passed: 1 }, { ...receipt(), engines: receipt().engines.slice(1) }]) {
    const f = fake({ preflight: async () => bad });
    await assert.rejects(stage(input(), f.deps)); assert.deepEqual([...f.calls], ['native']);
  }
});
test('business failure rejects stage and cannot yield preflight success receipt', async () => {
  const f = fake({ child: async () => { throw new Error('unit'); } });
  await assert.rejects(stage(input(), f.deps)); assert.deepEqual([...f.calls], ['native','preflight','write']);
});
test('same CA or wrong host/ports refuse before native/network/browser', async () => {
  for (const mutate of [x => x.negative.ca_pem = x.ca_pem, x => x.negative.host_origin = x.ready.app,
    x => x.negative.ca_origin = x.ready.app, x => x.provisioned.home = '/root']) {
    const value = input(); mutate(value); const f = fake();
    await assert.rejects(stage(value, f.deps)); assert.equal(f.calls.length, 0);
  }
});

test('failure checkpoint reports finite stage and engine without raw data', async () => {
  const { checkedFailure } = await import('./browser_stage.mjs');
  assert.deepEqual(checkedFailure('engine_wrong_host', 'webkit'), { schema_version: 1, stage: 'engine_wrong_host', engine: 'webkit' });
  for (const [stage, engine] of [['https://secret.invalid/token','webkit'],['business','firefox'],['engine_launch','none'],['input','secret-cookie']]) {
    assert.throws(() => checkedFailure(stage, engine), /^Error: SF08 checkpoint refused$/);
  }
  let last;
  const f = fake({ checkpoint: (stage, engine) => { last = checkedFailure(stage, engine); },
    preflight: async (_b, _p, _r, _n, checkpoint) => {
      checkpoint('engine_wrong_ca','firefox'); throw new Error('synthetic-raw-secret-not-printed');
    } });
  await assert.rejects(stage(input(), f.deps));
  assert.deepEqual(last, { schema_version:1, stage:'engine_wrong_ca', engine:'firefox' });
  assert.deepEqual([...f.calls], ['native']);
});

test('wrong-host diagnosis stays closed and limits WebKit detail labels to WebKit', async () => {
  const { checkedFailure } = await import('./browser_stage.mjs');
  for (const suffix of ['protocol_form', 'token_present', 'no_fixed_token', 'cancelled_present',
    'shape_non_error', 'shape_non_string', 'shape_oversize', 'shape_access_failed']) {
    const stage = 'engine_wrong_host_webkit_' + suffix;
    assert.equal(FAILURE_STAGES.includes(stage), true);
    assert.deepEqual(checkedFailure(stage, 'webkit'), { schema_version: 1, stage, engine: 'webkit' });
    for (const engine of ['none', 'chromium', 'firefox']) {
      assert.throws(() => checkedFailure(stage, engine), /^Error: SF08 checkpoint refused$/);
    }
  }
  for (const stage of ['engine_wrong_host_page_open', 'engine_wrong_host_page_close',
    'engine_wrong_host_response_received', 'engine_wrong_host_no_response',
    'engine_wrong_host_timeout', 'engine_wrong_host_unknown']) {
    assert.equal(FAILURE_STAGES.includes(stage), true);
    assert.deepEqual(checkedFailure(stage, 'webkit'), { schema_version: 1, stage, engine: 'webkit' });
  }
});


test('both one-shot factories are private dependency identities and never serialized into business', async () => {
  let hostCalls = 0, caCalls = 0;
  const hostFactory = () => { hostCalls++; throw new Error('private host key sentinel'); };
  const caFactory = () => { caCalls++; throw new Error('private CA key sentinel'); };
  const f = fake({ wrongHostFactory: hostFactory, wrongCAFactory: caFactory,
    preflight: async (_b, _p, _r, _n, _c, selectedHost, selectedCA) => {
    assert.equal(selectedHost, hostFactory); assert.equal(selectedCA, caFactory); return receipt();
  } });
  const value = input(); value.wrongHostFactory = 'caller-controlled-sentinel';
  value.wrongCAFactory = 'caller-controlled-CA-sentinel'; value.leaf_key = 'caller-key-sentinel';
  value.wrong_ca_key = 'caller-CA-key-sentinel';
  await stage(value, f.deps);
  assert.equal(hostCalls, 0, 'preflight owns the sole host attempt');
  assert.equal(caCalls, 0, 'preflight owns the sole CA attempt');
  assert.equal(JSON.stringify(f.calls.config).includes('sentinel'), false);
  assert.equal(JSON.stringify(f.calls.child.env).includes('caller-key-sentinel'), false);
  assert.equal(JSON.stringify(f.calls.child.env).includes('caller-CA-key-sentinel'), false);
});

test('default wrong-host factory reads bound work and ready CA and fails closed before business', async () => {
  let callerInvoked = false;
  const value = input(); value.wrongHostFactory = () => { callerInvoked = true; };
  const f = fake({ preflight: async (_b, _p, _r, _n, _c, factory) => {
    assert.equal(typeof factory, 'function');
    await factory(performance.now() + 10000);
    assert.fail('nonexistent owned credential directory must refuse');
  } });
  await assert.rejects(stage(value, f.deps), /^Error: SF08 browser trust preflight refused$/);
  assert.equal(callerInvoked, false);
  assert.deepEqual([...f.calls], ['native']);
});

test('default wrong-CA factory reads bound work and both CA roles and fails closed before business', async () => {
  let callerInvoked = false;
  const value = input(); value.wrongCAFactory = () => { callerInvoked = true; };
  const f = fake({ preflight: async (_b, _p, _r, _n, _c, _hostFactory, caFactory) => {
    assert.equal(typeof caFactory, 'function');
    const rows = [];
    await caFactory(performance.now() + 10000, value => rows.push(value));
    assert.deepEqual(rows, ['credential']);
    assert.fail('nonexistent owned wrong-CA credential directory must refuse');
  } });
  await assert.rejects(stage(value, f.deps), /^Error: SF08 browser trust preflight refused$/);
  assert.equal(callerInvoked, false);
  assert.deepEqual([...f.calls], ['native']);
});


test('one-shot checkpoint vocabulary is finite WebKit-only with no dynamic fields', () => {
  const categories = ['credential', 'setup', 'navigation', 'request', 'console_missing', 'console_malformed',
    'response', 'peer_legacy_certificate',
    'peer_connection_reset', 'peer_handshake_timeout', 'peer_no_native_code', 'peer_non_error',
    'peer_other_code', 'peer_other', 'peer_binding', 'socket', 'socket_eof', 'socket_close_unproved',
    'socket_error_reset_raw', 'socket_error_reset_tls', 'socket_error_other', 'socket_destroy_error',
    'lifecycle', 'deadline', 'join', 'cleanup', 'unknown'];
  const actual = FAILURE_STAGES.filter(stage => stage.startsWith('engine_wrong_host_one_shot_'));
  assert.deepEqual(actual, categories.map(category => 'engine_wrong_host_one_shot_' + category));
  for (const stage of actual) {
    assert.deepEqual(checkedFailure(stage, 'webkit'), { schema_version: 1, stage, engine: 'webkit' });
    for (const engine of ['none', 'chromium', 'firefox']) assert.throws(() => checkedFailure(stage, engine), /^Error: SF08 checkpoint refused$/);
  }
  for (const stage of ['engine_wrong_host_one_shot_PRIVATE_SENTINEL', 'engine_wrong_host_one_shot_peer_legacy_certificate_SUFFIX',
    'engine_wrong_host_one_shot_ERR_SSL_SSLV3_ALERT_BAD_CERTIFICATE', 'engine_wrong_host_one_shot_accepted'])
    assert.throws(() => checkedFailure(stage, 'webkit'), /^Error: SF08 checkpoint refused$/);
});

test('wrong-CA one-shot checkpoint vocabulary is finite WebKit-only with no dynamic fields', () => {
  const categories = ['credential', 'setup', 'navigation', 'request', 'console_missing', 'console_malformed',
    'response', 'peer_legacy_certificate',
    'peer_connection_reset', 'peer_handshake_timeout', 'peer_no_native_code', 'peer_non_error',
    'peer_other_code', 'peer_other', 'peer_binding', 'socket', 'socket_eof', 'socket_close_unproved',
    'socket_error_reset_raw', 'socket_error_reset_tls', 'socket_error_other', 'socket_destroy_error',
    'lifecycle', 'deadline', 'join', 'cleanup', 'unknown'];
  const actual = FAILURE_STAGES.filter(stage => stage.startsWith('engine_wrong_ca_one_shot_'));
  assert.deepEqual(actual, categories.map(category => 'engine_wrong_ca_one_shot_' + category));
  for (const stage of actual) {
    assert.deepEqual(checkedFailure(stage, 'webkit'), { schema_version: 1, stage, engine: 'webkit' });
    for (const engine of ['none', 'chromium', 'firefox']) assert.throws(() => checkedFailure(stage, engine), /^Error: SF08 checkpoint refused$/);
  }
  for (const stage of ['engine_wrong_ca_one_shot_PRIVATE_SENTINEL', 'engine_wrong_ca_one_shot_peer_other_SUFFIX',
    'engine_wrong_ca_one_shot_ERR_SSL_TLSV1_ALERT_UNKNOWN_CA', 'engine_wrong_ca_one_shot_accepted'])
    assert.throws(() => checkedFailure(stage, 'webkit'), /^Error: SF08 checkpoint refused$/);
});

test('credential failure diagnostic is fixed and callback error never changes stage refusal', async () => {
  for (const throwing of [false, true]) {
    const rows = [], f = fake({ preflight: async (_b, _p, _r, _n, _c, factory) => {
      await factory(performance.now() + 10000, value => { rows.push(value); if (throwing) throw new Error('PRIVATE_SENTINEL'); });
    } });
    await assert.rejects(stage(input(), f.deps), /^Error: SF08 browser trust preflight refused$/);
    assert.deepEqual(rows, ['credential']); assert.deepEqual([...f.calls], ['native']);
  }
});

test('native peer fingerprint checkpoints allow only two exact WebKit digest forms', () => {
  for (const role of ['ca', 'host']) {
    const prefix = `engine_wrong_${role}_one_shot_peer_code_sha256_`;
    const stage = prefix + 'a'.repeat(64);
    assert.deepEqual(checkedFailure(stage, 'webkit'), { schema_version: 1, stage, engine: 'webkit' });
    assert.equal(FAILURE_STAGES.includes(stage), false, 'static stage vocabulary stays unchanged');
    for (const engine of ['none', 'chromium', 'firefox'])
      assert.throws(() => checkedFailure(stage, engine), /^Error: SF08 checkpoint refused$/);
    for (const suffix of ['', 'a'.repeat(63), 'a'.repeat(65), 'A'.repeat(64), 'g'.repeat(64),
      'a'.repeat(64) + '\n', 'a'.repeat(64) + '\r', 'a'.repeat(64) + '\u2028', 'a'.repeat(64) + '\u2029', 'a'.repeat(64) + '_PRIVATE_SENTINEL', 'https://private.invalid/'])
      assert.throws(() => checkedFailure(prefix + suffix, 'webkit'), /^Error: SF08 checkpoint refused$/);
  }
  for (const role of ['ca_extra', 'host_extra', 'other'])
    assert.throws(() => checkedFailure(`engine_wrong_${role}_one_shot_peer_code_sha256_${'a'.repeat(64)}`, 'webkit'), /^Error: SF08 checkpoint refused$/);
});
