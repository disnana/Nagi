"""Pure closed-oracle/topology controls; these tests create no network namespace."""
import copy
import contextlib
import io
import importlib.util
import json
import os
import signal
import subprocess
import sys
import time
from unittest import mock
import tempfile
from types import SimpleNamespace
from pathlib import Path
import unittest

SOURCE = Path(__file__).resolve().parents[1] / 'ci' / 'sf06_owned_network.py'
spec = importlib.util.spec_from_file_location('owned', SOURCE)
owned = importlib.util.module_from_spec(spec)
spec.loader.exec_module(owned)


def profile(name):
    keys = ("root_anchors root_bytes config_bytes jobs queue job_roots dns_roots dns_children "
            "http_drivers waiters waiter_bytes senders whole_ms connect_ms attempts input_bytes "
            "sockets max_headers max_header_bytes max_http_buffer_bytes max_request_bytes "
            "max_response_bytes max_target_bytes").split()
    return dict(schema_version=1, case=name, elapsed_us=1234,
                counts=dict(dns_a=1, dns_aaaa=1, http=int(name == owned.CASES[0]), joins=3),
                configured_limits={key: 1 for key in keys})


def good(name, record=None):
    record = profile(name) if record is None else record
    return (f'running 1 test\ntest {name} ... ok\n'
            'sf06_production_profile ' + json.dumps(record) + '\n'
            'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.01s\n').encode()


def network():
    names = ['lo', 'owned0', 'owned1']
    links = [{'ifname': name} for name in names]
    addresses = [dict(ifname=name, addr_info=[dict(local=value)]) for name, value in
                  zip(names, ['127.0.0.1', '11.0.0.1', '11.0.0.2'])]
    routes = [{'dst': '127.0.0.0/8', 'dev': 'lo'}, {'dst': '11.0.0.0/30', 'dev': 'owned0'},
              {'dst': '11.0.0.2', 'dev': 'owned1', 'type': 'local'}]
    return links, addresses, routes, [{'dst': '::1', 'dev': 'lo', 'type': 'local'}]


class CaseControls(unittest.TestCase):
    def test_exact_two_identities_accept_lf_and_crlf(self):
        for name in owned.CASES:
            for raw in (good(name), good(name).replace(b'\n', b'\r\n')):
                self.assertEqual(owned.case_result(name, raw, 0, False), {'case': name, 'state': 'passed', 'profile': profile(name)})

    def test_nonzero_and_oversized_never_accept_passing_text(self):
        name = owned.CASES[0]
        for status, over in [(1, False), (101, False), (-1, False), (0, True)]:
            self.assertEqual(owned.case_result(name, good(name), status, over)['state'], 'failed')

    def test_zero_duplicate_other_case_and_ignored_refused(self):
        name = owned.CASES[0]
        for raw in [b'', good(name) * 2, good(owned.CASES[1]),
                    good(name).replace(b'1 test', b'0 tests'),
                    good(name).replace(b'0 ignored', b'1 ignored'),
                    good(name).replace(b' ... ok', b' ... ignored'),
                    good(name) + b'test other_case ... ok\n']:
            self.assertEqual(owned.case_result(name, raw, 0, False)['state'], 'failed')

    def test_bad_utf8_lone_cr_and_failure_rows_refused(self):
        name = owned.CASES[0]
        for raw in [good(name) + b'\xff', good(name).replace(b'\n', b'\r'),
                    good(name) + b'test other_case ... FAILED\n']:
            self.assertEqual(owned.case_result(name, raw, 0, False)['state'], 'failed')

    def test_profile_must_be_single_closed_finite_record(self):
        name = owned.CASES[0]
        for change in [dict(elapsed_us=0), dict(elapsed_us=True), dict(elapsed_us=float('nan')),
                       dict(extra='untrusted'), dict(case=owned.CASES[1]),
                       dict(counts=dict(dns_a=1, dns_aaaa=1, http=1, joins=2)),
                       dict(counts=dict(dns_a=True, dns_aaaa=1, http=1, joins=3)),
                       dict(configured_limits={})]:
            record = profile(name)
            record.update(change)
            self.assertEqual(owned.case_result(name, good(name, record), 0, False)['state'], 'failed')
        raw = good(name)
        self.assertEqual(owned.case_result(name, raw + b'sf06_production_profile {}\n', 0, False)['state'], 'failed')
        raw = raw.replace(b'"elapsed_us": 1234', b'"elapsed_us": 1234, "elapsed_us": 1234')
        self.assertEqual(owned.case_result(name, raw, 0, False)['state'], 'failed')

    def test_raw_payload_not_projected(self):
        name = owned.CASES[0]
        value = owned.case_result(name, b'private-payload\n' + good(name), 101, False)
        self.assertNotIn('private-payload', json.dumps(value))


class NamespaceControls(unittest.TestCase):
    def test_owned_topology_only(self):
        self.assertTrue(owned.topology_valid(*network()))

    def test_added_physical_link_or_address_refused(self):
        for mutate in ['link', 'address', 'missing']:
            data = network()
            if mutate == 'link':
                data[0].append({'ifname': 'eth0'})
            elif mutate == 'address':
                data[1][1]['addr_info'].append({'local': '192.0.2.1'})
            else:
                data[1][2]['addr_info'] = []
            self.assertFalse(owned.topology_valid(*data))

    def test_default_gateway_and_foreign_routes_refused(self):
        for route in [{'dst': 'default', 'dev': 'owned0'},
                      {'dst': '0.0.0.0/0', 'dev': 'owned0'},
                      {'dst': '11.0.0.0/30', 'dev': 'owned0', 'gateway': '11.0.0.1'},
                      {'dst': '10.0.0.0/8', 'dev': 'owned0'},
                      {'dst': '11.0.0.0/30', 'dev': 'eth0'}]:
            data = network()
            data[2].append(route)
            self.assertFalse(owned.topology_valid(*data))

    def test_ipv6_default_and_unexpected_unicast_refused(self):
        for route in [{'dst': 'default', 'dev': 'lo'}, {'dst': '2001:db8::/32', 'dev': 'owned1'}]:
            data = network()
            data[3].append(route)
            self.assertFalse(owned.topology_valid(*data))

    def test_invalid_shapes_fail_closed(self):
        for index, value in [(0, None), (1, [{'ifname': 'lo', 'addr_info': [{'local': 'bad'}]}]),
                             (2, [{'dst': None, 'dev': 'lo'}])]:
            data = list(network())
            data[index] = value
            self.assertFalse(owned.topology_valid(*data))

    def test_required_devices_immutable_across_copy(self):
        original = network()
        changed = copy.deepcopy(original)
        changed[0][1]['ifname'] = 'foreign'
        self.assertFalse(owned.topology_valid(*changed))
        self.assertTrue(owned.topology_valid(*original))


class PrivilegeControls(unittest.TestCase):
    def test_host_namespace_refused_before_any_network_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / 'native'
            binary.write_bytes(b'fixture-not-executed')
            binary.chmod(0o700)
            with mock.patch.object(owned.shutil, 'which', return_value='/utility'), \
                    mock.patch.object(owned.os, 'geteuid', return_value=0), \
                    mock.patch.object(owned, 'network_isolated', return_value=False), \
                    mock.patch.object(owned, 'run_ip') as run_ip, \
                    mock.patch.object(owned, 'read_ip') as read_ip, \
                    contextlib.redirect_stdout(io.StringIO()):
                status = owned.main(['--binary', str(binary), '--source-sha', 'a' * 40, '--inside',
                                     '--uid', '1001', '--gid', '1001', '--host-inode', '1234',
                                     '--binary-sha256', 'b' * 64])
            self.assertEqual(status, 1)
            run_ip.assert_not_called()
            read_ip.assert_not_called()

    def test_capabilities_and_no_new_privileges_before_native_execution(self):
        for mutation in ['CapEff', 'CapPrm', 'CapBnd', 'CapInh', 'CapAmb', 'NoNewPrivs']:
            values = dict.fromkeys(['CapEff', 'CapPrm', 'CapBnd', 'CapInh', 'CapAmb'], '0')
            values['NoNewPrivs'] = '1'
            values[mutation] = '0' if mutation == 'NoNewPrivs' else '1'
            text = ''.join(key + ': ' + value + '\n' for key, value in values.items())
            args = SimpleNamespace(uid=1001, gid=1001, host_pid_inode=1234)
            with mock.patch.object(owned.os, 'geteuid', return_value=1001), \
                    mock.patch.object(owned.os, 'getegid', return_value=1001), \
                    mock.patch.object(Path, 'read_text', return_value=text), \
                    mock.patch.object(owned, 'pid_isolated', return_value=True), \
                    mock.patch.object(owned, 'watch_parent'), \
                    mock.patch.object(owned, 'check_network') as check_network, \
                    mock.patch.object(owned, 'capture') as capture:
                with self.assertRaises(owned.Refused):
                    owned.execute(args)
            check_network.assert_not_called()
            capture.assert_not_called()



class CompleteInventoryControls(unittest.TestCase):
    def test_all_foreign_or_malformed_execution_candidates_refused(self):
        name = owned.CASES[0]
        extras = [
            b'test foreign::case ... FAILED\n', b'test unknown-case ... FAILED\n',
            b'test malformed\n', b'running nope\n', b'running 1 test\n',
            b'test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s\n',
            b'test result: nonsense\n',
            b'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s\n',
            b'sf06_production_profile\n',
        ]
        for extra in extras:
            with self.subTest(extra=extra):
                self.assertEqual(owned.case_result(name, good(name) + extra, 0, False)['state'], 'failed')
        for before, after in [(b'1 filtered out', b'2 filtered out'),
                              (b'0 measured', b'1 measured'),
                              (b'finished in 0.01s', b'finished in soon'),
                              (b'0 ignored', b'0 ignored, extra')]:
            self.assertEqual(owned.case_result(name, good(name).replace(before, after), 0, False)['state'], 'failed')
        outside = good(name).split(b'sf06_production_profile ', 1)
        record, tail = outside[1].split(b'\n', 1)
        self.assertEqual(owned.case_result(name, outside[0] + tail + b'sf06_production_profile ' + record + b'\n', 0, False)['state'], 'failed')

    def test_complete_listing_exact_footer_rows_and_multiplicity(self):
        raw = ('\n'.join(name + ': test' for name in owned.CASES) + '\n\n2 tests, 0 benchmarks\n').encode()
        self.assertTrue(owned.inventory_valid(raw, 0, False))
        self.assertTrue(owned.inventory_valid(raw.replace(b'\n', b'\r\n'), 0, False))
        variants = [raw + b'foreign::case: test\n', raw + b'test foreign::case ... FAILED\n',
                    raw + b'test result: FAILED\n', raw + b'2 tests, 0 benchmarks\n',
                    raw.replace(b'2 tests', b'3 tests'), raw.replace(b'0 benchmarks', b'1 benchmark'),
                    raw.replace(owned.CASES[1].encode(), owned.CASES[0].encode()),
                    raw.replace(b': test', b': malformed', 1), raw + b'\x00', raw + b'\xff',
                    raw.replace(b'\n', b'\r'), raw + b'private arbitrary text\n']
        for sample in variants:
            self.assertFalse(owned.inventory_valid(sample, 0, False))
        self.assertFalse(owned.inventory_valid(raw, 101, False))
        self.assertFalse(owned.inventory_valid(raw, 0, True))


class LifecycleControls(unittest.TestCase):
    def test_pid_namespace_requires_pid1_outer_difference_and_host_init_difference(self):
        for pid, own, outer, host, expected in [(1, 11, 22, 33, True),
                (2, 11, 22, 33, False), (1, 22, 22, 33, False), (1, 33, 22, 33, False)]:
            with mock.patch.object(owned.os, 'getpid', return_value=pid), \
                    mock.patch.object(owned.os, 'stat', side_effect=lambda path: SimpleNamespace(
                        st_ino=own if path == '/proc/self/ns/pid' else host)):
                self.assertEqual(owned.pid_isolated(outer, check_host_init=True), expected)

    def test_lease_requires_pid1_fifo_before_starting_watcher(self):
        for pid, mode in [(2, 0o010000), (1, 0o100000)]:
            with mock.patch.object(owned.os, 'getpid', return_value=pid), \
                    mock.patch.object(owned.os, 'fstat', return_value=SimpleNamespace(st_mode=mode)), \
                    mock.patch.object(owned.threading, 'Thread') as thread:
                with self.assertRaises(owned.Refused):
                    owned.watch_parent()
                thread.assert_not_called()

    def test_watcher_eof_unexpected_data_and_read_error_exit_without_signals(self):
        for returned in (b'', b'x', OSError('fixed test error')):
            called = []
            def read(_fd, _size):
                if isinstance(returned, Exception):
                    raise returned
                return returned
            def start(*, target, **_kwargs):
                called.append(target)
                return SimpleNamespace(start=lambda: None)
            with mock.patch.object(owned.os, 'getpid', return_value=1), \
                    mock.patch.object(owned.os, 'fstat', return_value=SimpleNamespace(st_mode=0o010000)), \
                    mock.patch.object(owned.threading, 'Thread', side_effect=start), \
                    mock.patch.object(owned.os, 'read', side_effect=read), \
                    mock.patch.object(owned.os, '_exit', side_effect=SystemExit) as exit_:
                owned.watch_parent()
                with self.assertRaises(SystemExit):
                    called[0]()
                exit_.assert_called_once_with(125)

    def test_real_pipe_eof_exits_python_watcher_process(self):
        # Only PID identity is stubbed here: this proves pipe/exec-thread EOF,
        # not the kernel PID-namespace descendant kill guarantee.
        code = ("import importlib.util,os,sys,time; "
                "spec=importlib.util.spec_from_file_location('watcher',sys.argv[1]); "
                "m=importlib.util.module_from_spec(spec); spec.loader.exec_module(m); "
                "m.os.getpid=lambda:1; m.watch_parent(); print('ready',flush=True); time.sleep(60)")
        process = subprocess.Popen([sys.executable, '-I', '-B', '-c', code, str(SOURCE)],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.DEVNULL, start_new_session=True, close_fds=True)
        try:
            import select
            ready, _, _ = select.select([process.stdout], [], [], 5)
            self.assertTrue(ready)
            self.assertEqual(process.stdout.readline(32), b'ready\n')
            process.stdin.close()
            self.assertEqual(process.wait(timeout=5), 125)
        finally:
            process.stdin.close()
            if process.poll() is None:
                process.kill()
            process.wait()
            process.stdout.close()

    def test_supervisor_closes_pipe_then_reaps_on_success_cancel_and_exception(self):
        for event in ('success', 'sigint', 'sigterm', 'exception'):
            events, handlers = [], {}
            pipe = SimpleNamespace(close=lambda: events.append('close'))
            first = True
            def wait():
                nonlocal first
                events.append('wait')
                if first:
                    first = False
                    if event.startswith('sig'):
                        handlers[signal.SIGINT if event == 'sigint' else signal.SIGTERM](0, None)
                    if event == 'exception':
                        raise RuntimeError('fixed failure')
                return 0
            process = SimpleNamespace(stdin=pipe, wait=wait)
            with mock.patch.object(owned.signal, 'getsignal', return_value=signal.SIG_DFL), \
                    mock.patch.object(owned.signal, 'signal', side_effect=lambda sig, handler: handlers.__setitem__(sig, handler)), \
                    mock.patch.object(owned.signal, 'pthread_sigmask', return_value=set()), \
                    mock.patch.object(owned.subprocess, 'Popen', return_value=process) as popen:
                if event == 'exception':
                    with self.assertRaises(RuntimeError):
                        owned.supervised(['fixed'])
                else:
                    self.assertEqual(owned.supervised(['fixed']), int(event != 'success'))
            self.assertEqual(events, ['wait', 'close', 'wait'])
            self.assertEqual(popen.call_args.kwargs['stdin'], subprocess.PIPE)
            self.assertTrue(popen.call_args.kwargs['close_fds'])
            self.assertEqual(handlers, {signal.SIGINT: signal.SIG_DFL, signal.SIGTERM: signal.SIG_DFL})

    def test_signal_after_launch_before_wait_still_closes_and_reaps(self):
        events, handlers = [], {}
        process = SimpleNamespace(stdin=SimpleNamespace(close=lambda: events.append('close')),
                                  wait=lambda: events.append('wait') or 0)
        pending = True
        def mask(how, _values):
            nonlocal pending
            if how == signal.SIG_SETMASK and pending:
                pending = False
                handlers[signal.SIGTERM](0, None)
            return set()
        with mock.patch.object(owned.signal, 'getsignal', return_value=signal.SIG_DFL), \
                mock.patch.object(owned.signal, 'signal', side_effect=lambda sig, handler: handlers.__setitem__(sig, handler)), \
                mock.patch.object(owned.signal, 'pthread_sigmask', side_effect=mask), \
                mock.patch.object(owned.subprocess, 'Popen', return_value=process):
            self.assertEqual(owned.supervised(['fixed']), 1)
        self.assertEqual(events, ['close', 'wait'])

    def test_all_subordinate_capture_stdin_is_devnull(self):
        process = SimpleNamespace(stdout=io.BytesIO(b''), wait=lambda **_kwargs: 0)
        with mock.patch.object(owned.subprocess, 'Popen', return_value=process) as popen:
            self.assertEqual(owned.capture(['fixed']), (b'', 0, False))
        self.assertEqual(popen.call_args.kwargs['stdin'], subprocess.DEVNULL)
        with mock.patch.object(owned.subprocess, 'run') as run:
            owned.run_ip('link', 'show')
        self.assertEqual(run.call_args.kwargs['stdin'], subprocess.DEVNULL)
        with mock.patch.object(owned.subprocess, 'check_output', return_value=b'[]') as read:
            owned.read_ip('link', 'show')
        self.assertEqual(read.call_args.kwargs['stdin'], subprocess.DEVNULL)

    def test_root_pid_check_precedes_network_mutation_and_setpriv_rearms(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / 'native'
            binary.write_bytes(b'fixture'); binary.chmod(0o700)
            args = ['--binary', str(binary), '--source-sha', 'a' * 40, '--inside',
                    '--uid', '1001', '--gid', '1001', '--host-inode', '1234',
                    '--host-pid-inode', '5678', '--binary-sha256', 'b' * 64]
            original_stat = owned.os.stat
            def proc_stat(path, *args, **kwargs):
                return SimpleNamespace(st_ino=11) if str(path).startswith('/proc/') else original_stat(path, *args, **kwargs)
            for isolated in (False, True):
                with mock.patch.object(owned.shutil, 'which', return_value='/utility'), \
                        mock.patch.object(owned.os, 'geteuid', return_value=0), \
                        mock.patch.object(owned, 'network_isolated', return_value=True), \
                        mock.patch.object(owned.os, 'stat', side_effect=proc_stat), \
                        mock.patch.object(owned, 'pid_isolated', return_value=isolated), \
                        mock.patch.object(owned, 'watch_parent') as watcher, \
                        mock.patch.object(owned, 'run_ip') as run, \
                        mock.patch.object(owned, 'read_ip') as read, \
                        contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(owned.main(args), 1)
                run.assert_not_called(); read.assert_not_called(); watcher.assert_not_called()
        # Separate source check prevents losing either watcher across exec.
        source = SOURCE.read_text()
        self.assertIn('"--pid", "--fork", "--kill-child"', source)
        self.assertNotIn('--mount-proc', source)
        self.assertEqual(source.count('    watch_parent()'), 2)
        self.assertIn('"--host-pid-inode", str(args.host_pid_inode)', source)



def namespace_probe_command(program, child):
    uid, gid = os.geteuid(), os.getegid()
    if uid <= 0:
        raise AssertionError('hosted probe requires an unprivileged owner')
    return ['sudo', '-n', 'unshare', '--net', '--pid', '--fork', '--kill-child', '--',
            'setpriv', '--reuid', str(uid), '--regid', str(gid), '--clear-groups',
            '--bounding-set=-all', '--inh-caps=-all', '--ambient-caps=-all', '--no-new-privs',
            sys.executable, '-I', '-B', '-c', program, str(SOURCE),
            str(os.stat('/proc/self/ns/pid').st_ino), child, str(uid), str(gid)]


class HostedProbeCommandControls(unittest.TestCase):
    def test_drop_to_owner_before_python_and_preserve_pid1_lease(self):
        with mock.patch.object(os, 'geteuid', return_value=1001), \
                mock.patch.object(os, 'getegid', return_value=1002), \
                mock.patch.object(os, 'stat', return_value=SimpleNamespace(st_ino=321)):
            command = namespace_probe_command('owned-program', 'owned-child')
        self.assertEqual(command[:8], ['sudo', '-n', 'unshare', '--net', '--pid', '--fork', '--kill-child', '--'])
        self.assertEqual(command[8:18], ['setpriv', '--reuid', '1001', '--regid', '1002', '--clear-groups',
            '--bounding-set=-all', '--inh-caps=-all', '--ambient-caps=-all', '--no-new-privs'])
        self.assertEqual(command[18:23], [sys.executable, '-I', '-B', '-c', 'owned-program'])
        self.assertEqual(command[-4:], ['321', 'owned-child', '1001', '1002'])
        self.assertNotIn('--mount-proc', command)
        with mock.patch.object(os, 'geteuid', return_value=0), \
                mock.patch.object(os, 'getegid', return_value=0):
            with self.assertRaises(AssertionError):
                namespace_probe_command('owned-program', 'owned-child')


@unittest.skipUnless(sys.platform == 'linux' and os.environ.get('NAGI_SF06_NAMESPACE_PROBE') == '1',
                     'hosted opt-in privileged PID-namespace lifecycle infrastructure probe')
class HostedLifecycleProbe(unittest.TestCase):
    def test_namespace_eof_reaps_setsid_descendant(self):
        # No route mutation/native binary/dependency/network service. This checks
        # real kernel PID1 EOF teardown, including a child in a separate session.
        child = ('import os,time; '
                 's=open("/proc/self/status").read(); '
                 'print(next(x for x in s.splitlines() if x.startswith("NSpid:")).split()[1],flush=True); '
                 'time.sleep(60)')
        program = '''import importlib.util,json,os,subprocess,sys
spec=importlib.util.spec_from_file_location("probe",sys.argv[1]);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
assert m.pid_isolated(int(sys.argv[2]),check_host_init=False)
assert os.geteuid()==int(sys.argv[4])!=0 and os.getegid()==int(sys.argv[5])
status=dict(line.split(":",1) for line in open("/proc/self/status").read().splitlines() if ":" in line)
assert all(int(status[key].strip(),16)==0 for key in ("CapEff","CapPrm","CapBnd","CapInh","CapAmb"))
assert status["NoNewPrivs"].strip()=="1"
m.watch_parent()
p=subprocess.Popen([sys.executable,"-I","-B","-c",sys.argv[3]],stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,start_new_session=True)
child=int(p.stdout.readline(32))
status=open("/proc/self/status").read()
parent=int(next(x for x in status.splitlines() if x.startswith("NSpid:")).split()[1])
print(json.dumps([parent,child]),flush=True)
p.wait()
'''
        # Exec as the same unprivileged owner before Python: /proc namespace
        # observations no longer depend on permission to ptrace a root process.
        command = namespace_probe_command(program, child)
        process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.DEVNULL, start_new_session=True, close_fds=True)
        line = []
        reader = __import__('threading').Thread(target=lambda: line.append(process.stdout.readline(128)), daemon=True)
        reader.start()
        try:
            reader.join(timeout=15)  # bounded readiness only, not teardown proof
            self.assertFalse(reader.is_alive(), 'namespace lifecycle probe readiness timed out')
            pids = json.loads(line[0])
            self.assertEqual(len(pids), 2)
            self.assertTrue(all(type(pid) is int and pid > 1 for pid in pids))
            namespaces = [os.stat(f'/proc/{pid}/ns/pid').st_ino for pid in pids]
            self.assertEqual(namespaces[0], namespaces[1])
            self.assertNotEqual(namespaces[0], os.stat('/proc/self/ns/pid').st_ino)
        finally:
            process.stdin.close()
            process.wait()  # actual namespace init exit + unshare waiter reap
            reader.join()
            process.stdout.close()
        self.assertEqual(process.returncode, 125)
        self.assertTrue(all(not Path(f'/proc/{pid}').exists() for pid in pids),
                        'namespace descendants survived EOF teardown')

if __name__ == '__main__':
    unittest.main()
