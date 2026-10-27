"""Select fail-closed CI coverage; publishing remains a separate approved action."""
from __future__ import annotations
import json
import os
import subprocess
from pathlib import Path
import changes

PLATFORMS = [
    dict(os='ubuntu-latest', platform='linux-x86_64', executable='nagic', rust_host='x86_64-unknown-linux-gnu', go_tests='29'),
    dict(os='windows-latest', platform='windows-x86_64', executable='nagic.exe', rust_host='x86_64-pc-windows-msvc', go_tests='28'),
    dict(os='macos-15', platform='macos-arm64', executable='nagic', rust_host='aarch64-apple-darwin', go_tests='29'),
    dict(os='macos-15-intel', platform='macos-x86_64', executable='nagic', rust_host='x86_64-apple-darwin', go_tests='29'),
]
# Only these reviewed front-end paths can use L1 native coverage. Everything
# else, including checker/emitter/lowering and all runtime ownership/task/
# auth/storage/network boundaries, requires the comprehensive level.
ROUTINE_RUST = frozenset({
    'compiler/src/lexer.rs', 'compiler/src/parser.rs', 'compiler/tests/enum_parser.rs',
})


def boolean(value):
    if value not in ('true', 'false'):
        raise ValueError('missing or invalid CI boolean')
    return value == 'true'


def expected(level, full, package, go):
    if level not in ('1', '2', '3'):
        raise ValueError('invalid CI level')
    full_selected = boolean(full)
    package_selected = boolean(package)
    go_selected = boolean(go)
    nagi = full_selected or package_selected
    comprehensive = level != '1' and nagi
    return dict(level=level, critical_checks=str(level == '1' and full_selected).lower(),
                comprehensive_checks=str(comprehensive).lower(), package_checks=str(comprehensive).lower(),
                go_checks=str(go_selected or package_selected).lower(),
                matrix=json.dumps({'include': PLATFORMS if level == '3' else PLATFORMS[:3]}, separators=(',', ':')))


def ordinary(path):
    """Only recognized source/test scopes can use the fast PR level."""
    if not changes.valid_path(path):
        return False
    if changes.is_docs_path(path) or changes.is_jetbrains_path(path):
        return True
    return path in ROUTINE_RUST


def select(event, paths, full, go, release, requested='default'):
    boolean(full)
    boolean(go)
    for component in ('nagi', 'vscode', 'jetbrains'):
        boolean(release.get('package_' + component))
        boolean(release.get('release_' + component))
    if requested not in ('default', 'release'):
        raise ValueError('invalid requested CI level')
    formal = any(boolean(release['release_' + name]) for name in ('nagi', 'vscode', 'jetbrains'))
    if formal or (event == 'workflow_dispatch' and requested == 'release'):
        level = '3'
    elif event == 'pull_request' and paths and all(ordinary(path) for path in paths):
        level = '1'
    else:
        level = '2'  # missing range, unknown/control-plane/dependency path, main, schedule, unfamiliar event
    return expected(level, full, release['package_nagi'], go)


def complete_paths(base, head):
    if any(not changes.COMMIT.fullmatch(ref) or set(ref) == {'0'} for ref in (base, head)):
        return None
    try:
        for ref in (base, head):
            if changes.git('rev-parse', '--verify', ref + '^{commit}').decode('ascii').strip() != ref:
                return None
        changes.git('merge-base', '--is-ancestor', base, head)
        raw = changes.git('diff', '--no-ext-diff', '--no-renames', '--name-only', '-z', base, head, '--')
        if not raw or not raw.endswith(b'\0'):
            return None
        paths = raw[:-1].decode('utf-8').split('\0')
        return paths if all(changes.valid_path(path) for path in paths) else None
    except (OSError, subprocess.SubprocessError, UnicodeError):
        return None


def validation_errors(needs):
    failures = []
    job = needs.get('ci-plan', {})
    if job.get('result') != 'success':
        failures.append('ci-plan: ' + str(job.get('result', 'missing')))
    selected = job.get('outputs', {})
    full = needs.get('changes', {}).get('outputs', {}).get('full_checks')
    go = needs.get('changes', {}).get('outputs', {}).get('go_checks')
    release = needs.get('release-plan', {}).get('outputs', {})
    try:
        for component in ('nagi', 'vscode', 'jetbrains'):
            boolean(release.get('release_' + component))
        wanted = expected(selected.get('level'), full, release.get('package_nagi'), go)
        if any(boolean(release['release_' + name]) for name in ('nagi', 'vscode', 'jetbrains')) and wanted['level'] != '3':
            raise ValueError('formal release requires level 3')
        for key, value in wanted.items():
            if selected.get(key) != value:
                raise ValueError('selector or matrix mismatch: ' + key)
    except (ValueError, TypeError, AttributeError) as error:
        failures.append('ci-plan: ' + str(error))
    return failures


def main():
    release = json.loads(os.environ['CI_RELEASE_PLAN'])
    result = select(os.environ['CI_EVENT'], complete_paths(os.environ.get('CI_BASE', ''), os.environ['CI_HEAD']),
                    os.environ['CI_FULL'], os.environ['CI_GO'], release,
                    os.environ.get('CI_REQUESTED', 'default'))
    with Path(os.environ['GITHUB_OUTPUT']).open('a', encoding='utf-8') as output:
        for key, value in result.items():
            output.write(f'{key}={value}\n')
    print(json.dumps(result, indent=2))

if __name__ == '__main__':
    main()
