"""Exact outer profile/cache experiment over verified #220 workflow wiring."""
import hashlib
import importlib.util
from pathlib import Path
import re


BASE_WORKFLOW_SHA256 = 'af6c58d8193e147924dc6c1d82ce914e599e0eb67d6fb8950b7b7c3ff9584a9b'


def _sibling(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


BASE = _sibling('platform_contract_projection')
BASE_JOB = BASE.PLATFORM_JOB
JOB_RE = re.compile(r'(?ms)^  platform-contracts:\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)')

OLD_ENV = """    env:
      NAGI_FAILURE_DIR: ${{ github.workspace }}/build/compiler-failures
      NAGI_GOLDEN_CAPTURE_DIR: ${{ github.workspace }}/build/compiler-failures/goldens
"""
PROFILE_ENV = """    env:
      CARGO_PROFILE_DEV_DEBUG: '0'
      CARGO_PROFILE_TEST_DEBUG: '0'
      CARGO_INCREMENTAL: '0'
      NAGI_FAILURE_DIR: ${{ github.workspace }}/build/compiler-failures
      NAGI_GOLDEN_CAPTURE_DIR: ${{ github.workspace }}/build/compiler-failures/goldens
"""

OLD_CACHE_KEY = (
    "key: nagi-platform-contracts-v1-${{ matrix.platform }}-${{ runner.os }}-"
    "${{ runner.arch }}-${{ steps.rust.outputs.cachekey }}-config-default-auth-browser-"
    "sf01-oracle-sf06-nodebug-noincremental-v1-${{ hashFiles('Cargo.lock') }}-"
    "${{ hashFiles('.github/workflows/ci.yml', 'scripts/ci/**', 'scripts/tests/**', "
    "'tests/test_sf06_http_client_signature_diagnostic.py') }}-"
    "${{ hashFiles('Cargo.toml', 'compiler/**', 'runtime/**', 'runtime-core/**') }}"
)
NEW_CACHE_KEY = (
    "key: nagi-platform-contracts-v2-${{ matrix.platform }}-${{ runner.os }}-"
    "${{ runner.arch }}-${{ steps.rust.outputs.cachekey }}-"
    "debug0-devtest-incr0-cfg-default-auth-browser-sf01-oracle-sf06-native-target-v2-"
    "${{ hashFiles('Cargo.lock') }}-"
    "${{ hashFiles('.github/workflows/ci.yml', 'scripts/ci/**', 'scripts/tests/**', "
    "'tests/test_sf06_http_client_signature_diagnostic.py') }}-"
    "${{ hashFiles('Cargo.toml', 'compiler/**', 'runtime/**', 'runtime-core/**') }}"
)


def _profile_job():
    if BASE_JOB.count(OLD_ENV) != 1 or BASE_JOB.count(OLD_CACHE_KEY) != 1:
        raise ValueError('verified platform-contract job anchors changed')
    job = BASE_JOB.replace(OLD_ENV, PROFILE_ENV, 1)
    job = job.replace(OLD_CACHE_KEY, NEW_CACHE_KEY, 1)
    return job


PROFILE_JOB = _profile_job()


def _workflow_hash(source):
    return hashlib.sha256(source.encode()).hexdigest()


def apply_workflow(source):
    """Apply only to the exact verified #220 workflow at 8381434."""
    if _workflow_hash(source) != BASE_WORKFLOW_SHA256:
        raise ValueError('profile experiment base is not the verified #220 workflow')
    found = list(JOB_RE.finditer(source))
    if len(found) != 1 or found[0].group() != BASE_JOB:
        raise ValueError('verified platform-contract job is missing or changed')
    return source[:found[0].start()] + PROFILE_JOB + source[found[0].end():]


def inverse_workflow(source):
    """Restore the complete verified #220 workflow byte-for-byte."""
    found = list(JOB_RE.finditer(source))
    if not found:
        if 'platform-contracts' in source or 'nagi-platform-contracts-v2-' in source:
            raise ValueError('platform-contract profile job is missing or partial')
        # A caller may already have consumed this outer layer before invoking
        # the existing historical inverses. No profile-specific residue may
        # remain in that state.
        return source
    if len(found) != 1:
        raise ValueError('platform-contract profile job is duplicated')
    if found[0].group() == BASE_JOB:
        # The outer edit is already absent. Leave unrelated source changes for
        # the existing historical inverse and its frozen workflow proof.
        return source
    if found[0].group() != PROFILE_JOB:
        raise ValueError('platform-contract profile or cache role changed')
    restored = source[:found[0].start()] + BASE_JOB + source[found[0].end():]
    return restored
