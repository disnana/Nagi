"""Balanced, complete Linux Cargo target execution with fail-closed receipts.

Change awareness changes execution order only. No path, timing or cache can
remove a target. Cargo remains responsible for build and libtest execution.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import time
import traceback

import levels
from nextest_evaluation import parse_libtest_listing

SHARDS = 4
WEIGHTS = {"build_generations": 179.11, "security_sf01_native": 101.55,
           "http_stdlib": 95.35, "error_routes": 94.32, "go_backend": 88.54,
           "auth_boundaries": 83.43}
# Cargo includes the CLI harness even though main.rs contains no unit tests.
EMPTY_TEST_TARGETS = {"nagic/bin/nagic"}
SUMMARY = re.compile(r"(?m)^test result: ok\. (\d+) passed; 0 failed; (\d+) ignored; 0 measured; 0 filtered out; finished in .+$")
EXECUTION_ENV = 'NAGI_CI_WORKSPACE_EXECUTION'
EXECUTION_PROTOCOL = 'stable-libtest-logfile-v1'
MAX_EVENT_BYTES = 16 * 1024 * 1024
TRANSCRIPT = None


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def inventory(metadata):
    # --workspace must select exactly the original virtual-workspace defaults.
    members = set(metadata['workspace_default_members'])
    if members != set(metadata['workspace_members']):
        raise ValueError('workspace selector differs from default-member graph')
    root = Path(metadata['workspace_root'])
    selectors, libraries, identities = {}, [], set()
    for package in metadata['packages']:
        if package['id'] not in members:
            continue
        for target in package['targets']:
            kinds = target['kind']
            if kinds == ['custom-build']:
                continue
            if len(kinds) != 1 or kinds[0] not in ('lib', 'bin', 'test', 'example'):
                raise ValueError('unsupported workspace target class')
            kind = kinds[0]
            if target.get('required-features'):
                raise ValueError('feature-gated targets need explicit workspace policy')
            member = dict(package=package['name'], kind=kind, name=target['name'],
                          source=Path(target['src_path']).relative_to(root).as_posix())
            member['id'] = '/'.join((member['package'], kind, member['name']))
            if member['id'] in identities:
                raise ValueError('duplicate workspace target identity')
            identities.add(member['id'])
            mode = 'test' if target['test'] else 'build'
            # Cargo addresses all libraries atomically. Other selectors can
            # intentionally name several package targets; keep that exact set.
            name = 'all' if kind == 'lib' else target['name']
            key = 'workspace/' + kind + '/' + name
            item = selectors.setdefault(key, dict(id=key, kind=kind, name=name, mode=mode, members=[]))
            if item['mode'] != mode:
                raise ValueError('ambiguous selector mixes test and build-only targets')
            item['members'].append(member)
            if kind == 'lib' and target.get('doctest', True):
                libraries.append(member['id'])
    result = list(selectors.values())
    for item in result:
        identities = [(member['kind'], member['name'], str((root / member['source']).resolve())) for member in item['members']]
        if len(identities) != len(set(identities)):
            raise ValueError('ambiguous selector target source identity')
        item['members'].sort(key=lambda member: member['id'])
    result.append(dict(id='workspace/doc', mode='doc', kind='doc', name='doc', libraries=sorted(libraries)))
    if len(result) < SHARDS:
        raise ValueError('empty workspace target inventory')
    return sorted(result, key=lambda row: row['id'])


def weight(item):
    if item['mode'] == 'doc':
        return 10
    if item['mode'] == 'build':
        return 2
    if item['kind'] == 'lib':
        return 120
    return WEIGHTS.get(item['name'], 15) * len(item.get('members', [None]))


def assignments(items):
    groups, loads = [[] for _ in range(SHARDS)], [0.0] * SHARDS
    for item in sorted(items, key=lambda row: (-weight(row), row['id'])):
        index = 0 if item['mode'] == 'doc' else min(range(SHARDS), key=lambda index: (loads[index], index))
        groups[index].append(item)
        loads[index] += weight(item)
    return groups


def affected(item, paths):
    if not paths:
        return True
    impacted = set()
    for path in paths:
        match = re.fullmatch(r'(compiler|runtime)/tests/([^/]+)\.rs', path)
        if match:
            package = {'compiler': 'nagic', 'runtime': 'nagi-runtime'}[match[1]]
            impacted.add(package + '/test/' + match[2])
        elif path.startswith('compiler/src/'):
            impacted.add('nagic/')
        elif path.startswith('runtime/src/'):
            impacted.update(('nagi-runtime/', 'nagi-runtime-core/'))
        elif path.startswith('runtime-core/'):
            impacted.add('nagi-runtime-core/')
        elif path.startswith(('docs/',)):
            continue
        else:
            # Unmapped fixture, dependency, control-plane or unfamiliar path.
            return True
    return any(member['id'].startswith(prefix) if prefix.endswith('/') else member['id'] == prefix for member in item.get('members', []) for prefix in impacted)


def ordered(items, paths):
    return sorted(items, key=lambda row: (not affected(row, paths), -weight(row), row['id']))


def command(item):
    if item['mode'] == 'doc':
        return ['cargo', 'test', '--locked', '--doc']
    args = ['cargo', 'test' if item['mode'] == 'test' else 'build', '--locked', '--workspace']
    if item['mode'] == 'build':
        args += ['--profile', 'test']
    args += ['--' + item['kind']]
    if item['kind'] != 'lib':
        args.append(item['name'])
    return args


def compiled_binaries(output, item):
    found = {}
    for line in output.splitlines():
        row = json.loads(line)
        if row.get('reason') != 'compiler-artifact' or not row.get('profile', {}).get('test') or not row.get('executable'):
            continue
        target = row['target']
        source = Path(target['src_path']).resolve()
        matches = [member for member in item['members']
                   if member['kind'] == target['kind'][0] and target['kind'] == [member['kind']]
                   and member['name'] == target['name'] and source == Path(member['source']).resolve()]
        if len(matches) != 1 or matches[0]['id'] in found:
            raise ValueError('unexpected, duplicate or ambiguous compiled test target')
        found[matches[0]['id']] = row['executable']
    if set(found) != {member['id'] for member in item['members']} or len(set(found.values())) != len(found):
        raise ValueError('missing or duplicate compiled test executable')
    return found


def execution_blocks(output, markers):
    if any(line.startswith('NAGI_CI_OUTER_EXECUTION ') and not re.fullmatch(r'NAGI_CI_OUTER_EXECUTION [0-9a-f]{64}', line)
           for line in output.splitlines()):
        raise ValueError('malformed outer execution boundary')
    starts = list(re.finditer(r'(?m)^NAGI_CI_OUTER_EXECUTION ([0-9a-f]{64})\r?$', output))
    expected = {marker: name for name, marker in markers.items()}
    blocks = {}
    for index, marker in enumerate(starts):
        if marker[1] not in expected or expected[marker[1]] in blocks:
            raise ValueError('unexpected or duplicate executed binary')
        end = starts[index + 1].start() if index + 1 < len(starts) else len(output)
        blocks[expected[marker[1]]] = output[marker.end():end]
    if set(blocks) != set(markers):
        raise ValueError('missing executed binary')
    return blocks


def check_execution(output, events, cases, ignored, item):
    if not ignored <= cases:
        raise ValueError('ignored tests outside listed inventory')
    if not cases and item['id'] not in EMPTY_TEST_TARGETS:
        raise ValueError('unexpected zero-case test target')
    seen, ignored_reasons = {}, {}
    # Stable libtest's --logfile is written by this outer harness only. Native
    # subprocess stdout can contain the same test names and complete summaries;
    # it is diagnostic output, never an outer case event.
    if len(events.encode()) > MAX_EVENT_BYTES:
        raise ValueError('outer libtest event log is too large')
    for line in events.splitlines():
        if line.startswith('ignored: '):
            # libtest appends the exact test name after the optional ignore
            # reason. Resolve only against the independently listed ignored
            # subset, with an explicit space delimiter and a unique suffix.
            matches = [name for name in ignored if line.endswith(' ' + name)
                       and len(line) >= len('ignored: ') + len(name) + 1]
            if len(matches) != 1:
                raise ValueError('unknown or ambiguous reasoned ignored event')
            status, name = 'ignored', matches[0]
            reason = line[len('ignored: '):-len(name) - 1]
        else:
            match = re.fullmatch(r'(ok|ignored|failed) (.+)', line)
            if match is None:
                raise ValueError('unrecognized outer libtest event')
            status, name = match.groups()
            reason = None
        if name in seen:
            raise ValueError('duplicate executed case')
        seen[name] = status
        if status == 'ignored':
            ignored_reasons[name] = reason
    wanted = {name: ('ignored' if name in ignored else 'ok') for name in cases}
    # Cargo waits for the harness. Its final summary follows all joined native
    # children; earlier summaries may legitimately belong to nested harnesses.
    summaries = [line for line in output.splitlines() if line.startswith('test result:')]
    final = SUMMARY.fullmatch(summaries[-1]) if summaries else None
    if seen != wanted or final is None or final.groups() != (str(len(cases - ignored)), str(len(ignored))):
        raise ValueError('missing, filtered, unsuccessful or unexpected executed cases')
    return dict(cases=sorted(cases), ignored=sorted(ignored), ignored_reasons=ignored_reasons,
                passed=len(cases - ignored))


def run(args, merge=False, extra_env=None):
    result = subprocess.run(args, text=True, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT if merge else subprocess.PIPE,
                            env={**os.environ, 'CARGO_TERM_COLOR': 'never', **(extra_env or {})})
    if TRANSCRIPT is not None:
        with TRANSCRIPT.open('a') as stream:
            stream.write('\nCOMMAND ' + json.dumps(args) + '\n')
            if extra_env:
                stream.write('SCOPED_RUNNER ' + json.dumps(extra_env, sort_keys=True) + '\n')
            stream.write(result.stdout)
            if result.stderr:
                stream.write('\nSTDERR\n' + result.stderr)
            stream.write('\nEXIT ' + str(result.returncode) + '\n')
    if result.stderr:
        print(result.stderr, file=sys.stderr, end='')
    if result.returncode:
        # Preserve Cargo/libtest failure diagnostics in the ordinary job log.
        print(result.stdout, file=sys.stderr, end='')
        result.check_returncode()
    return result.stdout


def execution_plan(binaries, directory, toolchain):
    hosts = re.findall(r'(?m)^host: ([a-zA-Z0-9_-]+)$', toolchain)
    if len(hosts) != 1:
        raise ValueError('missing exact Rust host for Cargo execution runner')
    runner_env = 'CARGO_TARGET_' + hosts[0].upper().replace('-', '_') + '_RUNNER'
    if runner_env in os.environ or EXECUTION_ENV in os.environ:
        raise ValueError('workspace execution runner environment is already configured')
    directory.mkdir(parents=True, exist_ok=False)
    members = {}
    for member, executable in binaries.items():
        marker = digest(dict(member=member, executable=str(Path(executable).resolve())))
        members[member] = dict(executable=str(Path(executable).resolve()), marker=marker,
                               events=str((directory / (marker + '.events')).resolve()))
    runner = shlex.join([sys.executable, '-B', str(Path(__file__).resolve()), '--record-binary'])
    plan = dict(protocol=EXECUTION_PROTOCOL, runner_env=runner_env, runner_command=runner,
                requested_harness_arguments=['--format=pretty'], members=members)
    manifest = directory / 'execution.json'
    manifest.write_text(json.dumps(plan) + '\n')
    return plan, {runner_env: runner, EXECUTION_ENV: str(manifest.resolve())}


def record_binary(arguments):
    """Cargo's runner preserves its cwd/environment, then execs the real binary.

    Remove only our scoped runner settings before exec: a nested Cargo or
    current_exe subprocess must neither record nor inherit outer log arguments.
    The exclusive start file rejects a genuine second outer invocation.
    """
    manifest = Path(os.environ[EXECUTION_ENV])
    if manifest.stat().st_size > 64 * 1024:
        raise ValueError('oversized workspace execution plan')
    plan = json.loads(manifest.read_text())
    if plan['protocol'] != EXECUTION_PROTOCOL or plan['requested_harness_arguments'] != ['--format=pretty']:
        raise ValueError('unexpected outer execution protocol')
    if len(arguments) != 2 or arguments[1:] != plan['requested_harness_arguments']:
        raise ValueError('unexpected outer harness arguments')
    executable = str(Path(arguments[0]).resolve())
    matches = [row for row in plan['members'].values() if row['executable'] == executable]
    if len(matches) != 1:
        raise ValueError('unexpected outer test executable')
    row = matches[0]
    events = Path(row['events'])
    if events.exists() or events.is_symlink():
        raise ValueError('outer event log already exists')
    actual_arguments = [*arguments[1:], '--logfile', str(events)]
    with events.with_suffix('.started').open('x') as stream:
        stream.write(json.dumps(dict(protocol=EXECUTION_PROTOCOL, executable=executable,
                                     cwd=os.getcwd(), requested_arguments=arguments[1:],
                                     actual_arguments=actual_arguments)) + '\n')
    environment = dict(os.environ)
    del environment[EXECUTION_ENV]
    del environment[plan['runner_env']]
    print('NAGI_CI_OUTER_EXECUTION ' + row['marker'], flush=True)
    os.execve(executable, [executable, *actual_arguments], environment)


def context():
    for flag in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTDOCFLAGS', 'CARGO_ENCODED_RUSTDOCFLAGS'):
        if os.environ.get(flag):
            raise ValueError('workspace shards require default compiler/doc flags')
    head = run(['git', 'rev-parse', 'HEAD']).strip()
    if not re.fullmatch('[0-9a-f]{40}', head) or head != os.environ['GITHUB_SHA']:
        raise ValueError('workspace source identity mismatch')
    return dict(head=head, toolchain=run(['rustc', '-vV']), flags='default',
                lock=hashlib.sha256(Path('Cargo.lock').read_bytes()).hexdigest(),
                workflow=hashlib.sha256(Path('.github/workflows/ci.yml').read_bytes()).hexdigest())


def workspace_inventory():
    return inventory(json.loads(run(['cargo', 'metadata', '--locked', '--no-deps', '--format-version=1'])))


def execute(index, destination, logs=None):
    global TRANSCRIPT
    directory = (logs or Path('build/ci-evidence/linux-workspace-logs')) / f'shard-{index}'
    directory.mkdir(parents=True, exist_ok=False)
    previous = TRANSCRIPT
    TRANSCRIPT = directory / 'orchestration.log'
    TRANSCRIPT.write_text('Workspace shard ' + str(index) + '\n')
    try:
        _execute(index, destination, directory)
    except BaseException:
        with TRANSCRIPT.open('a') as stream:
            stream.write('\nORCHESTRATION FAILURE\n' + traceback.format_exc())
        raise
    finally:
        TRANSCRIPT = previous


def _execute(index, destination, logs):
    identity = context()
    items = workspace_inventory()
    paths = levels.complete_paths(os.environ.get('CI_BASE', ''), identity['head'])
    results = []
    for item in ordered(assignments(items)[index], paths):
        started = time.monotonic()
        args = command(item)
        if item['mode'] == 'test':
            binaries = compiled_binaries(run(args + ['--no-run', '--message-format=json']), item)
            inventories = {}
            for member in item['members']:
                executable = binaries[member['id']]
                cases = parse_libtest_listing(run([executable, '--list', '--format=pretty']))
                ignored = parse_libtest_listing(run([executable, '--list', '--ignored', '--format=pretty']))
                inventories[member['id']] = (cases, ignored)
            plan, environment = execution_plan(binaries, logs / digest(item), identity['toolchain'])
            output = run(args + ['--', '--format=pretty'], merge=True, extra_env=environment)
            try:
                blocks = execution_blocks(output, {name: row['marker'] for name, row in plan['members'].items()})
                executed = []
                for member in item['members']:
                    cases, ignored = inventories[member['id']]
                    event_path = Path(plan['members'][member['id']]['events'])
                    if event_path.stat().st_size > MAX_EVENT_BYTES:
                        raise ValueError('outer libtest event log is too large')
                    checked = check_execution(blocks[member['id']], event_path.read_text(), cases, ignored, member)
                    executed.append(dict(member=member, **checked))
            except Exception:
                print(output, file=sys.stderr, end='')
                raise
            result = dict(binaries=executed, passed=sum(row['passed'] for row in executed), command=args, exit=0)
        else:
            # Build-only examples and Cargo doctests retain their original role.
            output = run(args)
            result = dict(passed=0, command=args, exit=0)
            if item['mode'] == 'doc':
                summaries = re.findall(r'(?m)^test result: ok\. (\d+) passed; 0 failed; (\d+) ignored; 0 measured; 0 filtered out;', output)
                if not summaries or len(summaries) != len(item['libraries']):
                    raise ValueError('missing Cargo doctest library summaries')
                result['summaries'] = summaries
                result['passed'] = sum(int(row[0]) for row in summaries)
                result['ignored_total'] = sum(int(row[1]) for row in summaries)
                result['failed_total'] = 0
                if result['passed'] <= 0:
                    raise ValueError('workspace doctests executed zero passing cases')
        results.append(dict(target=item, **result, seconds=round(time.monotonic() - started, 3)))
        print(item['id'], result['passed'], flush=True)
    if sum(row['passed'] for row in results) <= 0:
        raise ValueError('shard executed no passing cases')
    receipt = dict(schema=1, shard=index, context=identity, inventory=digest(items), targets=results)
    destination.mkdir(parents=True, exist_ok=True)
    (destination / f'shard-{index}.json').write_text(json.dumps(receipt, indent=2) + '\n')


def reconcile(receipts, items, identity):
    if len(receipts) != SHARDS:
        raise ValueError('missing or duplicate shard receipts')
    seen = set()
    groups = assignments(items)
    for receipt in receipts:
        index = receipt.get('shard')
        if type(index) is not int or index not in range(SHARDS) or index in seen:
            raise ValueError('unknown or duplicate shard')
        seen.add(index)
        if receipt.get('schema') != 1 or receipt.get('context') != identity or receipt.get('inventory') != digest(items):
            raise ValueError('receipt source, toolchain, flags or inventory mismatch')
        targets = receipt.get('targets', [])
        if sorted((row['target'] for row in targets), key=lambda row: row['id']) != sorted(groups[index], key=lambda row: row['id']):
            raise ValueError('missing, duplicate or unknown shard targets')
        total = 0
        for row in targets:
            item = row['target']
            if item['mode'] == 'test':
                binaries = row.get('binaries', [])
                if sorted((binary['member'] for binary in binaries), key=lambda member: member['id']) != item['members']:
                    raise ValueError('missing, duplicate or unexpected executable receipt')
                passed = 0
                for binary in binaries:
                    member = binary['member']
                    cases, ignored = binary.get('cases'), binary.get('ignored')
                    if not isinstance(cases, list) or not isinstance(ignored, list) or any(not isinstance(name, str) or not name for name in cases + ignored):
                        raise ValueError('missing named test inventory')
                    if len(cases) != len(set(cases)) or len(ignored) != len(set(ignored)) or not set(ignored) <= set(cases):
                        raise ValueError('duplicate cases or invalid ignored state')
                    reasons = binary.get('ignored_reasons')
                    if not isinstance(reasons, dict) or set(reasons) != set(ignored) or any(reason is not None and not isinstance(reason, str) for reason in reasons.values()):
                        raise ValueError('missing or invalid ignored reasons')
                    if not cases and member['id'] not in EMPTY_TEST_TARGETS:
                        raise ValueError('zero-case receipt')
                    if type(binary.get('passed')) is not int or binary['passed'] != len(cases) - len(ignored):
                        raise ValueError('test execution count mismatch')
                    passed += binary['passed']
                if row.get('passed') != passed or row.get('command') != command(item) or type(row.get('exit')) is not int or row['exit'] != 0:
                    raise ValueError('selector execution mismatch')
            elif item['mode'] == 'doc':
                summaries = row.get('summaries')
                if not isinstance(summaries, list) or not summaries or any(not isinstance(pair, list) or len(pair) != 2 or any(not isinstance(n, str) or not n.isdecimal() for n in pair) for pair in summaries):
                    raise ValueError('missing doctest execution')
                if len(summaries) != len(item['libraries']) or row.get('command') != command(item) or row.get('exit') != 0 or row.get('failed_total') != 0 or row.get('ignored_total') != sum(int(pair[1]) for pair in summaries) or row.get('passed', 0) <= 0 or row.get('passed') != sum(int(pair[0]) for pair in summaries):
                    raise ValueError('doctest count mismatch')
            elif row.get('passed') != 0 or row.get('command') != command(item) or type(row.get('exit')) is not int or row['exit'] != 0:
                raise ValueError('invalid build-only command, exit or count')
            if type(row.get('passed')) is not int or row['passed'] < 0:
                raise ValueError('invalid execution count')
            total += row['passed']
        if total <= 0:
            raise ValueError('shard executed zero passing cases')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--shard', type=int, choices=range(SHARDS))
    parser.add_argument('--receipts', type=Path, default=Path('build/ci-evidence/linux-workspace'))
    parser.add_argument('--aggregate', action='store_true')
    parser.add_argument('--record-binary', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.record_binary is not None:
        record_binary(args.record_binary)
    elif args.aggregate:
        files = list(args.receipts.rglob('*.json'))
        reconcile([json.loads(path.read_text()) for path in files], workspace_inventory(), context())
        print('All workspace targets and named cases reconciled.')
    elif args.shard is not None:
        execute(args.shard, args.receipts)
    else:
        parser.error('--shard or --aggregate is required')


if __name__ == '__main__':
    main()
