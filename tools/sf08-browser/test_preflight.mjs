import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { settings, probeEngine } from './preflight.mjs';

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
  const trace = { urls: [], closed: false, pageClosed: 0, contextClosed: false };
  const config = fault?.config ?? settings(provisioned, ready, negative);
  return { trace, async launch(options) {
    trace.launch = options;
    return { version: () => mode === 'version' ? 'unexpected' : versions[name],
      async close() { trace.closed = true; },
      async newContext(options) {
        trace.context = options;
        return { async close() { trace.contextClosed = true; }, async newPage() {
          return { async close() { trace.pageClosed++; }, async goto(url) {
            trace.urls.push(url);
            if (url.startsWith(config.negative.ca) || url.startsWith(config.negative.host)) {
              const reason = url.startsWith(config.negative.ca) ? 'ca' : 'host';
              const token = messages[name][reason === 'ca' ? 0 : 1];
              if (mode === 'accepted') return { status: () => 200 };
              if (mode === 'network') throw new Error('net::ERR_CONNECTION_REFUSED PRIVATE_SENTINEL');
              if (fault && fault.reason === reason) {
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
    if (name === 'firefox') assert.equal(type.trace.launch.firefoxUserPrefs['browser.policies.alternatePath'], provisioned.firefox_policy);
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
