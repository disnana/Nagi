import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash, generateKeyPairSync, X509Certificate } from 'node:crypto';
import { EventEmitter } from 'node:events';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import tls from 'node:tls';
import net from 'node:net';
import { spawnSync, spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import * as hostProof from './preflight.mjs';
import { settings, probeEngine, firefoxCAReadiness, firefoxWorkerReadiness, webkitCertificateWitness,
  probeWebkitWrongCA } from './preflight.mjs';
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
    { label: 'not_certificate', make: () => certificateWitnessFixture({ failure: () => ({ errorText: 'PRIVATE_SENTINEL' }) }),
      run: async f => { f.page.emit('requestfailed', f.request); assert.equal(await f.witness.refused(new Error('PRIVATE_SENTINEL')), false); } },
    ...[
      ['not_certificate_empty', ''],
      ['not_certificate_oversize', 'PRIVATE_SENTINEL'.repeat(8193)],
      ['not_certificate_cancelled', 'Operation was cancelled'],
      ['not_certificate_provisional_cancelled', 'Provisional navigation canceled.'],
      ['not_certificate_certificate_form', 'page.goto: Unacceptable TLS certificate at ' + negative.caOrigin + '/'],
      ['not_certificate_certificate_token', 'PRIVATE_SENTINEL Unacceptable TLS certificate'],
      ['not_certificate_dns_token', 'PRIVATE_SENTINEL Name or service not known'],
      ['not_certificate_connection_token', 'PRIVATE_SENTINEL Connection refused'],
      ['not_certificate_cancel_token', 'PRIVATE_SENTINEL Operation was cancelled'],
      ['not_certificate_tls_token', 'PRIVATE_SENTINEL TLS handshake failure'],
      ['not_certificate_multiple_tokens', 'PRIVATE_SENTINEL TLS Connection refused'],
    ].map(([label, errorText]) => ({ label,
      make: () => certificateWitnessFixture({ failure: () => ({ errorText }) }),
      run: async f => {
        f.page.emit('requestfailed', f.request);
        assert.equal(await f.witness.refused(new Error('Operation was cancelled')), false);
      },
    })),
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

test('WebKit non-certificate categories preserve every refusal and leak no request text', async () => {
  const certificate = 'Unacceptable TLS certificate';
  const url = negative.caOrigin + '/';
  for (const [errorText, diagnostic] of [
    ['page.goto: ' + certificate, 'not_certificate_certificate_form'],
    ['page.goto: Protocol error (Playwright.navigate): ' + certificate, 'not_certificate_certificate_form'],
    ['page.goto: Protocol error (Playwright.navigate): ' + certificate + ' at ' + url,
      'not_certificate_certificate_form'],
    ['page.goto: ' + certificate + ' at https://private.invalid/PRIVATE_SENTINEL',
      'not_certificate_certificate_token'],
    [certificate + '_SUFFIX', 'not_certificate_tls_token'],
    ['PREFIX_' + certificate, 'not_certificate_tls_token'],
    ['Connection refused_SUFFIX PRIVATE_SENTINEL', 'not_certificate'],
    ['PREFIX_Name or service not known PRIVATE_SENTINEL', 'not_certificate'],
    [certificate + ' PRIVATE_SENTINEL Name or service not known', 'not_certificate_multiple_tokens'],
    ['Operation was cancelled PRIVATE_SENTINEL TLS', 'not_certificate_multiple_tokens'],
    ['PRIVATE_SENTINEL https://private.invalid/?token=secret', 'not_certificate'],
  ]) {
    const f = certificateWitnessFixture({ failure: () => ({ errorText }) });
    f.page.emit('requestfailed', f.request);
    assert.equal(await f.witness.refused(new Error('Operation was cancelled')), false);
    assert.equal(f.witness.diagnostic(), diagnostic);
    const stages = ['engine_wrong_ca_webkit_cancelled_first_line', 'engine_wrong_host_webkit_cancelled_present']
      .map(base => checkedFailure(witnessStage(base, diagnostic), 'webkit'));
    assert.doesNotMatch(JSON.stringify(stages), /PRIVATE_SENTINEL|private\.invalid|secret|https?:|Operation|Unacceptable/);
    assert.equal(f.witness.stop(), false); // Poisoning remains mandatory.
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

// R1–R12: the one-shot proof adds evidence; it never accepts cancellation alone.
let hostFiles;
let caFiles;
const TEST_HOST_PROFILE = { hostname: 'sf08-wronghost.invalid', code: 'ERR_SSL_SSL/TLS_ALERT_BAD_CERTIFICATE' };
const TEST_CA_PROFILE = { hostname: 'localhost', code: 'ERR_SSL_TLSV1_ALERT_UNKNOWN_CA' };
function hostAPI(api = hostProof) {
  for (const name of ['readWebkitWrongHostCredentials', 'createWebkitWrongHostEndpoint', 'probeWebkitWrongHost'])
    assert.equal(typeof api[name], 'function', 'one-shot proof implementation required');
  return api;
}
function caAPI(api = hostProof) {
  for (const name of ['readWebkitWrongCACredentials', 'createWebkitWrongCAEndpoint', 'probeWebkitWrongCA'])
    assert.equal(typeof api[name], 'function', 'one-shot CA proof implementation required');
  return api;
}
function hostCertificateFiles() {
  if (hostFiles) return hostFiles;
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'sf08-one-shot-'));
  fs.chmodSync(work, 0o700);
  const owned = path.join(work, 'owned-unit'); fs.mkdirSync(owned, { mode: 0o700 });
  const script = 'import importlib.util,sys\nfrom pathlib import Path\ns=importlib.util.spec_from_file_location("fixed_fixture",sys.argv[1]);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);m.certificates(Path(sys.argv[2]),"positive")';
  const result = spawnSync('python3', ['-B', '-c', script,
    fileURLToPath(new URL('./context/fixture.py', import.meta.url)), owned], { stdio: 'ignore', timeout: 10000 });
  assert.equal(result.status, 0, 'owned certificate fixture unavailable');
  const ca = fs.readFileSync(path.join(owned, 'ca.pem'));
  const identity = fs.lstatSync(work);
  hostFiles = { work, owned, ca, caPath: path.join(owned, 'ca.pem') };
  process.once('exit', () => {
    const current = fs.lstatSync(work);
    if (current.dev === identity.dev && current.ino === identity.ino) fs.rmSync(work, { recursive: true });
  });
  return hostFiles;
}
function wrongCACertificateFiles() {
  if (caFiles) return caFiles;
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'sf08-one-shot-ca-'));
  fs.chmodSync(work, 0o700);
  const positive = path.join(work, 'owned-unit'), negativeDir = path.join(work, 'negative');
  fs.mkdirSync(positive, { mode: 0o700 }); fs.chmodSync(positive, 0o700);
  fs.mkdirSync(negativeDir, { mode: 0o700 }); fs.chmodSync(negativeDir, 0o700);
  const script = 'import importlib.util,sys\nfrom pathlib import Path\ns=importlib.util.spec_from_file_location("fixed_fixture",sys.argv[1]);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);m.certificates(Path(sys.argv[2]),sys.argv[3])';
  const fixture = fileURLToPath(new URL('./context/fixture.py', import.meta.url));
  for (const [directory, role] of [[positive, 'positive'], [negativeDir, 'wrong-ca']]) {
    const result = spawnSync('python3', ['-B', '-c', script, fixture, directory, role],
      { stdio: 'ignore', timeout: 10000 });
    assert.equal(result.status, 0, 'owned role certificate fixture unavailable');
  }
  const identity = fs.lstatSync(work);
  caFiles = { work, positive, negative: negativeDir,
    mainCA: fs.readFileSync(path.join(positive, 'ca.pem')),
    ca: fs.readFileSync(path.join(negativeDir, 'ca.pem')),
    caPath: path.join(negativeDir, 'ca.pem') };
  process.once('exit', () => {
    const current = fs.lstatSync(work);
    if (current.dev === identity.dev && current.ino === identity.ino) fs.rmSync(work, { recursive: true });
  });
  return caFiles;
}
class HostRaw extends EventEmitter {
  constructor() { super(); this.remoteAddress = '127.0.0.1'; this.remotePort = 31001; this.localAddress = '127.0.0.1'; this.localPort = 33005; this.destroyed = false; }
  destroy() { if (!this.destroyed) { this.destroyed = true; queueMicrotask(() => this.emit('close')); } }
}
class HostTLS extends HostRaw {}
async function hostPeerFixture(options = {}, api = hostProof) {
  const role = options.role ?? 'host';
  const profile = role === 'ca' ? TEST_CA_PROFILE : TEST_HOST_PROFILE;
  if (role === 'ca') caAPI(api); else hostAPI(api);
  const files = role === 'ca' ? wrongCACertificateFiles() : hostCertificateFiles();
  const credentials = role === 'ca'
    ? api.readWebkitWrongCACredentials(files.work, files.ca, files.mainCA)
    : api.readWebkitWrongHostCredentials(files.work, files.caPath, files.ca);
  options.credentialsSeen?.(credentials);
  let server, nativeOptions;
  const native = { TLSSocket: HostTLS, createSecureContext: () => { if (options.setupFailure) throw new Error('PRIVATE_SENTINEL'); return {}; }, createServer(opts) {
    nativeOptions = opts; server = new EventEmitter(); server.listening = false;
    server.address = () => ({ address: '127.0.0.1', family: 'IPv4', port: 33005 });
    server.listen = () => { if (options.listenFailure) throw new Error('PRIVATE_SENTINEL'); server.listening = true; queueMicrotask(() => server.emit('listening')); return server; };
    server.close = callback => { server.listening = false; if (!options.closeHang) queueMicrotask(() => { options.beforeServerClose?.(); server.emit('close'); callback(options.closeError ? new Error('PRIVATE_SENTINEL') : undefined); }); return server; };
    return server;
  } };
  const until = performance.now() + (options.budget ?? 1000);
  const endpoint = role === 'ca'
    ? await api.createWebkitWrongCAEndpoint(credentials, until, native, options.diagnose)
    : await api.createWebkitWrongHostEndpoint(credentials, until, native, options.diagnose);
  const raw = new HostRaw(), socket = new HostTLS();
  if (options.nodeMirroredErrors) {
    // Node 22 _tls_wrap installs these before SNICallback can own the TLS
    // socket. The first error listener forwards the SAME Error synchronously
    // through _tlsError/tlsClientError, then own(socket)'s error listener runs.
    let forwarded = false;
    const forward = error => {
      if (!forwarded) { forwarded = true; server.emit('tlsClientError', error, socket); }
    };
    socket.on('error', error => socket.emit('_tlsError', error));
    socket.on('_tlsError', forward);
    socket.on('error', forward);
  }
  function connect() { server.emit('connection', raw); }
  function sni(name = profile.hostname, receiver = socket) {
    nativeOptions.SNICallback.call(receiver, name, () => {});
  }
  function alert(code = profile.code, peer = socket, error = Object.assign(new Error('PRIVATE_SENTINEL'), { code })) {
    server.emit('tlsClientError', error, peer);
    return error;
  }
  return { endpoint, server, raw, socket, connect, sni, alert, until, profile, files, credentials };
}
function webkitConsoleError(url, overrides = {}) {
  const value = { type: 'error', text: 'Failed to load resource: Unacceptable TLS certificate',
    location: { url }, args: [], ...overrides };
  const method = item => typeof item === 'function' ? item : () => item;
  return { type: method(value.type), text: method(value.text),
    location: method(value.location), args: method(value.args) };
}
function hostBrowser(options = {}) {
  const role = options.role ?? 'host';
  const profile = role === 'ca' ? TEST_CA_PROFILE : TEST_HOST_PROFILE;
  const browser = new EventEmitter(), context = new EventEmitter(), page = new EventEmitter(), frame = {};
  browser.isConnected = () => !options.disconnected; page.isClosed = () => false; page.mainFrame = () => frame;
  browser.newContext = async config => { assert.deepEqual(config, { ignoreHTTPSErrors: false, serviceWorkers: 'block' }); return context; };
  context.newPage = async () => page;
  let contextsClosed = 0, pagesClosed = 0;
  let navigationWaiter, navigationTimer;
  const settleNavigation = (fulfilled, value) => {
    if (!navigationWaiter) return;
    const waiter = navigationWaiter; navigationWaiter = undefined;
    clearTimeout(navigationTimer);
    options.navigationSettled?.(fulfilled, value);
    (fulfilled ? waiter.resolve : waiter.reject)(value);
  };
  context.close = async () => { contextsClosed++; if (options.closeError) throw new Error('PRIVATE_SENTINEL'); context.emit('close'); };
  page.close = async () => {
    pagesClosed++; options.navigationOrder?.push('page-close');
    page.emit('close'); settleNavigation(false, new Error('Page closed'));
  };
  const navigate = async url => {
    options.navigationObserved?.(url);
    const request = { url: () => options.foreignURL ? url + 'PRIVATE_SENTINEL' : url,
      isNavigationRequest: () => !options.nonNavigation, frame: () => options.foreignFrame ? {} : frame,
      failure: () => ({ errorText: options.text ?? 'Operation was cancelled' }),
      response: options.response ?? (async () => null) };
    options.requestObserved?.(request);
    if (!options.missing) page.emit('requestfailed', request);
    if (options.duplicate) page.emit('requestfailed', request);
    if (options.httpResponse) page.emit('response', {});
    options.duringGoto?.({ browser, context, page, endpoint });
    if (options.resolved) return { status: () => 200 };
    throw options.error ?? new Error('Operation was cancelled');
  };
  page.goto = async url => { options.navigationOrder?.push('goto'); return navigate(url); };
  page.waitForNavigation = config => {
    options.navigationOrder?.push('wait');
    options.waiterOptions?.(config);
    if (options.waiterSetupError) throw new Error('PRIVATE_SENTINEL');
    assert.equal(navigationWaiter, undefined, 'one armed waiter');
    return new Promise((resolve, reject) => {
      navigationWaiter = { resolve, reject };
      navigationTimer = setTimeout(() => settleNavigation(false, new Error('Navigation timed out')), config.timeout);
    });
  };
  page.evaluate = async (action, url) => {
    options.navigationOrder?.push('evaluate');
    assert.match(String(action), /location\.assign\(target\)/);
    assert.equal(navigationWaiter === undefined, false, 'waiter armed before document trigger');
    if (options.evaluateError) throw new Error('PRIVATE_SENTINEL');
    if (options.evaluatePending) return new Promise(resolve => { options.releaseEvaluate?.(resolve); });
    if (!options.navigationPending) {
      navigate(url).then(value => settleNavigation(true, value), error => settleNavigation(false, error));
    }
    return options.evaluateValue;
  };
  const endpoint = options.endpoint ?? { url: `https://${profile.hostname}:33005/`,
    async finish() { options.duringJoin?.({ browser, context, page }); return options.peer !== false; },
    async close() { return options.join !== false; } };
  return { browser, context, page, endpoint, factory: async () => endpoint,
    closed: () => pagesClosed === 1 && contextsClosed === 1 };
}

test('document initiation prearms one waiter and triggers once before unchanged proof and join', async () => {
  const order = [], navigations = [], waiterOptions = [];
  const f = hostBrowser({ navigationOrder: order, navigationObserved: url => navigations.push(url),
    waiterOptions: value => waiterOptions.push(value),
    duringGoto: ({ page, endpoint }) => page.emit('console', webkitConsoleError(endpoint.url)) });
  const close = f.endpoint.close;
  f.endpoint.close = async () => { order.push('native-join'); return close(); };
  const rows = [];
  assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 10000,
    (...args) => rows.push(args)), true);
  assert.deepEqual(order, ['wait', 'evaluate', 'native-join', 'page-close']);
  assert.deepEqual(navigations, [f.endpoint.url]);
  assert.equal(waiterOptions.length, 1);
  assert.equal(waiterOptions[0].waitUntil, 'load');
  assert.ok(waiterOptions[0].timeout > 0 && waiterOptions[0].timeout <= 10000);
  assert.deepEqual(rows, []);
  assert.equal(f.closed(), true);
  const caOrder = [];
  const ca = hostBrowser({ role: 'ca', navigationOrder: caOrder,
    duringGoto: ({ page, endpoint }) => page.emit('console', webkitConsoleError(endpoint.url)) });
  assert.equal(await hostProof.probeWebkitWrongCA(ca.browser, ca.factory, performance.now() + 1000), true);
  assert.deepEqual(caOrder, ['goto', 'page-close'], 'legacy CA navigation stays unchanged');
});

test('document initiation cannot turn generic cancellation or unsafe navigation into certificate proof', async () => {
  for (const options of [
    { duringGoto: ({ page, endpoint }) => page.emit('console', webkitConsoleError(endpoint.url,
      { text: 'Failed to load resource: Operation was cancelled' })) },
    { httpResponse: true, resolved: true },
    { missing: true }, { duplicate: true }, { foreignURL: true }, { foreignFrame: true },
    { nonNavigation: true }, { peer: false }, { join: false },
    { evaluateValue: 'PRIVATE_SENTINEL' },
  ]) {
    const order = [];
    const f = hostBrowser({ navigationOrder: order,
      duringGoto: ({ page, endpoint }) => page.emit('console', webkitConsoleError(endpoint.url)), ...options });
    assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 1000), false);
    assert.equal(order.filter(value => value === 'evaluate').length, 1);
    assert.equal(order.includes('goto'), false);
    assert.equal(f.closed(), true);
  }
});

test('document initiation fault joins pending failure waiter after native join without unhandled rejection', async () => {
  const order = [], settled = [], unhandled = [];
  const onUnhandled = error => unhandled.push(error);
  process.on('unhandledRejection', onUnhandled);
  try {
    const f = hostBrowser({ evaluateError: true, navigationOrder: order,
      navigationSettled: fulfilled => settled.push(fulfilled) });
    const close = f.endpoint.close;
    f.endpoint.close = async () => { order.push('native-join'); return close(); };
    const rows = [];
    assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 1000,
      (kind, trace) => rows.push({ kind, trace })), false);
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(order, ['wait', 'evaluate', 'native-join', 'page-close']);
    assert.deepEqual(settled, [false]);
    assert.deepEqual(unhandled, []);
    assert.equal(f.closed(), true);
    assert.doesNotMatch(JSON.stringify(rows), /PRIVATE_SENTINEL/);
    const refusedOrder = [];
    const refused = hostBrowser({ waiterSetupError: true, navigationOrder: refusedOrder });
    assert.equal(await hostProof.probeWebkitWrongHost(refused.browser, refused.factory, performance.now() + 1000), false);
    assert.deepEqual(refusedOrder, ['wait', 'page-close'], 'no trigger after waiter setup refusal');
  } finally { process.off('unhandledRejection', onUnhandled); }
});

test('document initiation timeout and late proof retain original deadline and drain owned promises', { timeout: 2000 }, async () => {
  const settled = [], order = [], budgets = [];
  let releaseEvaluate;
  const f = hostBrowser({ evaluatePending: true, navigationOrder: order,
    releaseEvaluate: resolve => { releaseEvaluate = resolve; },
    navigationSettled: fulfilled => settled.push(fulfilled),
    waiterOptions: options => budgets.push(options.timeout) });
  const close = f.page.close;
  f.page.close = async () => { await close(); releaseEvaluate?.(); };
  const until = performance.now() + 35;
  assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, until), false);
  assert.equal(budgets.length, 1);
  assert.ok(budgets[0] > 0 && budgets[0] <= 35, 'waiter uses the original remaining budget');
  assert.equal(settled.length, 1);
  assert.equal(order.filter(value => value === 'evaluate').length, 1);
  assert.equal(f.closed(), true);
  // A stale callback after cleanup cannot create or resume another navigation.
  f.page.emit('console', webkitConsoleError(f.endpoint.url));
  assert.equal(order.filter(value => value === 'evaluate').length, 1);
  const pending = hostBrowser({ navigationPending: true });
  assert.equal(await hostProof.probeWebkitWrongHost(pending.browser, pending.factory, performance.now() + 30), false);
  assert.equal(pending.closed(), true);
});

test('one-shot R1 real EOF reset and idle are never certificate alerts', { timeout: 10000 }, async () => {
  hostAPI(); const files = hostCertificateFiles();
  for (const mode of ['eof', 'reset', 'idle']) {
    const until = performance.now() + 150;
    const endpoint = await hostProof.createWebkitWrongHostEndpoint(
      hostProof.readWebkitWrongHostCredentials(files.work, files.caPath, files.ca), until);
    const port = Number(new URL(endpoint.url).port);
    const socket = net.connect({ host: '127.0.0.1', port }); socket.on('error', () => {});
    await new Promise(resolve => socket.once('connect', resolve));
    if (mode === 'reset') socket.resetAndDestroy();
    else if (mode === 'eof') socket.end();
    assert.equal(await endpoint.finish(), false);
    socket.destroy(); await endpoint.close();
  }
});
test('one-shot R2 peer alert cannot replace bound main navigation or socket identity', async () => {
  for (const options of [{ missing: true }, { foreignURL: true }, { foreignFrame: true }, { nonNavigation: true }]) {
    hostAPI(); const f = hostBrowser(options);
    assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 1000), false);
    assert.equal(f.closed(), true);
  }
  const f = await hostPeerFixture(); f.connect(); f.sni(); f.alert(undefined, new HostTLS());
  assert.equal(await f.endpoint.finish(), false); await f.endpoint.close();
});
test('one-shot R3 only the exact observed peer alert code is eligible', async () => {
  for (const code of ['ERR_SSL_TLSV1_ALERT_UNKNOWN_CA', 'ERR_SSL_SSLV3_ALERT_CERTIFICATE_EXPIRED',
    'ERR_SSL_SSLV3_ALERT_CERTIFICATE_REVOKED', 'ERR_SSL_SSLV3_ALERT_HANDSHAKE_FAILURE',
    'ERR_SSL_TLSV1_ALERT_INTERNAL_ERROR', 'ERR_SSL_SSL/TLS_ALERT_BAD_CERTIFICATE_SUFFIX', 'ECONNRESET']) {
    const f = await hostPeerFixture(); f.connect(); f.sni(); f.alert(code);
    assert.equal(await f.endpoint.finish(), false); await f.endpoint.close();
  }
});
test('one-shot R4 exact certificate peer alert and either closed bound failure text settle once', async () => {
  const peer = await hostPeerFixture(); peer.connect(); peer.sni(); peer.alert();
  assert.equal(await peer.endpoint.finish(), true); assert.equal(await peer.endpoint.finish(), false);
  for (const text of ['Operation was cancelled', 'Unacceptable TLS certificate']) {
    hostAPI(); const f = hostBrowser({ text });
    assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 1000), true);
    assert.equal(f.closed(), true);
  }
  const f = hostBrowser({ peer: false });
  assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 1000), false);
});
test('one-shot R13 WebKit console ResourceError combines bound request and joined reset', { timeout: 5000 }, async () => {
  for (const role of ['host', 'ca']) {
    const peer = await hostPeerFixture({ role });
    const browser = hostBrowser({ role, endpoint: peer.endpoint, text: 'Unacceptable TLS certificate',
      duringGoto: ({ page }) => {
        assert.equal(page.listenerCount('console'), 1, 'console listener is installed before navigation');
        peer.connect(); peer.sni(); peer.alert('ECONNRESET');
        page.emit('console', webkitConsoleError(peer.endpoint.url));
      } });
    const probe = role === 'ca' ? hostProof.probeWebkitWrongCA : hostProof.probeWebkitWrongHost;
    assert.equal(await probe(browser.browser, browser.factory, performance.now() + 1000), true, role);
    assert.equal(browser.closed(), true);
  }
});

test('one-shot R13 exact native alert path remains valid without console witness', { timeout: 5000 }, async () => {
  const peer = await hostPeerFixture();
  peer.connect(); peer.sni(); peer.alert();
  const browser = hostBrowser({ endpoint: peer.endpoint, text: 'Unacceptable TLS certificate' });
  assert.equal(await hostProof.probeWebkitWrongHost(browser.browser, browser.factory,
    performance.now() + 1000), true);
  assert.equal(browser.closed(), true);
});

test('one-shot R18 console poison is branch-local only after the private exact native alert', { timeout: 8000 }, async () => {
  const cases = [
    { label: 'malformed before alert', console: (page, url) => page.emit('console', webkitConsoleError(url, { text: 'TLS error' })),
      beforeAlert: true },
    { label: 'accessor failure before alert', console: (page, url) => page.emit('console', webkitConsoleError(url, {
      type: () => { throw new Error('PRIVATE_SENTINEL'); },
    })), beforeAlert: true },
    { label: 'duplicate console after exact witness', console: (page, url) => {
      page.emit('console', webkitConsoleError(url));
      page.emit('console', webkitConsoleError(url));
    } },
  ];
  for (const role of ['host', 'ca']) for (const item of cases) {
    const peer = await hostPeerFixture({ role, nodeMirroredErrors: true });
    const browser = hostBrowser({ role, endpoint: peer.endpoint, text: 'Unacceptable TLS certificate',
      duringGoto: ({ page }) => {
        peer.connect(); peer.sni();
        if (item.beforeAlert) item.console(page, peer.endpoint.url);
        peer.alert();
        if (!item.beforeAlert) item.console(page, peer.endpoint.url);
      } });
    const probe = role === 'ca' ? hostProof.probeWebkitWrongCA : hostProof.probeWebkitWrongHost;
    assert.equal(await probe(browser.browser, browser.factory, peer.until), true, `${role}/${item.label}`);
    assert.equal(browser.closed(), true, `${role}/${item.label}`);
    assert.equal(peer.raw.destroyed && peer.socket.destroyed, true, `${role}/${item.label}: actual mock join`);
    assert.equal(peer.credentials.key, null, `${role}/${item.label}: key released after join`);
  }

  const exactVetoes = [
    { label: 'foreign bound request', browser: { foreignURL: true } },
    { label: 'HTTP response', browser: { httpResponse: true } },
    { label: 'global lifecycle veto', lifecycle: true },
    { label: 'failed private finish', nativeError: true },
    { label: 'failed actual join', peer: { closeHang: true, budget: 60 } },
  ];
  for (const item of exactVetoes) {
    const peer = await hostPeerFixture({ role: 'host', nodeMirroredErrors: true, ...item.peer });
    const browser = hostBrowser({ ...item.browser, endpoint: peer.endpoint, text: 'Unacceptable TLS certificate',
      duringGoto: ({ page }) => {
        page.emit('console', webkitConsoleError(peer.endpoint.url, { text: 'TLS error' }));
        peer.connect(); peer.sni();
        if (item.lifecycle) page.emit('crash');
        peer.alert(item.nativeError ? 'ERR_TLS_HANDSHAKE_TIMEOUT' : undefined);
      } });
    assert.equal(await hostProof.probeWebkitWrongHost(browser.browser, browser.factory, peer.until), false, item.label);
    assert.equal(browser.closed(), true, item.label);
  }

  let resetPage, resetPeer;
  resetPeer = await hostPeerFixture({ role: 'host', nodeMirroredErrors: true,
    beforeServerClose: () => resetPage.emit('console', webkitConsoleError(resetPeer.endpoint.url, { text: 'TLS error' })) });
  const resetBrowser = hostBrowser({ endpoint: resetPeer.endpoint, text: 'Unacceptable TLS certificate',
    duringGoto: ({ page }) => {
      resetPage = page;
      resetPeer.connect(); resetPeer.sni();
      page.emit('console', webkitConsoleError(resetPeer.endpoint.url));
      resetPeer.socket.emit('error', Object.assign(new Error('PRIVATE_SENTINEL'), { code: 'ECONNRESET' }));
    } });
  assert.equal(await hostProof.probeWebkitWrongHost(resetBrowser.browser, resetBrowser.factory, resetPeer.until), false,
    'a duplicate malformed console during the private reset join remains a veto after close');
  assert.equal(resetBrowser.closed(), true);
  assert.equal(resetPeer.credentials.key, null, 'reset endpoint join completes before refusing');

  let browser;
  let consoleListenersAtClose = 0;
  const stub = { url: 'https://sf08-wronghost.invalid:33005/',
    async finish() { return true; },
    async close() {
      consoleListenersAtClose = browser.page.listenerCount('console');
      browser.page.emit('console', webkitConsoleError(this.url, { text: 'TLS error' }));
      return true;
    } };
  browser = hostBrowser({ endpoint: stub, text: 'Operation was cancelled' });
  assert.equal(await hostProof.probeWebkitWrongHost(browser.browser, browser.factory,
    performance.now() + 1000), false, 'late console poison cannot be hidden by a custom successful close');
  assert.equal(consoleListenersAtClose, 1, 'console listener remains active through endpoint close');
});

test('one-shot R19 console poison is checked after synchronous listener-removal callbacks', async () => {
  let browser, consoleListenersAtRemoval = 0;
  const stub = { url: 'https://sf08-wronghost.invalid:33005/',
    async finish() { return true; }, async close() { return true; } };
  browser = hostBrowser({ endpoint: stub, text: 'Operation was cancelled' });
  const onRemoveListener = eventName => {
    if (eventName !== 'close') return;
    consoleListenersAtRemoval = browser.page.listenerCount('console');
    browser.page.emit('console', webkitConsoleError(stub.url, { text: 'TLS error' }));
  };
  browser.page.on('removeListener', onRemoveListener);
  const result = await hostProof.probeWebkitWrongHost(browser.browser, browser.factory, performance.now() + 1000);
  browser.page.off('removeListener', onRemoveListener);
  assert.equal(consoleListenersAtRemoval, 1, 'console listener is live during earlier listener removal');
  assert.equal(result, false, 'malformed console emitted by off() cannot bypass the final poison gate');
});

for (const role of ['host', 'ca']) test(`one-shot R16 Node mirrors the same bound reset Error before the owned TLS error listener (${role})`, { timeout: 5000 }, async () => {
  for (const consoleFirst of [false, true]) {
    const rows = [], peer = await hostPeerFixture({ role, nodeMirroredErrors: true,
      diagnose: kind => rows.push(kind) });
    const browser = hostBrowser({ role, endpoint: peer.endpoint, text: 'Unacceptable TLS certificate',
      duringGoto: ({ page }) => {
        if (consoleFirst) page.emit('console', webkitConsoleError(peer.endpoint.url));
        peer.connect(); peer.sni();
        peer.socket.emit('error', Object.assign(new Error('PRIVATE_SENTINEL'), { code: 'ECONNRESET' }));
        if (!consoleFirst) page.emit('console', webkitConsoleError(peer.endpoint.url));
      } });
    const probe = role === 'ca' ? hostProof.probeWebkitWrongCA : hostProof.probeWebkitWrongHost;
    assert.equal(await probe(browser.browser, browser.factory, peer.until), true, `${role}/${consoleFirst}`);
    assert.deepEqual(rows, [], 'a mirrored notification is not an independent failure');
    assert.equal(browser.closed(), true);
    assert.equal(peer.raw.destroyed && peer.socket.destroyed, true, 'actual owned socket destruction is joined');
    assert.equal(peer.credentials.key, null, 'credentials release waits for actual join');
    assert.equal(await peer.endpoint.finish(), false, 'proof is consumed once');
  }
});

test('one-shot R16 mirrored reset never authorizes other identities duplicates reordered errors or EOF', { timeout: 8000 }, async () => {
  const modes = ['different-error', 'raw-socket', 'different-tls', 'second-mirror', 'before-server',
    'changed-code', 'changed-tuple', 'duplicate-server', 'EOF', 'raw-EOF', 'prior-error', 'duplicate-SNI', 'duplicate-TCP'];
  for (const role of ['host', 'ca']) for (const mode of modes) {
    const rows = [], peer = await hostPeerFixture({ role, diagnose: kind => rows.push(kind) });
    const browser = hostBrowser({ role, endpoint: peer.endpoint, text: 'Unacceptable TLS certificate',
      duringGoto: ({ page }) => {
        const error = Object.assign(new Error('PRIVATE_SENTINEL'), { code: 'ECONNRESET' });
        peer.connect(); peer.sni();
        if (mode === 'prior-error') peer.raw.emit('error', new Error('PRIVATE_SENTINEL'));
        if (mode === 'duplicate-SNI') peer.sni();
        if (mode === 'duplicate-TCP') peer.connect();
        if (mode === 'before-server') peer.socket.emit('error', error);
        peer.alert('ECONNRESET', mode === 'different-tls' ? new HostTLS() : peer.socket, error);
        if (mode === 'changed-code') error.code = 'ERR_TLS_HANDSHAKE_TIMEOUT';
        if (mode === 'changed-tuple') peer.socket.remotePort++;
        if (mode === 'duplicate-server') peer.alert('ECONNRESET', peer.socket, error);
        if (mode === 'EOF') peer.socket.emit('end');
        if (mode === 'raw-EOF') peer.raw.emit('end');
        if (mode === 'raw-socket') peer.raw.emit('error', error);
        else peer.socket.emit('error', mode === 'different-error'
          ? Object.assign(new Error('PRIVATE_SENTINEL'), { code: 'ECONNRESET' }) : error);
        if (mode === 'second-mirror') peer.socket.emit('error', error);
        page.emit('console', webkitConsoleError(peer.endpoint.url));
      } });
    const probe = role === 'ca' ? hostProof.probeWebkitWrongCA : hostProof.probeWebkitWrongHost;
    assert.equal(await probe(browser.browser, browser.factory, peer.until), false, `${role}/${mode}`);
    assert.equal(browser.closed(), true, `${role}/${mode}`);
    assert.equal(rows.length, 1, `${role}/${mode}: first native veto stays latched`);
    assert.doesNotMatch(JSON.stringify(rows), /PRIVATE_SENTINEL|ECONNRESET/);
  }
});

test('one-shot R16 faithful mirror retains independent console request response lifecycle deadline and join vetoes', { timeout: 8000 }, async () => {
  const cases = [
    { label: 'console missing', console: false },
    { label: 'console malformed', console: { text: 'Unacceptable TLS certificate' } },
    { label: 'duplicate console', duplicateConsole: true },
    { label: 'foreign console', console: { location: { url: 'https://private.invalid/' } } },
    { label: 'request missing', missing: true }, { label: 'foreign URL', foreignURL: true },
    { label: 'foreign frame', foreignFrame: true }, { label: 'non-navigation', nonNavigation: true },
    { label: 'duplicate request', duplicate: true }, { label: 'HTTP response', httpResponse: true },
    { label: 'non-null response', response: async () => ({ status: () => 200 }) },
    { label: 'response throws', response: async () => { throw new Error('PRIVATE_SENTINEL'); } },
    { label: 'page crash', lifecycle: true }, { label: 'browser disconnected', disconnect: true },
    { label: 'lifecycle during actual join', lifecycleAtJoin: true },
    { label: 'application data', nativeData: true }, { label: 'secure connection', nativeSecure: true },
    { label: 'close error', closeError: true }, { label: 'close never joins', closeHang: true, budget: 50 },
    { label: 'original deadline', budget: 50,
      response: async () => { await new Promise(resolve => setTimeout(resolve, 80)); return null; } },
  ];
  for (const role of ['host', 'ca']) for (const item of cases) {
    let browser;
    const rows = [], peer = await hostPeerFixture({ role, nodeMirroredErrors: true,
      closeError: item.closeError, closeHang: item.closeHang, budget: item.budget ?? 1000,
      beforeServerClose: () => { if (item.lifecycleAtJoin) browser.page.emit('crash'); } });
    browser = hostBrowser({ ...item, role, endpoint: peer.endpoint, text: 'Unacceptable TLS certificate',
      duringGoto: ({ page, browser }) => {
        peer.connect(); peer.sni();
        peer.socket.emit('error', Object.assign(new Error('PRIVATE_SENTINEL'), { code: 'ECONNRESET' }));
        if (item.nativeData) peer.socket.emit('data', Buffer.from('PRIVATE_SENTINEL'));
        if (item.nativeSecure) peer.server.emit('secureConnection', peer.socket);
        if (item.console !== false) page.emit('console', webkitConsoleError(peer.endpoint.url, item.console));
        if (item.duplicateConsole) page.emit('console', webkitConsoleError(peer.endpoint.url));
        if (item.lifecycle) page.emit('crash');
        if (item.disconnect) browser.emit('disconnected');
      } });
    const probe = role === 'ca' ? hostProof.probeWebkitWrongCA : hostProof.probeWebkitWrongHost;
    assert.equal(await probe(browser.browser, browser.factory, peer.until, kind => rows.push(kind)), false, `${role}/${item.label}`);
    assert.equal(browser.closed(), true, `${role}/${item.label}`);
    assert.equal(rows.length, 1, `${role}/${item.label}`);
    assert.doesNotMatch(JSON.stringify(rows), /PRIVATE_SENTINEL|private\.invalid/);
  }
});

test('one-shot R16 standalone reset stays false and mirrored exact native alerts stay unchanged', { timeout: 5000 }, async () => {
  for (const role of ['host', 'ca']) {
    const reset = await hostPeerFixture({ role, nodeMirroredErrors: true });
    reset.connect(); reset.sni();
    reset.socket.emit('error', Object.assign(new Error('PRIVATE_SENTINEL'), { code: 'ECONNRESET' }));
    assert.equal(await reset.endpoint.finish(), false, `${role}: public finish cannot arm reset proof`);
    await reset.endpoint.close();
    const exact = await hostPeerFixture({ role, nodeMirroredErrors: true });
    exact.connect(); exact.sni();
    exact.socket.emit('error', Object.assign(new Error('PRIVATE_SENTINEL'), { code: exact.profile.code }));
    assert.equal(await exact.endpoint.finish(), true, `${role}: exact native alert still settles without console`);
    await exact.endpoint.close();
  }
});

test('one-shot R13 reset and console remain jointly bound and veto poisoned evidence', { timeout: 8000 }, async () => {
  const cases = [
    { label: 'reset only', native: 'reset', diagnostic: 'deadline' },
    { label: 'console only', console: [true], budget: 80 },
    { label: 'console without terminal after owned connection', native: 'idle', console: [true], budget: 250 },
    { label: 'generic console', native: 'reset', console: [{ text: 'TLS error' }], diagnostic: 'console_malformed' },
    { label: 'partial certificate text', native: 'reset', console: [{ text: 'Unacceptable TLS certificate' }], diagnostic: 'console_malformed' },
    { label: 'foreign console URL', native: 'reset', console: [{ location: { url: 'https://private.invalid/' } }] },
    { label: 'console arguments', native: 'reset', console: [{ args: [{}] }] },
    { label: 'console type accessor throws', native: 'reset', console: [{ type: () => { throw new Error('PRIVATE_SENTINEL'); } }] },
    { label: 'console method throws', native: 'reset', console: [{ text: () => { throw new Error('PRIVATE_SENTINEL'); } }] },
    { label: 'console location accessor throws', native: 'reset', console: [{ location: () => { throw new Error('PRIVATE_SENTINEL'); } }] },
    { label: 'console args accessor throws', native: 'reset', console: [{ args: () => { throw new Error('PRIVATE_SENTINEL'); } }] },
    { label: 'duplicate console', native: 'reset', console: [true, true] },
    { label: 'foreign page console', native: 'reset', foreignPage: true },
    { label: 'foreign navigation', native: 'reset', console: [true], foreignURL: true },
    { label: 'foreign frame navigation', native: 'reset', console: [true], foreignFrame: true },
    { label: 'non-navigation request', native: 'reset', console: [true], nonNavigation: true },
    { label: 'duplicate navigation', native: 'reset', console: [true], duplicate: true },
    { label: 'response veto', native: 'reset', console: [true], httpResponse: true },
    { label: 'non-null failed response', native: 'reset', console: [true], response: async () => ({ status: () => 200 }) },
    { label: 'lifecycle veto', native: 'reset', console: [true], lifecycle: true },
    { label: 'browser disconnect veto', native: 'reset', console: [true], disconnect: true },
    { label: 'secure connection veto', native: 'secure', console: [true] },
    { label: 'data veto', native: 'data', console: [true] },
    { label: 'EOF veto', native: 'eof', console: [true] },
    { label: 'other native error veto', native: 'other', console: [true] },
    { label: 'wrong SNI veto', native: 'wrong_sni', console: [true] },
    { label: 'wrong TCP tuple veto', native: 'wrong_tuple', console: [true] },
    { label: 'wrong TLS socket veto', native: 'wrong_socket', console: [true] },
    { label: 'duplicate native terminal veto', native: 'duplicate', console: [true] },
  ];
  for (const item of cases) {
    const diagnostics = [];
    const peer = await hostPeerFixture({ role: 'host', budget: item.budget ?? 1000 });
    const browser = hostBrowser({ endpoint: peer.endpoint, text: 'Unacceptable TLS certificate',
      foreignURL: item.foreignURL, foreignFrame: item.foreignFrame, nonNavigation: item.nonNavigation,
      duplicate: item.duplicate, httpResponse: item.httpResponse, response: item.response,
      duringGoto: ({ page }) => {
        for (const event of item.console ?? []) {
          if (event === true) page.emit('console', webkitConsoleError(peer.endpoint.url));
          else page.emit('console', webkitConsoleError(peer.endpoint.url, event));
        }
        if (item.foreignPage) new EventEmitter().emit('console', webkitConsoleError(peer.endpoint.url));
        if (item.lifecycle) page.emit('crash');
        if (item.disconnect) browser.browser.emit('disconnected');
        if (item.native === 'reset' || item.native === 'duplicate' || item.native === 'secure' ||
            item.native === 'data' || item.native === 'eof' || item.native === 'other' ||
            item.native === 'idle' || item.native === 'wrong_sni' || item.native === 'wrong_tuple' ||
            item.native === 'wrong_socket') {
          if (item.native === 'wrong_tuple') peer.raw.remotePort++;
          peer.connect();
          peer.sni(item.native === 'wrong_sni' ? 'wrong.example' : undefined);
          if (item.native === 'secure') peer.server.emit('secureConnection', peer.socket);
          if (item.native === 'data') peer.socket.emit('data', Buffer.from('private'));
          if (item.native === 'eof') peer.socket.emit('end');
          if (item.native === 'reset') peer.alert('ECONNRESET');
          if (item.native === 'duplicate') { peer.alert('ECONNRESET'); peer.alert('ECONNRESET'); }
          if (item.native === 'secure' || item.native === 'data' || item.native === 'eof') peer.alert('ECONNRESET');
          if (item.native === 'other') peer.alert('ERR_TLS_HANDSHAKE_TIMEOUT');
          if (item.native === 'wrong_sni' || item.native === 'wrong_tuple') peer.alert('ECONNRESET');
          if (item.native === 'wrong_socket') peer.alert('ECONNRESET', new HostTLS());
        }
      } });
    assert.equal(await hostProof.probeWebkitWrongHost(browser.browser, browser.factory,
      performance.now() + (item.budget ?? 1000), kind => diagnostics.push(kind)), false, item.label);
    assert.equal(browser.closed(), true, item.label);
    if (item.diagnostic) assert.deepEqual(diagnostics, [item.diagnostic], item.label);
    assert.doesNotMatch(JSON.stringify(diagnostics), /PRIVATE_SENTINEL|private\.invalid/, item.label);
  }
  for (const [endpointRole, probeRole] of [['host', 'ca'], ['ca', 'host']]) {
    const peer = await hostPeerFixture({ role: endpointRole });
    const browser = hostBrowser({ role: endpointRole, endpoint: peer.endpoint });
    const probe = probeRole === 'ca' ? hostProof.probeWebkitWrongCA : hostProof.probeWebkitWrongHost;
    assert.equal(await probe(browser.browser, browser.factory, performance.now() + 1000), false,
      `${endpointRole} endpoint cannot satisfy ${probeRole} role`);
    assert.equal(browser.closed(), true, `${endpointRole} endpoint cleanup`);
  }
});

test('one-shot R17 delayed console ordering waits only for exact proof within the original deadline', { timeout: 8000 }, async () => {
  const cases = [
    { label: 'joined host reset then exact console', role: 'host', event: 'exact', budget: 700, expected: true },
    { label: 'joined CA reset then exact console', role: 'ca', event: 'exact', budget: 700, expected: true },
    { label: 'joined reset without console', event: 'absent', budget: 140, expected: false },
    { label: 'joined reset then malformed console', event: 'malformed', budget: 700, expected: false },
    { label: 'joined reset then malformed then exact console', event: 'malformed_then_exact', budget: 700, expected: false },
    { label: 'joined reset then foreign console URL', event: 'foreign', budget: 700, expected: false },
    { label: 'joined reset then console args', event: 'args', budget: 700, expected: false },
    { label: 'joined reset then duplicate console', event: 'duplicate', budget: 700, expected: false },
    { label: 'joined reset then wrong console type', event: 'wrong_type', budget: 700, expected: false },
    { label: 'joined reset then response veto', event: 'response', budget: 700, expected: false },
    { label: 'joined reset then duplicate request while waiting', event: 'request_duplicate', budget: 700, expected: false },
    { label: 'joined reset then response veto after exact console', event: 'console_response', budget: 700, expected: false },
    { label: 'joined reset then duplicate request after exact console', event: 'console_request', budget: 700, expected: false },
    { label: 'joined reset then lifecycle veto after exact console', event: 'console_lifecycle', budget: 700, expected: false },
    { label: 'no native binding then late exact console', event: 'exact', nativeUnbound: true, budget: 700, expected: false },
    { label: 'exact console after the original deadline', event: 'after_deadline', budget: 220, expected: false },
  ];
  const timeoutMarker = Symbol('probe-still-pending');
  for (const item of cases) {
    const role = item.role ?? 'host';
    let page, observedRequest, probeSettled = false, settledBeforeEvent, joinedBeforeEvent = false;
    let lateTimer;
    const immediates = new Set();
    const watchdogs = new Set();
    let resolveDelivery;
    const delivery = new Promise(resolve => { resolveDelivery = resolve; });
    const withWatchdog = (promise, timeout, fallback = false) => new Promise(resolve => {
      let handle;
      const finish = value => {
        clearTimeout(handle);
        watchdogs.delete(handle);
        resolve(value);
      };
      handle = setTimeout(() => finish(fallback), timeout);
      watchdogs.add(handle);
      Promise.resolve(promise).then(finish, () => finish(false));
    });
    const scheduleImmediate = callback => {
      let handle;
      handle = setImmediate(() => { immediates.delete(handle); callback(); });
      immediates.add(handle);
    };
    let peer;
    const dispatch = () => {
      joinedBeforeEvent = peer.credentials.key === null && peer.raw.destroyed && peer.socket.destroyed;
      settledBeforeEvent = probeSettled;
      if (page) {
        if (['exact', 'after_deadline', 'console_lifecycle', 'console_response', 'console_request'].includes(item.event))
          page.emit('console', webkitConsoleError(peer.endpoint.url));
        else if (item.event === 'malformed' || item.event === 'malformed_then_exact')
          page.emit('console', webkitConsoleError(peer.endpoint.url, { text: 'TLS error' }));
        else if (item.event === 'foreign')
          page.emit('console', webkitConsoleError(peer.endpoint.url, { location: { url: 'https://foreign.invalid/' } }));
        else if (item.event === 'args')
          page.emit('console', webkitConsoleError(peer.endpoint.url, { args: [{}] }));
        else if (item.event === 'duplicate') {
          page.emit('console', webkitConsoleError(peer.endpoint.url));
          page.emit('console', webkitConsoleError(peer.endpoint.url));
        } else if (item.event === 'wrong_type')
          page.emit('console', webkitConsoleError(peer.endpoint.url, { type: 'log' }));
        else if (item.event === 'response') page.emit('response', {});
        else if (item.event === 'request_duplicate') page.emit('requestfailed', observedRequest);
        if (item.event === 'malformed_then_exact') page.emit('console', webkitConsoleError(peer.endpoint.url));
        if (item.event === 'console_response') page.emit('response', {});
        if (item.event === 'console_request') page.emit('requestfailed', observedRequest);
        if (item.event === 'console_lifecycle') page.emit('crash');
      }
      resolveDelivery(true);
    };
    peer = await hostPeerFixture({ role, budget: item.budget, beforeServerClose() {
      if (item.event === 'absent') return;
      if (item.event === 'after_deadline') {
        lateTimer = setTimeout(dispatch, item.budget + 40);
        return;
      }
      // The first immediate is queued by the server-close callback; the second
      // runs after the fixture's destroy turn and socket-close microtasks. For
      // the bound-reset cases, credential release proves native join completed.
      scheduleImmediate(() => scheduleImmediate(dispatch));
    } });
    const browser = hostBrowser({ role, endpoint: peer.endpoint, text: 'Unacceptable TLS certificate',
      requestObserved: request => { observedRequest = request; },
      duringGoto: ({ page: current }) => {
        page = current;
        peer.connect();
        if (item.nativeUnbound) { peer.sni('wrong.example'); peer.alert('ECONNRESET'); }
        else { peer.sni(); peer.alert('ECONNRESET'); }
      } });
    const diagnostics = [], eventTraces = [];
    const probePromise = (role === 'ca' ? hostProof.probeWebkitWrongCA : hostProof.probeWebkitWrongHost)(
      browser.browser, browser.factory, peer.until, (kind, trace) => { diagnostics.push(kind); eventTraces.push(trace); })
      .then(value => { probeSettled = true; return value; });
    let result;
    try {
      if (item.event === 'absent' || item.event === 'after_deadline' || item.nativeUnbound) {
        result = await probePromise;
        if (item.event !== 'absent') {
          const delivered = await withWatchdog(delivery, 2500);
          assert.equal(delivered, true, `${item.label}: delayed fixture callback must settle`);
          if (!item.nativeUnbound)
            assert.equal(joinedBeforeEvent, true, `${item.label}: native resources joined before event`);
        }
      } else {
        const delivered = await withWatchdog(delivery, 2500);
        assert.equal(delivered, true, `${item.label}: delayed fixture callback must settle`);
        assert.equal(joinedBeforeEvent, true, `${item.label}: native endpoint joined before event`);
        assert.equal(settledBeforeEvent, false, `${item.label}: probe waits for this event`);
        const outcome = await withWatchdog(probePromise, 250, timeoutMarker);
        const prompt = outcome !== timeoutMarker;
        result = prompt ? outcome : await probePromise;
        assert.equal(prompt, true, `${item.label}: console or hard veto wakes the bounded wait`);
      }
    } finally {
      if (lateTimer) clearTimeout(lateTimer);
      for (const handle of watchdogs) clearTimeout(handle);
      watchdogs.clear();
      for (const handle of immediates) clearImmediate(handle);
      resolveDelivery(false);
    }
    if (item.event === 'exact' && !item.nativeUnbound) assert.deepEqual(diagnostics, [], 'exact delayed proof diagnostic');
    assert.equal(result, item.expected, item.label);
    assert.equal(browser.closed(), true, `${item.label}: browser cleanup`);
    assert.equal(peer.credentials.key, null, `${item.label}: native join releases credentials`);
    assert.doesNotMatch(JSON.stringify(diagnostics), /PRIVATE_SENTINEL|foreign\.invalid/);
    if (item.expected === false) {
      assert.equal(eventTraces.length, 1, `${item.label}: one bounded failure trace`);
      assert.match(eventTraces[0], /^[ETXLAZUIRMNGVSFBYWDPCJKHOQ]{0,32}_0$/);
      const codes = eventTraces[0].slice(0, -2);
      assert.equal(codes.endsWith('O'), true, 'owned observation ended before intentional teardown');
      if (item.event === 'absent') {
        assert.deepEqual(diagnostics, ['deadline']);
        assert.match(codes, /FWD/, 'finish observed true then original wait expired');
        assert.doesNotMatch(codes, /[ETXLAZUI]/, 'complete original-window observation without a console callback');
      }
      for (const [mode, code] of [['malformed', 'X'], ['foreign', 'L'], ['args', 'A'], ['wrong_type', 'T'],
        ['duplicate', 'U'], ['console_response', 'E'], ['console_request', 'E'], ['console_lifecycle', 'E']]) {
        if (item.event === mode) assert.ok(codes.indexOf(code) > codes.indexOf('F'), `${mode}: callback observed after native finish`);
      }
      if (item.event === 'request_duplicate' || item.event === 'console_request')
        assert.equal(codes.split('R').length - 1, 2, 'both requestfailed callbacks retained');
    } else assert.deepEqual(eventTraces, [], 'successful probe does not emit failure telemetry');
    if (item.event === 'after_deadline') assert.equal(settledBeforeEvent, true, 'deadline refuses before late event');
  }
});

test('one-shot event observations classify once, cap at32, and never serialize console input', async () => {
  const variants = [
    { override: {}, expected: 'E' },
    { override: { type: 'log' }, expected: 'T' },
    { override: { text: 'PRIVATE_SENTINEL' }, expected: 'X' },
    { override: { location: { url: 'https://private.invalid/' } }, expected: 'L' },
    { override: { args: ['PRIVATE_SENTINEL'] }, expected: 'A' },
    { override: { text: () => { throw new Error('PRIVATE_SENTINEL'); } }, expected: 'Z' },
  ];
  for (const item of variants) {
    const records = [];
    const f = hostBrowser({ join: false, duringGoto: ({ page, endpoint }) => {
      page.emit('console', webkitConsoleError(endpoint.url, item.override));
    } });
    assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 1000,
      (kind, trace) => records.push({ kind, trace })), false);
    assert.equal(records.length, 1);
    assert.equal(typeof records[0].trace, 'string', 'a failure records its bounded event sequence');
    const codes = records[0].trace.slice(0, -2);
    assert.equal([...codes].filter(code => 'ETXLAZUI'.includes(code)).join(''), item.expected);
    if (item.expected === 'E') assert.ok(codes.indexOf('E') < codes.indexOf('F'), 'console before observed finish');
    assert.doesNotMatch(JSON.stringify(records), /PRIVATE_SENTINEL|private\.invalid/);
  }
  const records = [];
  const f = hostBrowser({ duringGoto: ({ page, endpoint }) => {
    for (let index = 0; index < 40; index++) page.emit('console', webkitConsoleError(endpoint.url));
  } });
  assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 1000,
    (kind, trace, fingerprint, timing) => records.push({ kind, trace, fingerprint, timing })), false);
  assert.equal(records.length, 1);
  // Console poison is no longer the global bad bit, so duplicate callbacks
  // remain truthfully classified as U through the same 32-event cap.
  assert.match(records[0].trace, /^GRMEU{28}_1$/,
    'first32 events only; overflow forbids complete-count/absence claims');
  assert.equal(records[0].trace.length, 34);
  assert.equal(records[0].timing.length, 96, 'overflow keeps exactly the first 32 aligned time samples');
  assert.equal(hostProof.validWebkitEventTiming(records[0].trace, records[0].timing), true);
});

test('wrong-host event timing is monotonic, failure-only, bounded, and aligned', async () => {
  const records = [];
  const f = hostBrowser({
    response: async () => { await new Promise(resolve => setTimeout(resolve, 70)); return null; },
    duringGoto: ({ page, endpoint }) => {
      page.emit('console', webkitConsoleError(endpoint.url, { text: 'TLS error' }));
    },
  });
  assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 2000,
    (kind, trace, fingerprint, timing) => records.push({ kind, trace, fingerprint, timing })), false);
  assert.equal(records.length, 1);
  assert.equal(records[0].kind, 'console_malformed');
  assert.equal(typeof records[0].timing, 'string');
  assert.equal(hostProof.validWebkitEventTiming(records[0].trace, records[0].timing), true);
  const codes = records[0].trace.slice(0, -2);
  assert.equal(records[0].timing.length, codes.length * 3, 'one fixed-width sample per retained event');
  const elapsed = Array.from({ length: codes.length }, (_, index) => {
    const token = records[0].timing.slice(index * 3, index * 3 + 3);
    return token === 'zzz' ? null : Number.parseInt(token, 36);
  });
  let previous = -1;
  for (const value of elapsed) {
    if (value === null) continue;
    assert.ok(value >= previous, 'known monotonic samples do not decrease');
    previous = value;
  }
  const navigationRejected = codes.indexOf('N'), finishStarted = codes.indexOf('S');
  assert.ok(navigationRejected >= 0 && finishStarted > navigationRejected);
  assert.ok(elapsed[finishStarted] - elapsed[navigationRejected] >= 50,
    'the bounded mock delay is visible between navigation rejection and endpoint finish');
  assert.doesNotMatch(JSON.stringify(records), /TLS error|private\.invalid|https:\/\//);

  const peer = await hostPeerFixture({ role: 'host' });
  peer.connect(); peer.sni(); peer.alert();
  const exact = hostBrowser({ endpoint: peer.endpoint, text: 'Unacceptable TLS certificate' });
  const successDiagnostics = [];
  assert.equal(await hostProof.probeWebkitWrongHost(exact.browser, exact.factory, peer.until,
    (...values) => successDiagnostics.push(values)), true);
  assert.deepEqual(successDiagnostics, [], 'successful proof emits no timing metadata');
});

test('probeEngine emits a timed WebKit failure checkpoint accepted by Python', async () => {
  const consoleText = 'Failed to load resource: Operation was cancelled';
  const type = fake('webkit', 'normal');
  const dedicated = hostBrowser({ text: 'Operation was cancelled',
    duringGoto: ({ page, endpoint }) => page.emit('console', webkitConsoleError(endpoint.url, { text: consoleText })) });
  const launch = type.launch.bind(type); let contexts = 0;
  type.launch = async options => {
    const browser = await launch(options), newContext = browser.newContext.bind(browser);
    browser.newContext = options => ++contexts === 1 ? newContext(options) : dedicated.browser.newContext(options);
    return browser;
  };
  const checkpoints = [];
  await assert.rejects(probeEngine('webkit', type, settings(provisioned, ready, negative),
    (stage, engine) => checkpoints.push({ stage, engine }), undefined, async () => dedicated.endpoint));
  const observed = checkpoints.find(({ stage }) => stage.includes('_events_') && stage.includes('_ms_'));
  assert.ok(observed, 'owned console mismatch records the temporary timing suffix');
  assert.equal(observed.engine, 'webkit');
  assert.deepEqual(checkedFailure(observed.stage, observed.engine),
    { schema_version: 1, stage: observed.stage, engine: 'webkit' });
  const match = observed.stage.match(/^engine_wrong_host_one_shot_console_malformed_events_([ETXLAZUIRMNGVSFBYWDPCJKHOQ]{0,32}_[01])_text_([a-f0-9]{64})_ms_([0-9a-z]*)$/);
  assert.ok(match, 'timing is restricted to the actual owned console-mismatch path');
  assert.equal(match[2], createHash('sha256').update(consoleText).digest('hex'));
  assert.equal(hostProof.validWebkitEventTiming(match[1], match[3]), true);
  assert.equal(dedicated.closed(), true, 'failure still joins and closes the owned context');
  const validator = 'import sys\nsys.path.insert(0, sys.argv[1])\nimport failure_checkpoint as f\nf.validate({"schema_version": 1, "stage": sys.argv[2], "engine": "webkit"})';
  const py = spawnSync('python3', ['-B', '-c', validator,
    path.dirname(fileURLToPath(import.meta.url)), observed.stage], { encoding: 'utf8' });
  assert.equal(py.status, 0, py.stderr || 'Python rejected the generated checkpoint');
});

test('text fingerprint is bounded owned failure metadata and never certificate proof', async () => {
  const text = 'Failed to load resource: unit PRIVATE_SENTINEL transport failure';
  let locationReads = 0;
  const accessor = {};
  Object.defineProperty(accessor, 'url', { get() { locationReads++; throw new Error('PRIVATE_SENTINEL'); } });
  const variants = [
    { name: 'owned bounded text', override: { text }, expected: createHash('sha256').update(text).digest('hex') },
    { name: 'oversize text', override: { text: 'x'.repeat(1025) } },
    { name: 'oversize UTF8', override: { text: '界'.repeat(400) } },
    { name: 'foreign location', override: { text, location: { url: 'https://private.invalid/' } } },
    { name: 'accessor location', override: { text, location: accessor } },
    { name: 'arguments present', override: { text, args: ['PRIVATE_SENTINEL'] } },
    { name: 'wrong type', override: { text, type: 'log' } },
  ];
  for (const item of variants) {
    const records = [];
    const f = hostBrowser({ duringGoto: ({ page, endpoint }) => {
      page.emit('console', webkitConsoleError(endpoint.url, item.override));
    } });
    assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 1000,
      (kind, trace, fingerprint) => records.push({ kind, trace, fingerprint })), false, item.name);
    assert.equal(records.length, 1);
    assert.equal(records[0].kind, 'console_malformed');
    assert.equal(records[0].fingerprint, item.expected, item.name);
    assert.match(records[0].trace, /^[ETXLAZUIRMNGVSFBYWDPCJKHOQ]{0,32}_0$/);
    assert.doesNotMatch(JSON.stringify(records), /PRIVATE_SENTINEL|private\.invalid/);
  }
  assert.equal(locationReads, 0, 'diagnostics never invoke a location getter skipped by the original text comparison');
});

test('bound request error class is diagnostic only and never substitutes for native proof', async () => {
  for (const text of ['Operation was cancelled', 'Unacceptable TLS certificate']) {
    const records = [];
    const f = hostBrowser({ text, peer: false });
    assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 1000,
      (kind, trace) => records.push({ kind, trace })), false, 'failed native finish remains a refusal');
    assert.equal(records.length, 1);
    assert.equal(records[0].trace.includes('Q'), text === 'Unacceptable TLS certificate');
    assert.match(records[0].trace, /GRMQ?NSBJKO_0/);
  }
});

test('one-shot R5 duplicate missing wrong SNI TCP and request events veto', async () => {
  for (const mode of ['no_sni', 'wrong_sni', 'duplicate_sni', 'duplicate_tcp', 'bad_receiver', 'wrong_tuple']) {
    const f = await hostPeerFixture(); f.connect();
    if (mode === 'wrong_tuple') f.socket.remotePort++;
    if (mode !== 'no_sni') f.sni(mode === 'wrong_sni' ? 'localhost' : undefined,
      mode === 'bad_receiver' ? {} : undefined);
    if (mode === 'duplicate_sni') f.sni();
    if (mode === 'duplicate_tcp') f.server.emit('connection', new HostRaw());
    f.alert(); assert.equal(await f.endpoint.finish(), false); await f.endpoint.close();
  }
  const f = hostBrowser({ duplicate: true });
  assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 1000), false);
});
test('one-shot R6 secure application HTTP and non-null or uncertain response queries veto', async () => {
  const f = await hostPeerFixture(); f.connect(); f.sni(); f.server.emit('secureConnection', f.socket); f.alert();
  assert.equal(await f.endpoint.finish(), false); await f.endpoint.close();
  for (const options of [{ httpResponse: true }, { resolved: true }, { response: async () => undefined },
    { response: async () => ({}) }, { response: async () => { throw new Error('PRIVATE_SENTINEL'); } },
    { response: () => new Promise(() => {}) }]) {
    const browser = hostBrowser(options);
    assert.equal(await hostProof.probeWebkitWrongHost(browser.browser, browser.factory, performance.now() + 35), false);
  }
});
test('one-shot R7 error shape and lifecycle veto through query and join', async () => {
  hostAPI();
  for (const error of [{}, ...['TimeoutError', 'TargetClosedError', 'AbortError'].map(name => Object.assign(new Error('PRIVATE_SENTINEL'), { name }))]) {
    const f = hostBrowser({ error });
    assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 1000), false);
  }
  for (const during of ['duringGoto', 'duringJoin']) for (const target of ['page', 'context', 'browser']) {
    const f = hostBrowser({ [during]: objects => objects[target].emit(target === 'browser' ? 'disconnected' : 'close') });
    assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 1000), false);
  }
});
test('one-shot R8 deadline setup join and cleanup never recover late success', async () => {
  hostAPI(); const f = hostBrowser();
  assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() - 1), false);
  for (const options of [{ join: false }, { closeError: true }, { text: 'PRIVATE_SENTINEL' }]) {
    const b = hostBrowser(options);
    assert.equal(await hostProof.probeWebkitWrongHost(b.browser, b.factory, performance.now() + 35), false);
  }
  const peer = await hostPeerFixture({ budget: 35, closeHang: true }); peer.connect(); peer.sni(); peer.alert();
  assert.equal(await peer.endpoint.finish(), false); assert.equal(await peer.endpoint.close(), false);
});
test('one-shot R9 fixed owned credential files reject path mode link and crypto violations', async () => {
  hostAPI(); const f = hostCertificateFiles();
  const read = () => hostProof.readWebkitWrongHostCredentials(f.work, f.caPath, f.ca);
  assert.throws(() => hostProof.readWebkitWrongHostCredentials(f.work, f.caPath + '/PRIVATE_SENTINEL', f.ca), /^Error: SF08 browser trust preflight refused$/);
  assert.throws(() => hostProof.readWebkitWrongHostCredentials(f.work, f.caPath, Buffer.from('PRIVATE_SENTINEL')), /^Error: SF08 browser trust preflight refused$/);
  for (const name of ['ca.pem', 'leaf.pem', 'leaf.key']) {
    const filename = path.join(f.owned, name), original = fs.readFileSync(filename);
    try {
      for (const mode of [0o644, 0o4600]) {
        fs.chmodSync(filename, mode); assert.throws(read, /^Error: SF08 browser trust preflight refused$/);
      }
      fs.chmodSync(filename, 0o600);
      fs.renameSync(filename, filename + '.saved');
      try {
        fs.symlinkSync(filename + '.saved', filename); assert.throws(read, /^Error: SF08 browser trust preflight refused$/);
        fs.unlinkSync(filename);
        assert.equal(spawnSync('mkfifo', ['-m', '600', filename], { stdio: 'ignore', timeout: 1000 }).status, 0);
        assert.throws(read, /^Error: SF08 browser trust preflight refused$/); fs.unlinkSync(filename);
      } finally {
        if (fs.existsSync(filename)) fs.unlinkSync(filename);
        fs.renameSync(filename + '.saved', filename);
      }
      fs.linkSync(filename, filename + '.hard'); assert.throws(read, /^Error: SF08 browser trust preflight refused$/); fs.unlinkSync(filename + '.hard');
      fs.writeFileSync(filename, Buffer.alloc(16385)); assert.throws(read, /^Error: SF08 browser trust preflight refused$/);
      fs.writeFileSync(filename, Buffer.alloc(0)); assert.throws(read, /^Error: SF08 browser trust preflight refused$/);
    } finally { fs.writeFileSync(filename, original, { mode: 0o600 }); fs.chmodSync(filename, 0o600); }
  }
  // Metadata controls exercise the actual reader around real opened files.
  const altered = (info, changes) => Object.assign(Object.create(info), changes);
  for (const kind of ['uid', 'device', 'root_inode', 'owned_inode', 'growth', 'mtime', 'close']) {
    let readOccurred = false, lastBuffer;
    const tracked = new Map();
    const io = { ...fs,
      openSync(filename, flags) { const fd = fs.openSync(filename, flags); tracked.set(fd, String(filename)); return fd; },
      fstatSync(fd, options) {
        const info = fs.fstatSync(fd, options);
        if (kind === 'uid') return altered(info, { uid: info.uid + 1n });
        if (kind === 'device' && info.isFile()) return altered(info, { dev: info.dev + 1n });
        if (readOccurred && info.isFile() && ['growth', 'mtime'].includes(kind))
          return altered(info, kind === 'growth' ? { size: info.size + 1n } : { mtimeNs: info.mtimeNs + 1n });
        return info;
      },
      lstatSync(filename, options) {
        const info = fs.lstatSync(filename, options);
        return ((kind === 'root_inode' && filename === f.work) || (kind === 'owned_inode' && filename === f.owned))
          ? altered(info, { ino: info.ino + 1n }) : info;
      },
      readSync(fd, data, ...args) { readOccurred = true; lastBuffer = data; return fs.readSync(fd, data, ...args); },
      closeSync(fd) { fs.closeSync(fd); if (kind === 'close' && tracked.get(fd)?.endsWith('/leaf.key')) throw new Error('PRIVATE_SENTINEL'); },
    };
    assert.throws(() => hostProof.readWebkitWrongHostCredentials(f.work, f.caPath, f.ca, io),
      /^Error: SF08 browser trust preflight refused$/);
    if (lastBuffer) assert.equal(lastBuffer.every(byte => byte === 0), true, 'failed read clears bounded buffer');
  }
  const keyPath = path.join(f.owned, 'leaf.key'), key = fs.readFileSync(keyPath);
  try {
    const foreign = generateKeyPairSync('ec', { namedCurve: 'prime256v1' }).privateKey.export({ type: 'pkcs8', format: 'pem' });
    fs.writeFileSync(keyPath, foreign); assert.throws(read, /^Error: SF08 browser trust preflight refused$/);
  } finally { fs.writeFileSync(keyPath, key); }
  const leafPath = path.join(f.owned, 'leaf.pem'), leaf = fs.readFileSync(leafPath);
  const extension = path.join(f.owned, 'controlled.ext');
  try {
    for (const mode of ['wrong_host_san', 'client_usage', 'foreign_signature']) {
      fs.writeFileSync(extension, 'basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=' +
        (mode === 'client_usage' ? 'clientAuth' : 'serverAuth') +
        '\nsubjectAltName=DNS:localhost,DNS:app.example.invalid,IP:127.0.0.1' +
        (mode === 'wrong_host_san' ? ',DNS:sf08-wronghost.invalid' : '') + '\n', { mode: 0o600 });
      const args = ['x509', '-req', '-in', 'leaf.csr', '-out', 'leaf.pem', '-days', '1', '-extfile', 'controlled.ext'];
      args.push(...(mode === 'foreign_signature' ? ['-signkey', 'leaf.key'] : ['-CA', 'ca.pem', '-CAkey', 'ca.key', '-CAcreateserial']));
      assert.equal(spawnSync('openssl', args, { cwd: f.owned, stdio: 'ignore', timeout: 10000 }).status, 0, mode);
      assert.throws(read, /^Error: SF08 browser trust preflight refused$/);
    }
  } finally { fs.writeFileSync(leafPath, leaf); fs.rmSync(extension, { force: true }); }
  const originalNow = Date.now;
  try {
    const signedLeaf = new X509Certificate(leaf);
    Date.now = () => Date.parse(signedLeaf.validTo) + 1;
    assert.throws(read, /^Error: SF08 browser trust preflight refused$/);
    Date.now = () => Date.parse(signedLeaf.validFrom) - 1;
    assert.throws(read, /^Error: SF08 browser trust preflight refused$/);
  } finally { Date.now = originalNow; }
  const credentials = read(); assert.throws(() => JSON.stringify(credentials), /^Error: SF08 browser trust preflight refused$/);
  const heldKey = credentials.key; credentials.release(); assert.equal(heldKey.every(byte => byte === 0), true);
});
test('one-shot R9 CA role binds distinct source trust and fixed owned negative credentials', async () => {
  caAPI(); const f = wrongCACertificateFiles();
  const read = () => hostProof.readWebkitWrongCACredentials(f.work, f.ca, f.mainCA);
  const negative = new X509Certificate(f.ca), main = new X509Certificate(f.mainCA);
  assert.equal(negative.ca, true); assert.equal(main.ca, true);
  assert.equal(negative.raw.equals(main.raw), false);
  assert.notEqual(negative.subject, main.subject); assert.notEqual(negative.issuer, main.issuer);
  assert.equal(negative.publicKey.export({ type: 'spki', format: 'der' })
    .equals(main.publicKey.export({ type: 'spki', format: 'der' })), false);
  assert.throws(() => hostProof.readWebkitWrongCACredentials(f.work, f.ca, f.ca), /^Error: SF08 browser trust preflight refused$/);
  assert.throws(() => hostProof.readWebkitWrongCACredentials(f.work, f.mainCA, f.ca), /^Error: SF08 browser trust preflight refused$/);
  assert.throws(() => hostProof.readWebkitWrongCACredentials(f.work, f.ca, Buffer.from('PRIVATE_SENTINEL')),
    /^Error: SF08 browser trust preflight refused$/);
  const altDir = path.join(f.positive, 'ca-role-negative-control'); fs.mkdirSync(altDir, { mode: 0o700 });
  const runRoot = (key, subject, output) => {
    const result = spawnSync('openssl', ['req', '-x509', '-key', key, '-out', output, '-days', '2',
      '-subj', subject, '-addext', 'basicConstraints=critical,CA:TRUE',
      '-addext', 'keyUsage=critical,keyCertSign,cRLSign'], { stdio: 'ignore', timeout: 10000 });
    assert.equal(result.status, 0, 'local alternate CA control fixture unavailable');
    return fs.readFileSync(output);
  };
  const sameName = runRoot(path.join(f.positive, 'ca.key'), '/CN=Nagi owned ephemeral SF08 wrong-ca CA', path.join(altDir, 'same-name.pem'));
  const sameKey = runRoot(path.join(f.negative, 'ca.key'), '/CN=Different owned CA', path.join(altDir, 'same-key.pem'));
  const sameNameCert = new X509Certificate(sameName), sameKeyCert = new X509Certificate(sameKey);
  assert.notEqual(sameNameCert.raw.equals(negative.raw), true);
  assert.equal(sameNameCert.subject, negative.subject); assert.equal(sameNameCert.issuer, negative.issuer);
  assert.equal(sameKeyCert.publicKey.export({ type: 'spki', format: 'der' })
    .equals(negative.publicKey.export({ type: 'spki', format: 'der' })), true);
  assert.throws(() => hostProof.readWebkitWrongCACredentials(f.work, f.ca, sameName),
    /^Error: SF08 browser trust preflight refused$/);
  assert.throws(() => hostProof.readWebkitWrongCACredentials(f.work, f.ca, sameKey),
    /^Error: SF08 browser trust preflight refused$/);
  const negativeDirectory = f.negative, originalDirectoryMode = fs.statSync(negativeDirectory).mode & 0o7777;
  try { fs.chmodSync(negativeDirectory, 0o755); assert.throws(read, /^Error: SF08 browser trust preflight refused$/); }
  finally { fs.chmodSync(negativeDirectory, originalDirectoryMode); }
  const caPath = path.join(negativeDirectory, 'ca.pem');
  fs.linkSync(caPath, caPath + '.hard');
  try { assert.throws(read, /^Error: SF08 browser trust preflight refused$/); }
  finally { fs.unlinkSync(caPath + '.hard'); }
  const credentials = read(); assert.throws(() => JSON.stringify(credentials), /^Error: SF08 browser trust preflight refused$/);
  const heldKey = credentials.key; credentials.release(); assert.equal(heldKey.every(byte => byte === 0), true);
});
test('one-shot CA credentials are role sealed and exact peer code alone can settle', async () => {
  caAPI();
  const caFiles = wrongCACertificateFiles(), hostFiles = hostCertificateFiles();
  const caCredentials = hostProof.readWebkitWrongCACredentials(caFiles.work, caFiles.ca, caFiles.mainCA);
  const hostCredentials = hostProof.readWebkitWrongHostCredentials(hostFiles.work, hostFiles.caPath, hostFiles.ca);
  await assert.rejects(hostProof.createWebkitWrongHostEndpoint(caCredentials, performance.now() + 1000),
    /^Error: SF08 browser trust preflight refused$/);
  await assert.rejects(hostProof.createWebkitWrongCAEndpoint(hostCredentials, performance.now() + 1000),
    /^Error: SF08 browser trust preflight refused$/);
  caCredentials.release(); hostCredentials.release();
  const accepted = await hostPeerFixture({ role: 'ca' });
  accepted.connect(); accepted.sni('localhost'); accepted.alert('ERR_SSL_TLSV1_ALERT_UNKNOWN_CA');
  assert.equal(await accepted.endpoint.finish(), true);
  assert.equal(await accepted.endpoint.finish(), false, 'native one-shot peer evidence is consumed once');
  assert.equal(await accepted.endpoint.close(), true);
  for (const code of ['ERR_SSL_SSL/TLS_ALERT_BAD_CERTIFICATE', 'ERR_SSL_SSLV3_ALERT_BAD_CERTIFICATE',
    'ERR_SSL_SSLV3_ALERT_CERTIFICATE_UNKNOWN', 'ERR_SSL_TLSV1_ALERT_CERTIFICATE_UNKNOWN',
    'ERR_SSL_TLSV1_ALERT_UNKNOWN_CA_SUFFIX', 'ECONNRESET', 'ERR_SSL_TLSV1_ALERT_UNKNOWN_CA']) {
    if (code === 'ERR_SSL_TLSV1_ALERT_UNKNOWN_CA') continue;
    const rejected = await hostPeerFixture({ role: 'ca' }); rejected.connect(); rejected.sni('localhost'); rejected.alert(code);
    assert.equal(await rejected.endpoint.finish(), false, code); await rejected.endpoint.close();
  }
  for (const error of [{ code: 'ERR_SSL_TLSV1_ALERT_UNKNOWN_CA' },
    Object.assign(new Error('PRIVATE_SENTINEL'), { code: 7 })]) {
    const rejected = await hostPeerFixture({ role: 'ca' }); rejected.connect(); rejected.sni('localhost');
    rejected.alert('ERR_SSL_TLSV1_ALERT_UNKNOWN_CA', rejected.socket, error);
    assert.equal(await rejected.endpoint.finish(), false); await rejected.endpoint.close();
  }
  for (const [mode, code, expected] of [['legacy', 'ERR_SSL_SSLV3_ALERT_BAD_CERTIFICATE', 'peer_legacy_certificate'],
    ['binding_first', 'ERR_SSL_SSLV3_ALERT_BAD_CERTIFICATE', 'peer_binding']]) {
    const rows = [], rejected = await hostPeerFixture({ role: 'ca', diagnose: value => rows.push(value) });
    rejected.connect(); rejected.sni(mode === 'binding_first' ? 'sf08-wronghost.invalid' : 'localhost'); rejected.alert(code);
    assert.equal(await rejected.endpoint.finish(), false); await rejected.endpoint.close();
    assert.deepEqual(rows, [expected]); assert.equal(JSON.stringify(rows).includes('PRIVATE_SENTINEL'), false);
  }
});
test('one-shot CA request lifecycle deadline join and host-binding vetoes remain strict', async () => {
  caAPI();
  for (const options of [{ missing: true }, { duplicate: true }, { foreignURL: true },
    { foreignFrame: true }, { nonNavigation: true }, { text: 'PRIVATE_SENTINEL' },
    { httpResponse: true }, { response: async () => undefined }, { response: async () => ({}) },
    { response: async () => { throw new Error('PRIVATE_SENTINEL'); } },
    { response: () => new Promise(() => {}) },
    { duringGoto: ({ page }) => page.emit('crash') },
    { duringJoin: ({ page }) => page.emit('crash') },
    { duringJoin: ({ context }) => context.emit('close') },
    { duringJoin: ({ browser }) => browser.emit('disconnected') }, { join: false }, { closeError: true }]) {
    const f = hostBrowser({ role: 'ca', ...options });
    assert.equal(await probeWebkitWrongCA(f.browser, f.factory, performance.now() + 1000), false,
      JSON.stringify(Object.keys(options)));
    assert.equal(f.closed(), true);
  }
  for (const text of ['Operation was cancelled', 'Unacceptable TLS certificate']) {
    const f = hostBrowser({ role: 'ca', text });
    assert.equal(await probeWebkitWrongCA(f.browser, f.factory, performance.now() + 1000), true,
      'bounded browser cancellation still requires the separately joined one-shot endpoint');
  }
  const wrongRole = hostBrowser();
  assert.equal(await probeWebkitWrongCA(wrongRole.browser, wrongRole.factory, performance.now() + 1000), false,
    'wrong-host endpoint cannot satisfy the CA hostname binding');
  const expired = hostBrowser({ role: 'ca' });
  assert.equal(await probeWebkitWrongCA(expired.browser, expired.factory, performance.now() - 1), false);
  const diagnostic = hostBrowser({ role: 'ca', peer: false }), rows = [];
  assert.equal(await probeWebkitWrongCA(diagnostic.browser, diagnostic.factory,
    performance.now() + 1000, value => rows.push(value)), false);
  assert.deepEqual(rows, ['peer_binding']);
});
test('one-shot CA endpoint rejects missing or foreign SNI TCP identity and lifecycle facts', async () => {
  caAPI();
  for (const mode of ['no_sni', 'wrong_sni', 'duplicate_sni', 'duplicate_tcp', 'bad_receiver', 'wrong_tuple']) {
    const f = await hostPeerFixture({ role: 'ca' }); f.connect();
    if (mode === 'wrong_tuple') f.socket.remotePort++;
    if (mode !== 'no_sni') f.sni(mode === 'wrong_sni' ? 'sf08-wronghost.invalid' : 'localhost',
      mode === 'bad_receiver' ? {} : undefined);
    if (mode === 'duplicate_sni') f.sni('localhost');
    if (mode === 'duplicate_tcp') f.server.emit('connection', new HostRaw());
    f.alert('ERR_SSL_TLSV1_ALERT_UNKNOWN_CA');
    assert.equal(await f.endpoint.finish(), false, mode); await f.endpoint.close();
  }
  const noJoin = await hostPeerFixture({ role: 'ca', closeHang: true, budget: 35 });
  noJoin.connect(); noJoin.sni('localhost'); noJoin.alert('ERR_SSL_TLSV1_ALERT_UNKNOWN_CA');
  assert.equal(await noJoin.endpoint.finish(), false); assert.equal(await noJoin.endpoint.close(), false);
});
test('one-shot R10 Node post-handshake hostname close remains a refusal', { timeout: 10000 }, async () => {
  hostAPI(); const f = hostCertificateFiles();
  const endpoint = await hostProof.createWebkitWrongHostEndpoint(
    hostProof.readWebkitWrongHostCredentials(f.work, f.caPath, f.ca), performance.now() + 1000);
  const client = tls.connect({ host: '127.0.0.1', port: Number(new URL(endpoint.url).port),
    servername: 'sf08-wronghost.invalid', ca: f.ca, rejectUnauthorized: true });
  client.on('error', () => {});
  assert.equal(await endpoint.finish(), false); client.destroy(); await endpoint.close();
});
test('one-shot R11 removing independent proof vetoes makes their negative control fail', async t => {
  hostAPI();
  const source = fs.readFileSync(new URL('./preflight.mjs', import.meta.url), 'utf8');
  const mutant = async (before, after) => {
    assert.equal(source.split(before).length, 2, 'unique mutation anchor');
    return import('data:text/javascript;base64,' + Buffer.from(source.replace(before, after)).toString('base64'));
  };
  for (const [anchor, replacement, options] of [
    ['request.url() !== endpoint.url || ', '', { foreignURL: true }],
    ['requests = Math.min(2, requests + 1);', 'requests = 1;', { duplicate: true }],
    ["for (const event of ['close', 'crash']) listen(page, event, () => hardVeto('lifecycle'));", '',
      { duringJoin: ({ page }) => page.emit('crash') }],
    ['function hostBudget(until) {\n  const remaining = Math.floor(until - performance.now());', 'function hostBudget(until) {\n  const remaining = 1000;', { expired: true }],
    ['if (joined !== true) good = false;', '', { join: false }],
    ['if (finished !== true) refuse();', '', { peer: false }],
  ]) {
    const original = hostBrowser(options), changed = hostBrowser(options);
    const until = () => performance.now() + (options.expired ? -1 : 1000);
    assert.equal(await hostProof.probeWebkitWrongHost(original.browser, original.factory, until()), false);
    const api = await mutant(anchor, replacement);
    assert.equal(await api.probeWebkitWrongHost(changed.browser, changed.factory, until()), true,
      'negative oracle detects removed veto');
  }
  for (const [anchor, replacement, action] of [
    ['error.code !== profile.code || bad', 'bad', f => { f.connect(); f.sni(); f.alert('ECONNRESET'); }],
    ['sniCount !== 1 || name !== profile.hostname || ', 'sniCount !== 1 || ', f => { f.connect(); f.sni('localhost'); f.alert(); }],
  ]) {
    const original = await hostPeerFixture(); action(original);
    assert.equal(await original.endpoint.finish(), false); await original.endpoint.close();
    const api = await mutant(anchor, replacement), changed = await hostPeerFixture({}, api); action(changed);
    assert.equal(await changed.endpoint.finish(), true, 'peer negative oracle detects removed veto'); await changed.endpoint.close();
  }
  for (const [caseId, anchor, replacement, action] of [
    ['ca_code', 'error.code !== profile.code || bad', 'bad', f => {
      f.connect(); f.sni('localhost'); f.alert('ERR_SSL_SSL/TLS_ALERT_BAD_CERTIFICATE');
    }],
    ['ca_sni', 'sniCount !== 1 || name !== profile.hostname || ', 'sniCount !== 1 || ', f => {
      f.connect(); f.sni('sf08-wronghost.invalid'); f.alert('ERR_SSL_TLSV1_ALERT_UNKNOWN_CA');
    }],
  ]) {
    const original = await hostPeerFixture({ role: 'ca' }); action(original);
    assert.equal(await original.endpoint.finish(), false); await original.endpoint.close();
    const api = await mutant(anchor, replacement), observed = [];
    const changed = await hostPeerFixture({ role: 'ca', diagnose: kind => {
      observed.push(['deadline', 'join', 'socket', 'peer_binding', 'response', 'setup'].includes(kind) ? kind : 'other');
    } }, api);
    const remaining = () => Math.max(0, Math.min(1000, Math.floor(changed.until - performance.now())));
    const atCreate = remaining(), began = performance.now();
    action(changed);
    const atAction = remaining(), beforeFinish = remaining();
    const proved = await changed.endpoint.finish();
    const facts = { case: caseId, first: observed[0] ?? 'none', atCreate, atAction, beforeFinish,
      afterFinish: remaining(), elapsed: Math.max(0, Math.min(10000, Math.floor(performance.now() - began))),
      listening: changed.server.listening, rawDestroyed: changed.raw.destroyed, tlsDestroyed: changed.socket.destroyed };
    t.diagnostic('ca_mutant=' + JSON.stringify(facts));
    assert.equal(proved, true, 'CA role negative oracle detects removed exact-code/SNI veto ' + JSON.stringify(facts));
    await changed.endpoint.close();
  }
});
test('one-shot R12 source-bound integration and original certificate oracle stay strict', async () => {
  hostAPI();
  const f = certificateWitnessFixture({ failure: () => ({ errorText: 'Operation was cancelled' }) });
  f.page.emit('requestfailed', f.request);
  assert.equal(await f.witness.refused(new Error('Operation was cancelled')), false); f.witness.stop();
  for (const name of ['chromium', 'firefox', 'webkit']) {
    const type = fake(name, 'normal'), caDedicated = hostBrowser({ role: 'ca' }), hostDedicated = hostBrowser();
    const launch = type.launch.bind(type); let contexts = 0, factories = 0;
    type.launch = async options => {
      const browser = await launch(options), newContext = browser.newContext.bind(browser);
      browser.newContext = options => {
        contexts++;
        return contexts === 1 ? newContext(options)
          : contexts === 2 ? caDedicated.browser.newContext(options) : hostDedicated.browser.newContext(options);
      };
      return browser;
    };
    let caFactories = 0;
    const result = await probeEngine(name, type, settings(provisioned, ready, negative), () => {}, undefined,
      async until => { factories++; assert.ok(until > performance.now() && until <= performance.now() + 10000); return hostDedicated.endpoint; },
      async until => { caFactories++; assert.ok(until > performance.now() && until <= performance.now() + 10000); return caDedicated.endpoint; });
    assert.deepEqual(result, { engine: name, positive_sans: 3, wrong_ca_refused: true, wrong_host_refused: true });
    assert.deepEqual(type.trace.urls, [...settings(provisioned, ready, negative).origins.map(origin => origin + '/'),
      ...(name === 'webkit' ? [] : [negative.caOrigin + '/', negative.hostOrigin + '/'])]);
    assert.equal(factories, name === 'webkit' ? 1 : 0);
    assert.equal(caFactories, name === 'webkit' ? 1 : 0);
    assert.equal(contexts, name === 'webkit' ? 3 : 1);
    if (name === 'webkit') {
      assert.equal(caDedicated.closed(), true); assert.equal(hostDedicated.closed(), true);
      assert.equal(type.trace.urls.some(url => url === negative.caOrigin + '/' || url === negative.hostOrigin + '/'), false,
        'both WebKit negatives use their selected one-shot endpoint directly');
    }
  }
  await assert.rejects(hostProof.runPreflight({}, provisioned, ready, negative), /^Error: SF08 browser trust preflight refused$/);
});

test('WebKit CA one-shot failure has no legacy navigation or host fallback', async () => {
  const type = fake('webkit', 'normal'), caDedicated = hostBrowser({ role: 'ca' }), hostDedicated = hostBrowser();
  const launch = type.launch.bind(type); let contexts = 0, caFactories = 0, hostFactories = 0;
  type.launch = async options => {
    const browser = await launch(options), newContext = browser.newContext.bind(browser);
    browser.newContext = options => {
      contexts++;
      return contexts === 1 ? newContext(options)
        : contexts === 2 ? caDedicated.browser.newContext(options) : hostDedicated.browser.newContext(options);
    };
    return browser;
  };
  await assert.rejects(probeEngine('webkit', type, settings(provisioned, ready, negative), () => {}, undefined,
    async () => { hostFactories++; return hostDedicated.endpoint; },
    async () => { caFactories++; return hostDedicated.endpoint; }),
  /^Error: SF08 browser trust preflight refused$/);
  assert.equal(caFactories, 1); assert.equal(hostFactories, 0);
  assert.deepEqual(type.trace.urls, settings(provisioned, ready, negative).origins.map(origin => origin + '/'));
  assert.equal(caDedicated.closed(), true);
  assert.equal(hostDedicated.closed(), false, 'host branch must not run after the selected CA proof fails');
});

test('omitting the CA factory preserves the legacy CA oracle while host one-shot stays selected', async () => {
  const type = fake('webkit', 'normal'), hostDedicated = hostBrowser();
  const launch = type.launch.bind(type); let contexts = 0, hostFactories = 0;
  type.launch = async options => {
    const browser = await launch(options), newContext = browser.newContext.bind(browser);
    browser.newContext = options => ++contexts === 1 ? newContext(options) : hostDedicated.browser.newContext(options);
    return browser;
  };
  const result = await probeEngine('webkit', type, settings(provisioned, ready, negative), () => {}, undefined,
    async () => { hostFactories++; return hostDedicated.endpoint; });
  assert.equal(result.wrong_ca_refused, true); assert.equal(result.wrong_host_refused, true);
  assert.equal(hostFactories, 1);
  assert.equal(type.trace.urls.includes(negative.caOrigin + '/'), true,
    'compatibility control must exercise the old strict CA navigation when no CA factory exists');
  assert.equal(type.trace.urls.includes(negative.hostOrigin + '/'), false);
  assert.equal(hostDedicated.closed(), true);
});

test('one-shot diagnostics report one fixed failure and never report success', async () => {
  const cases = [
    [{ missing: true }, 'request'], [{ foreignURL: true }, 'request'], [{ text: 'PRIVATE_SENTINEL' }, 'request'],
    [{ response: async () => undefined }, 'response'],
    [{ error: Object.assign(new Error('PRIVATE_SENTINEL'), { name: 'TimeoutError' }) }, 'navigation'],
    [{ duringJoin: ({ page }) => page.emit('crash') }, 'lifecycle'], [{ peer: false }, 'peer_binding'],
    [{ join: false }, 'join'], [{ closeError: true }, 'cleanup'],
  ];
  for (const [options, expected] of cases) {
    const without = hostBrowser(options), withDiagnostic = hostBrowser(options), rows = [];
    assert.equal(await hostProof.probeWebkitWrongHost(without.browser, without.factory, performance.now() + 1000), false);
    assert.equal(await hostProof.probeWebkitWrongHost(withDiagnostic.browser, withDiagnostic.factory, performance.now() + 1000,
      value => rows.push(value)), false);
    assert.deepEqual(rows, [expected]);
    assert.equal(JSON.stringify(rows).includes('PRIVATE_SENTINEL'), false);
  }
  for (const text of ['Operation was cancelled', 'Unacceptable TLS certificate']) {
    const f = hostBrowser({ text }), rows = [];
    assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 1000, value => rows.push(value)), true);
    assert.deepEqual(rows, []);
  }
  const expired = hostBrowser(), rows = [];
  assert.equal(await hostProof.probeWebkitWrongHost(expired.browser, expired.factory, performance.now() - 1, value => rows.push(value)), false);
  assert.deepEqual(rows, ['deadline']);
});

test('one-shot diagnostics separate legacy peer code from binding and other errors without accepting it', async () => {
  for (const [code, expected] of [['ERR_SSL_SSLV3_ALERT_BAD_CERTIFICATE', 'peer_legacy_certificate'],
    ['ECONNRESET', 'peer_connection_reset'], ['ERR_TLS_HANDSHAKE_TIMEOUT', 'peer_handshake_timeout'],
    ['PRIVATE_SENTINEL_ERR_SSL_SSLV3_ALERT_BAD_CERTIFICATE', 'peer_other_code']]) {
    const rows = [], f = await hostPeerFixture({ diagnose: value => rows.push(value) });
    f.connect(); f.sni(); f.alert(code);
    assert.equal(await f.endpoint.finish(), false); await f.endpoint.close();
    assert.deepEqual(rows, [expected]);
    assert.equal(JSON.stringify(rows).includes('PRIVATE_SENTINEL'), false);
  }
  const rows = [], f = await hostPeerFixture({ diagnose: value => rows.push(value) });
  f.connect(); f.sni('PRIVATE_SENTINEL'); f.alert('ERR_SSL_SSLV3_ALERT_BAD_CERTIFICATE');
  assert.equal(await f.endpoint.finish(), false); await f.endpoint.close();
  assert.deepEqual(rows, ['peer_binding'], 'first veto wins over later peer detail');
});

test('one-shot native peer classifies fixed non-SSL error facts without accepting or echoing them', async () => {
  const cases = [
    ['ECONNRESET', 'peer_connection_reset'],
    ['ERR_TLS_HANDSHAKE_TIMEOUT', 'peer_handshake_timeout'],
    ['ECONNREFUSED', 'peer_other_code'],
    ['PRIVATE_SENTINEL', 'peer_other_code'],
    ['ERR_SSL_PRIVATE\n', 'peer_other_code'],
    [7, 'peer_other_code'],
    [true, 'peer_other_code'],
    [Symbol('PRIVATE_SENTINEL'), 'peer_other_code'],
    [undefined, 'peer_no_native_code'],
    [null, 'peer_no_native_code'],
  ];
  for (const role of ['ca', 'host']) {
    for (const [code, expected] of cases) {
      const rows = [], f = await hostPeerFixture({ role, diagnose: value => rows.push(value) });
      f.connect(); f.sni();
      const error = new Error('PRIVATE_SENTINEL');
      Object.defineProperty(error, 'code', { value: code });
      f.server.emit('tlsClientError', error, f.socket);
      assert.equal(await f.endpoint.finish(), false, `${role}:${expected} cannot prove refusal`);
      await f.endpoint.close();
      assert.deepEqual(rows, [expected], `${role}:${String(expected)}`);
      assert.equal(JSON.stringify(rows).includes('PRIVATE_SENTINEL'), false);
    }
    const noCodeRows = [], noCode = await hostPeerFixture({ role, diagnose: value => noCodeRows.push(value) });
    noCode.connect(); noCode.sni();
    noCode.server.emit('tlsClientError', new Error('PRIVATE_SENTINEL'), noCode.socket);
    assert.equal(await noCode.endpoint.finish(), false);
    await noCode.endpoint.close();
    assert.deepEqual(noCodeRows, ['peer_no_native_code']);

    const nonErrorRows = [], nonError = await hostPeerFixture({ role, diagnose: value => nonErrorRows.push(value) });
    let nonErrorCodeReads = 0;
    const notError = { get code() { nonErrorCodeReads++; throw new Error('PRIVATE_SENTINEL'); } };
    nonError.connect(); nonError.sni(); nonError.server.emit('tlsClientError', notError, nonError.socket);
    assert.equal(await nonError.endpoint.finish(), false);
    await nonError.endpoint.close();
    assert.equal(nonErrorCodeReads, 0, 'non-Error payload properties are never read');
    assert.deepEqual(nonErrorRows, ['peer_non_error']);

    let objectCodeCoercions = 0;
    const objectRows = [], objectCode = await hostPeerFixture({ role, diagnose: value => objectRows.push(value) });
    objectCode.connect(); objectCode.sni();
    objectCode.server.emit('tlsClientError', Object.assign(new Error('PRIVATE_SENTINEL'), {
      code: { toString() { objectCodeCoercions++; throw new Error('PRIVATE_SENTINEL'); } },
    }), objectCode.socket);
    assert.equal(await objectCode.endpoint.finish(), false);
    await objectCode.endpoint.close();
    assert.equal(objectCodeCoercions, 0, 'non-primitive codes are never coerced');
    assert.deepEqual(objectRows, ['peer_other']);
  }

  const bindingRows = [], binding = await hostPeerFixture({ role: 'ca', diagnose: value => bindingRows.push(value) });
  binding.connect(); binding.sni('sf08-wronghost.invalid');
  binding.alert('ECONNRESET');
  assert.equal(await binding.endpoint.finish(), false);
  await binding.endpoint.close();
  assert.deepEqual(bindingRows, ['peer_binding'], 'the initial binding veto wins');

  const deadlineRows = [], deadline = await hostPeerFixture({ role: 'ca', budget: 35, diagnose: value => deadlineRows.push(value) });
  deadline.connect(); deadline.sni();
  await new Promise(resolve => setTimeout(resolve, 40));
  deadline.alert('ECONNRESET');
  assert.equal(await deadline.endpoint.finish(), false);
  await deadline.endpoint.close();
  assert.deepEqual(deadlineRows, ['deadline'], 'late non-SSL facts cannot replace the deadline veto');
});

test('one-shot finish labels missing peer facts as binding failure without claiming proof', async () => {
  const rows = [], f = await hostPeerFixture({ role: 'ca', diagnose: value => rows.push(value) });
  assert.equal(await f.endpoint.close(), true);
  assert.equal(await f.endpoint.finish(), false);
  assert.deepEqual(rows, ['peer_binding']);
});

test('one-shot diagnostics cannot turn callback throws or malformed facts into successful evidence', async () => {
  for (const observer of [undefined, null, false, true, {}, Symbol('PRIVATE_SENTINEL'), () => { throw new Error('PRIVATE_SENTINEL'); }]) {
    const f = hostBrowser({ peer: false });
    assert.equal(await hostProof.probeWebkitWrongHost(f.browser, f.factory, performance.now() + 1000, observer), false);
  }
  for (const first of ['credential', 'setup', 'PRIVATE_SENTINEL']) {
    const f = hostBrowser(), rows = [];
    const factory = async (_until, diagnose) => { diagnose(first); throw new Error('PRIVATE_SENTINEL'); };
    assert.equal(await hostProof.probeWebkitWrongHost(f.browser, factory, performance.now() + 1000, value => rows.push(value)), false);
    assert.deepEqual(rows, [first === 'PRIVATE_SENTINEL' ? 'setup' : first]);
  }
  const f = hostBrowser(), rows = [];
  const factory = async (_until, diagnose) => { diagnose('peer_other'); return f.endpoint; };
  assert.equal(await hostProof.probeWebkitWrongHost(f.browser, factory, performance.now() + 1000, value => rows.push(value)), true);
  assert.deepEqual(rows, [], 'even a supplied diagnostic latch cannot be emitted on success');
});


test('one-shot diagnostics retain first native veto across setup socket response deadline and join', async () => {
  const setup = [];
  await assert.rejects(hostPeerFixture({ setupFailure: true, diagnose: value => setup.push(value) }), /^Error: SF08 browser trust preflight refused$/);
  assert.deepEqual(setup, ['setup']);
  for (const [mode, expected] of [['socket', 'socket_eof'], ['response', 'response'], ['binding', 'peer_binding'], ['join', 'join'], ['deadline', 'deadline']]) {
    const rows = [], f = await hostPeerFixture({ closeError: mode === 'join', budget: mode === 'deadline' ? 35 : 1000,
      diagnose: value => rows.push(value) });
    if (mode !== 'deadline') {
      f.connect(); f.sni();
      if (mode === 'socket') f.raw.emit('end');
      if (mode === 'response') f.server.emit('secureConnection', f.socket);
      f.alert(undefined, mode === 'binding' ? new HostTLS() : undefined);
    }
    assert.equal(await f.endpoint.finish(), false); await f.endpoint.close();
    assert.deepEqual(rows, [expected]);
  }
});

test('one-shot socket event diagnostics identify the first veto and always refuse', { timeout: 5000 }, async () => {
  const cases = [
    { event: 'eof', expected: 'socket_eof' },
    { event: 'close', expected: 'socket_close_unproved' },
    { event: 'raw-reset', expected: 'socket_error_reset_raw' },
    { event: 'tls-reset', expected: 'socket_error_reset_tls' },
    { event: 'other-error', expected: 'socket_error_other' },
    { event: 'destroy-throws', expected: 'socket_destroy_error', budget: 80 },
  ];
  for (const item of cases) {
    const rows = [];
    const f = await hostPeerFixture({ role: 'ca', budget: item.budget ?? 1000,
      diagnose: value => rows.push(value) });
    f.connect(); f.sni();
    if (item.event === 'eof') f.raw.emit('end');
    if (item.event === 'close') { f.raw.emit('close'); f.socket.emit('close'); }
    if (item.event === 'raw-reset') f.raw.emit('error', Object.assign(new Error('PRIVATE_SENTINEL'), { code: 'ECONNRESET' }));
    if (item.event === 'tls-reset') f.socket.emit('error', Object.assign(new Error('PRIVATE_SENTINEL'), { code: 'ECONNRESET' }));
    if (item.event === 'other-error') {
      const error = new Error('PRIVATE_SENTINEL');
      Object.defineProperty(error, 'code', { get() { throw new Error('CODE_SENTINEL'); } });
      f.raw.emit('error', error);
    }
    if (item.event === 'destroy-throws') f.raw.destroy = () => { throw new Error('PRIVATE_SENTINEL'); };
    if (item.event === 'destroy-throws') assert.equal(await f.endpoint.close(), false, item.event);
    else {
      assert.equal(await f.endpoint.finish(), false, item.event);
      await f.endpoint.close();
    }
    assert.deepEqual(rows, [item.expected], item.event);
    assert.doesNotMatch(JSON.stringify(rows), /PRIVATE_SENTINEL|CODE_SENTINEL|ECONNRESET/);
  }

  const throwing = await hostPeerFixture({ role: 'ca', diagnose: () => { throw new Error('PRIVATE_SENTINEL'); } });
  throwing.connect(); throwing.sni(); throwing.raw.emit('end');
  assert.equal(await throwing.endpoint.finish(), false, 'observer failure cannot create certificate proof');
  await throwing.endpoint.close();
});

test('one-shot failure checkpoint emits only its fixed WebKit category and cannot expose caller facts', async () => {
  const stages = [], type = fake('webkit', 'normal');
  await assert.rejects(probeEngine('webkit', type, settings(provisioned, ready, negative),
    (stage, engine) => stages.push(checkedFailure(stage, engine)), undefined,
    async (_until, diagnose) => { diagnose('credential'); diagnose('PRIVATE_SENTINEL'); throw new Error('PRIVATE_SENTINEL'); }));
  assert.deepEqual(stages.at(-1), { schema_version: 1, stage: 'engine_wrong_host_one_shot_credential_events_O_0', engine: 'webkit' });
  assert.equal(stages.filter(row => row.stage.startsWith('engine_wrong_host_one_shot_')).length, 1);
  assert.equal(JSON.stringify(stages).includes('PRIVATE_SENTINEL'), false);
});

test('wrong-CA socket event category reaches its fixed checkpoint and stays refused', async () => {
  const type = fake('webkit', 'normal', { reason: 'ca', requestCertificate: true });
  const checkpoints = [];
  const endpoint = { url: negative.caOrigin + '/', async finish() { return false; }, async close() { return true; } };
  await assert.rejects(probeEngine('webkit', type, settings(provisioned, ready, negative),
    (stage, engine) => checkpoints.push({ stage, engine }), undefined, undefined,
    async (_until, diagnose) => { diagnose('socket_eof'); return endpoint; }),
  /^Error: SF08 browser trust preflight refused$/);
  assert.deepEqual(checkpoints.at(-1), { stage: 'engine_wrong_ca_one_shot_socket_eof_events_GRMQNSBJKO_0', engine: 'webkit' });
  assert.equal(type.trace.closed && type.trace.contextClosed, true);
});

test('wrong-CA one-shot checkpoint retains its fixed category and skips legacy navigation', async () => {
  const stages = [], type = fake('webkit', 'normal');
  await assert.rejects(probeEngine('webkit', type, settings(provisioned, ready, negative),
    (stage, engine) => stages.push(checkedFailure(stage, engine)), undefined,
    async () => { throw new Error('host branch must not precede CA'); },
    async (_until, diagnose) => { diagnose('credential'); diagnose('PRIVATE_SENTINEL'); throw new Error('PRIVATE_SENTINEL'); }));
  assert.deepEqual(stages.at(-1), { schema_version: 1,
    stage: 'engine_wrong_ca_one_shot_credential_events_O_0', engine: 'webkit' });
  assert.equal(stages.filter(row => row.stage.startsWith('engine_wrong_ca_one_shot_')).length, 1);
  assert.deepEqual(type.trace.urls, settings(provisioned, ready, negative).origins.map(origin => origin + '/'));
  assert.equal(JSON.stringify(stages).includes('PRIVATE_SENTINEL'), false);
});


test('one-shot diagnostic observer exceptions cannot change valid native peer proof or rejection', async () => {
  for (const accepted of [true, false]) {
    let calls = 0;
    const f = await hostPeerFixture({ diagnose: () => { calls++; throw new Error('PRIVATE_SENTINEL'); } });
    f.connect(); f.sni(); f.alert(accepted ? undefined : 'ECONNRESET');
    assert.equal(await f.endpoint.finish(), accepted); await f.endpoint.close();
    assert.equal(calls, accepted ? 0 : 1);
  }
  for (const [value, expected] of [[undefined, 'peer_other'], [null, 'peer_no_native_code'],
    [{}, 'peer_other'], ['PRIVATE_SENTINEL', 'peer_other_code'], [1, 'peer_other_code']]) {
    const rows = [], f = await hostPeerFixture({ diagnose: value => rows.push(value) });
    f.connect(); f.sni();
    const error = new Error('PRIVATE_SENTINEL');
    Object.defineProperty(error, 'code', { get() { if (value === undefined) throw new Error('PRIVATE_SENTINEL'); return value; } });
    f.server.emit('tlsClientError', error, f.socket);
    assert.equal(await f.endpoint.finish(), false); await f.endpoint.close();
    assert.deepEqual(rows, [expected]);
  }
});

test('one-shot real trusted CA handshake refuses without reentrant native teardown or child crash', { timeout: 10000 }, async () => {
  const files = wrongCACertificateFiles();
  const code = `
import fs from 'node:fs';
import { spawn } from 'node:child_process';
import { readWebkitWrongCACredentials, createWebkitWrongCAEndpoint } from ${JSON.stringify(new URL('./preflight.mjs', import.meta.url).href)};
const work = process.argv[1];
const credentials = readWebkitWrongCACredentials(work, fs.readFileSync(work+'/negative/ca.pem'), fs.readFileSync(work+'/owned-unit/ca.pem'));
const endpoint = await createWebkitWrongCAEndpoint(credentials, performance.now()+1500);
const python = ${JSON.stringify('import socket,ssl,sys\nctx=ssl.create_default_context(cafile=sys.argv[2]);ctx.minimum_version=ssl.TLSVersion.TLSv1_2\nassert ctx.check_hostname and ctx.verify_mode==ssl.CERT_REQUIRED\ntry:\n wire=ctx.wrap_socket(socket.create_connection(("127.0.0.1",int(sys.argv[1])),timeout=1),server_hostname="localhost");wire.recv(1);wire.close()\nexcept (ssl.SSLError,ConnectionResetError):pass')};
const child = spawn('python3', ['-B','-c',python,new URL(endpoint.url).port,work+'/negative/ca.pem'],{stdio:'ignore'});
const exited = new Promise(resolve => { child.once('error',()=>resolve(false));child.once('close',(code,signal)=>resolve(code===0&&signal===null)); });
const proved = await endpoint.finish(), joined = await endpoint.close(), childJoined = await exited;
process.stdout.write(JSON.stringify({proved,joined,childJoined})+'\\n');
`;
  assert.equal(spawnSync(process.execPath, ['--input-type=module', '--check'], { input: code, stdio: ['pipe', 'ignore', 'ignore'], timeout: 1000 }).status, 0, 'native control script must parse before execution');
  const result = await new Promise(resolve => {
    const child = spawn(process.execPath, ['--input-type=module', '-e', code, files.work], { stdio: ['ignore', 'pipe', 'ignore'] });
    let output = '', tooLarge = false;
    const timer = setTimeout(() => child.kill(), 5000);
    child.stdout.on('data', chunk => { if (output.length+chunk.length > 1024) { tooLarge = true; child.kill(); } else output += chunk; });
    child.once('error', () => { clearTimeout(timer); resolve({ code: null, signal: 'spawn-error', output: '' }); });
    child.once('close', (code, signal) => { clearTimeout(timer); resolve({ code, signal, output, tooLarge }); });
  });
  assert.equal(result.code, 0, 'the owned native child must finish, including valid-certificate negative control');
  assert.equal(result.signal, null);
  assert.equal(result.tooLarge, false);
  assert.deepEqual(JSON.parse(result.output), { proved: false, joined: true, childJoined: true });
});


test('deferred TLS teardown has no pending callback after successful finish or repeated close', async () => {
  const nativeImmediate = globalThis.setImmediate;
  let pending = 0, scheduled = 0;
  globalThis.setImmediate = (callback, ...args) => {
    pending++; scheduled++;
    return nativeImmediate(() => { try { callback(...args); } finally { pending--; } });
  };
  try {
    const f = await hostPeerFixture({ role: 'ca' });
    const key = f.credentials.key;
    f.connect(); f.sni(); f.alert();
    assert.equal(await f.endpoint.finish(), true);
    assert.equal(pending, 0);
    assert.equal(key.every(byte => byte === 0), true);
    const completed = scheduled;
    assert.equal(await f.endpoint.close(), true);
    assert.equal(await f.endpoint.close(), true);
    assert.equal(pending, 0, 'completed close must not reserve another destruction callback');
    assert.equal(scheduled, completed);
  } finally { globalThis.setImmediate = nativeImmediate; }
});

test('TLS veto is synchronous while a single actual destruction batch owns join and key release', async () => {
  const nativeImmediate = globalThis.setImmediate;
  let pending = 0, maximum = 0, drains = 0;
  globalThis.setImmediate = (callback, ...args) => {
    pending++; maximum = Math.max(maximum, pending);
    return nativeImmediate(() => { drains++; try { callback(...args); } finally { pending--; } });
  };
  try {
    const f = await hostPeerFixture({ role: 'ca' }), key = f.credentials.key;
    let closePromise, closedInsideBatch = false;
    f.raw.destroy = function () { if (!this.destroyed) { this.destroyed = true; this.emit('close'); } };
    f.socket.destroy = function () {
      if (this.destroyed) return;
      this.destroyed = true;
      closePromise = f.endpoint.close();
      f.server.emit('secureConnection', this); // Reentrant veto must never reenter destruction.
      this.emit('close');
      closedInsideBatch = true;
      assert.equal(key.every(byte => byte === 0), false, 'last socket close must not release during the destruction batch');
    };
    f.connect(); f.sni();
    f.server.emit('secureConnection', f.socket);
    assert.equal(f.raw.destroyed, false);
    assert.equal(f.socket.destroyed, false);
    assert.equal(key.every(byte => byte === 0), false);
    const finish = f.endpoint.finish();
    assert.equal(await finish, false, 'valid TLS/HTTP cannot become certificate-refusal proof');
    assert.equal(await closePromise, true);
    assert.equal(closedInsideBatch, true);
    assert.equal(key.every(byte => byte === 0), true);
    assert.equal(pending, 0);
    assert.equal(maximum, 1);
    assert.equal(drains, 1);
    assert.equal(await f.endpoint.close(), true);
    assert.equal(pending, 0);
  } finally { globalThis.setImmediate = nativeImmediate; }
});

test('server-created setup failure retains credentials until its actual close and destruction batch', async () => {
  let key, observedClose = false;
  await assert.rejects(hostPeerFixture({ role: 'ca', listenFailure: true,
    credentialsSeen(credentials) { key = credentials.key; },
    beforeServerClose() { observedClose = true; assert.equal(key.every(byte => byte === 0), false); },
  }), /^Error: SF08 browser trust preflight refused$/);
  assert.equal(observedClose, true);
  assert.equal(key.every(byte => byte === 0), true);
});

test('one-shot fingerprint hashes only bounded native SSL code on rejected bound peer events', async () => {
  const code = 'ERR_SSL_TLSV1_ALERT_ACCESS_DENIED';
  const kind = 'peer_code_sha256_' + createHash('sha256').update(code).digest('hex');
  for (const role of ['ca', 'host']) {
    const rows = [], f = await hostPeerFixture({ role, diagnose: value => rows.push(value) });
    f.connect(); f.sni(); f.alert(code);
    assert.equal(await f.endpoint.finish(), false); await f.endpoint.close();
    assert.deepEqual(rows, [kind]);
    assert.equal(JSON.stringify(rows).includes(code), false);
    assert.equal(JSON.stringify(rows).includes('PRIVATE_SENTINEL'), false);
  }
  const privateRows = [], privatePeer = await hostPeerFixture({ diagnose: value => privateRows.push(value) });
  const privateError = new Error(); let privateReads = 0;
  for (const field of ['message', 'stack', 'reason']) Object.defineProperty(privateError, field, {
    get() { privateReads++; throw new Error('PRIVATE_SENTINEL'); }
  });
  privateError.code = 'ERR_SSL_SSL/TLS_ALERT_ACCESS_DENIED';
  privatePeer.connect(); privatePeer.sni(); privatePeer.alert(undefined, undefined, privateError);
  assert.equal(await privatePeer.endpoint.finish(), false); await privatePeer.endpoint.close();
  assert.equal(privateReads, 0, 'native message/stack/reason must never be accessed');
  assert.deepEqual(privateRows, ['peer_code_sha256_' + createHash('sha256').update(privateError.code).digest('hex')]);
  for (const code of ['ERR_SSL_' + 'A'.repeat(121), 'ERR_SSL_PRIVATE\n', 'ERR_SSL_PRIVATE\r', 'ERR_SSL_PRIVATE\u2028', 'ERR_SSL_é', 'ERR_SSL_bad',
    'ERR_SSL_PRIVATE https://private.invalid/', 'ECONNREFUSED', '', 3, true, Symbol('PRIVATE_SENTINEL'), {}]) {
    const rows = [], f = await hostPeerFixture({ diagnose: value => rows.push(value) });
    f.connect(); f.sni(); f.alert(code);
    assert.equal(await f.endpoint.finish(), false); await f.endpoint.close();
    assert.deepEqual(rows, [typeof code === 'object' ? 'peer_other' : 'peer_other_code']);
  }
  const boundaryCode = 'ERR_SSL_' + 'A'.repeat(120), rows = [];
  assert.equal(boundaryCode.length, 128);
  const boundary = await hostPeerFixture({ diagnose: value => rows.push(value) });
  boundary.connect(); boundary.sni(); boundary.alert(boundaryCode);
  assert.equal(await boundary.endpoint.finish(), false); await boundary.endpoint.close();
  assert.deepEqual(rows, ['peer_code_sha256_' + createHash('sha256').update(boundaryCode).digest('hex')]);
});

test('one-shot fingerprint preserves binding first veto deadline and callback proof neutrality', async () => {
  const code = 'ERR_SSL_TLSV1_ALERT_ACCESS_DENIED';
  for (const mode of ['binding', 'socket', 'duplicate', 'deadline']) {
    const rows = [], f = await hostPeerFixture({ budget: mode === 'deadline' ? 35 : 1000, diagnose: value => rows.push(value) });
    f.connect(); f.sni();
    if (mode === 'binding') f.sni('PRIVATE_SENTINEL');
    if (mode === 'socket') f.raw.emit('end');
    if (mode === 'duplicate') f.alert('ECONNRESET');
    if (mode === 'deadline') {
      await new Promise(resolve => setTimeout(resolve, 40));
      f.alert(code);
    } else f.alert(code);
    assert.equal(await f.endpoint.finish(), false); await f.endpoint.close();
    assert.deepEqual(rows, [mode === 'binding' ? 'peer_binding' : mode === 'duplicate' ? 'peer_connection_reset' : mode === 'socket' ? 'socket_eof' : mode]);
  }
  for (const accepted of [true, false]) {
    const f = await hostPeerFixture({ diagnose: () => { throw new Error('PRIVATE_SENTINEL'); } });
    f.connect(); f.sni(); f.alert(accepted ? undefined : code);
    assert.equal(await f.endpoint.finish(), accepted); await f.endpoint.close();
  }
});

test('one-shot fingerprint propagates only exact digest kinds and only on failed probes', async () => {
  const kind = 'peer_code_sha256_' + 'a'.repeat(64);
  for (const role of ['ca', 'host']) {
    const stages = [], type = fake('webkit', 'normal'), f = hostBrowser({ role, peer: false });
    const factory = async (_until, diagnose) => {
      diagnose(kind, role === 'host' ? 'G_0' : undefined, undefined, role === 'host' ? '000' : undefined);
      diagnose('setup');
      return f.endpoint;
    };
    await assert.rejects(probeEngine('webkit', type, settings(provisioned, ready, negative),
      (stage, engine) => stages.push(checkedFailure(stage, engine)), undefined,
      role === 'host' ? factory : undefined, role === 'ca' ? factory : undefined));
    assert.deepEqual(stages.at(-1), { schema_version: 1, stage: `engine_wrong_${role}_one_shot_${kind}`, engine: 'webkit' });
  }
  for (const first of [kind + '\n', kind + '\r', kind + '\u2028', kind + '0', kind.toUpperCase(), 'peer_code_sha256_' + 'g'.repeat(64), 'PRIVATE_SENTINEL']) {
    const f = hostBrowser(), rows = [];
    const factory = async (_until, diagnose) => { diagnose(first); diagnose('setup'); throw new Error('PRIVATE_SENTINEL'); };
    assert.equal(await hostProof.probeWebkitWrongHost(f.browser, factory, performance.now() + 1000, value => rows.push(value)), false);
    assert.deepEqual(rows, ['setup']);
  }
  const f = hostBrowser(), rows = [];
  assert.equal(await hostProof.probeWebkitWrongHost(f.browser, async (_until, diagnose) => {
    diagnose(kind); return f.endpoint;
  }, performance.now() + 1000, value => rows.push(value)), true);
  assert.deepEqual(rows, []);
});

for (const role of ['host', 'ca']) test(`one-shot destroyed TLS metadata retains the earlier bound mirror identity (${role})`, async () => {
  for (const clearRemote of [false, true]) {
    const peer = await hostPeerFixture({ role, nodeMirroredErrors: true });
    const browser = hostBrowser({ role, endpoint: peer.endpoint,
      duringGoto: ({ page }) => {
        peer.connect(); peer.sni();
        peer.socket.destroy();
        peer.socket.localAddress = undefined; peer.socket.localPort = undefined;
        if (clearRemote) { peer.socket.remoteAddress = undefined; peer.socket.remotePort = undefined; }
        peer.socket.emit('error', Object.assign(new Error('PRIVATE_SENTINEL'), { code: 'ECONNRESET' }));
        page.emit('console', webkitConsoleError(peer.endpoint.url));
      } });
    const probe = role === 'ca' ? hostProof.probeWebkitWrongCA : hostProof.probeWebkitWrongHost;
    assert.equal(await probe(browser.browser, browser.factory, peer.until), true);
    assert.equal(browser.closed(), true);
    assert.equal(peer.credentials.key, null);
    assert.equal(peer.raw.destroyed && peer.socket.destroyed, true);
    assert.equal(await peer.endpoint.finish(), false, 'private proof remains single use');
  }
});

test('one-shot destroyed metadata cannot replace conflicting or absent initial binding and independent proof', async () => {
  const modes = ['not-destroyed', 'partial-local-address', 'partial-local-port',
    'remote-address-conflict', 'remote-port-conflict', 'throwing-getter',
    'missing-initial-binding', 'missing-SNI-binding', 'missing-console', 'prior-lifecycle'];
  for (const role of ['host', 'ca']) for (const mode of modes) {
    const peer = await hostPeerFixture({ role, nodeMirroredErrors: true });
    const browser = hostBrowser({ role, endpoint: peer.endpoint,
      duringGoto: ({ page }) => {
        if (mode === 'missing-initial-binding') peer.raw.localAddress = undefined;
        peer.connect();
        if (mode === 'missing-SNI-binding') peer.socket.localAddress = undefined;
        peer.sni();
        if (mode !== 'not-destroyed') peer.socket.destroy();
        const oldAddress = peer.socket.localAddress, oldPort = peer.socket.localPort;
        peer.socket.localAddress = undefined; peer.socket.localPort = undefined;
        if (mode === 'partial-local-address') peer.socket.localAddress = oldAddress;
        if (mode === 'partial-local-port') peer.socket.localPort = oldPort;
        if (mode === 'remote-address-conflict') peer.socket.remoteAddress = '127.0.0.2';
        if (mode === 'remote-port-conflict') peer.socket.remotePort++;
        if (mode === 'throwing-getter') Object.defineProperty(peer.socket, 'remoteAddress', {
          get() { throw new Error('PRIVATE_SENTINEL'); } });
        if (mode === 'prior-lifecycle') page.emit('crash');
        peer.socket.emit('error', Object.assign(new Error('PRIVATE_SENTINEL'), { code: 'ECONNRESET' }));
        if (mode !== 'missing-console') page.emit('console', webkitConsoleError(peer.endpoint.url));
      } });
    const probe = role === 'ca' ? hostProof.probeWebkitWrongCA : hostProof.probeWebkitWrongHost;
    assert.equal(await probe(browser.browser, browser.factory, peer.until), false, `${role}/${mode}`);
    assert.equal(browser.closed(), true);
    assert.equal(peer.credentials.key, null);
  }
});
