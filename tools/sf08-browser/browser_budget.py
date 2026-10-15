"""Explicit ephemeral-container resource profile; never consult host paths.

Sampled accounting is not a filesystem quota. The outer CI tmpfs has a hard
4GiB limit; this narrower browser profile bounds its work+trust trees. Symlink
metadata is counted without following browser-owned profile lock links.
"""
import os
from pathlib import Path
import shutil
import stat
import time

FREE_FLOOR = 64 * 1024 * 1024
WRITE_CAP = 256 * 1024 * 1024
ENTRY_CAP = 32768


class ResourceRefused(RuntimeError):
    def __init__(self, code):
        self.code = code
        super().__init__(code)


def directory(path):
    info = os.stat(path, follow_symlinks=False)
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
        raise ResourceRefused('capacity')
    return info.st_dev, info.st_ino


class BrowserBudget:
    def __init__(self, owner, work, base):
        self.owner, self.work, self.base = owner, Path(work), Path(base)
        self.base_identity = directory(self.base)
        if self.work.parent != self.base or not self.work.name.startswith('run-'):
            raise ResourceRefused('capacity')
        self.roots = {self.work: directory(self.work)}
        if self.roots[self.work][0] != self.base_identity[0]:
            raise ResourceRefused('capacity')

    def add_trust(self, trust):
        trust = Path(trust)
        if trust.parent != self.base or not trust.name.startswith('trust-') or len(self.roots) != 1:
            raise ResourceRefused('capacity')
        identity = directory(trust)
        if identity[0] != self.base_identity[0]:
            raise ResourceRefused('capacity')
        self.roots[trust] = identity

    def used(self, deadline):
        total = count = 0
        flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
        def scan(fd):
            nonlocal total, count
            with os.scandir(fd) as entries:
                for item in entries:
                    if time.monotonic() >= deadline:
                        raise ResourceRefused('timeout')
                    try:
                        info = item.stat(follow_symlinks=False)
                    except FileNotFoundError:
                        continue  # Atomic/SQLite/browser temporary files can vanish.
                    count += 1
                    if count > ENTRY_CAP:
                        raise ResourceRefused('capacity')
                    if stat.S_ISDIR(info.st_mode):
                        child = None
                        try:
                            child = os.open(item.name, flags, dir_fd=fd)
                            pinned = os.fstat(child)
                            if (pinned.st_dev,pinned.st_ino) != (info.st_dev,info.st_ino):
                                raise ResourceRefused('capacity')
                            scan(child)
                        except FileNotFoundError:
                            continue
                        finally:
                            if child is not None: os.close(child)
                    else:
                        # Never traverse a symlink or open body/keys/cookies.
                        total += info.st_size
                        if total > WRITE_CAP:
                            raise ResourceRefused('capacity')
        for root, identity in self.roots.items():
            if directory(root) != identity:
                raise ResourceRefused('capacity')
            fd = os.open(root, flags)
            try:
                info = os.fstat(fd)
                if (info.st_dev,info.st_ino) != identity:
                    raise ResourceRefused('capacity')
                scan(fd)
            finally:
                os.close(fd)
            if directory(root) != identity:
                raise ResourceRefused('capacity')
        return total

    def check(self, deadline, *, cleanup=False):
        if directory(self.base) != self.base_identity or shutil.disk_usage(self.base).free < FREE_FLOOR:
            raise ResourceRefused('capacity')
        if time.monotonic() >= deadline:
            raise ResourceRefused('timeout')
        if not cleanup and self.owner.cancelled:
            raise ResourceRefused('cancelled')
        self.used(deadline)
