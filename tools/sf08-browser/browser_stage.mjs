// One owned leader: C3 preflight, then the same trust environment for business.
// Raw reports stay private; stdout/stderr of all tools are discarded.
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { spawn } from 'node:child_process';
import https from 'node:https';
import { runPreflight, settings } from './preflight.mjs';

export function trustEnv(home, inherited) {
  return { ...inherited, HOME: home, XDG_DATA_HOME: home + '/.local/share',
    XDG_CONFIG_HOME: home + '/.config', XDG_CACHE_HOME: home + '/.cache' };
}

export function businessConfig(config, work, testDir, devices) {
  const env = trustEnv(config.home, process.env);
  const firefoxEnv = { ...env, PLAYWRIGHT_FIREFOX_POLICIES_JSON: config.policy };
  return { testDir, timeout: 45000, expect: { timeout: 3000 }, fullyParallel: false,
    workers: 1, forbidOnly: true, retries: 0,
    outputDir: path.join(work, 'private-test-output'),
    reporter: [['json', { outputFile: path.join(work, 'browser-report.json') }]],
    use: { baseURL: config.origins[0], ignoreHTTPSErrors: false, actionTimeout: 3000,
      navigationTimeout: 5000, screenshot: 'off', video: 'off', trace: 'off',
      serviceWorkers: 'block', launchOptions: { env } },
    projects: [
      { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
      { name: 'firefox', use: { ...devices['Desktop Firefox'], launchOptions: { env: firefoxEnv } } },
      { name: 'webkit', use: { ...devices['Desktop Safari'] } },
    ], metadata: { execution: 'real-browser-and-TLS-required', localOrigins: 3 } };
}

export async function businessChild(node, cli, configFile, env) {
  await new Promise((resolve, reject) => {
    const child = spawn(node, [cli, 'test', '--config', configFile, '--forbid-only'],
      { env, stdio: 'ignore' }); // Inherits this capsule-owned leader's session.
    child.once('error', () => reject(new Error('SF08 business failed')));
    child.once('close', (code, signal) => code === 0 && signal === null ? resolve() : reject(new Error('SF08 business failed')));
  });
}

export function nativeWrongCA(origin, ca) {
  return new Promise((resolve, reject) => {
    let timer;
    const request = https.get(origin + '/', { ca, rejectUnauthorized: true, agent: false }, response => {
      response.destroy();
      clearTimeout(timer);
      reject(new Error('SF08 native negative failed'));
    });
    timer = setTimeout(() => request.destroy(new Error('SF08 native negative failed')), 5000);
    request.on('error', error => {
      clearTimeout(timer);
      if (['UNABLE_TO_VERIFY_LEAF_SIGNATURE', 'UNABLE_TO_GET_ISSUER_CERT_LOCALLY',
           'SELF_SIGNED_CERT_IN_CHAIN'].includes(error?.code)) resolve();
      else reject(new Error('SF08 native negative failed'));
    });
  });
}

export async function stage(input, deps = {}) {
  const checkpoint = deps.checkpoint ?? (() => {});
  checkpoint('settings', 'none');
  const provisioned = { ...input.provisioned, caPEM: Buffer.from(input.ca_pem, 'base64') };
  const negative = { caOrigin: input.negative.ca_origin, hostOrigin: input.negative.host_origin,
    caPEM: Buffer.from(input.negative.ca_pem, 'base64') };
  const config = settings(provisioned, input.ready, negative);
  const browsers = deps.browsers ?? await import(pathToFileURL(input.playwright_module).href);
  checkpoint('native_wrong_ca', 'none');
  await (deps.nativeNegative ?? nativeWrongCA)(config.negative.ca, config.caPEM);
  const receipt = await (deps.preflight ?? runPreflight)(browsers, provisioned, input.ready, negative, checkpoint);
  if (JSON.stringify(receipt) !== JSON.stringify({ schema_version: 1, transport_preflight_passed: true,
    engines: ['chromium', 'firefox', 'webkit'].map(engine => ({ engine, positive_sans: 3,
      wrong_ca_refused: true, wrong_host_refused: true })) })) throw new Error('SF08 preflight failed');
  checkpoint('business_config', 'none');
  const configPath = path.join(input.work, 'business.config.mjs');
  const configValue = businessConfig(config, input.work, input.test_dir, browsers.devices);
  (deps.write ?? ((file, data) => fs.writeFileSync(file, data, { flag: 'wx', mode: 0o600 })))(
    configPath, 'export default ' + JSON.stringify(configValue) + ';\n');
  const env = trustEnv(config.home, { ...process.env, NODE_PATH: input.node_modules,
    // Fixed adjacent, already-bound source; never a caller-supplied module URL.
    SF08_PREFLIGHT_MODULE: new URL('./preflight.mjs', import.meta.url).href,
    SF08_WORK_DIR: input.work, SF08_CONTROL_SOCKET: input.ready.control,
    SF08_OBSERVATION_FILE: path.join(input.work, 'observations.json'),
    SF08_APP_ORIGIN: input.ready.app, SF08_UI_ORIGIN: input.ready.ui, SF08_DENIED_ORIGIN: input.ready.denied });
  checkpoint('business', 'none');
  await (deps.child ?? businessChild)(process.execPath, input.playwright_cli, configPath, env);
  return receipt;
}

function readInput(file) {
  const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NONBLOCK | fs.constants.O_NOFOLLOW);
  try {
    const before = fs.fstatSync(fd, { bigint: true });
    if (!before.isFile() || before.uid !== BigInt(process.getuid()) || (before.mode & 0o777n) !== 0o600n || before.size > 32768n) throw new Error();
    const buf = Buffer.alloc(32769);
    let size = 0, n;
    while (size < buf.length && (n = fs.readSync(fd, buf, size, buf.length - size, null))) size += n;
    const after = fs.fstatSync(fd, { bigint: true });
    if (BigInt(size) !== before.size || ['dev', 'ino', 'size', 'mtimeNs', 'ctimeNs'].some(k => before[k] !== after[k])) throw new Error();
    return JSON.parse(buf.subarray(0, size).toString('utf8'));
  } finally { fs.closeSync(fd); }
}

export const FAILURE_STAGES = ['none','settings','native_wrong_ca','guard','native_positive','native_wrong_host',
  'engine_launch','engine_positive','engine_positive_ca','engine_positive_host','engine_positive_timeout',
  'engine_positive_response','engine_positive_tls_unknown','engine_positive_unknown',
  'engine_wrong_ca','engine_wrong_ca_response_received','engine_wrong_ca_bad_signature',
  'engine_wrong_ca_hostname','engine_wrong_ca_timeout','engine_wrong_ca_no_response',
  'engine_wrong_ca_other_tls','engine_wrong_ca_unknown',
  'engine_wrong_ca_webkit_protocol_form','engine_wrong_ca_webkit_token_present','engine_wrong_ca_webkit_no_fixed_token',
  'engine_wrong_ca_webkit_cancelled_first_line','engine_wrong_ca_webkit_cancelled_call_log','engine_wrong_ca_webkit_cancelled_other',
  'engine_wrong_ca_webkit_shape_non_error','engine_wrong_ca_webkit_shape_non_string','engine_wrong_ca_webkit_shape_oversize','engine_wrong_ca_webkit_shape_access_failed',
  'engine_wrong_ca_diag_bad_signature_present','engine_wrong_ca_diag_unknown_issuer_present',
  'engine_wrong_ca_diag_hostname_present','engine_wrong_ca_diag_other_tls_present',
  'engine_wrong_ca_diag_multiple_codes','engine_wrong_ca_diag_no_fixed_code',
  'engine_wrong_ca_shape_non_error','engine_wrong_ca_shape_non_string',
  'engine_wrong_ca_shape_oversize','engine_wrong_ca_shape_access_failed','engine_wrong_host',
  'engine_wrong_host_page_open','engine_wrong_host_page_close','engine_wrong_host_response_received',
  'engine_wrong_host_no_response','engine_wrong_host_timeout','engine_wrong_host_unknown',
  'engine_wrong_host_webkit_protocol_form','engine_wrong_host_webkit_token_present',
  'engine_wrong_host_webkit_no_fixed_token','engine_wrong_host_webkit_cancelled_present',
  'engine_wrong_host_webkit_shape_non_error','engine_wrong_host_webkit_shape_non_string',
  'engine_wrong_host_webkit_shape_oversize','engine_wrong_host_webkit_shape_access_failed','engine_close',
  'business_config','business','receipt','input'];
const FIREFOX_WRONG_CA_DIAGNOSTIC_STAGES = [
  'engine_wrong_ca_diag_bad_signature_present','engine_wrong_ca_diag_unknown_issuer_present',
  'engine_wrong_ca_diag_hostname_present','engine_wrong_ca_diag_other_tls_present',
  'engine_wrong_ca_diag_multiple_codes','engine_wrong_ca_diag_no_fixed_code',
  'engine_wrong_ca_shape_non_error','engine_wrong_ca_shape_non_string',
  'engine_wrong_ca_shape_oversize','engine_wrong_ca_shape_access_failed'];
const WEBKIT_WRONG_CA_DIAGNOSTIC_STAGES = ["engine_wrong_ca_webkit_protocol_form", "engine_wrong_ca_webkit_token_present", "engine_wrong_ca_webkit_no_fixed_token",
  "engine_wrong_ca_webkit_cancelled_first_line", "engine_wrong_ca_webkit_cancelled_call_log", "engine_wrong_ca_webkit_cancelled_other",
  "engine_wrong_ca_webkit_shape_non_error", "engine_wrong_ca_webkit_shape_non_string", "engine_wrong_ca_webkit_shape_oversize", "engine_wrong_ca_webkit_shape_access_failed"];
const WEBKIT_WRONG_HOST_DIAGNOSTIC_STAGES = [
  'engine_wrong_host_webkit_protocol_form','engine_wrong_host_webkit_token_present',
  'engine_wrong_host_webkit_no_fixed_token','engine_wrong_host_webkit_cancelled_present',
  'engine_wrong_host_webkit_shape_non_error','engine_wrong_host_webkit_shape_non_string',
  'engine_wrong_host_webkit_shape_oversize','engine_wrong_host_webkit_shape_access_failed'];
export const FAILURE_ENGINES = ['none','chromium','firefox','webkit'];
export function checkedFailure(stage, engine) {
  if (!FAILURE_STAGES.includes(stage) || !FAILURE_ENGINES.includes(engine) ||
      (stage.startsWith('engine_') !== (engine !== 'none')) ||
      (FIREFOX_WRONG_CA_DIAGNOSTIC_STAGES.includes(stage) && engine !== 'firefox') ||
      (WEBKIT_WRONG_CA_DIAGNOSTIC_STAGES.includes(stage) && engine !== 'webkit') ||
      (WEBKIT_WRONG_HOST_DIAGNOSTIC_STAGES.includes(stage) && engine !== 'webkit') ||
      (['engine_wrong_ca_bad_signature','engine_wrong_ca_other_tls'].includes(stage) && engine !== 'firefox') ||
      (stage === 'engine_wrong_ca_hostname' && engine === 'webkit')) throw new Error('SF08 checkpoint refused');
  return { schema_version: 1, stage, engine };
}
export function failureCheckpoint(work, stage, engine) {
  const value = checkedFailure(stage, engine);
  if (!/^\/run\/nagi-sf08\/run-[A-Za-z0-9_-]+$/.test(work)) throw new Error('SF08 checkpoint refused');
  const fd = fs.openSync(work, fs.constants.O_RDONLY | fs.constants.O_DIRECTORY | fs.constants.O_NOFOLLOW);
  try {
    const info = fs.fstatSync(fd);
    if (info.uid !== process.getuid() || (info.mode & 0o777) !== 0o700) throw new Error('SF08 checkpoint refused');
    // Anchored to the held owned root; fresh file only, no path returned in output.
    fs.writeFileSync('/proc/self/fd/' + fd + '/stage-failure.json', JSON.stringify(value),
      { flag: fs.constants.O_WRONLY | fs.constants.O_CREAT | fs.constants.O_EXCL | fs.constants.O_NOFOLLOW, mode: 0o600 });
  } finally { fs.closeSync(fd); }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  let input;
  let last = checkedFailure('input', 'none');
  const checkpoint = (stage, engine) => { last = checkedFailure(stage, engine); };
  try {
    process.umask(0o077);
    if (process.argv.length !== 3) throw new Error();
    input = readInput(process.argv[2]);
    if (process.argv[2] !== path.join(input.work, 'browser-input.json')) throw new Error();
    await stage(input, { checkpoint });
    checkpoint('receipt', 'none');
    fs.writeFileSync(path.join(input.work, 'preflight.json'), JSON.stringify({ schema_version: 1,
      engines: 3, positive_sans: 9, wrong_ca_refusals: 3, wrong_host_refusals: 3 }), { flag: 'wx', mode: 0o600 });
  } catch {
    if (input) { try { failureCheckpoint(input.work, last.stage, last.engine); } catch {} }
    process.exitCode = 1;
  }
}
