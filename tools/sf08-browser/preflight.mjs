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
// Diagnostic-only labels from pinned Playwright Firefox NSS/SSL mapping.
// SEC_ERROR_BAD_SIGNATURE is retained as a legacy diagnostic label; the pinned
// Helper.js explicit switch does not emit it. None of these labels authorizes a refusal.
const FIREFOX_NSS_SSL_DIAGNOSTIC_TOKENS = Object.freeze([
  'SEC_ERROR_UNKNOWN_ISSUER', 'SSL_ERROR_BAD_CERT_DOMAIN', 'SEC_ERROR_BAD_SIGNATURE',
  'SEC_ERROR_UNTRUSTED_ISSUER', 'SEC_ERROR_CA_CERT_INVALID',
  'SEC_ERROR_EXPIRED_CERTIFICATE', 'SEC_ERROR_REVOKED_CERTIFICATE',
  'SEC_ERROR_UNTRUSTED_CERT', 'SEC_ERROR_INADEQUATE_KEY_USAGE',
  'SEC_ERROR_CERT_SIGNATURE_ALGORITHM_DISABLED', 'SEC_ERROR_UNKNOWN',
  'SSL_ERROR_NO_CERTIFICATE', 'SSL_ERROR_BAD_CERTIFICATE',
  'SSL_ERROR_UNSUPPORTED_CERTIFICATE_TYPE', 'SSL_ERROR_UNSUPPORTED_VERSION',
  'SSL_ERROR_UNKNOWN',
]);
// This closed NSS/SSL list is diagnostic-only; matching never changes the refusal oracle.
const REFUSED = 'SF08 browser trust preflight refused';

function refuse() { throw new Error(REFUSED); }

function exactTLSFailureToken(error, token, probeURL) {
  if (!(error instanceof Error) || typeof error.message !== 'string' || error.message.length > 8192) return false;
  const newline = error.message.indexOf('\n');
  const firstLine = newline < 0 ? error.message : error.message.slice(0, newline);
  const existingForm = firstLine === token ||
    firstLine === 'page.goto: ' + token ||
    firstLine === 'page.goto: ' + token + ' at ' + probeURL;
  return existingForm;
}

function expectedTLSFailure(error, name, reason, probeURL) {
  return exactTLSFailureToken(error, TLS_ERROR[name][reason], probeURL);
}

// Failure-only facts; expectedTLSFailure remains the sole acceptance oracle.
function wrongCAFailureOutcome(error, name, probeURL) {
  try {
    if (!(error instanceof Error)) return 'unknown';
    if (expectedTLSFailure(error, name, 'ca', probeURL)) return 'expected_rejection';
    if (error.name === 'TimeoutError') return 'timeout';
    const matches = [];
    if (name === 'firefox') {
      if (exactTLSFailureToken(error, 'SEC_ERROR_BAD_SIGNATURE', probeURL)) matches.push('bad_signature');
      if (exactTLSFailureToken(error, TLS_ERROR.firefox.host, probeURL)) matches.push('hostname');
      for (const token of ['SEC_ERROR_UNTRUSTED_ISSUER', 'SEC_ERROR_CA_CERT_INVALID']) {
        if (exactTLSFailureToken(error, token, probeURL)) matches.push('other_tls');
      }
    } else if (name === 'chromium' && exactTLSFailureToken(error, TLS_ERROR.chromium.host, probeURL)) {
      matches.push('hostname');
    }
    return matches.length === 1 ? matches[0] : 'unknown';
  } catch {
    return 'unknown';
  }
}

function isASCIIIdentifierUnit(unit) {
  return unit >= 48 && unit <= 57 || unit >= 65 && unit <= 90 ||
    unit >= 97 && unit <= 122 || unit === 95;
}

// Failure-only lexical observation. This does not authenticate the source of a
// code, alter the TLS oracle, or turn any new outcome into an accepted refusal.
function firefoxWrongCADiagnosticOutcome(error) {
  try {
    if (!(error instanceof Error)) return 'shape_non_error';
    const hasOwnMessage = Object.hasOwn(error, 'message');
    const message = error.message;
    if (!hasOwnMessage || typeof message !== 'string') return 'shape_non_string';
    if (message.length > 8192) return 'shape_oversize';
    const present = [];
    for (const token of FIREFOX_NSS_SSL_DIAGNOSTIC_TOKENS) {
      let offset = 0;
      while (offset < message.length) {
        const index = message.indexOf(token, offset);
        if (index < 0) break;
        const end = index + token.length;
        const before = index === 0 ? -1 : message.charCodeAt(index - 1);
        const after = end === message.length ? -1 : message.charCodeAt(end);
        if (!isASCIIIdentifierUnit(before) && !isASCIIIdentifierUnit(after)) {
          present.push(token);
          break;
        }
        offset = index + 1;
      }
    }
    if (present.length > 1) return 'diag_multiple_codes';
    if (present.length === 0) return 'diag_no_fixed_code';
    switch (present[0]) {
      case 'SEC_ERROR_BAD_SIGNATURE': return 'diag_bad_signature_present';
      case 'SEC_ERROR_UNKNOWN_ISSUER': return 'diag_unknown_issuer_present';
      case 'SSL_ERROR_BAD_CERT_DOMAIN': return 'diag_hostname_present';
      default: return 'diag_other_tls_present';
    }
  } catch {
    return 'shape_access_failed';
  }
}


// Failure-only lexical facts. These labels never authorize a TLS refusal.
function webkitWrongCADiagnosticOutcome(error, probeURL) {
  try {
    if (!(error instanceof Error)) return 'webkit_shape_non_error';
    const own = Object.hasOwn(error, 'message');
    const message = error.message;
    if (!own || typeof message !== 'string') return 'webkit_shape_non_string';
    if (message.length > 8192) return 'webkit_shape_oversize';
    const token = TLS_ERROR.webkit.ca;
    const firstLine = message.split('\n', 1)[0];
    const prefix = 'page.goto: Protocol error (Playwright.navigate): ' + token;
    if (firstLine === prefix || firstLine === prefix + ' at ' + probeURL)
      return 'webkit_protocol_form';
    let index = message.indexOf(token);
    while (index >= 0) {
      const end = index + token.length;
      if ((index === 0 || !isASCIIIdentifierUnit(message.charCodeAt(index - 1))) &&
          (end === message.length || !isASCIIIdentifierUnit(message.charCodeAt(end))))
        return 'webkit_token_present';
      index = message.indexOf(token, index + 1);
    }
    const cancelled = 'Operation was cancelled';
    index = message.indexOf(cancelled);
    while (index >= 0) {
      const end = index + cancelled.length;
      if ((index === 0 || !isASCIIIdentifierUnit(message.charCodeAt(index - 1))) &&
          (end === message.length || !isASCIIIdentifierUnit(message.charCodeAt(end))))
        return 'webkit_cancelled_present';
      index = message.indexOf(cancelled, index + 1);
    }
    return 'webkit_no_fixed_token';
  } catch {
    return 'webkit_shape_access_failed';
  }
}

function webkitWrongHostDiagnosticOutcome(error, probeURL) {
  try {
    if (!(error instanceof Error)) return 'webkit_shape_non_error';
    const own = Object.hasOwn(error, 'message');
    const message = error.message;
    if (!own || typeof message !== 'string') return 'webkit_shape_non_string';
    if (message.length > 8192) return 'webkit_shape_oversize';
    const token = TLS_ERROR.webkit.host;
    const firstLine = message.split('\n', 1)[0];
    const prefix = 'page.goto: Protocol error (Playwright.navigate): ' + token;
    if (firstLine === prefix || firstLine === prefix + ' at ' + probeURL)
      return 'webkit_protocol_form';
    let index = message.indexOf(token);
    while (index >= 0) {
      const end = index + token.length;
      if ((index === 0 || !isASCIIIdentifierUnit(message.charCodeAt(index - 1))) &&
          (end === message.length || !isASCIIIdentifierUnit(message.charCodeAt(end))))
        return 'webkit_token_present';
      index = message.indexOf(token, index + 1);
    }
    const cancelled = 'Operation was cancelled';
    index = message.indexOf(cancelled);
    while (index >= 0) {
      const end = index + cancelled.length;
      if ((index === 0 || !isASCIIIdentifierUnit(message.charCodeAt(index - 1))) &&
          (end === message.length || !isASCIIIdentifierUnit(message.charCodeAt(end))))
        return 'webkit_cancelled_present';
      index = message.indexOf(cancelled, index + 1);
    }
    return 'webkit_no_fixed_token';
  } catch {
    return 'webkit_shape_access_failed';
  }
}

function wrongHostFailureOutcome(error, name, probeURL) {
  try {
    if (!(error instanceof Error)) return name === 'webkit' ? 'webkit_shape_non_error' : 'unknown';
    if (error.name === 'TimeoutError') return 'timeout';
    return name === 'webkit' ? webkitWrongHostDiagnosticOutcome(error, probeURL) : 'unknown';
  } catch {
    return 'unknown';
  }
}

function wrongCAFailureStage(outcome) {
  switch (outcome) {
    case 'response_received': return 'engine_wrong_ca_response_received';
    case 'bad_signature': return 'engine_wrong_ca_bad_signature';
    case 'hostname': return 'engine_wrong_ca_hostname';
    case 'timeout': return 'engine_wrong_ca_timeout';
    case 'no_response': return 'engine_wrong_ca_no_response';
    case 'other_tls': return 'engine_wrong_ca_other_tls';
    case 'diag_bad_signature_present': return 'engine_wrong_ca_diag_bad_signature_present';
    case 'diag_unknown_issuer_present': return 'engine_wrong_ca_diag_unknown_issuer_present';
    case 'diag_hostname_present': return 'engine_wrong_ca_diag_hostname_present';
    case 'diag_other_tls_present': return 'engine_wrong_ca_diag_other_tls_present';
    case 'diag_multiple_codes': return 'engine_wrong_ca_diag_multiple_codes';
    case 'diag_no_fixed_code': return 'engine_wrong_ca_diag_no_fixed_code';
    case 'shape_non_error': return 'engine_wrong_ca_shape_non_error';
    case 'shape_non_string': return 'engine_wrong_ca_shape_non_string';
    case 'shape_oversize': return 'engine_wrong_ca_shape_oversize';
    case 'shape_access_failed': return 'engine_wrong_ca_shape_access_failed';
    case 'webkit_protocol_form': return 'engine_wrong_ca_webkit_protocol_form';
    case 'webkit_token_present': return 'engine_wrong_ca_webkit_token_present';
    case 'webkit_cancelled_present': return 'engine_wrong_ca_webkit_cancelled_present';
    case 'webkit_no_fixed_token': return 'engine_wrong_ca_webkit_no_fixed_token';
    case 'webkit_shape_non_error': return 'engine_wrong_ca_webkit_shape_non_error';
    case 'webkit_shape_non_string': return 'engine_wrong_ca_webkit_shape_non_string';
    case 'webkit_shape_oversize': return 'engine_wrong_ca_webkit_shape_oversize';
    case 'webkit_shape_access_failed': return 'engine_wrong_ca_webkit_shape_access_failed';
    default: return 'engine_wrong_ca_unknown';
  }
}

function wrongHostFailureStage(outcome) {
  switch (outcome) {
    case 'response_received': return 'engine_wrong_host_response_received';
    case 'no_response': return 'engine_wrong_host_no_response';
    case 'timeout': return 'engine_wrong_host_timeout';
    case 'webkit_protocol_form': return 'engine_wrong_host_webkit_protocol_form';
    case 'webkit_token_present': return 'engine_wrong_host_webkit_token_present';
    case 'webkit_cancelled_present': return 'engine_wrong_host_webkit_cancelled_present';
    case 'webkit_no_fixed_token': return 'engine_wrong_host_webkit_no_fixed_token';
    case 'webkit_shape_non_error': return 'engine_wrong_host_webkit_shape_non_error';
    case 'webkit_shape_non_string': return 'engine_wrong_host_webkit_shape_non_string';
    case 'webkit_shape_oversize': return 'engine_wrong_host_webkit_shape_oversize';
    case 'webkit_shape_access_failed': return 'engine_wrong_host_webkit_shape_access_failed';
    default: return 'engine_wrong_host_unknown';
  }
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

function readinessTimeout() {
  const error = new Error(REFUSED);
  error.name = 'TimeoutError';
  throw error;
}

// Fresh credential-free page only, before any business operation. A successful
// response is returned exactly once; only pre-HTTP certificate refusal retries.
export async function firefoxCAReadiness(page, probeURL, timing = {}) {
  const url = new URL(probeURL);
  if (url.protocol !== 'https:' || url.hostname !== 'localhost' || url.username || url.password ||
      url.pathname !== '/' || url.search || url.hash || url.origin + '/' !== probeURL ||
      Number(url.port) < 1024 || Number(url.port) > 65535) refuse();
  const now = timing.now ?? (() => performance.now());
  const delay = timing.delay ?? (ms => new Promise(resolve => setTimeout(resolve, ms)));
  const until = now() + 10000;
  for (let attempt = 0; attempt < 20; attempt++) {
    const remaining = until - now();
    if (remaining < 1) readinessTimeout();
    let response;
    try {
      response = await page.goto(probeURL, { waitUntil: 'load', timeout: Math.floor(remaining) });
    } catch (error) {
      if (error?.name !== 'Error' || !expectedTLSFailure(error, 'firefox', 'ca', probeURL)) throw error;
      // No HTTP response was accepted, so this cannot replay a business action.
      if (attempt === 19) throw error;
      const left = until - now();
      if (left < 1) readinessTimeout();
      await delay(Math.min(500, left));
      continue;
    }
    if (now() >= until) readinessTimeout();
    return response; // Missing/non-200 responses are immediate refusal by caller.
  }
  refuse();
}

// The business runner launches a different Firefox profile. Prove readiness on
// that actual worker Browser too, before its existing per-test reset and bodies.
export async function firefoxWorkerReadiness(browser, origin, timing) {
  const context = await browser.newContext({ ignoreHTTPSErrors: false, serviceWorkers: 'block' });
  try {
    const page = await context.newPage();
    try {
      const response = await firefoxCAReadiness(page, origin + '/', timing);
      if (!response || response.status() !== 200) refuse();
    } finally { await page.close(); }
  } finally { await context.close(); }
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

export async function probeEngine(name, browserType, config, checkpoint = () => {}, readinessTiming) {
  if (!Object.hasOwn(VERSIONS, name)) refuse();
  let browser;
  let failed = true;
  try {
    checkpoint('engine_launch', name);
    const env = { ...process.env, HOME: config.home, XDG_DATA_HOME: config.home + '/.local/share',
      XDG_CONFIG_HOME: config.home + '/.config', XDG_CACHE_HOME: config.home + '/.cache' };
    if (name === 'firefox') env.PLAYWRIGHT_FIREFOX_POLICIES_JSON = config.policy;
    browser = await browserType.launch({ headless: true, env });
    if (browser.version() !== VERSIONS[name]) refuse();
    // Explicit false: neither request context nor launch gets an insecure flag.
    const context = await browser.newContext({ ignoreHTTPSErrors: false, serviceWorkers: 'block' });
    try {
      checkpoint('engine_positive', name);
      for (const [index, origin] of config.origins.entries()) {
        const page = await context.newPage();
        const probeURL = origin + '/';
        try {
          let response;
          try {
            response = name === 'firefox' && index === 0
              ? await firefoxCAReadiness(page, probeURL, readinessTiming)
              : await page.goto(probeURL, { waitUntil: 'load', timeout: 10000 });
          }
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
        if (reason === 'host') checkpoint('engine_wrong_host_page_open', name);
        const page = await context.newPage();
        let rejected = false;
        let failureOutcome = 'unknown';
        const probeURL = config.negative[reason] + '/';
        try {
          const response = await page.goto(probeURL, { waitUntil: 'load', timeout: 10000 });
          if (reason === 'ca') failureOutcome = response === null || response === undefined
            ? 'no_response' : 'response_received';
          else failureOutcome = response === null || response === undefined
            ? 'no_response' : 'response_received';
        } catch (error) {
          // Preserve the exact acceptance oracle; only failure gets finite facts.
          if (reason === 'ca') {
            failureOutcome = wrongCAFailureOutcome(error, name, probeURL);
            if (failureOutcome === 'unknown' && name === 'firefox') {
              failureOutcome = firefoxWrongCADiagnosticOutcome(error);
            } else if (failureOutcome === 'unknown' && name === 'webkit') {
              failureOutcome = webkitWrongCADiagnosticOutcome(error, probeURL);
            }
            rejected = failureOutcome === 'expected_rejection';
          } else {
            try {
              rejected = expectedTLSFailure(error, name, reason, probeURL);
            } catch { rejected = false; }
            if (!rejected) failureOutcome = wrongHostFailureOutcome(error, name, probeURL);
          }
        } finally {
          if (reason === 'host') checkpoint('engine_wrong_host_page_close', name);
          await page.close();
        }
        if (!rejected) {
          if (reason === 'ca') checkpoint(wrongCAFailureStage(failureOutcome), name);
          else checkpoint(wrongHostFailureStage(failureOutcome), name);
          refuse();
        }
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
