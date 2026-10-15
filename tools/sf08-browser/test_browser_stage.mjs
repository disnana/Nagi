import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { stage, businessConfig, trustEnv } from './browser_stage.mjs';
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
  const c = f.calls.config;
  assert.equal(c.timeout, 45000); assert.equal(c.retries, 0); assert.equal(c.workers, 1);
  assert.equal(c.use.ignoreHTTPSErrors, false); assert.equal(c.use.launchOptions.env.HOME, home);
  assert.equal(c.projects[1].use.launchOptions.env.HOME, home);
  assert.equal(c.projects[1].use.launchOptions.firefoxUserPrefs['browser.policies.alternatePath'], input().provisioned.firefox_policy);
  assert.equal(c.projects[2].use.userAgent, 'unit-desktop-safari');
  assert.deepEqual(c.reporter, [['json', { outputFile: input().work + '/browser-report.json' }]]);
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
