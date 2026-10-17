"""Linux-only, test-coordinator receipt validation. No process or trust changes."""
import json
import os
from pathlib import Path
import re
import stat
import sys


JSON_LIMIT = 4096
CA_LIMIT = 16384
_READ = os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC


class InvalidReceipt(ValueError):
    """Fixed diagnostic only: never include untrusted JSON or paths."""


def _reject():
    raise InvalidReceipt("invalid owned fixture receipt") from None


def _pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            _reject()
        result[key] = value
    return result


def _constant(_value):
    _reject()


def _shape(value, keys):
    if type(value) is not dict or set(value) != keys:
        _reject()
    if type(value["schema_version"]) is not int or value["schema_version"] != 1:
        _reject()


class OwnedRoot:
    """Pin an existing absolute owned0700 work root; caller owns its creation.

    Child opens are anchored to this held directory fd, never re-resolved through
    receipt-supplied parents. The path identity is rechecked before/after reads.
    This does not establish child exit, TLS validation, freshness, or browser pass.
    """

    def __init__(self, path):
        self.fd = None
        if sys.platform != "linux":
            _reject()
        raw = os.fspath(path)
        if type(raw) is not str or not os.path.isabs(raw) or os.path.normpath(raw) != raw:
            _reject()
        self.path = Path(raw)
        try:
            self.fd = os.open(raw, _READ | os.O_DIRECTORY)
            current = os.fstat(self.fd)
            self._directory(current)
            self.identity = (current.st_dev, current.st_ino)
            self._check_root()
        except (OSError, ValueError):
            self.close()
            _reject()

    @staticmethod
    def _directory(info):
        if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid()
                or stat.S_IMODE(info.st_mode) != 0o700):
            _reject()

    @staticmethod
    def _regular(info, limit):
        if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid()
                or stat.S_IMODE(info.st_mode) != 0o600 or not 0 <= info.st_size <= limit):
            _reject()

    def _check_root(self):
        if self.fd is None:
            _reject()
        for info in (os.fstat(self.fd), os.stat(self.path, follow_symlinks=False)):
            self._directory(info)
            if (info.st_dev, info.st_ino) != self.identity:
                _reject()

    def _read_json(self, name):
        fd = None
        try:
            self._check_root()
            fd = os.open(name, _READ, dir_fd=self.fd)
            before = os.fstat(fd)
            self._regular(before, JSON_LIMIT)
            data = bytearray()
            while len(data) <= JSON_LIMIT:
                chunk = os.read(fd, JSON_LIMIT + 1 - len(data))
                if not chunk:
                    break
                data.extend(chunk)
            after = os.fstat(fd)
            self._regular(after, JSON_LIMIT)
            signature = lambda info: (info.st_dev, info.st_ino, info.st_size,
                                      info.st_mtime_ns, info.st_ctime_ns)
            if len(data) != before.st_size or signature(before) != signature(after):
                _reject()
            self._check_root()
            return json.loads(data.decode("utf-8"), object_pairs_hook=_pairs,
                              parse_constant=_constant)
        except (OSError, ValueError, RecursionError):
            _reject()
        finally:
            if fd is not None:
                os.close(fd)

    def _ready_paths(self, ca, control):
        if type(ca) is not str or type(control) is not str:
            _reject()
        # Producer: mkdtemp(prefix="owned-", dir=work) / "ca.pem".
        parent = Path(ca).parent.name
        if (not re.fullmatch(r"owned-[A-Za-z0-9_-]+", parent)
                or ca != str(self.path / parent / "ca.pem")
                or control != str(self.path / "control.sock")):
            _reject()
        directory = certificate = None
        try:
            self._check_root()
            directory = os.open(parent, _READ | os.O_DIRECTORY, dir_fd=self.fd)
            self._directory(os.fstat(directory))
            certificate = os.open("ca.pem", _READ, dir_fd=directory)
            self._regular(os.fstat(certificate), CA_LIMIT)
            sock = os.stat("control.sock", dir_fd=self.fd, follow_symlinks=False)
            if (not stat.S_ISSOCK(sock.st_mode) or sock.st_uid != os.getuid()
                    or stat.S_IMODE(sock.st_mode) != 0o600):
                _reject()
            self._check_root()
        except (OSError, ValueError):
            _reject()
        finally:
            if certificate is not None:
                os.close(certificate)
            if directory is not None:
                os.close(directory)

    def ready(self):
        value = self._read_json("ready.json")
        _shape(value, {"schema_version", "app", "ui", "denied", "ca", "control"})
        ports = []
        for key, host in (("app", "localhost"), ("ui", "app.example.invalid"),
                          ("denied", "127.0.0.1")):
            origin = value[key]
            if type(origin) is not str:
                _reject()
            match = re.fullmatch(r"https://" + re.escape(host) + r":([1-9][0-9]{0,4})", origin)
            if match is None or not 1024 <= int(match[1]) <= 65535:
                _reject()
            ports.append(int(match[1]))
        if len(set(ports)) != 3:
            _reject()
        self._ready_paths(value["ca"], value["control"])
        return value

    def closed(self):
        value = self._read_json("closed.json")
        positive = {"frontends_joined", "http_joined", "prelogin_closed", "auth_closed",
                    "runtime_exit_zero"}
        _shape(value, positive | {"schema_version", "fixture_failed", "resets"})
        if any(value[key] is not True for key in positive) or value["fixture_failed"] is not False:
            _reject()
        if type(value["resets"]) is not int or not 0 <= value["resets"] <= 16:
            _reject()
        return value

    def close(self):
        if self.fd is not None:
            os.close(self.fd)
            self.fd = None

    def __enter__(self):
        return self

    def __exit__(self, *_exc):
        self.close()
