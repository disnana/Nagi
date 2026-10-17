"""Shared bounded fixture protocol. No native CLI, host budget or cached paths.

Extracted protocol bodies from audited native core r1; that frozen core remains
unchanged. Callers supply their own ownership and explicit resource budget.
"""
import hashlib
import importlib
import json
import os
from pathlib import Path
import re
import socket
import ssl
import stat
import struct
import sys
import time
from urllib.parse import urlsplit

CODES = {'binding', 'cancelled', 'startup_exit', 'ready', 'ca', 'tls', 'control',
         'stop_ack', 'close', 'child_exit', 'ownership', 'timeout', 'unexpected'}

class Rejected(RuntimeError):
    def __init__(self, code):
        self.code = code if code in CODES else 'unexpected'
        super().__init__(self.code)


def require(value, code):
    if not value:
        raise Rejected(code)


def components(bindings):
    sys.dont_write_bytecode = True
    os.environ['PYTHONDONTWRITEBYTECODE'] = '1'
    paths = [str(Path(bindings[name]['path']).parent)
             for name in ('owned_processes', 'owned_json', 'report_reader', 'report_validator')]
    sys.path[:0] = paths
    loaded = {}
    for name in ('owned_processes', 'owned_json', 'report_validator', 'report_reader'):
        module = importlib.import_module(name)
        require(Path(module.__file__).resolve() == Path(bindings[name]['path']).resolve(), 'binding')
        loaded[name] = module
    return loaded


def wait_exit(owner, leader, deadline, budget, *, cleanup=False):
    while True:
        budget.check(deadline, cleanup=cleanup)
        status = owner.peek(leader)
        if status is not None:
            require(status.si_code == os.CLD_EXITED and status.si_status == 0, 'child_exit')
            return
        time.sleep(0.02)


def wait_ready(root, owner, leader, budget):
    deadline = time.monotonic() + 30
    while True:
        budget.check(deadline)
        require(owner.peek(leader) is None, 'startup_exit')
        root._check_root()
        try:
            os.stat('ready.json', dir_fd=root.fd, follow_symlinks=False)
        except FileNotFoundError:
            root._check_root()
            time.sleep(0.02)
            continue
        # Once any object exists, malformed/symlink/FIFO never becomes a retry.
        return root.ready()


def ready_ca_bytes(root, ready):
    directory = fd = None
    try:
        root._check_root()
        ca = ready['ca']
        require(type(ca) is str, 'ca')
        parent = Path(ca).parent.name
        require(re.fullmatch(r'owned-[A-Za-z0-9_-]+', parent) is not None
                and ca == str(root.path / parent / 'ca.pem'), 'ca')
        flags = os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC
        directory = os.open(parent, flags | os.O_DIRECTORY, dir_fd=root.fd)
        root._directory(os.fstat(directory))
        fd = os.open('ca.pem', flags, dir_fd=directory)
        before = os.fstat(fd)
        root._regular(before, 16384)
        data = bytearray()
        while len(data) <= 16384:
            chunk = os.read(fd, 16385 - len(data))
            if not chunk:
                break
            data.extend(chunk)
        after = os.fstat(fd)
        root._regular(after, 16384)
        signature = lambda info: (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)
        require(0 < len(data) == before.st_size and signature(before) == signature(after), 'ca')
        root._check_root()
        return bytes(data)
    except (OSError, ValueError):
        raise Rejected('ca') from None
    finally:
        if fd is not None:
            os.close(fd)
        if directory is not None:
            os.close(directory)


def tls_probes(root, ready, owner, leader, budget):
    data = ready_ca_bytes(root, ready)
    # Same-fd verified bytes are the trust input; never reopen a CA path.
    context = ssl.create_default_context(cadata=data.decode('ascii'))
    for key, host in (('app', 'localhost'), ('ui', 'app.example.invalid'), ('denied', '127.0.0.1')):
        deadline = time.monotonic() + 5
        budget.check(deadline)
        require(owner.peek(leader) is None, 'startup_exit')
        url = urlsplit(ready[key])
        wire = None
        try:
            with socket.create_connection(('127.0.0.1', url.port), timeout=5) as tcp:
                tcp.settimeout(max(.001, deadline - time.monotonic()))
                wire = context.wrap_socket(tcp, server_hostname=host)
                with wire:
                    wire.settimeout(max(.001, deadline - time.monotonic()))
                    wire.sendall(f'GET / HTTP/1.1\r\nHost: {url.netloc}\r\nConnection: close\r\n\r\n'.encode('ascii'))
                    raw = bytearray()
                    while True:
                        budget.check(deadline)
                        wire.settimeout(max(.001, deadline - time.monotonic()))
                        chunk = wire.recv(32769 - len(raw))
                        if not chunk:
                            break
                        raw.extend(chunk)
                        require(len(raw) <= 32768, 'tls')
            require(bytes(raw).startswith(b'HTTP/1.1 200 ') and b'\r\n\r\n' in raw, 'tls')
        except (OSError, ValueError):
            raise Rejected('tls') from None
    return hashlib.sha256(data).hexdigest()


def stop_fixture(root, ready, owner, leader, budget):
    deadline = time.monotonic() + 30
    budget.check(deadline, cleanup=True)
    root._check_root()
    require(ready['control'] == str(root.path / 'control.sock'), 'control')
    info = os.stat('control.sock', dir_fd=root.fd, follow_symlinks=False)
    require(stat.S_ISSOCK(info.st_mode) and info.st_uid == os.getuid()
            and stat.S_IMODE(info.st_mode) == 0o600, 'control')
    require(owner.peek(leader) is None, 'child_exit')
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as peer:
        peer.settimeout(max(.001, deadline - time.monotonic()))
        # Magic fd path anchors the connect to the same held work inode.
        peer.connect(f'/proc/self/fd/{root.fd}/control.sock')
        credentials = peer.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, struct.calcsize('3i'))
        pid, uid, _gid = struct.unpack('3i', credentials)
        require(pid == leader.pid and uid == os.getuid(), 'control')
        peer.sendall(b'{"op":"stop"}\n')
        data = bytearray()
        while True:
            budget.check(deadline, cleanup=True)
            peer.settimeout(max(.001, deadline - time.monotonic()))
            chunk = peer.recv(129 - len(data))
            if not chunk:
                break
            data.extend(chunk)
            require(len(data) <= 128, 'stop_ack')
    parse_stop_ack(data)
    root._check_root()


def strict_pairs(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'stop_ack')
        result[key] = value
    return result


def parse_stop_ack(data):
    try:
        require(len(data) <= 128 and data.endswith(b'\n') and data.count(b'\n') == 1, 'stop_ack')
        value = json.loads(data.decode('ascii'), object_pairs_hook=strict_pairs)
        require(type(value) is dict and set(value) == {'stopping'} and value['stopping'] is True, 'stop_ack')
    except (ValueError, UnicodeError):
        raise Rejected('stop_ack') from None


def accept_browser_report(root, armed, owner, browser):
    # This does not reap; final acceptance also requires global capsule closure.
    status = owner.peek(browser)
    require(status is not None and status.si_code == os.CLD_EXITED and status.si_status == 0, 'child_exit')
    summary = armed.accept()
    require(summary == {'accepted': True, 'cases': 4, 'projects': 3, 'executions': 12}, 'close')
    return summary


def completed(mode, closed, closure, fixture, report=None):
    # closed has already passed OwnedRoot.closed's exact schema/type checks.
    require(closure is not None and closure.closed and closure.echild_observed and fixture.returncode == 0, 'ownership')
    require(type(closed['resets']) is int and closed['resets'] == (12 if mode == 'browser' else 0), 'close')
    if mode == 'browser':
        require(report == {'accepted': True, 'cases': 4, 'projects': 3, 'executions': 12}, 'close')
    else:
        require(mode == 'native' and report is None, 'close')


