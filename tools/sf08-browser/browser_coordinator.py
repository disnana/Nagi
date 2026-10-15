"""Private/CI browser-mode integration. Dedicated Linux CLI; no deletion.

Binding SHA is supplied by the build/packaging owner, never inferred from an old
ELF. The capsule owns every fixture/provision/negative/browser leader.
"""
import argparse
import base64
import failure_checkpoint
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import socket
import sys
import tempfile
import time

HERE = Path(__file__).resolve().parent
BASE = Path('/run/nagi-sf08')
CODES = {'binding', 'scope', 'capacity', 'ready', 'tls', 'negative', 'provision',
         'browser', 'report', 'stop_ack', 'close', 'ownership', 'cancelled', 'timeout', 'passed', 'unexpected'}


class Refused(RuntimeError):
    pass


def require(value):
    if not value:
        raise Refused('SF08 browser integration refused')


def sha(path):
    value = hashlib.sha256()
    with open(path, 'rb') as stream:
        while chunk := stream.read(1024 * 1024):
            value.update(chunk)
    return value.hexdigest()


def tree(root):
    observed = {}
    for parent, dirs, files in os.walk(root, followlinks=False):
        require(all(not Path(parent, name).is_symlink() for name in dirs))
        for name in files:
            path = Path(parent, name)
            require(not path.is_symlink() and path.is_file())
            observed[str(path.relative_to(root))] = {'bytes': path.stat().st_size, 'sha256': sha(path)}
    return observed


def item_check(item):
    require(type(item) is dict and set(item) == {'path', 'bytes', 'sha256'})
    p = Path(item['path'])
    require(p.is_absolute() and not p.is_symlink() and p.is_file()
            and type(item['bytes']) is int and p.stat().st_size == item['bytes'] and sha(p) == item['sha256'])


def check_bindings(path, expected):
    require(re.fullmatch('[0-9a-f]{64}', expected) is not None and path.stat().st_size <= 262144 and sha(path) == expected)
    value = json.loads(path.read_bytes())
    require(set(value) == {'schema_version', 'files', 'source_root', 'source_manifest', 'build_receipt', 'binary', 'node_modules'}
            and type(value['schema_version']) is int and value['schema_version'] == 1)
    required = {'protocol', 'budget', 'owned_processes', 'owned_json', 'report_reader', 'report_validator', 'fixture',
        'coordinator', 'stage', 'negative', 'provision_worker', 'provision', 'preflight', 'spec', 'observations', 'control',
        'playwright_module', 'playwright_cli', 'playwright_package', 'test_package', 'lock', 'required_projection', 'failure_checkpoint'}
    require(set(value['files']) == required)
    for item in list(value['files'].values()) + [value['source_manifest'], value['build_receipt'], value['binary']]:
        item_check(item)
    require(Path(value['files']['coordinator']['path']).resolve() == Path(__file__).resolve())
    manifest = json.loads(Path(value['source_manifest']['path']).read_bytes())
    required_paths = json.loads(Path(value['files']['required_projection']['path']).read_bytes())
    require(type(required_paths) is list and len(required_paths) == 106 and len(set(required_paths)) == 106
            and set(required_paths) <= set(manifest) and tree(Path(value['source_root'])) == manifest)
    receipt = json.loads(Path(value['build_receipt']['path']).read_bytes())
    require(receipt['passed'] is True and type(receipt['exit_code']) is int and receipt['exit_code'] == 0
            and receipt['source_unchanged'] is True and receipt['binary'] == value['binary']
            and receipt['source_files'] == len(manifest))
    for key in ('test_package', 'playwright_package'):
        require(json.loads(Path(value['files'][key]['path']).read_bytes())['version'] == '1.64.1')
    require(Path(value['node_modules']).is_absolute())
    return value


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def trust_receipt(value, ca_sha, image):
    require(type(value) is dict and set(value) == {'schema_version', 'home', 'ca', 'ca_sha256',
        'firefox_policy', 'declared_image', 'stores_provisioned', 'browser_trust_verified'})
    require(type(value['schema_version']) is int and value['schema_version'] == 1
            and value['stores_provisioned'] is True and value['browser_trust_verified'] is False
            and value['ca_sha256'] == ca_sha and value['declared_image'] == image)
    require(type(value['home']) is str and re.fullmatch(r'/run/nagi-sf08/trust-[A-Za-z0-9_-]+/home', value['home']) is not None)
    parent = str(Path(value['home']).parent)
    require(value['ca'] == parent + '/ca.pem' and value['firefox_policy'] == parent + '/firefox-policy.json')
    return value


def negative_receipt(value, ready, ca):
    require(type(value) is dict and set(value) == {'schema_version', 'ca_origin', 'ca_pem'}
            and type(value['schema_version']) is int and value['schema_version'] == 1
            and type(value['ca_origin']) is str and type(value['ca_pem']) is str)
    match = re.fullmatch(r'https://localhost:([1-9][0-9]{0,4})', value['ca_origin'])
    require(match is not None and 1024 <= int(match[1]) <= 65535
            and all(origin.rsplit(':', 1)[1] != match[1] for origin in (ready['app'], ready['ui'], ready['denied'])))
    pem = base64.b64decode(value['ca_pem'], validate=True)
    require(0 < len(pem) <= 16384 and pem != ca)
    return {**value, 'host_origin': 'https://sf08-wronghost.invalid:' + ready['app'].rsplit(':', 1)[1]}


def wait_object(root, name, core, owner, leader, budget, seconds):
    deadline = time.monotonic() + seconds
    while True:
        budget.check(deadline)
        require(owner.peek(leader) is None)
        root._check_root()
        try:
            os.stat(name, dir_fd=root.fd, follow_symlinks=False)
        except FileNotFoundError:
            time.sleep(.02)
            continue
        return root._read_json(name)  # Same-fd nonblocking regular/600/bounded loader.


def exclusive(root, name, data):
    root._check_root()
    fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=root.fd)
    with os.fdopen(fd, 'wb') as stream:
        stream.write(data)
    root._check_root()


def run(binding_path, binding_sha):
    # No process, owned work, CA or HOME effects until bindings + C3 guard pass.
    owner = root = fixture = ready = closure = negative = browser = budget = None
    read_fd = write_fd = None
    stop_attempted = stopped = False
    closed = report = None
    phase = code = 'binding'
    success = False
    preflight_ok = False
    diagnostic = None
    value = loaded = core = resources = None
    try:
        value = check_bindings(binding_path, binding_sha)
        f = value['files']
        core = load('sf08_bound_protocol', f['protocol']['path'])
        resources = load('sf08_bound_budget', f['budget']['path'])
        loaded = core.components(f)
        provision = load('sf08_bound_provision', f['provision']['path'])
        phase = 'scope'
        provision.check_scope()
        # All owned paths are below the guarded ephemeral tmpfs; host HOME unchanged.
        os.umask(0o077)
        work = Path(tempfile.mkdtemp(prefix='run-', dir=BASE))
        require(len(os.fsencode(work / 'control.sock')) < 104)
        root = loaded['owned_json'].OwnedRoot(work)
        owner = loaded['owned_processes'].LinuxProcessOwner.activate_in_dedicated_cli()
        budget = resources.BrowserBudget(owner, work, BASE)
        budget.check(time.monotonic() + 1)
        phase = 'ready'
        fixture = owner.spawn('fixture', [sys.executable, f['fixture']['path'], '--runtime', value['binary']['path'], '--work-dir', str(work)])
        ready = core.wait_ready(root, owner, fixture, budget)
        phase = 'tls'
        ca = core.ready_ca_bytes(root, ready)
        core.tls_probes(root, ready, owner, fixture, budget)
        # Immutable CA copy comes from the checked same-fd bytes, not a second path read.
        exclusive(root, 'ca-for-trust.pem', ca)
        phase = 'negative'
        read_fd, write_fd = os.pipe2(os.O_CLOEXEC)
        negative = owner.spawn('negative', [sys.executable, f['negative']['path'], '--work', str(work),
            '--fixture', f['fixture']['path'], '--stop-fd', str(read_fd)], pass_fds=(read_fd,))
        os.close(read_fd)
        read_fd = None
        neg = negative_receipt(wait_object(root, 'negative.json', core, owner, negative, budget, 45), ready, ca)
        phase = 'provision'
        trust_leader = owner.spawn('provision', [sys.executable, f['provision_worker']['path'], '--work', str(work)])
        core.wait_exit(owner, trust_leader, time.monotonic() + 120, budget)
        trust = trust_receipt(root._read_json('trust.json'), hashlib.sha256(ca).hexdigest(), provision.IMAGE)
        budget.add_trust(Path(trust['home']).parent)
        phase = 'browser'
        armed = loaded['report_reader'].FreshPlaywrightReport(root)
        payload = {'work': str(work), 'ready': ready, 'provisioned': trust,
            'ca_pem': base64.b64encode(ca).decode('ascii'), 'negative': neg,
            'playwright_module': f['playwright_module']['path'], 'playwright_cli': f['playwright_cli']['path'],
            'node_modules': value['node_modules'], 'test_dir': str(Path(f['spec']['path']).parent)}
        exclusive(root, 'browser-input.json', json.dumps(payload, separators=(',', ':')).encode())
        browser_temp = work / 'browser-temp'
        browser_temp.mkdir(mode=0o700)
        browser = owner.spawn('browser', ['env', 'TMPDIR=' + str(browser_temp), 'node', f['stage']['path'], str(work / 'browser-input.json')])
        # Harness-only budget, not a change to business/token/security deadlines.
        core.wait_exit(owner, browser, time.monotonic() + 900, budget)
        phase = 'report'
        preflight = root._read_json('preflight.json')
        require(preflight == {'schema_version': 1, 'engines': 3, 'positive_sans': 9,
            'wrong_ca_refusals': 3, 'wrong_host_refusals': 3} and all(type(v) is int for v in preflight.values()))
        preflight_ok = True
        report = core.accept_browser_report(root, armed, owner, browser)
        phase = 'stop_ack'
        stop_attempted = True
        core.stop_fixture(root, ready, owner, fixture, budget)
        stopped = True
        phase = 'close'
        core.wait_exit(owner, fixture, time.monotonic() + 45, budget)
        closed = root.closed()
        os.close(write_fd)
        write_fd = None
        core.wait_exit(owner, negative, time.monotonic() + 15, budget)
        success, code = True, 'passed'
    except BaseException as error:
        code = phase if phase in CODES else 'unexpected'
        if type(error) in (getattr(resources, 'ResourceRefused', None), getattr(core, 'Rejected', None)):
            if error.code in {'capacity', 'cancelled', 'timeout'}:
                code = error.code
    finally:
        for fd in (read_fd, write_fd):
            if fd is not None:
                try:
                    os.close(fd)
                except OSError:
                    success, code = False, 'ownership'
        if owner is not None:
            if ready is not None and not stop_attempted:
                try:
                    stop_attempted = True
                    core.stop_fixture(root, ready, owner, fixture, budget)
                    core.wait_exit(owner, fixture, time.monotonic() + 45, budget, cleanup=True)
                except BaseException:
                    pass
            try:
                closure = owner.close_all(timeout=20, grace=.05)
            except BaseException:
                success, code = False, 'ownership'
    try:
        require(value is not None and check_bindings(binding_path, binding_sha) == value)
        if success:
            core.completed('browser', closed, closure, fixture, report)
            budget.check(time.monotonic() + 1)
    except BaseException:
        success, code = False, 'ownership' if success else code
    if not success:
        stage, engine = 'none', 'none'
        if root is not None and browser is not None and closure and closure.closed:
            try:
                checkpoint = failure_checkpoint.validate(root._read_json('stage-failure.json'))
                stage, engine = checkpoint['stage'], checkpoint['engine']
            except BaseException:
                pass  # Missing/invalid facts never emit raw input or create success.
        diagnostic = failure_checkpoint.validate({'schema_version':1,'phase':code,'stage':stage,'engine':engine},outer=True)
        try:
            require(loaded is not None)
            with loaded['owned_json'].OwnedRoot(binding_path.parent) as destination:
                exclusive(destination,'browser-failure.json',json.dumps(diagnostic,separators=(',', ':')).encode())
        except BaseException:
            pass  # Parent uses fixed unavailable; original failure remains nonzero.
    if root is not None:
        try:
            root.close()
        except OSError:
            success, code = False, 'ownership'
    return {'schema_version': 1, 'mode': 'browser', 'status': 'passed' if success else 'failed', 'code': code,
        'browser_stage_started': browser is not None, 'browser_executions_accepted': 12 if success else 0,
        'preflight_verified': preflight_ok, 'stop_ack_verified': stopped,
        'closed_receipt_verified': closed is not None, 'resets_accepted': 12 if success else 0,
        'global_closure': bool(closure and closure.closed), 'actual_echild': bool(closure and closure.echild_observed),
        'failure_stage': diagnostic['stage'] if diagnostic else 'none',
        'failure_engine': diagnostic['engine'] if diagnostic else 'none',
        'retain_data': True}, 0 if success else 1


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('--bindings', type=Path, required=True)
    p.add_argument('--binding-sha', required=True)
    args = p.parse_args()
    record, result = run(args.bindings, args.binding_sha)
    print(json.dumps(record, separators=(',', ':'), sort_keys=True))
    raise SystemExit(result)
