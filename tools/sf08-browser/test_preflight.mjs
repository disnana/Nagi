import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { EventEmitter } from 'node:events';
import { settings, probeEngine, firefoxCAReadiness, firefoxWorkerReadiness, webkitCertificateWitness } from './preflight.mjs';
import { checkedFailure, FAILURE_STAGES, WEBKIT_WITNESS_DIAGNOSTICS, WEBKIT_WITNESS_STAGE_LIST } from './browser_stage.mjs';

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

function certificateWitnessFixture(overrides = {}, until = performance.now() + 10000) {
  const page = new EventEmitter(), context = new EventEmitter(), browser = new EventEmitter();
  const frame = {}, url = negative.caOrigin + '/';
  page.mainFrame = () => frame;
  page.isClosed = () => false;
  browser.isConnected = () => true;
  const request = { url: () => url, frame: () => frame, isNavigationRequest: () => true,
    failure: () => ({ errorText: messages.webkit[0] }), response: async () => null, ...overrides };
  const witness = webkitCertificateWitness(page, context, browser, url, until);
  return { page, context, browser, request, witness };
}

const witnessStage = (stage, diagnostic) => `${stage}_witness_${diagnostic}`;

test('WebKit certificate event proves one bound failed navigation, never cancellation alone', async () => {
  const f = certificateWitnessFixture();
  f.page.emit('requestfailed', f.request);
  assert.equal(await f.witness.refused(new Error('Operation was cancelled')), true);
  assert.equal(await f.witness.refused(new Error('Operation was cancelled')), false);
  assert.equal(f.page.listenerCount('requestfailed'), 0);
  assert.equal(f.witness.stop(), true);
  for (const target of [f.page, f.context, f.browser]) assert.equal(target.eventNames().length, 0);
  f.page.emit('requestfailed', f.request); // Teardown cannot provide more evidence.
});

test('WebKit certificate witness rejects missing duplicate foreign or malformed evidence', async () => {
  for (const [label, overrides] of [
    ['wrong URL', { url: () => negative.hostOrigin + '/' }],
    ['subresource', { isNavigationRequest: () => false }],
    ['subframe', { frame: () => ({}) }],
    ['null failure', { failure: () => null }],
    ['generic cancellation', { failure: () => ({ errorText: 'Operation was cancelled' }) }],
    ['network failure', { failure: () => ({ errorText: 'net::ERR_CONNECTION_REFUSED' }) }],
    ['token suffix', { failure: () => ({ errorText: messages.webkit[0] + ' PRIVATE_SENTINEL' }) }],
    ['nonstring', { failure: () => ({ errorText: 1 }) }],
    ['failure getter throws', { failure: () => Object.defineProperty({}, 'errorText', {
      get() { throw new Error('PRIVATE_SENTINEL'); },
    }) }],
    ['method throws', { frame: () => { throw new Error('PRIVATE_SENTINEL'); } }],
    ['HTTP response', { response: async () => ({ status: () => 200 }) }],
    ['response query throws', { response: () => { throw new Error('PRIVATE_SENTINEL'); } }],
  ]) {
    const f = certificateWitnessFixture(overrides);
    f.page.emit('requestfailed', f.request);
    assert.equal(await f.witness.refused(new Error('Operation was cancelled')), false, label);
    f.witness.stop();
  }
  for (const count of [0, 2]) {
    const f = certificateWitnessFixture();
    for (let i = 0; i < count; i++) f.page.emit('requestfailed', f.request);
    assert.equal(await f.witness.refused(new Error('Operation was cancelled')), false);
    f.witness.stop();
  }
});

test('WebKit certificate witness keeps response closure timeout and original deadline vetoes', async () => {
  for (const event of ['response', 'close', 'crash', 'context-close', 'disconnected']) {
    const f = certificateWitnessFixture();
    f.page.emit('requestfailed', f.request);
    if (event === 'context-close') f.context.emit('close');
    else if (event === 'disconnected') f.browser.emit(event);
    else f.page.emit(event);
    assert.equal(await f.witness.refused(new Error('Operation was cancelled')), false, event);
    f.witness.stop();
  }
  for (const error of [Object.assign(new Error('PRIVATE_SENTINEL'), { name: 'TimeoutError' }),
                       { message: 'Unacceptable TLS certificate' }]) {
    const f = certificateWitnessFixture(); f.page.emit('requestfailed', f.request);
    assert.equal(await f.witness.refused(error), false); f.witness.stop();
  }
  const expired = certificateWitnessFixture({}, performance.now() - 1);
  expired.page.emit('requestfailed', expired.request);
  assert.equal(await expired.witness.refused(new Error('Operation was cancelled')), false);
  expired.witness.stop();
  const late = certificateWitnessFixture();
  late.witness.stop(); late.page.emit('requestfailed', late.request);
  assert.equal(await late.witness.refused(new Error('Operation was cancelled')), false);
  const stopped = certificateWitnessFixture(); stopped.page.emit('requestfailed', stopped.request);
  stopped.witness.stop();
  assert.equal(await stopped.witness.refused(new Error('Operation was cancelled')), false);
  const pending = certificateWitnessFixture({ response: () => new Promise(() => {}) }, performance.now() + 25);
  pending.page.emit('requestfailed', pending.request);
  assert.equal(await pending.witness.refused(new Error('Operation was cancelled')), false);
  pending.witness.stop();
});

test('WebKit witness exposes only fixed failure facts', async () => {
  const cases = [
    { label: 'missing_event', make: () => certificateWitnessFixture(), run: async f => {
      assert.equal(await f.witness.refused(new Error('PRIVATE_SENTINEL')), false);
    } },
    { label: 'duplicate_event', make: () => certificateWitnessFixture(), run: async f => {
      f.page.emit('requestfailed', f.request); f.page.emit('requestfailed', f.request);
      assert.equal(await f.witness.refused(new Error('PRIVATE_SENTINEL')), false);
    } },
    { label: 'foreign_request_url', make: () => certificateWitnessFixture({ url: () => negative.hostOrigin + '/PRIVATE_SENTINEL' }),
      run: async f => { f.page.emit('requestfailed', f.request); assert.equal(await f.witness.refused(new Error('PRIVATE_SENTINEL')), false); } },
    { label: 'foreign_request_non_navigation', make: () => certificateWitnessFixture({ isNavigationRequest: () => false }),
      run: async f => { f.page.emit('requestfailed', f.request); assert.equal(await f.witness.refused(new Error('PRIVATE_SENTINEL')), false); } },
    { label: 'foreign_request_frame', make: () => certificateWitnessFixture({ frame: () => ({}) }),
      run: async f => { f.page.emit('requestfailed', f.request); assert.equal(await f.witness.refused(new Error('PRIVATE_SENTINEL')), false); } },
    { label: 'not_certificate', make: () => certificateWitnessFixture({ failure: () => ({ errorText: 'Operation was cancelled' }) }),
      run: async f => { f.page.emit('requestfailed', f.request); assert.equal(await f.witness.refused(new Error('PRIVATE_SENTINEL')), false); } },
    { label: 'malformed', make: () => certificateWitnessFixture({ failure: () => null }),
      run: async f => { f.page.emit('requestfailed', f.request); assert.equal(await f.witness.refused(new Error('PRIVATE_SENTINEL')), false); } },
    { label: 'http_response', make: () => certificateWitnessFixture(), run: async f => {
      f.page.emit('response', { url: () => 'https://private.invalid/PRIVATE_SENTINEL' });
      assert.equal(await f.witness.refused(new Error('PRIVATE_SENTINEL')), false);
      const queried = certificateWitnessFixture({ response: async () => ({ status: () => 200 }) });
      queried.page.emit('requestfailed', queried.request);
      assert.equal(await queried.witness.refused(new Error('PRIVATE_SENTINEL')), false);
      assert.equal(queried.witness.diagnostic(), 'http_response');
      queried.witness.stop();
    } },
    { label: 'lifecycle', make: () => certificateWitnessFixture(), run: async f => {
      f.context.emit('close'); assert.equal(await f.witness.refused(new Error('PRIVATE_SENTINEL')), false);
    } },
    { label: 'error_shape', make: () => certificateWitnessFixture(), run: async f => {
      assert.equal(await f.witness.refused(Object.assign(new Error('PRIVATE_SENTINEL'), { name: 'TimeoutError' })), false);
    } },
    { label: 'deadline', make: () => certificateWitnessFixture({}, performance.now() - 1), run: async f => {
      f.page.emit('requestfailed', f.request); assert.equal(await f.witness.refused(new Error('PRIVATE_SENTINEL')), false);
    } },
    { label: 'response_query_fault', make: () => certificateWitnessFixture({ response: () => { throw new Error('PRIVATE_SENTINEL'); } }),
      run: async f => { f.page.emit('requestfailed', f.request); assert.equal(await f.witness.refused(new Error('PRIVATE_SENTINEL')), false); } },
    { label: 'response_query_timeout', make: () => certificateWitnessFixture({ response: () => new Promise(() => {}) }, performance.now() + 20),
      run: async f => { f.page.emit('requestfailed', f.request); assert.equal(await f.witness.refused(new Error('PRIVATE_SENTINEL')), false); } },
    { label: 'certificate_event', make: () => certificateWitnessFixture(), run: async f => {
      f.page.emit('requestfailed', f.request);
      assert.equal(await f.witness.refused(new Error('Operation was cancelled')), true);
    } },
  ];
  assert.equal(WEBKIT_WITNESS_DIAGNOSTICS.length, cases.length);
  for (const item of cases) {
    const f = item.make();
    await item.run(f);
    assert.equal(f.witness.diagnostic(), item.label);
    assert.equal(WEBKIT_WITNESS_DIAGNOSTICS.includes(f.witness.diagnostic()), true);
    assert.doesNotMatch(JSON.stringify({ diagnostic: f.witness.diagnostic() }),
      /PRIVATE_SENTINEL|private\.invalid|Unacceptable|Operation|https?:/);
    f.witness.stop();
  }
});

function fake(name, mode, fault) {
  const trace = { urls: [], timeouts: [], closed: false, pageClosed: 0, contextClosed: false };
  const config = fault?.config ?? settings(provisioned, ready, negative);
  return { trace, async launch(options) {
    trace.launch = options;
    const browserEvents = new EventEmitter();
    return { version: () => mode === 'version' ? 'unexpected' : versions[name],
      on: browserEvents.on.bind(browserEvents), off: browserEvents.off.bind(browserEvents),
      isConnected: () => !trace.closed,
      async close() { trace.closed = true; },
      async newContext(options) {
        trace.context = options;
        const contextEvents = new EventEmitter();
        return { on: contextEvents.on.bind(contextEvents), off: contextEvents.off.bind(contextEvents),
          async close() { trace.contextClosed = true; }, async newPage() {
          if (fault?.reason === 'host' && fault.kind === 'page-open' && trace.urls.length === 4)
            throw new Error('PRIVATE_SENTINEL');
          const pageEvents = new EventEmitter(), frame = {};
          return { on: pageEvents.on.bind(pageEvents), off: pageEvents.off.bind(pageEvents),
            mainFrame: () => frame, isClosed: () => false,
            async close() {
            trace.pageClosed++;
            if (fault?.reason === 'host' && fault.kind === 'page-close' &&
                trace.urls.at(-1) === config.negative.host + '/') throw new Error('PRIVATE_SENTINEL');
          }, async goto(url, options) {
            trace.urls.push(url);
            trace.timeouts.push(options.timeout);
            if (url.startsWith(config.negative.ca) || url.startsWith(config.negative.host)) {
              const reason = url.startsWith(config.negative.ca) ? 'ca' : 'host';
              const token = messages[name][reason === 'ca' ? 0 : 1];
              if (fault?.reason === reason && fault.requestCertificate) {
                pageEvents.emit('requestfailed', {
                  url: () => url, isNavigationRequest: () => true, frame: () => frame,
                  failure: () => ({ errorText: messages.webkit[0] }), response: async () => null,
                });
              }
              if (mode === 'accepted') return { status: () => 200 };
              if (mode === 'network') throw new Error('net::ERR_CONNECTION_REFUSED PRIVATE_SENTINEL');
              if (fault && fault.reason === reason) {
                if (fault.kind === 'resolved-response') return { status: () => 200 };
                if (fault.kind === 'null-response') return null;
                if (fault.kind === 'webkit-protocol') throw new Error(
                  'page.goto: Protocol error (Playwright.navigate): ' + token + ' at ' + url);
                if (fault.kind === 'cancelled') throw new Error('Operation was cancelled');
                if (fault.kind === 'protocol-wrapper') throw new Error(
                  'page.goto: Protocol error (browsingContext.navigate): ' + (fault.token ?? token) + ' at ' + url);
                if (fault.kind === 'raw-error') throw new Error(fault.message);
                if (fault.kind === 'named-error') throw Object.assign(new Error(fault.message), { name: fault.errorName });
                if (fault.kind === 'raw-non-error') throw { message: fault.message };
                if (fault.kind === 'message-getter-throws') {
                  const error = new Error();
                  Object.defineProperty(error, 'message', { get() { throw new Error('PRIVATE_SENTINEL'); } });
                  throw error;
                }
                if (fault.kind === 'message-non-string') {
                  const error = new Error();
                  Object.defineProperty(error, 'message', { value: 7 });
                  throw error;
                }
                if (fault.kind === 'message-missing') {
                  const error = new Error();
                  delete error.message;
                  throw error;
                }
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

test('WebKit probe uses the bound certificate event without accepting cancellation or HTTP', async () => {
  for (const reason of ['ca', 'host']) {
    const type = fake('webkit', 'normal', { reason, kind: 'cancelled', requestCertificate: true });
    const checkpoints = [];
    const result = await probeEngine('webkit', type, settings(provisioned, ready, negative),
      (stage, engine) => checkpoints.push({ stage, engine }));
    assert.equal(result.wrong_ca_refused, true);
    assert.equal(result.wrong_host_refused, true);
    assert.equal(checkpoints.some(item => item.stage.includes('_witness_')), false);
    assert.equal(type.trace.urls.length, 5);
    assert.equal(type.trace.closed, true);
    assert.equal(type.trace.contextClosed, true);
    for (const fault of [
      { reason, kind: 'cancelled' },
      { reason, kind: 'timeout', requestCertificate: true },
      ...['TimeoutError', 'TargetClosedError', 'AbortError'].map(errorName => ({
        reason, kind: 'named-error', message: messages.webkit[0], errorName, requestCertificate: true,
      })),
      { reason, kind: 'raw-non-error', message: messages.webkit[0], requestCertificate: true },
      { reason, kind: 'resolved-response', requestCertificate: true },
      { reason, kind: 'null-response', requestCertificate: true },
    ]) {
      const refused = fake('webkit', 'normal', fault);
      await assert.rejects(probeEngine('webkit', refused, settings(provisioned, ready, negative)));
      assert.equal(refused.trace.closed, true);
      assert.equal(refused.trace.contextClosed, true);
    }
  }
});

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
  return new Error(form === 'bare' ? token : form === 'prefix' ? 'page.goto: ' + token :
    form === 'protocol' ? 'page.goto: Protocol error (browsingContext.navigate): ' + token + ' at ' + url :
      'page.goto: ' + token + ' at ' + url);
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
  const time = clock(); const events = []; let attempts = 0;
  const unsupported = fake('firefox', undefined, { positiveStep: url => {
    if (url === ready.app + '/' && ++attempts === 1) throw issuer('protocol', url);
    return { status: () => 200 };
  } });
  await assert.rejects(probeEngine('firefox', unsupported, settings(provisioned, ready, negative),
    (stage, engine) => events.push({ stage, engine }), time.timing),
  /^Error: SF08 browser trust preflight refused$/);
  assert.equal(attempts, 1);
  assert.equal(time.delays.length, 0);
  assert.deepEqual(events.at(-1), { stage: 'engine_positive_unknown', engine: 'firefox' });
  assert.equal(unsupported.trace.pageClosed, 1);
  assert.equal(unsupported.trace.contextClosed && unsupported.trace.closed, true);
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
  const owned = worker(url => { if (++attempts === 1) throw issuer('at-url', url); return { status: () => 200 }; });
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
    'engine_wrong_ca_other_tls', 'engine_wrong_ca_unknown',
    'engine_wrong_ca_diag_bad_signature_present','engine_wrong_ca_diag_unknown_issuer_present',
    'engine_wrong_ca_diag_hostname_present','engine_wrong_ca_diag_other_tls_present',
    'engine_wrong_ca_diag_multiple_codes','engine_wrong_ca_diag_no_fixed_code',
    'engine_wrong_ca_shape_non_error','engine_wrong_ca_shape_non_string',
    'engine_wrong_ca_shape_oversize','engine_wrong_ca_shape_access_failed'];
  for (const stage of stages) {
    assert.ok(FAILURE_STAGES.includes(stage), stage);
    const value = checkedFailure(stage, 'firefox');
    assert.deepEqual(value, { schema_version: 1, stage, engine: 'firefox' });
    assert.deepEqual(Object.keys(value).sort(), ['engine','schema_version','stage']);
  }
  assert.throws(() => checkedFailure('engine_wrong_ca_PRIVATE_SECRET', 'firefox'),
    /^Error: SF08 checkpoint refused$/);
  for (const [stage, engine] of [
    ['engine_wrong_ca_bad_signature', 'chromium'],
    ['engine_wrong_ca_other_tls', 'webkit'],
    ['engine_wrong_ca_diag_bad_signature_present', 'chromium'],
    ['engine_wrong_ca_diag_multiple_codes', 'webkit'],
    ['engine_wrong_ca_shape_access_failed', 'chromium'],
    ['engine_wrong_ca_hostname', 'webkit'],
  ]) assert.throws(() => checkedFailure(stage, engine), /^Error: SF08 checkpoint refused$/);

  const expectedEvents = [];
  const expected = fake('firefox');
  const result = await probeEngine('firefox', expected, settings(provisioned, ready, negative),
    (stage, engine) => expectedEvents.push({ stage, engine }));
  assert.equal(result.wrong_ca_refused, true);
  assert.ok(expectedEvents.some(event => event.stage === 'engine_wrong_ca'));
  assert.equal(expectedEvents.some(event => event.stage.startsWith('engine_wrong_ca_')), false);

  const wrappedExpectedEvents = [];
  const wrappedExpected = fake('firefox', undefined, { reason: 'ca', kind: 'protocol-wrapper',
    token: 'SEC_ERROR_UNKNOWN_ISSUER' });
  await assert.rejects(probeEngine('firefox', wrappedExpected, settings(provisioned, ready, negative),
    (stage, engine) => wrappedExpectedEvents.push({ stage, engine })),
  /^Error: SF08 browser trust preflight refused$/);
  assert.deepEqual(wrappedExpectedEvents.at(-1),
    { stage: 'engine_wrong_ca_diag_unknown_issuer_present', engine: 'firefox' });
  assert.equal(wrappedExpected.trace.contextClosed && wrappedExpected.trace.closed, true);

  const wrappedHostEvents = [];
  const wrappedHost = fake('firefox', undefined, { reason: 'host', kind: 'protocol-wrapper',
    token: 'SSL_ERROR_BAD_CERT_DOMAIN' });
  await assert.rejects(probeEngine('firefox', wrappedHost, settings(provisioned, ready, negative),
    (stage, engine) => wrappedHostEvents.push({ stage, engine })),
  /^Error: SF08 browser trust preflight refused$/);
  assert.deepEqual(wrappedHostEvents.at(-1), { stage: 'engine_wrong_host_unknown', engine: 'firefox' });
  assert.equal(wrappedHost.trace.contextClosed && wrappedHost.trace.closed, true);

  const cases = [
    { mode: 'accepted', stage: 'engine_wrong_ca_response_received' },
    { fault: 'bad-signature', stage: 'engine_wrong_ca_bad_signature' },
    { fault: 'hostname', stage: 'engine_wrong_ca_hostname' },
    { fault: 'timeout', stage: 'engine_wrong_ca_timeout' },
    { fault: 'no-response', stage: 'engine_wrong_ca_no_response' },
    { fault: 'other-tls', stage: 'engine_wrong_ca_other_tls' },
    { fault: 'unknown', stage: 'engine_wrong_ca_diag_no_fixed_code' },
    { fault: 'extra-code-text', stage: 'engine_wrong_ca_diag_bad_signature_present' },
    { fault: 'protocol-wrapper', token: 'SEC_ERROR_BAD_SIGNATURE', stage: 'engine_wrong_ca_diag_bad_signature_present' },
    { fault: 'protocol-wrapper', token: 'SSL_ERROR_BAD_CERT_DOMAIN', stage: 'engine_wrong_ca_diag_hostname_present' },
    { fault: 'protocol-wrapper', token: 'SEC_ERROR_UNTRUSTED_ISSUER', stage: 'engine_wrong_ca_diag_other_tls_present' },
    { fault: 'protocol-wrapper', token: 'SEC_ERROR_CA_CERT_INVALID', stage: 'engine_wrong_ca_diag_other_tls_present' },
    { fault: 'raw-error', message: 'page.goto: Protocol error (Page.navigate): SEC_ERROR_UNKNOWN_ISSUER at ' + negative.caOrigin + '/',
      stage: 'engine_wrong_ca_diag_unknown_issuer_present' },
    { fault: 'raw-error', message: 'page.goto: Protocol error (browsingContext.navigate): SEC_ERROR_UNKNOWN_ISSUER at ' + negative.caOrigin + '/extra',
      stage: 'engine_wrong_ca_diag_unknown_issuer_present' },
    { fault: 'raw-error', message: 'page.goto: Protocol error (browsingContext.navigate): SEC_ERROR_UNKNOWN_ISSUER at ' + negative.caOrigin + '/ trailing',
      stage: 'engine_wrong_ca_diag_unknown_issuer_present' },
    { fault: 'raw-error', message: 'page.goto: Error: Protocol error (browsingContext.navigate): SEC_ERROR_UNKNOWN_ISSUER at ' + negative.caOrigin + '/',
      stage: 'engine_wrong_ca_diag_unknown_issuer_present' },
    { fault: 'raw-error', message: 'page.goto: Protocol error (browsingContext.navigate): SEC_ERROR_PRIVATE_SENTINEL at ' + negative.caOrigin + '/',
      stage: 'engine_wrong_ca_diag_no_fixed_code' },
    { fault: 'raw-error', message: 'page.goto: Protocol error (browsingContext.navigate): SEC_ERROR_UNKNOWN_ISSUER SEC_ERROR_BAD_SIGNATURE at ' + negative.caOrigin + '/',
      stage: 'engine_wrong_ca_diag_multiple_codes' },
    { fault: 'raw-error', message: 'page.goto: Protocol error (browsingContext.navigate):\nSEC_ERROR_UNKNOWN_ISSUER at ' + negative.caOrigin + '/',
      stage: 'engine_wrong_ca_diag_unknown_issuer_present' },
    { fault: 'raw-error', message: 'page.goto: Protocol error (browsingContext.navigate): SEC_ERROR_UNKNOWN_ISSUER at ' + negative.caOrigin + '/ token=PRIVATE_SENTINEL',
      stage: 'engine_wrong_ca_diag_unknown_issuer_present' },
    { fault: 'raw-error', message: 'page.goto: Protocol error (browsingContext.navigate): SEC_ERROR_UNKNOWN_ISSUER at ' + negative.caOrigin + '/ ' + 'x'.repeat(8192),
      stage: 'engine_wrong_ca_shape_oversize' },
    { fault: 'raw-non-error', message: 'page.goto: Protocol error (browsingContext.navigate): SEC_ERROR_UNKNOWN_ISSUER at ' + negative.caOrigin + '/',
      stage: 'engine_wrong_ca_shape_non_error' },
  ];
  for (const item of cases) {
    const type = fake('firefox', item.mode, item.fault ? {
      reason: 'ca', kind: item.fault, token: item.token, message: item.message,
    } : undefined);
    const events = [];
    await assert.rejects(probeEngine('firefox', type, settings(provisioned, ready, negative),
      (stage, engine) => events.push({ stage, engine })),
    /^Error: SF08 browser trust preflight refused$/);
    assert.deepEqual(events[events.length - 1], { stage: item.stage, engine: 'firefox' });
    assert.equal(type.trace.pageClosed >= 1, true);
    assert.equal(type.trace.contextClosed && type.trace.closed, true);
    assert.doesNotMatch(JSON.stringify(events), /PRIVATE_SENTINEL|private\.invalid|secret|unit-secret|SEC_ERROR|SSL_ERROR/);
  }

  for (const name of ['chromium', 'webkit']) {
    const type = fake(name, undefined, { reason: 'ca', kind: 'raw-error',
      message: 'page.goto: Protocol error (browsingContext.navigate): SEC_ERROR_UNKNOWN_ISSUER at ' + negative.caOrigin + '/' });
    const events = [];
    await assert.rejects(probeEngine(name, type, settings(provisioned, ready, negative),
      (stage, engine) => events.push({ stage, engine })),
    /^Error: SF08 browser trust preflight refused$/);
    assert.deepEqual(events.at(-1), { stage: name === 'webkit'
      ? witnessStage('engine_wrong_ca_webkit_no_fixed_token', 'missing_event') : 'engine_wrong_ca_unknown', engine: name });
    assert.doesNotMatch(JSON.stringify(events), /SEC_ERROR|SSL_ERROR|PRIVATE_SENTINEL/);
  }
});

test('closed Firefox wrong-CA diagnostics classify bounded code presence and shape only as failure', async () => {
  const helperMappedOtherTokens = [
    'SEC_ERROR_EXPIRED_CERTIFICATE', 'SEC_ERROR_REVOKED_CERTIFICATE', 'SEC_ERROR_UNTRUSTED_CERT',
    'SEC_ERROR_INADEQUATE_KEY_USAGE', 'SEC_ERROR_CERT_SIGNATURE_ALGORITHM_DISABLED', 'SEC_ERROR_UNKNOWN',
    'SSL_ERROR_NO_CERTIFICATE', 'SSL_ERROR_BAD_CERTIFICATE', 'SSL_ERROR_UNSUPPORTED_CERTIFICATE_TYPE',
    'SSL_ERROR_UNSUPPORTED_VERSION', 'SSL_ERROR_UNKNOWN',
  ];
  const cases = [
    ...helperMappedOtherTokens.map(token => ({ message: 'bounded diagnostic: ' + token,
      stage: 'engine_wrong_ca_diag_other_tls_present' })),
    { message: 'NS_ERROR_NET_TIMEOUT', stage: 'engine_wrong_ca_diag_no_fixed_code' },
    { message: 'SEC_ERROR_UNKNOWN_ISSUER_EXTENSION', stage: 'engine_wrong_ca_diag_no_fixed_code' },
    { message: 'wrapped:\r\nSEC_ERROR_BAD_SIGNATURE at https://wrong.invalid/path?key=PRIVATE_SENTINEL',
      stage: 'engine_wrong_ca_diag_bad_signature_present' },
    { message: 'wrapper\nSEC_ERROR_UNKNOWN_ISSUER at ' + negative.hostOrigin + '/',
      stage: 'engine_wrong_ca_diag_unknown_issuer_present' },
    { message: 'Firefox says SSL_ERROR_BAD_CERT_DOMAIN at https://wrong.invalid/',
      stage: 'engine_wrong_ca_diag_hostname_present' },
    { message: 'wrapper\r\nSEC_ERROR_UNTRUSTED_ISSUER at https://wrong.invalid/',
      stage: 'engine_wrong_ca_diag_other_tls_present' },
    { message: 'wrapper\nSEC_ERROR_CA_CERT_INVALID at https://wrong.invalid/',
      stage: 'engine_wrong_ca_diag_other_tls_present' },
    { message: 'context SEC_ERROR_UNKNOWN_ISSUER\nthen SEC_ERROR_BAD_SIGNATURE',
      stage: 'engine_wrong_ca_diag_multiple_codes' },
    { message: 'repeat SEC_ERROR_BAD_SIGNATURE\r\nSEC_ERROR_BAD_SIGNATURE',
      stage: 'engine_wrong_ca_diag_bad_signature_present' },
    { message: 'PREFIX_SEC_ERROR_BAD_SIGNATURE', stage: 'engine_wrong_ca_diag_no_fixed_code' },
    { message: 'SEC_ERROR_BAD_SIGNATURE_SUFFIX', stage: 'engine_wrong_ca_diag_no_fixed_code' },
    { message: 'sec_error_bad_signature', stage: 'engine_wrong_ca_diag_no_fixed_code' },
    { message: 'SEC_ERROR_PRIVATE_SENTINEL', stage: 'engine_wrong_ca_diag_no_fixed_code' },
    { message: 'private certificate data https://private.invalid/?token=PRIVATE_SENTINEL',
      stage: 'engine_wrong_ca_diag_no_fixed_code' },
    { message: 'x'.repeat(8192 - 'SEC_ERROR_BAD_SIGNATURE'.length - 1) + ' SEC_ERROR_BAD_SIGNATURE',
      stage: 'engine_wrong_ca_diag_bad_signature_present' },
    { message: 'SEC_ERROR_BAD_SIGNATURE' + 'x'.repeat(8193),
      stage: 'engine_wrong_ca_shape_oversize' },
    { kind: 'raw-non-error', message: 'SEC_ERROR_BAD_SIGNATURE PRIVATE_SENTINEL',
      stage: 'engine_wrong_ca_shape_non_error' },
    { kind: 'message-non-string', stage: 'engine_wrong_ca_shape_non_string' },
    { kind: 'message-missing', stage: 'engine_wrong_ca_shape_non_string' },
    { kind: 'message-getter-throws', stage: 'engine_wrong_ca_shape_access_failed' },
  ];
  for (const item of cases) {
    const events = [];
    const type = fake('firefox', undefined, { reason: 'ca', kind: item.kind ?? 'raw-error', message: item.message });
    await assert.rejects(probeEngine('firefox', type, settings(provisioned, ready, negative),
      (stage, engine) => events.push({ stage, engine })),
    /^Error: SF08 browser trust preflight refused$/);
    assert.deepEqual(events.at(-1), { stage: item.stage, engine: 'firefox' });
    assert.equal(type.trace.contextClosed && type.trace.closed, true);
    assert.doesNotMatch(JSON.stringify(events), /PRIVATE_SENTINEL|private\.invalid|https?:|SEC_ERROR|SSL_ERROR|token=/i);
  }

  const exact = fake('firefox');
  const exactEvents = [];
  assert.equal((await probeEngine('firefox', exact, settings(provisioned, ready, negative),
    (stage, engine) => exactEvents.push({ stage, engine }))).wrong_ca_refused, true);
  assert.equal(exactEvents.some(event => event.stage.startsWith('engine_wrong_ca_diag_')), false);
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


test('WebKit wrong-CA diagnostics remain failure-only and emit fixed labels', async () => {
  const url = negative.caOrigin + '/';
  const prefix = 'page.goto: Protocol error (Playwright.navigate): Unacceptable TLS certificate';
  const cases = [
    { message: prefix, outcome: 'webkit_protocol_form' },
    { message: prefix + ' at ' + url, outcome: 'webkit_protocol_form' },
    { message: prefix + '\nCall log: PRIVATE_SENTINEL', outcome: 'webkit_protocol_form' },
    { message: prefix + ' at https://private.invalid/', outcome: 'webkit_token_present' },
    { message: 'page.goto: Protocol error (Page.navigate): Unacceptable TLS certificate', outcome: 'webkit_token_present' },
    { message: 'failure\nCall log: Unacceptable TLS certificate', outcome: 'webkit_token_present' },
    { message: 'Unacceptable TLS certificate trailing', outcome: 'webkit_token_present' },
    { message: 'Unacceptable TLS certificate_SUFFIX', outcome: 'webkit_no_fixed_token' },
    { message: 'PREFIX_Unacceptable TLS certificate', outcome: 'webkit_no_fixed_token' },
    { message: 'unacceptable TLS certificate', outcome: 'webkit_no_fixed_token' },
    { message: 'SEC_ERROR_UNKNOWN_ISSUER', outcome: 'webkit_no_fixed_token' },
    { message: 'net::ERR_CONNECTION_REFUSED PRIVATE_SENTINEL', outcome: 'webkit_no_fixed_token' },
    { message: 'Unacceptable TLS certificate' + 'x'.repeat(8193), outcome: 'webkit_shape_oversize' },
    { kind: 'raw-non-error', message: 'Unacceptable TLS certificate', outcome: 'webkit_shape_non_error' },
    { kind: 'message-non-string', outcome: 'webkit_shape_non_string' },
    { kind: 'message-missing', outcome: 'webkit_shape_non_string' },
    { kind: 'message-getter-throws', outcome: 'webkit_shape_access_failed' },
  ];
  for (const item of cases) {
    const type = fake('webkit', undefined, { reason: 'ca', kind: item.kind ?? 'raw-error', message: item.message });
    const events = [];
    await assert.rejects(probeEngine('webkit', type, settings(provisioned, ready, negative),
      (stage, engine) => events.push({ stage, engine })), /^Error: SF08 browser trust preflight refused$/);
    assert.deepEqual(events.at(-1), { stage: witnessStage('engine_wrong_ca_' + item.outcome,
      item.kind === 'raw-non-error' ? 'error_shape' : 'missing_event'), engine: 'webkit' });
    assert.equal(type.trace.contextClosed && type.trace.closed, true);
    assert.doesNotMatch(JSON.stringify(events), /PRIVATE_SENTINEL|private\.invalid|Unacceptable|Protocol|certificate/);
  }
});

test('new WebKit failure checkpoints refuse other engines', () => {
  for (const suffix of ['webkit_protocol_form', 'webkit_token_present', 'webkit_no_fixed_token',
    'webkit_cancelled_first_line', 'webkit_cancelled_call_log', 'webkit_cancelled_other',
    'webkit_shape_non_error', 'webkit_shape_non_string', 'webkit_shape_oversize', 'webkit_shape_access_failed']) {
    const stage = 'engine_wrong_ca_' + suffix;
    assert.equal(FAILURE_STAGES.includes(stage), true);
    assert.deepEqual(checkedFailure(stage, 'webkit'), { schema_version: 1, stage, engine: 'webkit' });
    for (const engine of ['none', 'chromium', 'firefox']) {
      assert.throws(() => checkedFailure(stage, engine), /^Error: SF08 checkpoint refused$/);
    }
  }
  const unknown = witnessStage('engine_wrong_host_webkit_no_fixed_token', 'missing_event');
  const cancelled = witnessStage('engine_wrong_host_webkit_cancelled_present', 'missing_event');
  assert.ok(WEBKIT_WITNESS_STAGE_LIST.includes(unknown));
  assert.ok(WEBKIT_WITNESS_STAGE_LIST.includes(cancelled));
  assert.notEqual(unknown, cancelled);
  for (const base of ['engine_wrong_ca_unknown','engine_wrong_ca_webkit_cancelled_first_line',
    'engine_wrong_host_unknown','engine_wrong_host_webkit_cancelled_present']) {
    for (const diagnostic of WEBKIT_WITNESS_DIAGNOSTICS) {
      const stage = witnessStage(base, diagnostic);
      assert.ok(WEBKIT_WITNESS_STAGE_LIST.includes(stage), stage);
      assert.deepEqual(checkedFailure(stage, 'webkit'), { schema_version: 1, stage, engine: 'webkit' });
      for (const engine of ['none','chromium','firefox']) {
        assert.throws(() => checkedFailure(stage, engine), /^Error: SF08 checkpoint refused$/);
      }
    }
  }
  for (const stage of [witnessStage('engine_wrong_host_webkit_cancelled_present', 'private_token'),
    'engine_wrong_host_webkit_cancelled_present_witness_unknown']) {
    assert.throws(() => checkedFailure(stage, 'webkit'), /^Error: SF08 checkpoint refused$/);
  }
});


test('WebKit wrong-CA cancellation labels distinguish first-line and call-log text only', async () => {
  const url = negative.caOrigin + '/';
  const cases = [
    { message: 'Operation was cancelled', outcome: 'webkit_cancelled_first_line' },
    { message: 'page.goto: Operation was cancelled', outcome: 'webkit_cancelled_first_line' },
    { message: 'page.goto: Protocol error (Playwright.navigate): Operation was cancelled at ' + url,
      outcome: 'webkit_cancelled_first_line' },
    { message: 'failure\nCall log: Operation was cancelled PRIVATE_SENTINEL',
      outcome: 'webkit_cancelled_call_log' },
    { message: 'Navigation failed\nCall log:\n  - Operation was cancelled PRIVATE_SENTINEL',
      outcome: 'webkit_cancelled_call_log' },
    { message: 'failure\nOperation was cancelled PRIVATE_SENTINEL', outcome: 'webkit_cancelled_other' },
    { message: 'Operation was cancelled_SUFFIX PRIVATE_SENTINEL', outcome: 'webkit_no_fixed_token' },
    { message: 'PRIVATE_SENTINEL https://private.invalid/?token=secret', outcome: 'webkit_no_fixed_token' },
  ];
  for (const { message, outcome } of cases) {
    const type = fake('webkit', undefined, { reason: 'ca', kind: 'raw-error', message });
    const events = [];
    await assert.rejects(probeEngine('webkit', type, settings(provisioned, ready, negative),
      (stage, engine) => events.push({ stage, engine })), /^Error: SF08 browser trust preflight refused$/);
    const stage = witnessStage('engine_wrong_ca_' + outcome, 'missing_event');
    assert.deepEqual(events.at(-1), { stage, engine: 'webkit' });
    assert.deepEqual(checkedFailure(events.at(-1).stage, 'webkit'),
      { schema_version: 1, stage, engine: 'webkit' });
    assert.equal(type.trace.contextClosed && type.trace.closed, true);
    assert.doesNotMatch(JSON.stringify(events), /PRIVATE_SENTINEL|private\.invalid|secret|Operation|Call log/);
  }
});

test('WebKit wrong-host failures receive bounded labels without changing acceptance', async () => {
  const cases = [
    { kind: 'resolved-response', stage: witnessStage('engine_wrong_host_response_received', 'missing_event') },
    { kind: 'null-response', stage: witnessStage('engine_wrong_host_no_response', 'missing_event') },
    { kind: 'timeout', stage: witnessStage('engine_wrong_host_timeout', 'error_shape') },
    { kind: 'webkit-protocol', stage: witnessStage('engine_wrong_host_webkit_protocol_form', 'missing_event') },
    { kind: 'raw-error', message: 'Navigation failed\nCall log: Unacceptable TLS certificate',
      stage: witnessStage('engine_wrong_host_webkit_token_present', 'missing_event') },
    { kind: 'cancelled', stage: witnessStage('engine_wrong_host_webkit_cancelled_present', 'missing_event') },
    { kind: 'unknown', stage: witnessStage('engine_wrong_host_webkit_no_fixed_token', 'missing_event') },
    { kind: 'non-error', stage: witnessStage('engine_wrong_host_webkit_shape_non_error', 'error_shape') },
    { kind: 'oversized', stage: witnessStage('engine_wrong_host_webkit_shape_oversize', 'missing_event') },
    { kind: 'message-non-string', stage: witnessStage('engine_wrong_host_webkit_shape_non_string', 'missing_event') },
    { kind: 'message-missing', stage: witnessStage('engine_wrong_host_webkit_shape_non_string', 'missing_event') },
    { kind: 'message-getter-throws', stage: witnessStage('engine_wrong_host_webkit_shape_access_failed', 'missing_event') },
    { kind: 'page-open', stage: 'engine_wrong_host_page_open' },
    { kind: 'page-close', stage: 'engine_wrong_host_page_close' },
  ];
  for (const item of cases) {
    const type = fake('webkit', undefined, { reason: 'host', ...item });
    const events = [];
    await assert.rejects(probeEngine('webkit', type, settings(provisioned, ready, negative),
      (stage, engine) => events.push({ stage, engine })),
    /^Error: SF08 browser trust preflight refused$/);
    assert.deepEqual(events.at(-1), { stage: item.stage, engine: 'webkit' });
    assert.deepEqual(checkedFailure(item.stage, 'webkit'),
      { schema_version: 1, stage: item.stage, engine: 'webkit' });
    if (item.stage.startsWith('engine_wrong_host_webkit_') || item.stage.includes('_witness_')) {
      for (const engine of ['none', 'chromium', 'firefox']) {
        assert.throws(() => checkedFailure(item.stage, engine), /^Error: SF08 checkpoint refused$/);
      }
    }
    assert.equal(type.trace.closed && type.trace.contextClosed, true);
    assert.doesNotMatch(JSON.stringify(events),
      /PRIVATE_SENTINEL|Unacceptable TLS certificate|Protocol error|private\.invalid|sf08-wronghost/);
  }
});
