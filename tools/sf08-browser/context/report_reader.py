"""Owned SF08 report-file boundary; no browser/process/trust operations.

The coordinator must arm before starting its exclusively owned browser command,
set umask077, and call accept only after that exact process has exited. Absence
at arm time plus the caller's owned-run discipline provides run freshness;
this module alone does not prove process exit, source identity or browser trust.
"""
import json
import math
import os
import stat

from owned_json import OwnedRoot
from report_validator import validate_report, ReportValidationError


REPORT_NAME = 'browser-report.json'
REPORT_LIMIT = 1024 * 1024
DEPTH_LIMIT = 64


class InvalidBrowserReport(ValueError):
    pass


def _reject():
    raise InvalidBrowserReport('invalid owned browser report') from None


def _pairs(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            _reject()
        out[key] = value
    return out


def _constant(_text):
    _reject()


def _float(text):
    value = float(text)
    if not math.isfinite(value):
        _reject()
    return value


def _signature(info):
    return (info.st_dev, info.st_ino, info.st_size,
            info.st_mtime_ns, info.st_ctime_ns)


def _regular(info):
    if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid()
            or stat.S_IMODE(info.st_mode) != 0o600
            or not 0 <= info.st_size <= REPORT_LIMIT):
        _reject()


def _depth(value):
    pending = [(value, 0)]
    while pending:
        item, depth = pending.pop()
        if depth > DEPTH_LIMIT:
            _reject()
        if type(item) is dict:
            pending.extend((child, depth + 1) for child in item.values())
        elif type(item) is list:
            pending.extend((child, depth + 1) for child in item)


class FreshPlaywrightReport:
    def __init__(self, root: OwnedRoot):
        self.root = root
        self.consumed = False
        try:
            root._check_root()
            try:
                os.stat(REPORT_NAME, dir_fd=root.fd, follow_symlinks=False)
            except FileNotFoundError:
                root._check_root()
                return
        except (OSError, ValueError):
            _reject()
        _reject()

    def accept(self):
        # A failed attempt also consumes this arm; a later file may not turn a
        # rejected/missing report into success for the same run.
        if self.consumed:
            _reject()
        self.consumed = True
        fd = None
        try:
            self.root._check_root()
            fd = os.open(REPORT_NAME, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW
                         | os.O_CLOEXEC, dir_fd=self.root.fd)
            before = os.fstat(fd)
            _regular(before)
            data = bytearray()
            while len(data) <= REPORT_LIMIT:
                chunk = os.read(fd, REPORT_LIMIT + 1 - len(data))
                if not chunk:
                    break
                data.extend(chunk)
            after = os.fstat(fd)
            _regular(after)
            if len(data) != before.st_size or _signature(before) != _signature(after):
                _reject()
            self.root._check_root()
            value = json.loads(data.decode('utf-8'), object_pairs_hook=_pairs,
                               parse_constant=_constant, parse_float=_float)
            _depth(value)
            return validate_report(value)
        except (OSError, ValueError, RecursionError, ReportValidationError):
            _reject()
        finally:
            if fd is not None:
                os.close(fd)
