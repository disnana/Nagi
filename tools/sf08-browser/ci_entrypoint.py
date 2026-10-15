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
import shutil
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


def await_zero(owner, leader, seconds):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        co.require(not owner.cancelled)
        state = owner.peek(leader)
        if state is not None:
            co.require(state.si_code == os.CLD_EXITED and state.si_status == 0)
            return
        time.sleep(.05)
    co.require(False)


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
        prerequisites = owner.spawn('prerequisites', ['/bin/bash', '-c',
            'apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends libnss3-tools build-essential pkg-config'])
        await_zero(owner, prerequisites, 240)
        # Owned container HOME/cache only; no runner HOME or shared Cargo/npm cache.
        home = work/'home'
        home.mkdir(mode=0o700)
        env = ['env', 'HOME='+str(home), 'CARGO_HOME='+str(work/'cargo-home'),
            'CARGO_TARGET_DIR='+str(work/'target'), 'CARGO_INCREMENTAL=0', 'RUSTFLAGS=-C debuginfo=0']
        phase = 'build'
        leader = owner.spawn('build', env + ['cargo', 'build', '--locked', '--manifest-path',
            str(source/'tools/sf08-fixture-runtime/Cargo.toml')])
        await_zero(owner, leader, 1200)
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
    except BaseException:
        passed = False
    finally:
        if owner is not None:
            try:
                closure = owner.close_all(timeout=20, grace=.05)
            except BaseException:
                passed, phase = False, 'ownership'
        if closure is None or not closure.closed or not closure.echild_observed:
            passed, phase = False, 'ownership'
    if not passed and work is not None and closure and closure.closed:
        try:
            boundary = co.load('owned_json', HERE/'context/owned_json.py')
            with boundary.OwnedRoot(work) as root:
                diagnostic = failure_checkpoint.validate(root._read_json('browser-failure.json'), outer=True)
        except BaseException:
            pass
    print(json.dumps({'schema_version': 1, 'status': 'passed' if passed else 'failed', 'code': phase,
        'actual_echild': bool(closure and closure.echild_observed),
        'browser_executions_accepted': 12 if passed else 0, 'resets_accepted': 12 if passed else 0,
        'failure_phase': diagnostic['phase'] if diagnostic else 'unavailable',
        'failure_stage': diagnostic['stage'] if diagnostic else 'unavailable',
        'failure_engine': diagnostic['engine'] if diagnostic else 'unavailable',
        'retain_data': True}, separators=(',', ':')))
    return 0 if passed else 1


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('--checkout', type=Path, required=True)
    raise SystemExit(run(p.parse_args().checkout))
