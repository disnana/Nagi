// Candidate transport preflight library. The capsule/coordinator supplies
// already-owned, schema-validated ready/provision metadata and native probes.
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import https from 'node:https';
import { createHash } from 'node:crypto';

const VERSIONS = { chromium: '156.0.8078.4', firefox: '157.0', webkit: '27.2' };
const TLS_ERROR = {
  chromium: { ca: 'net::ERR_CERT_AUTHORITY_INVALID', host: 'net::ERR_CERT_COMMON_NAME_INVALID' },
  firefox: { ca: 'SEC_ERROR_UNKNOWN_ISSUER', host: 'SSL_ERROR_BAD_CERT_DOMAIN' },
  webkit: { ca: 'Unacceptable TLS certificate', host: 'Unacceptable TLS certificate' },
};
// Frozen candidate vocabulary only; real pinned-engine error text remains unverified.
const REFUSED = 'SF08 browser trust preflight refused';

function refuse() { throw new Error(REFUSED); }

function expectedTLSFailure(error, name, reason, probeURL) {
  if (!(error instanceof Error) || typeof error.message !== 'string' || error.message.length > 8192) return false;
  const newline = error.message.indexOf('\n');
  const firstLine = newline < 0 ? error.message : error.message.slice(0, newline);
  const token = TLS_ERROR[name][reason];
  return firstLine === token ||
    firstLine === 'page.goto: ' + token ||
    firstLine === 'page.goto: ' + token + ' at ' + probeURL;
}

function positiveFailureStage(error, name, probeURL) {
  if (error instanceof Error && error.name === 'TimeoutError') return 'engine_positive_timeout';
  const ca = expectedTLSFailure(error, name, 'ca', probeURL);
  const host = expectedTLSFailure(error, name, 'host', probeURL);
  if (ca && host) return 'engine_positive_tls_unknown';
  if (ca) return 'engine_positive_ca';
  if (host) return 'engine_positive_host';
  return 'engine_positive_unknown';
}

function checkedSettings(provisioned, ready, negative) {
  const root = '/run/nagi-sf08/';
  if (provisioned.schema_version !== 1 || provisioned.stores_provisioned !== true ||
      provisioned.browser_trust_verified !== false ||
      typeof provisioned.home !== 'string' || !/^\/run\/nagi-sf08\/trust-[A-Za-z0-9_-]+\/home$/.test(provisioned.home) ||
      provisioned.firefox_policy !== path.dirname(provisioned.home) + '/firefox-policy.json' ||
      provisioned.ca !== path.dirname(provisioned.home) + '/ca.pem' ||
      provisioned.declared_image !== 'sha256:bdfda2a8fdf48707c5e50a49bd3e06d8d0c759153dc8ee057d8a81c3450bb3e9' ||
      !Buffer.isBuffer(provisioned.caPEM) || !provisioned.caPEM.length || provisioned.caPEM.length > 16384 ||
      !/^[0-9a-f]{64}$/.test(provisioned.ca_sha256) ||
      createHash('sha256').update(provisioned.caPEM).digest('hex') !== provisioned.ca_sha256 ||
      !Buffer.isBuffer(negative.caPEM) || !negative.caPEM.length || negative.caPEM.length > 16384 ||
      createHash('sha256').update(negative.caPEM).digest('hex') === provisioned.ca_sha256) refuse();
  const origins = ['app', 'ui', 'denied'].map(key => ready[key]);
  const hosts = ['localhost', 'app.example.invalid', '127.0.0.1'];
  const ports = origins.map((origin, i) => {
    if (typeof origin !== 'string') refuse();
    const parsed = new URL(origin);
    if (parsed.protocol !== 'https:' || parsed.hostname !== hosts[i] || parsed.username || parsed.password ||
        parsed.pathname !== '/' || parsed.search || parsed.hash || parsed.origin !== origin ||
        Number(parsed.port) < 1024 || Number(parsed.port) > 65535) refuse();
    return parsed.port;
  });
  if (new Set(ports).size !== 3 || ready.schema_version !== 1 ||
      typeof ready.ca !== 'string' || !ready.ca.startsWith(root) || path.normalize(ready.ca) !== ready.ca) refuse();
  const ca = new URL(negative.caOrigin);
  const host = new URL(negative.hostOrigin);
  if (ca.origin !== negative.caOrigin || ca.protocol !== 'https:' || ca.hostname !== 'localhost' ||
      Number(ca.port) < 1024 || Number(ca.port) > 65535 || ca.username || ca.password ||
      ca.pathname !== '/' || ca.search || ca.hash || ports.includes(ca.port) ||
      host.origin !== negative.hostOrigin || host.protocol !== 'https:' ||
      host.hostname !== 'sf08-wronghost.invalid' || host.port !== ports[0] ||
      host.username || host.password || host.pathname !== '/' || host.search || host.hash) refuse();
  return { home: provisioned.home, policy: provisioned.firefox_policy,
    origins, caPEM: provisioned.caPEM, wrongCAPEM: negative.caPEM,
    negative: { ca: negative.caOrigin, host: negative.hostOrigin } };
}

export function settings(...args) {
  try { return checkedSettings(...args); } catch { refuse(); }
}

function nativeTLSGet(origin, ca) {
  return new Promise((resolve, reject) => {
    let timer;
    const done = error => { clearTimeout(timer); error ? reject(error) : resolve(); };
    const request = https.get(origin + '/', { ca, rejectUnauthorized: true, agent: false }, response => {
      let bytes = 0;
      response.on('data', data => {
        bytes += data.length;
        if (bytes > 32768) request.destroy(new Error(REFUSED));
      });
      response.on('end', () => done(response.statusCode === 200 ? undefined : new Error(REFUSED)));
      response.on('error', done);
    });
    timer = setTimeout(() => request.destroy(new Error(REFUSED)), 5000);
    request.on('error', done);
  });
}

export async function probeEngine(name, browserType, config, checkpoint = () => {}) {
  if (!Object.hasOwn(VERSIONS, name)) refuse();
  let browser;
  let failed = true;
  try {
    checkpoint('engine_launch', name);
    browser = await browserType.launch({ headless: true,
      env: { ...process.env, HOME: config.home, XDG_DATA_HOME: config.home + '/.local/share',
        XDG_CONFIG_HOME: config.home + '/.config', XDG_CACHE_HOME: config.home + '/.cache' },
      ...(name === 'firefox' ? { firefoxUserPrefs: { 'browser.policies.alternatePath': config.policy } } : {}),
    });
    if (browser.version() !== VERSIONS[name]) refuse();
    // Explicit false: neither request context nor launch gets an insecure flag.
    const context = await browser.newContext({ ignoreHTTPSErrors: false, serviceWorkers: 'block' });
    try {
      checkpoint('engine_positive', name);
      for (const origin of config.origins) {
        const page = await context.newPage();
        const probeURL = origin + '/';
        try {
          let response;
          try { response = await page.goto(probeURL, { waitUntil: 'load', timeout: 10000 }); }
          catch (error) { checkpoint(positiveFailureStage(error, name, probeURL), name); refuse(); }
          if (!response) { checkpoint('engine_positive_response', name); refuse(); }
          let status;
          try { status = response.status(); }
          catch { checkpoint('engine_positive_response', name); refuse(); }
          if (status !== 200) { checkpoint('engine_positive_response', name); refuse(); }
        } finally { await page.close(); }
      }
      for (const reason of ['ca', 'host']) {
        checkpoint(reason === 'ca' ? 'engine_wrong_ca' : 'engine_wrong_host', name);
        const page = await context.newPage();
        let rejected = false;
        const probeURL = config.negative[reason] + '/';
        try {
          await page.goto(probeURL, { waitUntil: 'load', timeout: 10000 });
        } catch (error) {
          // Classify only a bounded exact first line; never emit raw browser text.
          rejected = expectedTLSFailure(error, name, reason, probeURL);
        } finally { await page.close(); }
        if (!rejected) refuse();
      }
    } finally { await context.close(); }
    failed = false;
    return { engine: name, positive_sans: 3, wrong_ca_refused: true, wrong_host_refused: true };
  } catch {
    refuse();
  } finally {
    if (browser) {
      if (!failed) checkpoint('engine_close', name);
      try { await browser.close(); } catch { refuse(); }
    }
  }
}

export async function runPreflight(browserTypes, provisioned, ready, negative, checkpoint = () => {}) {
  // Read-only guard child. This cannot provision or alter any host trust store.
  // Browser process containment, timeout/descendant reaping is the outer capsule's job.
  checkpoint('guard', 'none');
  const guard = spawnSync('python3', [fileURLToPath(new URL('./provision.py', import.meta.url)), '--check'],
    { stdio: 'ignore', timeout: 5000 });
  if (guard.status !== 0 || guard.error || guard.signal) refuse();
  const config = settings(provisioned, ready, negative);
  try {
    checkpoint('native_positive', 'none');
    for (const origin of config.origins) await nativeTLSGet(origin, config.caPEM);
    // Prove the wrong-CA endpoint is live under its own ordinary chain, without
    // installing that CA or weakening verification in any browser.
    await nativeTLSGet(config.negative.ca, config.wrongCAPEM);
    checkpoint('native_wrong_host', 'none');
    let refusedHost = false;
    try { await nativeTLSGet(config.negative.host, config.caPEM); }
    catch (error) { refusedHost = error?.code === 'ERR_TLS_CERT_ALTNAME_INVALID'; }
    if (!refusedHost) refuse();
  } catch { refuse(); }
  const results = [];
  for (const name of Object.keys(VERSIONS)) results.push(await probeEngine(name, browserTypes[name], config, checkpoint));
  return { schema_version: 1, transport_preflight_passed: true, engines: results };
}
