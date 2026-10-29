"""Reversible infrastructure edits; preserve the inherited command-body proof.

Only named, checked cache/pin/checkout/retention edits are removed. This is not a
test-result adapter. Publication also has an executable fail-closed gate.
"""
import re
import importlib.util
from pathlib import Path

PINS = {
    ('actions/checkout', 'v4'): '11d5960a326750d5838078e36cf38b85af677262',
    ('actions/cache', 'v4'): '0057852bfaa89a56745cba8c7296529d2fc39830',
    ('actions/setup-python', 'v5'): 'a26af69be951a213d495a4c3e4e4022e16d87065',
    ('actions/upload-artifact', 'v4'): 'ea165f8d65b6e75b540449e92b4886f43607fa02',
    ('actions/download-artifact', 'v4'): 'd3f86a106a0bac45b974a628896c90dbdf5c8093',
    ('actions/setup-node', 'v4'): '49933ea5288caeca8642d1e84afbd3f7d6820020',
    ('actions/configure-pages', 'v5'): '983d7736d9b0ae728b81ab479565c72886d7745b',
    ('actions/setup-go', 'v7'): 'b7ad1dad31e06c5925ef5d2fc7ad053ef454303e',
    ('actions/deploy-pages', 'v4'): 'd6db90164ac5ed86f2b6aed7e0febac5b3c0c03e',
    ('actions/upload-pages-artifact', 'v3'): '56afc609e74202658d3ffba0e8f6dda462b719fa',
    ('dtolnay/rust-toolchain', 'stable'): 'b725eda525362e7cf34af9a7243cac1e3f01a7ec',
    ('dtolnay/rust-toolchain', '1.88.0'): '360a4e93673de6630fac2478bc9e35244b01a759',
}
USES = re.compile(r'(uses:\s+)([\w.-]+/[\w./-]+)@([^\s#]+)([^\n]*)')
JOB = re.compile(r'^  [a-z][a-z0-9-]*:\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)', re.M | re.S)
STEP = re.compile(r'^      - .*?(?=^      - |\Z)', re.M | re.S)
SAVE_IF = "success() && github.event_name == 'push' && github.ref == 'refs/heads/main' && steps.nagi_rust_cache.outputs.cache-hit != 'true' && steps.nagi_rust_cache.outputs.cache-primary-key != ''"
CRITICAL_SAVE_IF = "success() && github.event_name == 'pull_request' && steps.nagi_rust_cache.outputs.cache-hit != 'true' && steps.nagi_rust_cache.outputs.cache-primary-key != ''"
CANCELLABLE_JOBS = frozenset({'ci-plan', 'jetbrains', 'sf08-browser', 'nagi-package', 'ready', 'publish-release'})


def cancellation_edit(job, inverse=False):
    """Change only the six failure-independent job guards, never cleanup steps."""
    if job.split(':', 1)[0].strip() not in CANCELLABLE_JOBS:
        return job
    header, separator, steps = job.partition('    steps:\n')
    original = r'^(    if: (?:>-\n      )?)always\(\)'
    guarded = original + r' && !cancelled\(\)'
    if inverse and header.count('!cancelled()') != 1:
        raise ValueError('missing or duplicated job cancellation guard')
    pattern = guarded if inverse else original
    replacement = r'\1always()' + ('' if inverse else ' && !cancelled()')
    restored, count = re.subn(pattern, replacement, header, flags=re.M)
    if count != 1:
        raise ValueError('changed job cancellation guard')
    return restored + separator + steps


def action_key(action, ref):
    return ('/'.join(action.split('/')[:2]), ref)


def pin(source):
    def replace(match):
        lead, action, ref, comment = match.groups()
        if re.fullmatch('[0-9a-f]{40}', ref):
            return match.group()
        sha = PINS.get(action_key(action, ref))
        if sha is None:
            raise ValueError('unverified floating action')
        return f'{lead}{action}@{sha} # ci-pin:{ref}' + comment
    return USES.sub(replace, source)


def unpin(source):
    def replace(match):
        lead, action, sha, comment = match.groups()
        marker = re.match(r' # ci-pin:([^\s]+)(.*)', comment)
        if not marker:
            return match.group()
        ref, original_comment = marker.groups()
        if PINS.get(action_key(action, ref)) != sha:
            raise ValueError('action pin is not the reviewed ref')
        return f'{lead}{action}@{ref}' + original_comment
    return USES.sub(replace, source)


def save_step(path, critical=False):
    return (f"      - name: Save validated Rust cache ({'PR-local' if critical else 'main only'}) # ci-cache-save\n"
            f"        if: {CRITICAL_SAVE_IF if critical else SAVE_IF}\n"
            "        uses: actions/cache/save@v4\n"
            "        with:\n" + path +
            "          key: ${{ steps.nagi_rust_cache.outputs.cache-primary-key }}\n")


def cache_edit(job, inverse=False):
    if job.split(':', 1)[0].strip() in ('linux-workspace', 'windows-contracts'):
        spec = importlib.util.spec_from_file_location(
            'parallel_cache_projection', Path(__file__).with_name('parallel_cache_projection.py'))
        policy = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(policy)
        return policy.validate_restore_only_job(job, unpin)
    blocks = list(STEP.finditer(job))
    cache_ref = r'(?:v4|0057852bfaa89a56745cba8c7296529d2fc39830)'
    candidates = [m for m in blocks if re.search(r'uses: actions/cache(?:/restore)?@' + cache_ref, m.group())]
    if not candidates:
        if '# ci-cache-save' in job:
            raise ValueError('orphaned cache save')
        return job
    if len(candidates) != 1:
        raise ValueError('ambiguous Rust cache')
    match = candidates[0]
    block = match.group()
    if not re.search(r'key: nagi-', block):
        raise ValueError('unexpected cache owner')
    path = re.search(r'^          path: \|\n(?:^            .*\n)+', block, re.M)
    if path is None:
        raise ValueError('unsupported cache path')
    # L1 has a separate, Rust build/dependency PR cache namespace; it never executes on
    # main. Keeping its cache PR-local avoids a namespace that is never saved.
    saved = save_step(path.group(), job.startswith('  critical:\n'))
    if inverse:
        # A save step must be the final ordinary step, with an exact guard/path.
        if job.count(saved) != 1 or not job.rstrip().endswith(saved.rstrip()):
            raise ValueError('missing, moved, duplicated or weakened cache save')
        if block.count('        id: nagi_rust_cache\n') != 1:
            raise ValueError('missing cache identity')
        restored = block.replace('        id: nagi_rust_cache\n', '').replace('actions/cache/restore@', 'actions/cache@')
        job = job[:match.start()] + restored + job[match.end():]
        return job.replace(saved, '', 1)
    if '        id:' in block or '# ci-cache-save' in job:
        raise ValueError('cache policy already applied')
    changed = block.replace('actions/cache@', 'actions/cache/restore@')
    lines = changed.splitlines(keepends=True)
    index = next(i for i, line in enumerate(lines) if 'uses:' in line)
    lines.insert(index + 1, '        id: nagi_rust_cache\n')
    return job[:match.start()] + ''.join(lines) + job[match.end():] + saved


def isolation_step(block):
    if re.search(r'uses: actions/checkout@', block):
        if 'persist-credentials:' not in block:
            lines = block.splitlines(keepends=True)
            index = next((i for i, line in enumerate(lines) if line == '        with:\n'), None)
            if index is None:
                index = next(i for i, line in enumerate(lines) if 'uses:' in line)
                lines[index + 1:index + 1] = ['        with:\n', '          persist-credentials: false # ci-isolation\n']
            else:
                lines.insert(index + 1, '          persist-credentials: false # ci-isolation\n')
            block = ''.join(lines)
    if re.search(r'uses: actions/upload-artifact@', block) and 'retention-days:' not in block:
        days = 14 if re.search(r'name: (?:release-|jetbrains-common-candidate)', block) else 7
        if '        with:\n' not in block:
            raise ValueError('artifact has no explicit inputs')
        block = block.replace('        with:\n', f'        with:\n          retention-days: {days} # ci-retention\n', 1)
    return block


def infrastructure(source):
    source = JOB.sub(lambda m: cancellation_edit(m.group()), source)
    source = JOB.sub(lambda m: cache_edit(m.group()), source)
    source = STEP.sub(lambda m: isolation_step(m.group()), source)
    return pin(source)


def inverse_infrastructure(source):
    source = unpin(source)
    source = JOB.sub(lambda m: cancellation_edit(m.group(), inverse=True), source)
    for line in source.splitlines():
        if '# ci-isolation' in line and line != '          persist-credentials: false # ci-isolation':
            raise ValueError('checkout isolation weakened')
        if '# ci-retention' in line and not re.fullmatch(r'          retention-days: (7|14) # ci-retention', line):
            raise ValueError('artifact retention changed')
    source = source.replace('          persist-credentials: false # ci-isolation\n', '')
    source = re.sub(r'^          retention-days: (7|14) # ci-retention\n', '', source, flags=re.M)
    source = re.sub(r'^        with:\n(?=^\S|^ {0,8}\S|\Z)', '', source, flags=re.M)
    return JOB.sub(lambda m: cache_edit(m.group(), inverse=True), source)


def validate_infrastructure(source):
    for match in USES.finditer(source):
        if not re.fullmatch('[0-9a-f]{40}', match.group(3)):
            raise ValueError('external actions must be pinned')
    for match in STEP.finditer(source):
        block = match.group()
        if re.search(r'uses: actions/checkout@', block) and 'persist-credentials: false' not in block:
            raise ValueError('checkout credentials must not persist')
        if re.search(r'uses: actions/upload-artifact@', block) and 'retention-days:' not in block:
            raise ValueError('artifact retention must be explicit')
    inverse_infrastructure(source)
