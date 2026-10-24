"""Restore-only shared caches for exactly the two reviewed parallel jobs.

The existing jobs remain the only writers of their immutable shared keys.
Undo only this cache policy before the unchanged relocation/history proofs.
"""
import importlib.util
from pathlib import Path
import re


def _sibling(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


OLD_JOBS = {
    'linux-workspace': _sibling('linux_workspace_projection').JOB,
    'windows-contracts': _sibling('windows_contract_projection').JOB,
}
ANCHORS = {'linux-workspace': '  rust-1-88:\n', 'windows-contracts': '  nagi-regression:\n'}
PREFIXES = {
    'linux-workspace': ('nagi-rust-v1-linux-workspace-', 'nagi-rust-v1-checks-'),
    'windows-contracts': ('nagi-rust-v1-windows-contracts-windows-x86_64-',
                          'nagi-rust-v1-package-windows-x86_64-'),
}


def _restore_only(name, original):
    role, shared = PREFIXES[name]
    own_restore = re.findall(r'^            ' + re.escape(role) + r'.*\n', original, re.M)
    if (original.count('          key: ' + role) != 1 or len(own_restore) != 1
            or original.count('      - name: Save validated Rust cache') != 1):
        raise ValueError('parallel cache historical policy mismatch')
    source = original.replace('          key: ' + role, '          key: ' + shared, 1)
    source = source.replace(own_restore[0], '', 1)
    return source[:source.index('      - name: Save validated Rust cache')]


NEW_JOBS = {name: _restore_only(name, original) for name, original in OLD_JOBS.items()}


def _replace(source, before, after):
    for name in OLD_JOBS:
        if (len(re.findall(r'(?m)^  ' + re.escape(name) + r':$', source)) != 1
                or source.count(before[name] + ANCHORS[name]) != 1):
            raise ValueError('parallel restore-only cache policy mismatch')
        source = source.replace(before[name], after[name], 1)
    return source


def apply_workflow(source):
    """Exact forward transform for component roundtrips, not arbitrary edits."""
    return _replace(source, OLD_JOBS, NEW_JOBS)


def inverse_workflow(source):
    return _replace(source, NEW_JOBS, OLD_JOBS)


def validate_restore_only_job(job, unpin):
    name = job.split(':', 1)[0].strip()
    if name not in NEW_JOBS:
        raise ValueError('unreviewed restore-only Rust job')
    expected = NEW_JOBS[name]
    # Infrastructure's canonical inverse removes only reviewed pin/isolation/
    # retention fields. Accept that exact profile as well as the pinned job.
    plain = unpin(expected).replace('          persist-credentials: false # ci-isolation\n', '')
    plain = plain.replace('          retention-days: 7 # ci-retention\n', '')
    plain = re.sub(r'^        with:\n(?=^\S|^ {0,8}\S|\Z)', '', plain, flags=re.M)
    if job not in (expected, plain):
        raise ValueError('changed restore-only Rust job')
    return job
