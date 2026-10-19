"""One fixed Windows temp-volume A/B; private raw output, existing closed v2 facts.

Diagnostic only: a passing B never repairs a failed A. No runtime setting,
request timeout, durability flag, test filter or test parallelism is changed.
"""
from __future__ import annotations

import argparse
import ctypes
from ctypes import wintypes
import hashlib
import importlib
import json
import ntpath
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import threading
import time
import tomllib

RAW_LIMIT = 256 * 1024
CARGO_LIMIT = 16 * 1024 * 1024
SUMMARY_LIMIT = 64 * 1024
WATCHDOG = 30
SCHEMA = 'nagi-windows-temp-volume-comparison-v1'
REASONS = frozenset({'complete', 'same_volume', 'topology_unknown', 'temp_api_fallback',
    'temp_binding', 'artifact_refused', 'binary_changed', 'source_refused', 'browser_refused',
    'inventory_refused', 'collector_refused', 'raw_oversize', 'invalid_utf8', 'watchdog',
    'spawn_failed', 'child_not_joined', 'invalid_exit', 'cleanup_failed', 'output_refused', 'internal_error'})


class Refused(Exception):
    def __init__(self, reason):
        self.reason = reason if type(reason) is str and reason in REASONS else 'internal_error'
        super().__init__('Windows temp-volume comparison refused')


class NativeWindows:
    """Three read-only Win32 APIs; no raw path, volume ID or WinError is rendered."""
    def __init__(self, kernel32=None):
        if kernel32 is None:
            if os.name != 'nt':
                raise Refused('topology_unknown')
            kernel32 = ctypes.WinDLL('kernel32', use_last_error=True)
        self.api_kind = 'get_temp_path2' if hasattr(kernel32, 'GetTempPath2W') else 'get_temp_path'
        self.get_temp = getattr(kernel32, 'GetTempPath2W', None) or kernel32.GetTempPathW
        self.get_temp.argtypes = [wintypes.DWORD, wintypes.LPWSTR]
        self.get_temp.restype = wintypes.DWORD
        self.get_mount = kernel32.GetVolumePathNameW
        self.get_mount.argtypes = [wintypes.LPCWSTR, wintypes.LPWSTR, wintypes.DWORD]
        self.get_mount.restype = wintypes.BOOL
        self.get_volume = kernel32.GetVolumeNameForVolumeMountPointW
        self.get_volume.argtypes = [wintypes.LPCWSTR, wintypes.LPWSTR, wintypes.DWORD]
        self.get_volume.restype = wintypes.BOOL

    def temp_root(self):
        buf = ctypes.create_unicode_buffer(32768)
        count = self.get_temp(len(buf), buf)
        if not 0 < count < len(buf) or not buf.value or not ntpath.isabs(buf.value):
            raise Refused('topology_unknown')
        return buf.value

    def volume_id(self, path):
        mount = ctypes.create_unicode_buffer(32768)
        volume = ctypes.create_unicode_buffer(64)
        if not self.get_mount(str(path), mount, len(mount)) or not self.get_volume(mount.value, volume, len(volume)):
            raise Refused('topology_unknown')
        if re.fullmatch(r'\\\\\?\\Volume\{[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}\}\\', volume.value) is None:
            raise Refused('topology_unknown')
        return volume.value.lower()


def _hash_file(path):
    try:
        if path.is_symlink():
            raise Refused('artifact_refused')
        with path.open('rb') as stream:
            info = os.fstat(stream.fileno())
            if not stat.S_ISREG(info.st_mode) or not 0 < info.st_size <= 512 * 1024 * 1024:
                raise Refused('artifact_refused')
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        return digest
    except OSError:
        raise Refused('artifact_refused') from None


def _read(path, limit, reason):
    try:
        if path.is_symlink():
            raise Refused(reason)
        with path.open('rb') as stream:
            if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
                raise Refused(reason)
            raw = stream.read(limit + 1)
        if len(raw) > limit:
            raise Refused(reason)
        return raw
    except OSError:
        raise Refused(reason) from None


def _unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise Refused('artifact_refused')
        value[key] = item
    return value


def _invalid_constant(value):
    raise Refused('artifact_refused')


def select_binary(repo, target_root, messages, cargo_exit):
    """Reject ambiguity/other executable targets; never guess from a target glob."""
    try:
        if type(cargo_exit) is not int or cargo_exit != 0:
            raise Refused('artifact_refused')
        repo, target_root = repo.resolve(strict=True), target_root.resolve(strict=True)
        manifest = repo / 'runtime/Cargo.toml'
        root_toml = tomllib.loads(_read(repo / 'Cargo.toml', 65536, 'artifact_refused').decode('utf-8'))
        runtime_toml = tomllib.loads(_read(manifest, 65536, 'artifact_refused').decode('utf-8'))
        if runtime_toml['package']['name'] != 'nagi-runtime' or runtime_toml['package']['version'] != {'workspace': True}:
            raise Refused('artifact_refused')
        version = root_toml['workspace']['package']['version']
        package_id = 'path+' + manifest.parent.as_uri() + '#nagi-runtime@' + version
        records = [json.loads(line, object_pairs_hook=_unique_object, parse_constant=_invalid_constant) for line in _read(messages, CARGO_LIMIT, 'artifact_refused').decode('utf-8').splitlines()]
        if not records or len(records) > 4096 or any(type(row) is not dict for row in records):
            raise Refused('artifact_refused')
        if (set(records[-1]) != {'reason', 'success'} or records[-1].get('reason') != 'build-finished' or
                records[-1].get('success') is not True or sum(row.get('reason') == 'build-finished' for row in records) != 1):
            raise Refused('artifact_refused')
        selected = []
        for row in records:
            if row.get('reason') not in {'compiler-artifact', 'compiler-message', 'build-script-executed', 'build-finished'}:
                raise Refused('artifact_refused')
            if row.get('reason') != 'compiler-artifact':
                continue
            target = row.get('target')
            if type(target) is not dict:
                raise Refused('artifact_refused')
            wanted = target.get('name') == 'session_csrf_public_api'
            if not wanted and row.get('executable') is None:
                continue
            if (not wanted or row.get('package_id') != package_id or
                    Path(row['manifest_path']).resolve(strict=True) != manifest or
                    target.get('kind') != ['test'] or target.get('crate_types') != ['bin'] or
                    Path(target['src_path']).resolve(strict=True) != repo / 'runtime/tests/session_csrf_public_api.rs' or
                    type(row.get('profile')) is not dict or row['profile'].get('test') is not True or
                    type(row.get('executable')) is not str or type(row.get('filenames')) is not list):
                raise Refused('artifact_refused')
            executable = Path(row['executable'])
            if executable.is_symlink():
                raise Refused('artifact_refused')
            executable = executable.resolve(strict=True)
            if executable.suffix.lower() != '.exe' or not executable.is_relative_to(target_root):
                raise Refused('artifact_refused')
            if row['filenames'].count(row['executable']) != 1:
                raise Refused('artifact_refused')
            selected.append(executable)
        if len(selected) != 1:
            raise Refused('artifact_refused')
        return selected[0], _hash_file(selected[0])
    except (OSError, ValueError, KeyError, TypeError, UnicodeError):
        raise Refused('artifact_refused') from None


def capture_process(argv, *, cwd, env, timeout):
    """At most RAW_LIMIT+1 bytes in private RAM; no unbounded communicate()."""
    started = time.monotonic()
    try:
        proc = subprocess.Popen(argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    except OSError:
        raise Refused('spawn_failed') from None
    data, fault = [], []
    def read():
        try:
            data.append(proc.stdout.read(RAW_LIMIT + 1))
            if len(data[0]) > RAW_LIMIT:
                try:
                    proc.kill()
                except OSError:
                    pass
        except (OSError, ValueError):
            fault.append(True)
    reader = threading.Thread(target=read, daemon=True)
    try:
        reader.start()
    except RuntimeError:
        try:
            proc.kill()
            proc.wait(timeout=3)
        except (OSError, subprocess.TimeoutExpired):
            return {'raw': b'', 'returncode': None, 'elapsed_ms': 0, 'closed': False, 'fault': 'child_not_joined'}
        proc.stdout.close()
        return {'raw': b'', 'returncode': proc.returncode, 'elapsed_ms': 0, 'closed': True, 'fault': 'spawn_failed'}
    timed_out = False
    try:
        proc.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        timed_out = True
        try:
            proc.kill()
        except OSError:
            pass
        try:
            proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            return {'raw': b'', 'returncode': None, 'elapsed_ms': 0, 'closed': False, 'fault': 'child_not_joined'}
    reader.join(timeout=3)
    closed = not reader.is_alive() and proc.poll() is not None
    if closed:
        proc.stdout.close()
    elapsed = min(33000, max(0, int((time.monotonic() - started) * 1000)))
    return {'raw': data[0] if data else b'', 'returncode': proc.returncode,
            'elapsed_ms': elapsed, 'closed': closed,
            'fault': 'watchdog' if timed_out else 'child_not_joined' if not closed or fault else None}


# Only a boolean crosses this private pipe; no native root/GUID leaves the child.
BINDING_CODE = '''import importlib.util,ntpath,os,sys
s=importlib.util.spec_from_file_location("_temp_volume_probe",sys.argv[1]);m=importlib.util.module_from_spec(s);sys.modules[s.name]=m;s.loader.exec_module(m)
n=m.NativeWindows();a=ntpath.normcase(ntpath.normpath(n.temp_root()));b=ntpath.normcase(ntpath.normpath(os.environ["TMP"]))
print("1" if n.api_kind=="get_temp_path2" and a==b and n.volume_id(a)==n.volume_id(b) else "0")
'''


def _platform_roots(env, native, result):
    result['temp_api'] = native.api_kind if native.api_kind in {'get_temp_path2', 'get_temp_path'} else 'unknown'
    if result['temp_api'] != 'get_temp_path2':
        raise Refused('temp_api_fallback')
    try:
        original = Path(native.temp_root()).resolve(strict=True)
        runner = Path(env['RUNNER_TEMP']).resolve(strict=True)
        if not original.is_dir() or not runner.is_dir():
            raise Refused('topology_unknown')
        a_volume, b_volume = native.volume_id(original), native.volume_id(runner)
        if type(a_volume) is not str or not a_volume or type(b_volume) is not str or not b_volume:
            raise Refused('topology_unknown')
        if a_volume == b_volume:
            result['topology'] = 'same_volume'
            raise Refused('same_volume')
        result['topology'] = 'distinct_volumes'
        return original, runner
    except (KeyError, OSError, ValueError):
        raise Refused('topology_unknown') from None


def _binding_argv():
    return [sys.executable, '-I', '-B', '-c', BINDING_CODE, str(Path(__file__).resolve())]


def check_platform(*, env, native=None, capture=capture_process):
    """Read-only precheck. Main independently rechecks; no cached attestation."""
    result = {'schema': 'nagi-windows-temp-volume-platform-v1', 'status': 'infra_failed',
              'reason': 'internal_error', 'topology': 'unknown', 'temp_api': 'unknown'}
    try:
        _, runner = _platform_roots(env, native or NativeWindows(), result)
        captured = capture(_binding_argv(), cwd=runner,
            env={**env, 'TMP': str(runner), 'TEMP': str(runner)}, timeout=3)
        cap = captured if type(captured) is dict else vars(captured)
        if (cap.get('closed') is not True or cap.get('fault') or type(cap.get('returncode')) is not int or cap['returncode'] != 0 or
                cap.get('raw') not in {b'1\n', b'1\r\n'}):
            raise Refused('temp_binding')
        result.update(status='ready', reason='complete')
    except Refused as error:
        result['reason'] = error.reason
    except Exception:
        result['reason'] = 'topology_unknown'
    return result, 0 if result['status'] == 'ready' else 1


def _source_bound(repo, collector):
    for name, expected in [('session_csrf_public_api.rs', collector.CSRF_REUSABLE_SOURCE),
                           ('browser_login_public_api.rs', collector.BROWSER_NATIVE_CALL_SOURCE)]:
        raw = _read(repo / 'runtime/tests' / name, 65536, 'source_refused')
        try:
            canonical = raw.decode('utf-8').replace('\r\n', '\n').encode('utf-8')
        except UnicodeError:
            raise Refused('source_refused') from None
        if hashlib.sha256(canonical).hexdigest() != expected:
            raise Refused('source_refused')


def _closed_arm(label, captured, browser, browser_exit, collector):
    raw, code = captured['raw'], captured['returncode']
    record, _ = collector.diagnose(raw, 'valid', code, browser, 'valid', browser_exit)
    if not collector._closed_record_shape(record, False) or record['status'] not in {'passed', 'tests_failed'}:
        raise Refused('collector_refused')
    suite = record['suites']['session']
    if (suite['input_status'] != 'valid' or suite['diagnostic_status'] != 'valid' or
            suite['inventory_status'] not in {'passed', 'failed'} or suite['running_count'] != 11 or
            suite['passed_count'] + suite['failed_count'] != 11 or suite['ignored_count'] != 0):
        raise Refused('collector_refused')
    observed = collector.csrf_reusable_observation(raw, code, record=record)
    if observed is not None and not collector._csrf_observation_valid(observed):
        raise Refused('collector_refused')
    native = collector.session_native_call_lines(raw, code, record)
    if type(native) is not list or len(native) > collector.NATIVE_CALL_FACT_LIMIT:
        raise Refused('collector_refused')
    for line in native:
        if line == 'session_native_call=unavailable':
            continue
        if type(line) is not str or not line.startswith('session_native_call='):
            raise Refused('collector_refused')
        try:
            fact = json.loads(line.removeprefix('session_native_call='))
        except (ValueError, TypeError):
            raise Refused('collector_refused') from None
        if not collector._native_call_output_valid(fact):
            raise Refused('collector_refused')
    return {'arm': label, 'elapsed_ms': captured['elapsed_ms'], 'v2': record,
            'reusable_failure': observed,
            'native_calls': native}


def compare(*, repo, target_root, messages, cargo_exit, browser_raw, browser_exit,
            env, native=None, capture=capture_process, collector=None):
    result = {'schema': SCHEMA, 'status': 'infra_failed', 'reason': 'internal_error',
              'topology': 'unknown', 'temp_api': 'unknown', 'order': 'A_then_B',
              'order_confound': True, 'causality': 'unproven', 'scope': 'all_11_fixture_roots',
              'exe_sha256': None, 'source_sha256': None, 'browser_reused': False,
              'cleanup': 'not_created', 'arms': []}
    owned, owned_identity, can_remove = None, None, True
    def run(argv, environment, seconds):
        nonlocal can_remove
        can_remove = False
        value = capture(argv, cwd=repo, env=environment, timeout=seconds)
        # Production and mock boundary both remain strictly bounded.
        cap = value if type(value) is dict else vars(value)
        can_remove = cap.get('closed') is True
        if not can_remove:
            raise Refused('child_not_joined')
        if cap.get('fault'):
            raise Refused(cap['fault'])
        if type(cap.get('raw')) is not bytes or len(cap['raw']) > RAW_LIMIT:
            raise Refused('raw_oversize')
        try:
            cap['raw'].decode('utf-8')
        except UnicodeError:
            raise Refused('invalid_utf8') from None
        if type(cap.get('returncode')) is not int or cap['returncode'] not in {0, 101}:
            raise Refused('invalid_exit')
        if type(cap.get('elapsed_ms')) is not int or not 0 <= cap['elapsed_ms'] <= 33000:
            raise Refused('internal_error')
        return cap
    try:
        repo = repo.resolve(strict=True)
        if collector is None:
            sys.path.insert(0, str(repo / 'scripts/ci'))
            collector = importlib.import_module('session_deadline_diagnostic')
        if len(collector.SESSION_CASES) != 11 or len(collector.BROWSER_CASES) != 10 or 'RUST_TEST_THREADS' in env:
            raise Refused('inventory_refused')
        _source_bound(repo, collector)
        result['source_sha256'] = collector.CSRF_REUSABLE_SOURCE
        executable, digest = select_binary(repo, target_root, messages, cargo_exit)
        result['exe_sha256'] = digest
        browser = _read(browser_raw, RAW_LIMIT, 'browser_refused')
        shared, _ = collector.diagnose(None, 'missing', 1, browser, 'valid', browser_exit)
        if not collector._closed_record_shape(shared, False) or shared['suites']['browser']['inventory_status'] != 'passed' or shared['suites']['browser']['diagnostic_status'] != 'valid':
            raise Refused('browser_refused')
        result['browser_reused'] = True
        native = native or NativeWindows()
        original, runner = _platform_roots(env, native, result)
        owned = Path(tempfile.mkdtemp(prefix='nagi-temp-volume-', dir=runner))
        identity = owned.lstat()
        owned_identity = (identity.st_dev, identity.st_ino)
        if native.volume_id(original) == native.volume_id(owned):
            raise Refused('topology_unknown')
        b_env = {**env, 'TMP': str(owned), 'TEMP': str(owned)}
        binding = run(_binding_argv(), b_env, 3)
        if binding['returncode'] != 0 or binding['raw'] not in {b'1\n', b'1\r\n'}:
            raise Refused('temp_binding')
        def unchanged():
            if _hash_file(executable) != digest:
                raise Refused('binary_changed')
            _source_bound(repo, collector)
        unchanged()
        listed = run([str(executable), '--list'], env, 3)
        lines = [line for line in listed['raw'].decode('ascii').splitlines() if line]
        expected = {name + ': test' for name in collector.SESSION_CASES} | {'11 tests, 0 benchmarks'}
        if listed['returncode'] != 0 or len(lines) != 12 or set(lines) != expected:
            raise Refused('inventory_refused')
        unchanged()
        for label, environment in [('A', dict(env)), ('B', b_env)]:
            unchanged()
            captured = run([str(executable)], environment, WATCHDOG)
            unchanged()
            result['arms'].append(_closed_arm(label, captured, browser, browser_exit, collector))
        result['status'] = 'tests_passed' if all(arm['v2']['status'] == 'passed' for arm in result['arms']) else 'tests_failed'
        result['reason'] = 'complete'
    except Refused as error:
        result['reason'] = error.reason
    except Exception:
        # Never expose a path, native exception, child output or import traceback.
        result['reason'] = 'internal_error'
    finally:
        if owned is not None:
            if can_remove:
                try:
                    now = owned.lstat()
                    if owned.is_symlink() or (now.st_dev, now.st_ino) != owned_identity:
                        raise OSError('owned directory identity changed')
                    shutil.rmtree(owned)
                    result['cleanup'] = 'removed'
                except Exception:
                    result.update(status='infra_failed', reason='cleanup_failed', cleanup='failed')
            else:
                result['cleanup'] = 'retained'
    return result, 0 if result['status'] == 'tests_passed' else 1


class ClosedParser(argparse.ArgumentParser):
    def error(self, message):
        self.exit(2, 'Windows temp-volume comparison usage refused\n')


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if argv == ['--check-platform']:
        result, code = check_platform(env=dict(os.environ))
        print(json.dumps(result, ensure_ascii=True, sort_keys=True, separators=(',', ':')))
        return code
    parser = ClosedParser()
    for name in ['repo-root', 'target-root', 'cargo-json', 'browser-raw', 'output-dir']:
        parser.add_argument('--' + name, type=Path, required=True)
    for name in ['cargo-exit-code', 'browser-exit-code']:
        parser.add_argument('--' + name, type=int, required=True)
    args = parser.parse_args(argv)
    try:
        args.output_dir.mkdir(mode=0o700, parents=False, exist_ok=False)
    except Exception:
        print('windows_temp_volume status=infra_failed reason=output_refused')
        return 1
    result, code = compare(repo=args.repo_root, target_root=args.target_root, messages=args.cargo_json,
        cargo_exit=args.cargo_exit_code, browser_raw=args.browser_raw, browser_exit=args.browser_exit_code,
        env=dict(os.environ))
    try:
        encoded = (json.dumps(result, ensure_ascii=True, allow_nan=False, sort_keys=True, separators=(',', ':')) + '\n').encode('ascii')
        if len(encoded) > SUMMARY_LIMIT:
            raise Refused('output_refused')
        (args.output_dir / 'comparison.json').write_bytes(encoded)
        for arm in result['arms']:
            (args.output_dir / (arm['arm'] + '.v2.json')).write_text(json.dumps(arm['v2'], ensure_ascii=True, sort_keys=True, separators=(',', ':')) + '\n', encoding='ascii')
    except Exception:
        print('windows_temp_volume status=infra_failed reason=output_refused')
        return 1
    print('windows_temp_volume status=' + result['status'] + ' reason=' + result['reason'] + ' topology=' + result['topology'])
    return code


if __name__ == '__main__':
    raise SystemExit(main())
