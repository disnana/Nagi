// Candidate transport preflight library. The capsule/coordinator supplies
// already-owned, schema-validated ready/provision metadata and native probes.
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import https from 'node:https';
import { createHash, createPrivateKey, createPublicKey, X509Certificate } from 'node:crypto';
import fs from 'node:fs';
import tls from 'node:tls';

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

// Navigation-error classification only. The probe may additionally require a
// strictly bound certificate-specific requestfailed witness; these diagnostic
// labels themselves never authorize a rejection.
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

function containsFixedToken(message, token, start = 0) {
  let index = message.indexOf(token, start);
  while (index >= 0) {
    const end = index + token.length;
    if ((index === 0 || !isASCIIIdentifierUnit(message.charCodeAt(index - 1))) &&
        (end === message.length || !isASCIIIdentifierUnit(message.charCodeAt(end)))) return true;
    index = message.indexOf(token, index + 1);
  }
  return false;
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
    const firstLineForms = [cancelled, 'page.goto: ' + cancelled,
      'page.goto: Protocol error (Playwright.navigate): ' + cancelled,
      'page.goto: Protocol error (Playwright.navigate): ' + cancelled + ' at ' + probeURL];
    if (firstLineForms.includes(firstLine)) return 'webkit_cancelled_first_line';
    const callLogMarker = '\nCall log:';
    const callLog = message.indexOf(callLogMarker);
    if (callLog >= 0 && containsFixedToken(message, cancelled, callLog + callLogMarker.length))
      return 'webkit_cancelled_call_log';
    if (containsFixedToken(message, cancelled)) return 'webkit_cancelled_other';
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
    case 'webkit_cancelled_first_line': return 'engine_wrong_ca_webkit_cancelled_first_line';
    case 'webkit_cancelled_call_log': return 'engine_wrong_ca_webkit_cancelled_call_log';
    case 'webkit_cancelled_other': return 'engine_wrong_ca_webkit_cancelled_other';
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

function webkitNonCertificateDiagnostic(errorText, probeURL) {
  // Failure-only lexical facts, never certificate evidence. The original
  // request remains poisoned for every branch below; no raw text is retained.
  if (errorText.length > 8192) return 'not_certificate_oversize';
  if (errorText.length === 0) return 'not_certificate_empty';
  if (errorText === 'Operation was cancelled') return 'not_certificate_cancelled';
  if (errorText === 'Provisional navigation canceled.') return 'not_certificate_provisional_cancelled';
  const certificate = TLS_ERROR.webkit.ca;
  for (const prefix of ['page.goto: ', 'page.goto: Protocol error (Playwright.navigate): ']) {
    if (errorText === prefix + certificate || errorText === prefix + certificate + ' at ' + probeURL)
      return 'not_certificate_certificate_form';
  }
  const matches = [];
  const certificateToken = containsFixedToken(errorText, certificate);
  if (certificateToken) matches.push('certificate_token');
  if (['Name or service not known', 'Temporary failure in name resolution',
    'No address associated with hostname', 'Could not resolve host']
    .some(token => containsFixedToken(errorText, token))) matches.push('dns_token');
  if (['Connection refused', 'Connection reset by peer', 'Could not connect',
    'Connection terminated unexpectedly']
    .some(token => containsFixedToken(errorText, token))) matches.push('connection_token');
  if (['Operation was cancelled', 'Provisional navigation canceled.']
    .some(token => containsFixedToken(errorText, token))) matches.push('cancel_token');
  // The certificate phrase already includes TLS; count it as one token family.
  if (!certificateToken && ['TLS', 'SSL'].some(token => containsFixedToken(errorText, token)))
    matches.push('tls_token');
  if (matches.length > 1) return 'not_certificate_multiple_tokens';
  return matches.length === 1 ? 'not_certificate_' + matches[0] : 'not_certificate';
}

// Playwright 1.64.1 preserves Network.loadingFailed.errorText in request.failure
// separately from the provisional navigation error used by page.goto. Generic
// cancellation is never evidence: this alternative requires the exact failed
// main-document request and a certificate-specific network failure, no response,
// and no closure/crash. It never retries or extends the original probe budget.
export function webkitCertificateWitness(page, context, browser, probeURL, until) {
  let active = true, poisoned = false, ended = false, stopped = false, attempted = false,
    failed = null, count = 0, diagnosis = null;
  const diagnose = (value, override = false) => {
    if (override || diagnosis === null) diagnosis = value;
  };
  const installed = [];
  const listen = (target, event, fn, network = false) => {
    target.on(event, fn);
    installed.push({ target, event, fn, network });
  };
  const detach = onlyNetwork => {
    for (let i = installed.length - 1; i >= 0; i--) {
      const item = installed[i];
      if (onlyNetwork && !item.network) continue;
      try { item.target.off(item.event, item.fn); }
      catch { poisoned = true; diagnose('lifecycle'); }
      installed.splice(i, 1);
    }
  };
  const vetoed = () => {
    if (poisoned) return true;
    if (ended) { diagnose('lifecycle'); return true; }
    try {
      if (page.isClosed() || !browser.isConnected()) { diagnose('lifecycle'); return true; }
      return false;
    } catch { diagnose('lifecycle'); return true; }
  };
  try {
    const frame = page.mainFrame();
    if (!frame) throw new Error(REFUSED);
    listen(page, 'requestfailed', request => {
      if (!active) return;
      count++;
      if (count !== 1) {
        poisoned = true;
        diagnose('duplicate_event', true);
        return;
      }
      try {
        const url = request?.url();
        if (typeof url !== 'string') { poisoned = true; diagnose('malformed'); return; }
        if (url !== probeURL) { poisoned = true; diagnose('foreign_request_url'); return; }
        const navigation = request.isNavigationRequest();
        if (typeof navigation !== 'boolean') { poisoned = true; diagnose('malformed'); return; }
        if (navigation !== true) { poisoned = true; diagnose('foreign_request_non_navigation'); return; }
        const requestFrame = request.frame();
        if (requestFrame !== frame) { poisoned = true; diagnose('foreign_request_frame'); return; }
        const failure = request.failure();
        if (failure === null || (typeof failure !== 'object' && typeof failure !== 'function')) {
          poisoned = true; diagnose('malformed'); return;
        }
        const errorText = failure.errorText;
        if (typeof errorText !== 'string') { poisoned = true; diagnose('malformed'); return; }
        if (errorText !== TLS_ERROR.webkit.ca) {
          poisoned = true;
          diagnose(webkitNonCertificateDiagnostic(errorText, probeURL));
          return;
        }
        failed = request;
      } catch { poisoned = true; diagnose('malformed'); }
    }, true);
    // A fresh page has no previous HTTP traffic. Even a redirect is a veto.
    listen(page, 'response', () => { if (active) { poisoned = true; diagnose('http_response', true); } }, true);
    for (const event of ['close', 'crash']) listen(page, event, () => { ended = true; diagnose('lifecycle', true); });
    listen(context, 'close', () => { ended = true; diagnose('lifecycle', true); });
    listen(browser, 'disconnected', () => { ended = true; diagnose('lifecycle', true); });
  } catch { poisoned = true; diagnose('malformed'); }
  return {
    vetoed,
    async refused(error) {
      if (stopped || attempted) { diagnose('error_shape'); return false; }
      attempted = true;
      // Freeze navigation observations before querying the already failed
      // request; teardown or delayed events cannot become evidence.
      active = false;
      detach(true);
      let timer;
      try {
        let errorShape = false;
        try { errorShape = !(error instanceof Error) || error.name !== 'Error'; }
        catch { errorShape = true; }
        if (errorShape) { diagnose('error_shape'); return false; }
        if (vetoed()) return false;
        if (count !== 1) { diagnose(count === 0 ? 'missing_event' : 'duplicate_event'); return false; }
        if (!failed) { diagnose('malformed'); return false; }
        const remaining = Math.floor(until - performance.now());
        if (!Number.isFinite(remaining) || remaining < 1 || remaining > 10000) {
          diagnose('deadline');
          return false;
        }
        const responseResult = await Promise.race([
          Promise.resolve(failed.response()).then(value => ({ kind: value === null ? 'none' :
            value === undefined ? 'malformed' : 'response' }), () => ({ kind: 'fault' })),
          new Promise(resolve => { timer = setTimeout(() => resolve({ kind: 'timeout' }), remaining); }),
        ]);
        if (responseResult.kind === 'response') { diagnose('http_response', true); return false; }
        if (responseResult.kind === 'malformed') { diagnose('malformed'); return false; }
        if (responseResult.kind === 'fault') { diagnose('response_query_fault'); return false; }
        if (responseResult.kind === 'timeout') { diagnose('response_query_timeout'); return false; }
        if (!(performance.now() < until)) { diagnose('deadline'); return false; }
        if (vetoed()) return false;
        return responseResult.kind === 'none';
      } catch { diagnose('response_query_fault'); return false; }
      finally { clearTimeout(timer); }
    },
    diagnostic() {
      if (diagnosis !== null) return diagnosis;
      if (count === 0) return 'missing_event';
      if (count > 1) return 'duplicate_event';
      return failed ? 'certificate_event' : 'malformed';
    },
    stop() { active = false; detach(false); const good = !vetoed(); stopped = true; return good; },
  };
}

const WRONG_HOST = 'sf08-wronghost.invalid';
const PEER_CERTIFICATE_CODE = 'ERR_SSL_SSL/TLS_ALERT_BAD_CERTIFICATE';
const HOST_PROFILE = Object.freeze({ hostname: WRONG_HOST, code: PEER_CERTIFICATE_CODE });
const CA_PROFILE = Object.freeze({ hostname: 'localhost', code: 'ERR_SSL_TLSV1_ALERT_UNKNOWN_CA' });
// Failure-only fixed vocabulary. These facts never authorize a TLS result.
const HOST_FAILURE_STAGES = Object.freeze({
  credential: 'engine_wrong_host_one_shot_credential',
  setup: 'engine_wrong_host_one_shot_setup',
  navigation: 'engine_wrong_host_one_shot_navigation',
  request: 'engine_wrong_host_one_shot_request',
  response: 'engine_wrong_host_one_shot_response',
  peer_legacy_certificate: 'engine_wrong_host_one_shot_peer_legacy_certificate',
  peer_other: 'engine_wrong_host_one_shot_peer_other',
  peer_binding: 'engine_wrong_host_one_shot_peer_binding',
  socket: 'engine_wrong_host_one_shot_socket',
  lifecycle: 'engine_wrong_host_one_shot_lifecycle',
  deadline: 'engine_wrong_host_one_shot_deadline',
  join: 'engine_wrong_host_one_shot_join',
  cleanup: 'engine_wrong_host_one_shot_cleanup',
  unknown: 'engine_wrong_host_one_shot_unknown',
});
const CA_FAILURE_STAGES = Object.freeze({
  credential: 'engine_wrong_ca_one_shot_credential',
  setup: 'engine_wrong_ca_one_shot_setup',
  navigation: 'engine_wrong_ca_one_shot_navigation',
  request: 'engine_wrong_ca_one_shot_request',
  response: 'engine_wrong_ca_one_shot_response',
  peer_legacy_certificate: 'engine_wrong_ca_one_shot_peer_legacy_certificate',
  peer_other: 'engine_wrong_ca_one_shot_peer_other',
  peer_binding: 'engine_wrong_ca_one_shot_peer_binding',
  socket: 'engine_wrong_ca_one_shot_socket',
  lifecycle: 'engine_wrong_ca_one_shot_lifecycle',
  deadline: 'engine_wrong_ca_one_shot_deadline',
  join: 'engine_wrong_ca_one_shot_join',
  cleanup: 'engine_wrong_ca_one_shot_cleanup',
  unknown: 'engine_wrong_ca_one_shot_unknown',
});
function peerCodeFingerprintKind(kind) {
  return typeof kind === 'string' && kind.length === 81 && /^peer_code_sha256_[a-f0-9]{64}$/.test(kind);
}
function hostDiagnosticKind(kind) {
  return typeof kind === 'string' && (Object.hasOwn(HOST_FAILURE_STAGES, kind) || peerCodeFingerprintKind(kind));
}
function hostFirstDiagnostic(observer) {
  let seen = false;
  return kind => {
    if (seen || !hostDiagnosticKind(kind)) return;
    seen = true;
    try { if (typeof observer === 'function') observer(kind); } catch {}
  };
}

const ownedHostCredentials = new WeakMap();
function hostBudget(until) {
  const remaining = Math.floor(until - performance.now());
  if (!Number.isFinite(remaining) || remaining < 1 || remaining > 10000) refuse();
  return remaining;
}
async function hostWithin(operation, until) {
  let timer;
  try {
    const remaining = hostBudget(until);
    return await Promise.race([Promise.resolve(operation),
      new Promise((_, reject) => { timer = setTimeout(() => reject(new Error(REFUSED)), remaining); })]);
  } finally { clearTimeout(timer); }
}

// Only fixed children of two fd-pinned owned directories are read. No key is
// accepted from JSON/env, serialized, or included in an error or diagnostic.
export function readWebkitWrongHostCredentials(work, caPath, caPEM, io = fs) {
  return readWebkitCredentials(work, caPath, caPEM, io, HOST_PROFILE);
}

export function readWebkitWrongCACredentials(work, wrongCAPEM, mainCAPEM, io = fs) {
  return readWebkitCredentials(work, path.join(work, 'negative', 'ca.pem'), wrongCAPEM, io, CA_PROFILE, mainCAPEM);
}

function readWebkitCredentials(work, caPath, caPEM, io, profile, mainCAPEM) {
  let rootFD, ownedFD, cert, key, caBytes, result, bad = false;
  const uid = typeof process.getuid === 'function' ? BigInt(process.getuid()) : -1n;
  const directory = info => {
    if (!info.isDirectory() || info.uid !== uid || (info.mode & 0o7777n) !== 0o700n) refuse();
    return info;
  };
  const identity = (a, b) => a.dev === b.dev && a.ino === b.ino;
  const same = (a, b) => ['dev', 'ino', 'mode', 'uid', 'nlink', 'size', 'mtimeNs', 'ctimeNs'].every(name => a[name] === b[name]);
  const flags = io.constants.O_RDONLY | io.constants.O_NONBLOCK | io.constants.O_NOFOLLOW;
  try {
    if (process.platform !== 'linux' || uid < 0n || typeof work !== 'string' ||
        !path.isAbsolute(work) || path.normalize(work) !== work || typeof caPath !== 'string' ||
        !Buffer.isBuffer(caPEM) || caPEM.length < 1 || caPEM.length > 16384) refuse();
    const child = path.basename(path.dirname(caPath));
    if ((profile === HOST_PROFILE ? !/^owned-[A-Za-z0-9_-]+$/.test(child) : child !== 'negative') ||
        caPath !== path.join(work, child, 'ca.pem')) refuse();
    rootFD = io.openSync(work, flags | io.constants.O_DIRECTORY);
    const root = directory(io.fstatSync(rootFD, { bigint: true }));
    ownedFD = io.openSync(`/proc/self/fd/${rootFD}/${child}`, flags | io.constants.O_DIRECTORY);
    const owned = directory(io.fstatSync(ownedFD, { bigint: true }));
    if (owned.dev !== root.dev) refuse();
    const checkDirectories = () => {
      if (!identity(root, directory(io.fstatSync(rootFD, { bigint: true }))) ||
          !identity(root, directory(io.lstatSync(work, { bigint: true }))) ||
          !identity(owned, directory(io.fstatSync(ownedFD, { bigint: true }))) ||
          !identity(owned, directory(io.lstatSync(path.join(work, child), { bigint: true })))) refuse();
    };
    const read = name => {
      checkDirectories();
      let fd, data;
      try {
        const fixed = `/proc/self/fd/${ownedFD}/${name}`;
        fd = io.openSync(fixed, flags);
        const before = io.fstatSync(fd, { bigint: true });
        const regular = info => info.isFile() && info.uid === uid && info.dev === owned.dev &&
          (info.mode & 0o7777n) === 0o600n && info.nlink === 1n && info.size > 0n && info.size <= 16384n;
        if (!regular(before) || !same(before, io.lstatSync(fixed, { bigint: true }))) refuse();
        data = Buffer.alloc(16385);
        let size = 0, count;
        while (size < data.length && (count = io.readSync(fd, data, size, data.length - size, null)) !== 0) size += count;
        const after = io.fstatSync(fd, { bigint: true });
        if (!regular(after) || !same(before, after) || !same(after, io.lstatSync(fixed, { bigint: true })) ||
            BigInt(size) !== before.size) refuse();
        checkDirectories();
        return data.subarray(0, size);
      } catch { data?.fill(0); refuse(); }
      finally { if (fd !== undefined) { try { io.closeSync(fd); } catch { data?.fill(0); refuse(); } } }
    };
    caBytes = read('ca.pem'); cert = read('leaf.pem'); key = read('leaf.key');
    const authority = new X509Certificate(caPEM), loadedCA = new X509Certificate(caBytes), leaf = new X509Certificate(cert);
    const now = Date.now();
    const live = certificate => Number.isFinite(Date.parse(certificate.validFrom)) &&
      Number.isFinite(Date.parse(certificate.validTo)) && Date.parse(certificate.validFrom) <= now && now < Date.parse(certificate.validTo);
    if (profile === CA_PROFILE) {
      if (!Buffer.isBuffer(mainCAPEM) || mainCAPEM.length < 1 || mainCAPEM.length > 16384) refuse();
      const main = new X509Certificate(mainCAPEM);
      if (!main.ca || !main.verify(main.publicKey) || !live(main) || main.raw.equals(authority.raw) ||
          main.subject === authority.subject || main.issuer === authority.issuer ||
          main.publicKey.export({ type: 'spki', format: 'der' })
            .equals(authority.publicKey.export({ type: 'spki', format: 'der' }))) refuse();
    }
    const hostOptions = { subject: 'never', wildcards: false, partialWildcards: false, multiLabelWildcards: false, singleLabelSubdomains: false };
    if (!authority.raw.equals(loadedCA.raw) || !authority.ca || !authority.verify(authority.publicKey) ||
        !live(authority) || leaf.ca || !live(leaf) || !leaf.checkIssued(authority) || !leaf.verify(authority.publicKey) ||
        !Array.isArray(leaf.keyUsage) || !leaf.keyUsage.includes('1.3.6.1.5.5.7.3.1') ||
        leaf.subjectAltName !== 'DNS:localhost, DNS:app.example.invalid, IP Address:127.0.0.1' ||
        !leaf.checkHost('localhost', hostOptions) || !leaf.checkHost('app.example.invalid', hostOptions) ||
        !leaf.checkIP('127.0.0.1') || leaf.checkHost(WRONG_HOST, hostOptions) !== undefined ||
        !createPublicKey(createPrivateKey(key)).export({ type: 'spki', format: 'der' })
          .equals(leaf.publicKey.export({ type: 'spki', format: 'der' }))) refuse();
    checkDirectories();
    result = Object.freeze({ get cert() { return cert; }, get key() { return key; },
      release() { key?.fill(0); key = null; cert = null; }, toJSON() { refuse(); } });
  } catch { bad = true; }
  finally {
    caBytes?.fill(0);
    for (const fd of [ownedFD, rootFD]) if (fd !== undefined) { try { io.closeSync(fd); } catch { bad = true; } }
  }
  if (bad || !result) { key?.fill(0); refuse(); }
  ownedHostCredentials.set(result, profile);
  return result;
}

// The native dependency is fixed in the browser stage. Its optional explicit
// argument exists only for the source-bound unit controls, never receipt data.
export async function createWebkitWrongHostEndpoint(credentials, until, native = tls, diagnose) {
  return createWebkitCertificateEndpoint(credentials, until, native, diagnose, HOST_PROFILE);
}

export async function createWebkitWrongCAEndpoint(credentials, until, native = tls, diagnose) {
  return createWebkitCertificateEndpoint(credentials, until, native, diagnose, CA_PROFILE);
}

async function createWebkitCertificateEndpoint(credentials, until, native, diagnose, profile) {
  const note = hostFirstDiagnostic(diagnose);
  const failure = kind => {
    const remaining = Math.floor(until - performance.now());
    note(!Number.isFinite(remaining) || remaining < 1 || remaining > 10000 ? 'deadline' : kind);
  };
  if (ownedHostCredentials.get(credentials) !== profile) { failure('setup'); refuse(); }
  ownedHostCredentials.delete(credentials);
  let server, secureContext, timer, port, rawTuple, sniSocket, bad = false,
    connections = 0, sniCount = 0, alerts = 0, peer = false, ready = false,
    closing = false, closedEvent = false, closeCallback = false, joined = false, used = false,
    destroyPending = false;
  const sockets = new Set(), listeners = [];
  let readyResolve, readyReject, joinedResolve;
  const listening = new Promise((resolve, reject) => { readyResolve = resolve; readyReject = reject; });
  const join = new Promise(resolve => { joinedResolve = resolve; });
  const tuple = socket => {
    if (socket.remoteAddress !== '127.0.0.1' || socket.localAddress !== '127.0.0.1' ||
        !Number.isInteger(socket.remotePort) || socket.remotePort < 1 || socket.remotePort > 65535 ||
        socket.localPort !== port) refuse();
    return [socket.remoteAddress, socket.remotePort, socket.localAddress, socket.localPort];
  };
  const sameTuple = (a, b) => a && b && a.every((value, index) => value === b[index]);
  const listen = (target, event, callback) => { target.on(event, callback); listeners.push({ target, event, callback }); };
  const complete = () => {
    if (!joined && !destroyPending && closeCallback && closedEvent && sockets.size === 0 && server?.listening === false) {
      joined = true; clearTimeout(timer);
      for (const { target, event, callback } of listeners) target.off(event, callback);
      listeners.length = 0; secureContext = null; credentials.release(); joinedResolve(true);
    }
  };
  const stopAccepting = () => {
    if (closing) return;
    closing = true;
    try { server.close(error => { if (error) { failure('join'); bad = true; } closeCallback = true; complete(); }); }
    catch { failure('join'); bad = true; readyReject(new Error(REFUSED)); }
  };
  // TLS callbacks may still be on the native handshake stack. Keep the veto
  // synchronous, but destroy the owned raw/TLS wrappers in one later turn.
  // Join and credential release wait for the complete destruction batch.
  const destroy = () => {
    if (destroyPending || joined) return;
    destroyPending = true;
    setImmediate(() => {
      try { for (const socket of sockets) { try { socket.destroy(); } catch { failure('socket'); bad = true; } } }
      finally { destroyPending = false; complete(); }
    });
  };
  const veto = (kind = 'socket') => { failure(kind); bad = true; stopAccepting(); destroy(); };
  const own = socket => {
    if (sockets.has(socket)) return;
    sockets.add(socket);
    listen(socket, 'close', () => { sockets.delete(socket); if (!peer) { failure('socket'); bad = true; } complete(); });
    listen(socket, 'end', () => veto('socket'));
    listen(socket, 'error', () => { if (!peer) veto(); });
  };
  try {
    const remaining = hostBudget(until);
    secureContext = native.createSecureContext({ cert: credentials.cert, key: credentials.key, minVersion: 'TLSv1.2' });
    server = native.createServer({ cert: credentials.cert, key: credentials.key, minVersion: 'TLSv1.2',
      ALPNProtocols: ['http/1.1'], handshakeTimeout: remaining,
      SNICallback(name, callback) {
        try {
          hostBudget(until);
          sniCount = Math.min(2, sniCount + 1);
          if (sniCount !== 1 || name !== profile.hostname || !(this instanceof native.TLSSocket) ||
              connections !== 1 || !sameTuple(rawTuple, tuple(this))) refuse();
          sniSocket = this; own(this); listen(this, 'data', () => veto('response'));
          callback(null, secureContext);
        } catch { veto('peer_binding'); callback(new Error(REFUSED)); }
      } });
    listen(server, 'connection', socket => {
      connections = Math.min(2, connections + 1);
      own(socket);
      try {
        hostBudget(until);
        if (connections !== 1) refuse();
        rawTuple = tuple(socket);
        stopAccepting(); // One-shot: subsequent accepted callbacks still veto.
      } catch { veto('peer_binding'); }
    });
    listen(server, 'secureConnection', socket => { own(socket); veto('response'); });
    listen(server, 'tlsClientError', (error, socket) => {
      alerts = Math.min(2, alerts + 1);
      try {
        hostBudget(until);
        if (alerts !== 1 || connections !== 1 || sniCount !== 1 || socket !== sniSocket ||
            !(socket instanceof native.TLSSocket) || !(error instanceof Error) ||
            error.code !== profile.code || bad) refuse();
        peer = true; destroy();
      } catch {
        // Inspect only a rejected event, never widen the exact acceptance code.
        let kind = 'peer_other';
        try {
          if (alerts !== 1 || connections !== 1 || sniCount !== 1 || socket !== sniSocket ||
              !(socket instanceof native.TLSSocket)) kind = 'peer_binding';
          else if (error instanceof Error) {
            hostBudget(until);
            const code = error.code;
            if (code === 'ERR_SSL_SSLV3_ALERT_BAD_CERTIFICATE') kind = 'peer_legacy_certificate';
            else if (typeof code === 'string' && code.length <= 128 &&
                code.match(/^ERR_SSL_[A-Z0-9_/]+$/)?.[0] === code)
              kind = 'peer_code_sha256_' + createHash('sha256').update(code).digest('hex');
          }
        } catch {}
        veto(kind);
      }
    });
    listen(server, 'error', () => { readyReject(new Error(REFUSED)); veto('setup'); });
    listen(server, 'close', () => { closedEvent = true; complete(); });
    listen(server, 'listening', () => {
      try {
        hostBudget(until);
        const address = server.address();
        if (!address || address.address !== '127.0.0.1' || address.family !== 'IPv4' ||
            !Number.isInteger(address.port) || address.port < 1024 || address.port > 65535) refuse();
        port = address.port; ready = true; readyResolve();
      } catch { readyReject(new Error(REFUSED)); veto('setup'); }
    });
    timer = setTimeout(() => veto('deadline'), remaining);
    server.listen({ host: '127.0.0.1', port: 0, exclusive: true });
    await hostWithin(listening, until);
  } catch {
    failure('setup'); bad = true; clearTimeout(timer);
    if (server) { stopAccepting(); destroy(); try { await hostWithin(join, until); } catch {} }
    if (!server) credentials.release();
    refuse();
  }
  return Object.freeze({ url: `https://${profile.hostname}:${port}/`,
    async finish() {
      if (used) { failure('peer_binding'); return false; }
      used = true;
      try { await hostWithin(join, until); hostBudget(until); }
      catch { veto('join'); return false; }
      return ready && joined && peer && !bad && connections === 1 && sniCount === 1 && alerts === 1;
    },
    async close() {
      stopAccepting(); destroy();
      try { await hostWithin(join, until); hostBudget(until); return joined; }
      catch { failure('join'); return false; }
    } });
}

// An independent proof branch. It never clears the original witness's poison.
// Cancellation gains no authority; only the separately joined native peer fact
// plus every bound-request/lifecycle/deadline condition can complete this probe.
export async function probeWebkitWrongHost(browser, factory, until, diagnose) {
  return probeWebkitCertificate(browser, factory, until, diagnose, HOST_PROFILE);
}

export async function probeWebkitWrongCA(browser, factory, until, diagnose) {
  return probeWebkitCertificate(browser, factory, until, diagnose, CA_PROFILE);
}

async function probeWebkitCertificate(browser, factory, until, diagnose, profile) {
  let firstFailure, phase = 'setup';
  const note = hostFirstDiagnostic(kind => { firstFailure = kind; });
  const failure = kind => {
    const remaining = Math.floor(until - performance.now());
    note(!Number.isFinite(remaining) || remaining < 1 || remaining > 10000 ? 'deadline' : kind);
  };
  let context, page, endpoint, bad = false, requests = 0, failed, good = false;
  const listeners = [];
  const listen = (target, event, callback) => { target.on(event, callback); listeners.push({ target, event, callback }); };
  const live = () => {
    hostBudget(until);
    if (bad || page.isClosed() || !browser.isConnected()) { failure('lifecycle'); refuse(); }
  };
  try {
    hostBudget(until);
    if (typeof factory !== 'function') refuse();
    context = await hostWithin(browser.newContext({ ignoreHTTPSErrors: false, serviceWorkers: 'block' }), until);
    page = await hostWithin(context.newPage(), until);
    for (const event of ['close', 'crash']) listen(page, event, () => { failure('lifecycle'); bad = true; });
    listen(context, 'close', () => { failure('lifecycle'); bad = true; });
    listen(browser, 'disconnected', () => { failure('lifecycle'); bad = true; });
    listen(page, 'response', () => { failure('response'); bad = true; });
    const frame = page.mainFrame();
    if (!frame) refuse();
    endpoint = await hostWithin(Promise.resolve(factory(until, note)), until);
    const url = new URL(endpoint.url);
    if (url.protocol !== 'https:' || url.hostname !== profile.hostname || url.username || url.password ||
        url.pathname !== '/' || url.search || url.hash || url.origin + '/' !== endpoint.url ||
        Number(url.port) < 1024 || Number(url.port) > 65535 || typeof endpoint.finish !== 'function' ||
        typeof endpoint.close !== 'function') refuse();
    listen(page, 'requestfailed', request => {
      requests = Math.min(2, requests + 1);
      try {
        live();
        if (requests !== 1 || request.url() !== endpoint.url || request.isNavigationRequest() !== true ||
            request.frame() !== frame) refuse();
        const failure = request.failure();
        if (!failure || !['Operation was cancelled', 'Unacceptable TLS certificate'].includes(failure.errorText)) refuse();
        failed = request;
      } catch { failure('request'); bad = true; }
    });
    live();
    phase = 'navigation';
    let navigationError;
    try { await hostWithin(page.goto(endpoint.url, { waitUntil: 'load', timeout: hostBudget(until) }), until); failure('response'); bad = true; }
    catch (error) { navigationError = error; }
    if (!(navigationError instanceof Error) || navigationError.name !== 'Error') refuse();
    live();
    phase = requests !== 1 || !failed ? 'request' : 'response';
    if (requests !== 1 || !failed || await hostWithin(failed.response(), until) !== null) refuse();
    live();
    phase = 'peer_other';
    if (await hostWithin(endpoint.finish(), until) !== true) refuse();
    live();
    good = true;
  } catch { failure(phase); good = false; }
  finally {
    if (endpoint) { try { if (await hostWithin(endpoint.close(), until) !== true) good = false; if (!good) failure('join'); } catch { failure('join'); good = false; } }
    // Lifecycle vetoes remain armed through actual endpoint join. Detach only
    // these owned callbacks before intentional page/context teardown.
    for (const { target, event, callback } of listeners) { try { target.off(event, callback); } catch { failure('cleanup'); good = false; } }
    if (page) { try { await hostWithin(page.close(), until); } catch { failure('cleanup'); good = false; } }
    if (context) { try { await hostWithin(context.close(), until); } catch { failure('cleanup'); good = false; } }
    try { hostBudget(until); } catch { failure('deadline'); good = false; }
  }
  if (!(good && !bad)) {
    try { if (typeof diagnose === 'function') diagnose(firstFailure ?? 'unknown'); } catch {}
  }
  return good && !bad;
}

export async function probeEngine(name, browserType, config, checkpoint = () => {}, readinessTiming, wrongHostFactory, wrongCAFactory) {
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
        const factory = reason === 'host' ? wrongHostFactory : wrongCAFactory;
        if (name === 'webkit' && factory !== undefined) {
          let detail = 'unknown';
          const probe = reason === 'host' ? probeWebkitWrongHost : probeWebkitWrongCA;
          const stages = reason === 'host' ? HOST_FAILURE_STAGES : CA_FAILURE_STAGES;
          if (!await probe(browser, factory, performance.now() + 10000,
            kind => { if (hostDiagnosticKind(kind)) detail = kind; })) {
            checkpoint(peerCodeFingerprintKind(detail) ? `engine_wrong_${reason}_one_shot_${detail}` : stages[detail], name);
            refuse();
          }
          continue;
        }
        if (reason === 'host') checkpoint('engine_wrong_host_page_open', name);
        const page = await context.newPage();
        let rejected = false;
        let failureOutcome = 'unknown';
        const probeURL = config.negative[reason] + '/';
        const witness = name === 'webkit'
          ? webkitCertificateWitness(page, context, browser, probeURL, performance.now() + 10000)
          : null;
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
          if (!rejected && witness) rejected = await witness.refused(error);
          // Mandatory lifecycle/timeout vetoes also apply to the original
          // exact TLS-message path, not only the alternative event witness.
          if (witness && (!(error instanceof Error) || error.name !== 'Error' || witness.vetoed()))
            rejected = false;
        } finally {
          if (witness && !witness.stop()) rejected = false;
          if (reason === 'host') checkpoint('engine_wrong_host_page_close', name);
          await page.close();
        }
        if (!rejected) {
          const stage = reason === 'ca' ? wrongCAFailureStage(failureOutcome) : wrongHostFailureStage(failureOutcome);
          checkpoint(witness ? `${stage}_witness_${witness.diagnostic()}` : stage, name);
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

export async function runPreflight(browserTypes, provisioned, ready, negative, checkpoint = () => {}, wrongHostFactory, wrongCAFactory) {
  // The production entrypoint never falls back to either legacy WebKit path.
  // Direct probeEngine controls still exercise that original strict oracle.
  if (typeof wrongHostFactory !== 'function' || typeof wrongCAFactory !== 'function') refuse();
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
  for (const name of Object.keys(VERSIONS)) results.push(await probeEngine(name, browserTypes[name], config, checkpoint, undefined, wrongHostFactory, wrongCAFactory));
  return { schema_version: 1, transport_preflight_passed: true, engines: results };
}
