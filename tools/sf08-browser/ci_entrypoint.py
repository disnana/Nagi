"""Only run inside the ephemeral CI image. Fresh source/target, no cache reuse.

No upload/delete authority. Build/npm/browser commands are capsule-owned leaders.
All tool output remains in container-private storage (capsule discards output).
"""
import argparse
import failure_checkpoint
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import sys
import tempfile
import time
import browser_coordinator as co
import provision

HERE = Path(__file__).resolve().parent
ROOT_META = {'Cargo.toml', 'Cargo.lock', 'compiler/Cargo.toml', 'compiler/src/lib.rs', 'compiler/src/main.rs'}
CANONICAL_FILES = {'tools/sf08-fixture-runtime/Cargo.toml', 'tools/sf08-fixture-runtime/Cargo.lock',
           'tools/sf08-fixture-runtime/src/main.rs', 'tools/sf08-fixture-runtime/src/expiry.rs',
           'runtime/tests/support/browser_login.rs', 'runtime/tests/support/cors.rs'}
MAX_CARGO_DIAGNOSTIC_BYTES = 64 * 1024
MAX_RUST_ERROR_CODES = 8
RUST_ERROR_PREFIX = re.compile(rb'error\[E')
RUST_ERROR_CODE = re.compile(rb'error\[E([0-9]{4})\]')
CARGO_CAUSE_MARKERS = {
    'lockfile_stale': (b'lock file ', b'needs to be updated but --locked was passed to prevent this'),
    'manifest_or_workspace': (b'failed to parse manifest at ', b'failed to load manifest for workspace member ',
        b"current package believes it's in a workspace when it's not"),
    'registry_or_transport': (b'failed to download from ', b'failed to fetch ', b'network failure',
        b'could not resolve proxy', b'certificate verify failed', b'connection timed out'),
    'toolchain': (b'could not execute process ', b'rustc is not installed'),
    'native_toolchain_or_build_script': (b'failed to run custom build command',),
}


def item(path):
    return {'path': str(path), 'bytes': path.stat().st_size, 'sha256': co.sha(path)}


def exclusive(path, value):
    with path.open('x', encoding='utf8') as stream:
        json.dump(value, stream, sort_keys=True, separators=(',', ':'))


def projection(checkout, target):
    required = json.loads((HERE / 'projection-required.json').read_bytes())
    co.require(type(required) is list and len(required) == len(set(required)) and len(required) == 106
               and ROOT_META | CANONICAL_FILES <= set(required))
    # Current actual runtime tree only; excludes .git, targets, caches and symlinks.
    co.require((checkout / 'runtime').is_dir() and not (checkout / 'runtime').is_symlink())
    runtime = co.tree(checkout / 'runtime')
    co.require(runtime and all(not any(part.startswith('.') or part == 'target' for part in Path(name).parts) for name in runtime))
    selected = ROOT_META | {'runtime/' + name for name in runtime}
    co.require(set(required) - CANONICAL_FILES <= selected)
    target.mkdir(mode=0o700)
    for relative in sorted(selected | CANONICAL_FILES):
        src = checkout / relative
        boundary = checkout
        cursor = src
        while cursor != boundary:
            co.require(not cursor.is_symlink())
            cursor = cursor.parent
        co.require(src.is_file())
        dest = target / relative
        dest.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        shutil.copyfile(src, dest)
    observed = co.tree(target)
    co.require(set(observed) == selected | CANONICAL_FILES and set(required) <= set(observed))
    return observed


class ChildWaitFailure(RuntimeError):
    def __init__(self, detail, leader=None):
        self.detail = detail
        self.leader = leader
        super().__init__(detail['class'])


def wait_failure(status):
    code = getattr(status, 'si_code', None)
    value = getattr(status, 'si_status', None)
    if type(code) is not int or type(value) is not int:
        return {'class': 'unknown'}
    if code == os.CLD_EXITED:
        if not 0 <= value <= 255:
            return {'class': 'unknown'}
        return None if value == 0 else {'class': 'child_exit', 'status': value}
    if code in (os.CLD_KILLED, os.CLD_DUMPED) and 1 <= value < signal.NSIG:
        return {'class': 'child_signal', 'status': value}
    return {'class': 'unknown'}


def cargo_diagnostic(sample, truncated, eof, capture_error):
    unknown = {'class': 'unknown', 'rust_codes': []}
    if (type(sample) is not bytes or len(sample) > MAX_CARGO_DIAGNOSTIC_BYTES
            or type(truncated) is not bool or truncated or type(eof) is not bool or not eof
            or type(capture_error) is not bool or capture_error):
        return unknown
    try:
        decoded = sample.decode('utf-8')
    except UnicodeDecodeError:
        return unknown
    if any((ord(char) < 32 and char not in '\r\n\t') or ord(char) == 127 for char in decoded):
        return unknown
    codes = set()
    cursor = 0
    while True:
        prefix = RUST_ERROR_PREFIX.search(sample, cursor)
        if prefix is None:
            break
        match = RUST_ERROR_CODE.match(sample, prefix.start())
        if match is None:
            return unknown
        codes.add('E' + match.group(1).decode('ascii'))
        if len(codes) > MAX_RUST_ERROR_CODES:
            return unknown
        cursor = match.end()
    ordered = sorted(codes)
    if ordered:
        return {'class': 'rustc_error', 'rust_codes': ordered}
    lowered = sample.lower()
    classes = set()
    for name, markers in CARGO_CAUSE_MARKERS.items():
        if name == 'lockfile_stale':
            matched = all(marker in lowered for marker in markers)
        elif name == 'toolchain':
            matched = (b'could not execute process ' in lowered and b'rustc' in lowered
                or any(marker in lowered for marker in markers[1:]))
        elif name == 'native_toolchain_or_build_script':
            matched = any(marker in lowered for marker in markers) or (
                b'linking with ' in lowered and b' failed' in lowered) or (
                b'linker ' in lowered and b' not found' in lowered)
        else:
            matched = any(marker in lowered for marker in markers)
        if matched:
            classes.add(name)
    cause = next(iter(classes)) if len(classes) == 1 else 'unknown'
    return {'class': cause, 'rust_codes': []}


def await_zero(owner, leader, seconds):
    return await_zero_until(owner, leader, time.monotonic() + seconds)


def await_zero_until(owner, leader, deadline):
    while time.monotonic() < deadline:
        if owner.cancelled:
            raise ChildWaitFailure({'class': 'cancelled'}, leader)
        drain = getattr(owner, 'drain_captured_stderr', None)
        blocked = drain() if callable(drain) else None
        if blocked is not None:
            raise ChildWaitFailure({'class': 'capture_refused'}, blocked)
        state = owner.peek(leader)
        if state is not None:
            failure = wait_failure(state)
            if failure is not None:
                raise ChildWaitFailure(failure, leader)
            return
        time.sleep(.05)
    raise ChildWaitFailure({'class': 'timeout'}, leader)


class PrerequisiteFailure(ChildWaitFailure):
    def __init__(self, detail, leader, stage):
        self.prerequisite_stage = stage
        super().__init__(detail, leader)


def run_prerequisites(owner):
    # A single budget covers both apt commands, including admission/spawn time.
    # Keep all output private and keep the ownership capsule unchanged.
    deadline = time.monotonic() + 240
    options = ['-o', 'Acquire::http::Timeout=30', '-o', 'Acquire::https::Timeout=30',
               '-o', 'Acquire::Retries=0']
    commands = (
        ('apt_update', ['apt-get', *options, '-o', 'APT::Update::Error-Mode=any', 'update', '-qq']),
        ('apt_install', ['env', 'DEBIAN_FRONTEND=noninteractive', 'apt-get', *options,
                         'install', '-y', '--no-install-recommends',
                         'libnss3-tools', 'build-essential', 'pkg-config']),
    )
    for stage, argv in commands:
        leader = None
        try:
            if owner.cancelled:
                raise ChildWaitFailure({'class': 'cancelled'})
            if time.monotonic() >= deadline:
                raise ChildWaitFailure({'class': 'timeout'})
            leader = owner.spawn('prerequisite_' + stage, argv)
            await_zero_until(owner, leader, deadline)
        except BaseException as error:
            detail = error.detail if isinstance(error, ChildWaitFailure) else {'class': 'unknown'}
            raise PrerequisiteFailure(detail, leader, stage) from error


def browser_report_diagnostic(failure, phase, closure, outer_diagnostic, work):
    """Read the finite sidecar only for a failed, closed coordinator browser run."""
    if (phase != 'browser' or type(failure) is not dict or failure.get('class') != 'child_exit'
            or type(failure.get('status')) is not int or not 1 <= failure['status'] <= 255
            or closure is None or closure.closed is not True or closure.echild_observed is not True
            or type(outer_diagnostic) is not dict or outer_diagnostic.get('phase') not in ('browser', 'report')
            or work is None):
        return None
    report_validator = None
    try:
        report_validator = co.load('sf08_ci_report_validator', HERE/'context/report_validator.py')
        boundary = co.load('owned_json', HERE/'context/owned_json.py')
        with boundary.OwnedRoot(work) as root:
            raw_projection = root._read_json('browser-report-diagnostic.json')
        return report_validator.validate_failure_projection(raw_projection)
    except BaseException:
        if report_validator is not None:
            return report_validator.unavailable_failure_projection()
        return None


def binding(source, manifest, build, binary, modules):
    mapping = {'protocol': HERE/'protocol.py', 'budget': HERE/'browser_budget.py', 'fixture': HERE/'context/fixture.py',
        'coordinator': HERE/'browser_coordinator.py', 'stage': HERE/'browser_stage.mjs',
        'negative': HERE/'negative_endpoint.py', 'provision_worker': HERE/'provision_worker.py',
        'provision': HERE/'provision.py', 'preflight': HERE/'preflight.mjs',
        'spec': modules.parent/'tests/security-foundation.browser.spec.ts', 'observations': modules.parent/'tests/observations.ts',
        'control': modules.parent/'tests/control.ts', 'playwright_module': modules/'playwright/index.mjs',
        'playwright_cli': modules/'playwright/cli.js', 'playwright_package': modules/'playwright/package.json',
        'test_package': modules/'@playwright/test/package.json', 'lock': HERE/'package-lock.json',
        'required_projection': HERE/'projection-required.json', 'failure_checkpoint': HERE/'failure_checkpoint.py'}
    mapping.update({n: HERE/'context'/f'{n}.py' for n in ('owned_processes','owned_json','report_reader','report_validator')})
    return {'schema_version': 1, 'files': {n:item(p) for n,p in mapping.items()}, 'source_root': str(source),
        'source_manifest': item(manifest), 'build_receipt': item(build), 'binary': item(binary), 'node_modules': str(modules)}


def run(checkout):
    owner = closure = work = None
    diagnostic = None
    failure = None
    failure_leader = build_leader = None
    prerequisite_stage = None
    passed = False
    phase = 'scope'
    try:
        provision.check_scope()  # No mkdir, npm, apt, Cargo, CA or HOME changes before this.
        os.umask(0o077)
        co.require(checkout.is_absolute() and not checkout.is_symlink())
        work = Path(tempfile.mkdtemp(prefix='ci-', dir=provision.BASE))
        source = work/'source'
        phase = 'projection'
        before = projection(checkout, source)
        manifest = work/'source-manifest.json'
        exclusive(manifest, before)
        owned = co.load('owned_processes', HERE/'context/owned_processes.py')
        owner = owned.LinuxProcessOwner.activate_in_dedicated_cli()
        phase = 'prerequisites'
        run_prerequisites(owner)
        # Owned container HOME/cache only; no runner HOME or shared Cargo/npm cache.
        home = work/'home'
        home.mkdir(mode=0o700)
        env = ['env', 'HOME='+str(home), 'CARGO_HOME='+str(work/'cargo-home'),
            'CARGO_TARGET_DIR='+str(work/'target'), 'CARGO_INCREMENTAL=0', 'RUSTFLAGS=-C debuginfo=0']
        phase = 'build'
        build_leader = owner.spawn('build', env + ['cargo', 'build', '--locked', '--manifest-path',
            str(source/'tools/sf08-fixture-runtime/Cargo.toml')], capture_stderr=True)
        await_zero(owner, build_leader, 1200)
        co.require(co.tree(source) == before)
        binary = work/'target/debug/nagi-sf08-fixture-runtime'
        co.require(binary.is_file() and os.access(binary, os.X_OK))
        receipt = work/'build.json'
        exclusive(receipt, {'passed': True, 'exit_code': 0, 'source_unchanged': True,
            'source_files': len(before), 'binary': item(binary), 'scope': 'fresh actual-checkout projection build'})
        phase = 'npm'
        package = work/'package'
        package.mkdir(mode=0o700)
        for name in ('package.json', 'package-lock.json'):
            shutil.copyfile(HERE/name, package/name)
        shutil.copyfile(HERE/'tsconfig.json', package/'tsconfig.json')
        (package/'tests').mkdir(mode=0o700)
        for name in ('security-foundation.browser.spec.ts','observations.ts','control.ts'):
            shutil.copyfile(HERE/'business'/name,package/'tests'/name)
        # --prefix avoids changing coordinator cwd; lock SRI; no browser download.
        install = owner.spawn('npm', ['env', 'HOME='+str(home), 'npm_config_cache='+str(work/'npm-cache'),
            'PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1', 'npm', 'ci', '--prefix', str(package), '--ignore-scripts', '--no-audit', '--no-fund'])
        await_zero(owner, install, 300)
        co.require((package/'package-lock.json').read_bytes() == (HERE/'package-lock.json').read_bytes())
        phase = 'typecheck'
        typecheck = owner.spawn('typecheck', ['node', str(package/'node_modules/typescript/lib/tsc.js'),
            '-p', str(package/'tsconfig.json')])
        await_zero(owner, typecheck, 60)
        co.require((package/'tsconfig.json').read_bytes() == (HERE/'tsconfig.json').read_bytes())
        for name in ('security-foundation.browser.spec.ts','observations.ts','control.ts'):
            co.require((package/'tests'/name).read_bytes() == (HERE/'business'/name).read_bytes())
        phase = 'binding'
        document = binding(source, manifest, receipt, binary, package/'node_modules')
        bindings = work/'bindings.json'
        exclusive(bindings, document)
        expected = co.sha(bindings)
        co.check_bindings(bindings, expected)
        phase = 'browser'
        child = owner.spawn('coordinator', [sys.executable, str(HERE/'browser_coordinator.py'),
            '--bindings', str(bindings), '--binding-sha', expected])
        await_zero(owner, child, 1200)
        co.require(co.tree(source) == before and co.check_bindings(bindings, expected) == document)
        passed, phase = True, 'passed'
    except BaseException as error:
        passed = False
        if isinstance(error, ChildWaitFailure):
            failure = error.detail
            failure_leader = error.leader
            if isinstance(error, PrerequisiteFailure):
                prerequisite_stage = error.prerequisite_stage
    finally:
        if owner is not None:
            try:
                closure = owner.close_all(timeout=20, grace=.05)
            except BaseException:
                passed, phase = False, 'ownership'
        if closure is None or not closure.closed or not closure.echild_observed:
            passed, phase = False, 'ownership'
    if passed and build_leader is not None:
        try:
            _sample, _truncated, _eof, capture_error = owner.stderr_capture(build_leader)
        except BaseException:
            capture_error = True
        if capture_error:
            passed, phase = False, 'build'
            failure = {'class': 'capture_refused'}
            failure_leader = build_leader
    if failure is not None and failure_leader is not None and getattr(failure_leader, 'capture_stderr', False):
        try:
            sample, truncated, eof, capture_error = owner.stderr_capture(failure_leader)
            cargo = cargo_diagnostic(sample, truncated, eof, capture_error)
            if failure['class'] in ('timeout', 'cancelled', 'capture_refused'):
                cargo = {'class': 'unknown', 'rust_codes': []}
        except BaseException:
            cargo = {'class': 'unknown', 'rust_codes': []}
        failure = {**failure, 'diagnostic': cargo}
    business_diagnostic = None
    if not passed and work is not None and closure and closure.closed and closure.echild_observed:
        try:
            boundary = co.load('owned_json', HERE/'context/owned_json.py')
            with boundary.OwnedRoot(work) as root:
                diagnostic = failure_checkpoint.validate(root._read_json('browser-failure.json'), outer=True)
        except BaseException:
            pass
    if not passed:
        business_diagnostic = browser_report_diagnostic(failure, phase, closure, diagnostic, work)
    summary = {'schema_version': 1, 'status': 'passed' if passed else 'failed', 'code': phase,
        'actual_echild': bool(closure and closure.echild_observed),
        'browser_executions_accepted': 12 if passed else 0, 'resets_accepted': 12 if passed else 0,
        'failure_phase': diagnostic['phase'] if diagnostic else 'unavailable',
        'failure_stage': diagnostic['stage'] if diagnostic else 'unavailable',
        'failure_engine': diagnostic['engine'] if diagnostic else 'unavailable',
        'retain_data': True}
    if not passed:
        summary['failure'] = failure or {'class': 'unknown'}
        if (phase == 'prerequisites' and closure and closure.closed and closure.echild_observed
                and prerequisite_stage in ('apt_update', 'apt_install')):
            summary['prerequisite_stage'] = prerequisite_stage
        if business_diagnostic is not None:
            summary['browser_report_diagnostic'] = business_diagnostic
    print(json.dumps(summary, separators=(',', ':')))
    return 0 if passed else 1


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('--checkout', type=Path, required=True)
    raise SystemExit(run(p.parse_args().checkout))
