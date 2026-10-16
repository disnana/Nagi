import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { settings, probeEngine, firefoxCAReadiness, firefoxWorkerReadiness } from './preflight.mjs';
import { checkedFailure, FAILURE_STAGES } from './browser_stage.mjs';

const CA = Buffer.from('unit-ca-only-not-a-certificate');
const provisioned = { schema_version: 1, stores_provisioned: true, browser_trust_verified: false,
  home: '/run/nagi-sf08/trust-unit/home', ca: '/run/nagi-sf08/trust-unit/ca.pem',
  caPEM: CA, ca_sha256: createHash('sha256').update(CA).digest('hex'),
  declared_image: 'sha256:bdfda2a8fdf48707c5e50a49bd3e06d8d0c759153dc8ee057d8a81c3450bb3e9',
  firefox_policy: '/run/nagi-sf08/trust-unit/firefox-policy.json' };
const ready = { schema_version: 1, app: 'https://localhost:33001', ui: 'https://app.example.invalid:33002',
  denied: 'https://127.0.0.1:33003', ca: '/run/nagi-sf08/fixture/owned-ca/ca.pem' };
const negative = { caOrigin: 'https://localhost:33004', hostOrigin: 'https://sf08-wronghost.invalid:33001',
  caPEM: Buffer.from('unit-wrong-ca-not-a-certificate') };
const messages = { chromium: ['net::ERR_CERT_AUTHORITY_INVALID', 'net::ERR_CERT_COMMON_NAME_INVALID'],
  firefox: ['SEC_ERROR_UNKNOWN_ISSUER', 'SSL_ERROR_BAD_CERT_DOMAIN'],
  webkit: ['Unacceptable TLS certificate', 'Unacceptable TLS certificate'] };
const versions = { chromium: '156.0.8078.4', firefox: '157.0', webkit: '27.2' };

function fake(name, mode, fault) {
  const trace = { urls: [], timeouts: [], closed: false, pageClosed: 0, contextClosed: false };
  const config = fault?.config ?? settings(provisioned, ready, negative);
  return { trace, async launch(options) {
    trace.launch = options;
    return { version: () => mode === 'version' ? 'unexpected' : versions[name],
      async close() { trace.closed = true; },
      async newContext(options) {
        trace.context = options;
        return { async close() { trace.contextClosed = true; }, async newPage() {
          return { async close() { trace.pageClosed++; }, async goto(url, options) {
            trace.urls.push(url);
            trace.timeouts.push(options.timeout);
            if (url.startsWith(config.negative.ca) || url.startsWith(config.negative.host)) {
              const reason = url.startsWith(config.negative.ca) ? 'ca' : 'host';
              const token = messages[name][reason === 'ca' ? 0 : 1];
              if (mode === 'accepted') return { status: () => 200 };
              if (mode === 'network') throw new Error('net::ERR_CONNECTION_REFUSED PRIVATE_SENTINEL');
              if (fault && fault.reason === reason) {
                if (fault.kind === 'bad-signature') throw new Error('page.goto: SEC_ERROR_BAD_SIGNATURE at ' + url);
                if (fault.kind === 'hostname') throw new Error('page.goto: ' + messages[name][1] + ' at ' + url);
                if (fault.kind === 'other-tls') throw new Error('page.goto: SEC_ERROR_UNTRUSTED_ISSUER at ' + url);
                if (fault.kind === 'timeout') {
                  const error = new Error('PRIVATE_SENTINEL https://private.invalid/secret?token=unit-secret');
                  error.name = 'TimeoutError';
                  throw error;
                }
                if (fault.kind === 'no-response') return null;
                if (fault.kind === 'unknown') throw new Error('PRIVATE_SENTINEL https://private.invalid/secret?token=unit-secret');
                if (fault.kind === 'extra-code-text') throw new Error('page.goto: SEC_ERROR_BAD_SIGNATURE trailing annotation');
                if (fault.kind === 'url-marker') throw new Error('page.goto: net::ERR_CONNECTION_REFUSED at ' + url);
                if (fault.kind === 'calllog-marker') throw new Error('Navigation failed\nCall log:\n  - navigating to ' + url + ' with marker ' + token);
                if (fault.kind === 'extra-text') throw new Error(token + ' trailing annotation');
                if (fault.kind === 'oversized') throw new Error(token + 'x'.repeat(8193));
                if (fault.kind === 'non-error') throw { message: token };
              }
              if (mode === 'form:bare') throw new Error(token);
              if (mode === 'form:prefix') throw new Error('page.goto: ' + token);
              throw new Error('page.goto: ' + token + ' at ' + url);
            }
            if (fault?.positiveStep) return fault.positiveStep(url, options);
            if (fault?.positive) {
              if (fault.positive === 'ca') throw new Error('page.goto: ' + messages[name][0] + ' at ' + url);
              if (fault.positive === 'host') throw new Error('page.goto: ' + messages[name][1] + ' at ' + url);
              if (fault.positive === 'extra-tls-text') throw new Error('page.goto: ' + messages[name][0] + ' trailing annotation');
              if (fault.positive === 'timeout') {
                const error = new Error('PRIVATE_SENTINEL https://private.invalid/secret');
                error.name = 'TimeoutError';
                throw error;
              }
              if (fault.positive === 'missing-response') return null;
              if (fault.positive === 'bad-status') return { status: () => 503 };
              if (fault.positive === 'status-error') return { status() {
                throw new Error('page.goto: ' + messages[name][0] + ' at ' + url);
              } };
              if (fault.positive === 'unknown') throw new Error('PRIVATE_SENTINEL https://private.invalid/secret');
            }
            return { status: () => mode === 'status' ? 503 : 200 };
          } };
        } };
      } };
  } };
}

test('strict endpoint/profile settings and CA hash binding', () => {
  assert.equal(settings(provisioned, ready, negative).origins.length, 3);
  for (const value of [{ ...provisioned, browser_trust_verified: true }, { ...provisioned, home: '/root' },
    { ...provisioned, ca_sha256: '0'.repeat(64) }, { ...provisioned, caPEM: Buffer.alloc(16385) },
    { ...provisioned, firefox_policy: '/etc/policies.json' }]) {
    assert.throws(() => settings(value, ready, negative), /^Error: SF08 browser trust preflight refused$/);
  }
  for (const value of [{ ...ready, app: 'http://localhost:33001' }, { ...ready, ui: ready.app },
    { ...ready, app: 'https://user@localhost:33001' }, { ...ready, app: ready.app + '/' },
    { ...ready, ca: '/run/nagi-sf08/../private/ca.pem' }]) assert.throws(() => settings(provisioned, value, negative));
  for (const value of [{ ...negative, caPEM: CA }, { ...negative, caOrigin: ready.app },
    { ...negative, hostOrigin: ready.app }, { ...negative, hostOrigin: 'https://sf08-wronghost.invalid:33002' }]) {
    assert.throws(() => settings(provisioned, ready, value));
  }
});

function clock() {
  const state = { ms: 0, delays: [] };
  state.timing = { now: () => state.ms, delay: async ms => { state.delays.push(ms); state.ms += ms; } };
  return state;
}
function issuer(form, url) {
  const token = 'SEC_ERROR_UNKNOWN_ISSUER';
  return new Error(form === 'bare' ? token : form === 'prefix' ? 'page.goto: ' + token : 'page.goto: ' + token + ' at ' + url);
}
function worker(step, closePage = () => {}) {
  const trace = { calls: [], pageClosed: 0, contextClosed: 0, browserClosed: 0 };
  const browser = { async newContext(options) {
    trace.options = options;
    return { async newPage() { return {
      async goto(url, options) { trace.calls.push({ url, options }); return step(url, options); },
      async close() { trace.pageClosed++; closePage(); },
    }; }, async close() { trace.contextClosed++; } };
  }, async close() { trace.browserClosed++; } };
  return { browser, trace };
}

test('Firefox initial CA readiness accepts only fixed issuer forms and never repeats a response', async () => {
  for (const form of ['bare', 'prefix', 'at-url']) {
    const time = clock(); let attempts = 0;
    const type = fake('firefox', undefined, { positiveStep: url => {
      if (url === ready.app + '/' && ++attempts <= 2) throw issuer(form, url);
      return { status: () => 200 };
    } });
    const result = await probeEngine('firefox', type, settings(provisioned, ready, negative), () => {}, time.timing);
    assert.equal(result.positive_sans, 3);
    assert.equal(attempts, 3);
    assert.deepEqual(type.trace.urls, [ready.app + '/', ready.app + '/', ready.app + '/', ready.ui + '/', ready.denied + '/', negative.caOrigin + '/', negative.hostOrigin + '/']);
    assert.deepEqual(type.trace.timeouts.slice(0, 3), [10000, 9500, 9000]);
    assert.deepEqual(time.delays, [500, 500]);
    assert.equal(type.trace.pageClosed, 5);
    assert.equal(type.trace.contextClosed && type.trace.closed, true);
  }
});

test('Firefox unknown issuer forever fails after at most 20 attempts within original 10s', async () => {
  const time = clock(); const events = [];
  const type = fake('firefox', undefined, { positive: 'ca' });
  await assert.rejects(probeEngine('firefox', type, settings(provisioned, ready, negative),
    (stage, engine) => events.push({ stage, engine }), time.timing), /^Error: SF08 browser trust preflight refused$/);
  assert.equal(type.trace.urls.length, 20);
  assert.equal(time.delays.length, 19);
  assert.equal(time.ms, 9500);
  assert.equal(type.trace.timeouts[19], 500);
  assert.deepEqual(events.at(-1), { stage: 'engine_positive_ca', engine: 'firefox' });
  assert.equal(type.trace.pageClosed, 1);
  assert.equal(type.trace.contextClosed && type.trace.closed, true);
});

test('hostname, timeout, cancellation, network and non-exact issuer failures are never retried', async () => {
  const errors = [new Error('SSL_ERROR_BAD_CERT_DOMAIN'), new Error('net::ERR_CONNECTION_REFUSED'),
    new Error('SEC_ERROR_UNKNOWN_ISSUER trailing annotation'), new Error('Navigation failed\nSEC_ERROR_UNKNOWN_ISSUER'),
    new TypeError('SEC_ERROR_UNKNOWN_ISSUER'), { message: 'SEC_ERROR_UNKNOWN_ISSUER' }];
  for (const name of ['TimeoutError', 'AbortError']) {
    const error = new Error('SEC_ERROR_UNKNOWN_ISSUER'); error.name = name; errors.push(error);
  }
  for (const error of errors) {
    const time = clock(); const owned = worker(() => { throw error; });
    await assert.rejects(firefoxWorkerReadiness(owned.browser, ready.app, time.timing));
    assert.equal(owned.trace.calls.length, 1);
    assert.equal(time.delays.length, 0);
    assert.equal(owned.trace.pageClosed, 1);
    assert.equal(owned.trace.contextClosed, 1);
  }
});

test('missing, non-200 and throwing status never retry after any HTTP response', async () => {
  for (const response of [null, { status: () => 503 }, { status() { throw issuer('bare'); } }]) {
    const time = clock(); const owned = worker(() => response);
    await assert.rejects(firefoxWorkerReadiness(owned.browser, ready.app, time.timing));
    assert.equal(owned.trace.calls.length, 1);
    assert.equal(time.delays.length, 0);
    assert.equal(owned.trace.contextClosed, 1);
  }
});

test('total deadline does not reset and late HTTP success fails without replay', async () => {
  const time = clock(); let attempts = 0;
  const owned = worker((url, options) => {
    attempts++;
    if (attempts === 1) { time.ms += 8000; throw issuer('at-url', url); }
    assert.equal(options.timeout, 1500);
    time.ms += 1500;
    return { status: () => 200 };
  });
  await assert.rejects(firefoxWorkerReadiness(owned.browser, ready.app, time.timing), error => error.name === 'TimeoutError');
  assert.equal(owned.trace.calls.length, 2);
  assert.deepEqual(time.delays, [500]);
  assert.equal(time.ms, 10000);
  assert.equal(owned.trace.pageClosed, 1);
  assert.equal(owned.trace.contextClosed, 1);
});

test('backoff stall or cancellation never starts another navigation and always closes context', async () => {
  for (const cancel of [false, true]) {
    const time = clock();
    time.timing.delay = async ms => {
      time.delays.push(ms);
      if (cancel) { const error = new Error('unit cancellation'); error.name = 'AbortError'; throw error; }
      time.ms += 10001;
    };
    const owned = worker(url => { throw issuer('at-url', url); });
    await assert.rejects(firefoxWorkerReadiness(owned.browser, ready.app, time.timing));
    assert.equal(owned.trace.calls.length, 1);
    assert.equal(time.delays.length, 1);
    assert.equal(owned.trace.pageClosed, 1);
    assert.equal(owned.trace.contextClosed, 1);
  }
});

test('only Firefox first positive uses readiness; other SANs and engines remain immediate', async () => {
  for (const name of Object.keys(versions)) {
    const time = clock();
    const type = fake(name, undefined, { positiveStep: url => {
      if (name !== 'firefox' || url === ready.ui + '/') throw new Error(messages[name][0]);
      return { status: () => 200 };
    } });
    await assert.rejects(probeEngine(name, type, settings(provisioned, ready, negative), () => {}, time.timing));
    assert.equal(type.trace.urls.length, name === 'firefox' ? 2 : 1);
    assert.equal(time.delays.length, 0);
    assert.equal(type.trace.contextClosed && type.trace.closed, true);
  }
});

test('business readiness uses its actual worker Browser and fresh strict context, never closes Browser', async () => {
  const time = clock(); let attempts = 0;
  const owned = worker(url => { if (++attempts === 1) throw issuer('bare', url); return { status: () => 200 }; });
  await firefoxWorkerReadiness(owned.browser, ready.app, time.timing);
  assert.equal(attempts, 2);
  assert.deepEqual(owned.trace.options, { ignoreHTTPSErrors: false, serviceWorkers: 'block' });
  assert.ok(owned.trace.calls.every(call => call.url === ready.app + '/' && !Object.hasOwn(call.options, 'postData')));
  assert.equal(owned.trace.pageClosed, 1);
  assert.equal(owned.trace.contextClosed, 1);
  assert.equal(owned.trace.browserClosed, 0);
});

test('business readiness closes context if page creation or page close fails', async () => {
  const owned = worker(() => ({ status: () => 200 }), () => { throw new Error('unit close'); });
  await assert.rejects(firefoxWorkerReadiness(owned.browser, ready.app, clock().timing));
  assert.equal(owned.trace.contextClosed, 1);
  let contextClosed = 0;
  const browser = { async newContext() { return { async newPage() { throw new Error('unit create'); },
    async close() { contextClosed++; } }; } };
  await assert.rejects(firefoxWorkerReadiness(browser, ready.app, clock().timing));
  assert.equal(contextClosed, 1);
});

test('readiness URL is only owned localhost HTTPS root without credentials/query/fragment', async () => {
  for (const url of [ready.ui + '/', ready.app + '/login', ready.app + '/?token=unit', ready.app + '/#unit',
    'https://user@localhost:33001/', 'http://localhost:33001/', 'https://localhost:443/']) {
    let calls = 0;
    await assert.rejects(firefoxCAReadiness({ async goto() { calls++; } }, url, clock().timing));
    assert.equal(calls, 0);
  }
});

for (const name of Object.keys(versions)) {
  test(`${name}: three SAN positives plus two actual-error-class negatives`, async () => {
    const type = fake(name);
    const result = await probeEngine(name, type, settings(provisioned, ready, negative));
    assert.deepEqual(result, { engine: name, positive_sans: 3, wrong_ca_refused: true, wrong_host_refused: true });
    assert.equal(type.trace.urls.length, 5);
    assert.equal(type.trace.pageClosed, 5);
    assert.equal(type.trace.context.ignoreHTTPSErrors, false);
    assert.equal(type.trace.closed && type.trace.contextClosed, true);
    assert.equal(type.trace.launch.env.HOME, provisioned.home);
    assert.equal(type.trace.launch.env.XDG_DATA_HOME, provisioned.home + '/.local/share');
    assert.equal(type.trace.launch.env.PLAYWRIGHT_FIREFOX_POLICIES_JSON,
      name === 'firefox' ? provisioned.firefox_policy : process.env.PLAYWRIGHT_FIREFOX_POLICIES_JSON);
    assert.equal(Object.hasOwn(type.trace.launch, 'firefoxUserPrefs'), false);
    assert.equal(type.trace.launch.args, undefined);
  });
  test(`${name}: accepted negatives/network errors/version mismatch fail closed and close`, async () => {
    for (const mode of ['accepted', 'network', 'version', 'status']) {
      const type = fake(name, mode);
      await assert.rejects(probeEngine(name, type, settings(provisioned, ready, negative)),
        /^Error: SF08 browser trust preflight refused$/);
      assert.equal(type.trace.closed, true);
    }
  });
}


test('startup policy path is Firefox-child-only and overrides inherited value without mutating parent', async () => {
  const key = 'PLAYWRIGHT_FIREFOX_POLICIES_JSON';
  const previous = process.env[key];
  const inherited = '/caller/untrusted-policies.json';
  process.env[key] = inherited;
  try {
    const config = settings(provisioned, ready, negative);
    for (const name of Object.keys(versions)) {
      const type = fake(name);
      await probeEngine(name, type, config);
      assert.equal(type.trace.launch.env[key], name === 'firefox' ? config.policy : inherited);
      assert.equal(Object.hasOwn(type.trace.launch, 'firefoxUserPrefs'), false);
      assert.equal(process.env[key], inherited);
    }
  } finally {
    if (previous === undefined) delete process.env[key];
    else process.env[key] = previous;
  }
});

test('all fixed TLS error grammars remain accepted by fake engines', async () => {
  for (const name of Object.keys(versions)) {
    for (const form of ['bare', 'prefix', 'at-url']) {
      const type = fake(name, 'form:' + form);
      const result = await probeEngine(name, type, settings(provisioned, ready, negative));
      assert.equal(result.engine, name);
      assert.equal(type.trace.closed && type.trace.contextClosed, true);
    }
  }
});

test('wrong-CA failure output uses finite codes and preserves only the original exact rejection as pass', async () => {
  const stages = ['engine_wrong_ca_response_received', 'engine_wrong_ca_bad_signature',
    'engine_wrong_ca_hostname', 'engine_wrong_ca_timeout', 'engine_wrong_ca_no_response',
    'engine_wrong_ca_other_tls', 'engine_wrong_ca_unknown'];
  for (const stage of stages) {
    assert.ok(FAILURE_STAGES.includes(stage), stage);
    assert.deepEqual(checkedFailure(stage, 'firefox'), { schema_version: 1, stage, engine: 'firefox' });
  }
  assert.throws(() => checkedFailure('engine_wrong_ca_PRIVATE_SECRET', 'firefox'),
    /^Error: SF08 checkpoint refused$/);
  for (const [stage, engine] of [
    ['engine_wrong_ca_bad_signature', 'chromium'],
    ['engine_wrong_ca_other_tls', 'webkit'],
    ['engine_wrong_ca_hostname', 'webkit'],
  ]) assert.throws(() => checkedFailure(stage, engine), /^Error: SF08 checkpoint refused$/);

  const expectedEvents = [];
  const expected = fake('firefox');
  const result = await probeEngine('firefox', expected, settings(provisioned, ready, negative),
    (stage, engine) => expectedEvents.push({ stage, engine }));
  assert.equal(result.wrong_ca_refused, true);
  assert.ok(expectedEvents.some(event => event.stage === 'engine_wrong_ca'));
  assert.equal(expectedEvents.some(event => event.stage.startsWith('engine_wrong_ca_')), false);

  const cases = [
    { mode: 'accepted', stage: 'engine_wrong_ca_response_received' },
    { fault: 'bad-signature', stage: 'engine_wrong_ca_bad_signature' },
    { fault: 'hostname', stage: 'engine_wrong_ca_hostname' },
    { fault: 'timeout', stage: 'engine_wrong_ca_timeout' },
    { fault: 'no-response', stage: 'engine_wrong_ca_no_response' },
    { fault: 'other-tls', stage: 'engine_wrong_ca_other_tls' },
    { fault: 'unknown', stage: 'engine_wrong_ca_unknown' },
    { fault: 'extra-code-text', stage: 'engine_wrong_ca_unknown' },
  ];
  for (const item of cases) {
    const type = fake('firefox', item.mode, item.fault ? { reason: 'ca', kind: item.fault } : undefined);
    const events = [];
    await assert.rejects(probeEngine('firefox', type, settings(provisioned, ready, negative),
      (stage, engine) => events.push({ stage, engine })),
    /^Error: SF08 browser trust preflight refused$/);
    assert.deepEqual(events[events.length - 1], { stage: item.stage, engine: 'firefox' });
    assert.equal(type.trace.pageClosed >= 1, true);
    assert.equal(type.trace.contextClosed && type.trace.closed, true);
    assert.doesNotMatch(JSON.stringify(events), /PRIVATE_SENTINEL|private\.invalid|secret|unit-secret|SEC_ERROR|SSL_ERROR/);
  }
});

for (const name of Object.keys(versions)) {
  for (const reason of ['ca', 'host']) {
    for (const kind of ['url-marker', 'calllog-marker', 'extra-text', 'oversized', 'non-error']) {
      test(name + ' ' + reason + ': rejects ' + kind, async () => {
        const token = messages[name][reason === 'ca' ? 0 : 1];
        const config = settings(provisioned, ready, negative);
        if (kind === 'url-marker') config.negative[reason] += '/' + token;
        const type = fake(name, undefined, { reason, kind, config });
        await assert.rejects(probeEngine(name, type, config),
          /^Error: SF08 browser trust preflight refused$/);
        assert.equal(type.trace.closed, true);
        assert.equal(type.trace.contextClosed, true);
      });
    }
  }
}

test('positive preflight failures use only fixed checkpoint stages', () => {
  const stages = ['engine_positive_ca', 'engine_positive_host', 'engine_positive_timeout',
    'engine_positive_response', 'engine_positive_tls_unknown', 'engine_positive_unknown'];
  for (const stage of stages) assert.ok(FAILURE_STAGES.includes(stage), stage);
  for (const stage of stages) assert.deepEqual(checkedFailure(stage, 'firefox'),
    { schema_version: 1, stage, engine: 'firefox' });
  assert.throws(() => checkedFailure('engine_positive_private_path', 'firefox'),
    /^Error: SF08 checkpoint refused$/);
});

test('positive navigation failures classify closed facts without leaking exception text', async () => {
  const cases = [
    { name: 'chromium', fault: 'ca', expected: 'engine_positive_ca' },
    { name: 'firefox', fault: 'host', expected: 'engine_positive_host' },
    { name: 'firefox', fault: 'timeout', expected: 'engine_positive_timeout' },
    { name: 'chromium', fault: 'missing-response', expected: 'engine_positive_response' },
    { name: 'webkit', fault: 'ca', expected: 'engine_positive_tls_unknown' },
    { name: 'firefox', fault: 'extra-tls-text', expected: 'engine_positive_unknown' },
    { name: 'chromium', fault: 'unknown', expected: 'engine_positive_unknown' },
    { name: 'firefox', fault: 'bad-status', expected: 'engine_positive_response' },
    { name: 'firefox', fault: 'status-error', expected: 'engine_positive_response' },
  ];
  for (const item of cases) {
    const type = fake(item.name, undefined, { positive: item.fault });
    const events = [];
    await assert.rejects(probeEngine(item.name, type, settings(provisioned, ready, negative),
      (stage, engine) => events.push({ stage, engine })),
    /^Error: SF08 browser trust preflight refused$/);
    assert.deepEqual(events[events.length - 1], { stage: item.expected, engine: item.name });
    assert.equal(type.trace.pageClosed, 1);
    assert.equal(type.trace.contextClosed && type.trace.closed, true);
    assert.doesNotMatch(JSON.stringify(events), /PRIVATE_SENTINEL|private\.invalid|secret/);
  }
});
