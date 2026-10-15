"""Linux ownership capsule. Importing this module changes no process state.

Only activate from a dedicated, single-threaded CLI which spawns all of its own
children through this object and has no other waiters, child watchers or ptrace.
Owner-specific stop is deliberately only group/leader stop, never tree closure.
"""
from __future__ import annotations

import ctypes
from dataclasses import dataclass, field
import math
import os
from pathlib import Path
import signal
import subprocess
import sys
import time


class Unsupported(RuntimeError):
    pass


class OwnershipError(RuntimeError):
    pass


def _proc_directory(pid: int) -> int:
    return os.open(f"/proc/{pid}", os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW)


def _ppid_at(directory: int) -> int:
    fd = os.open("stat", os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW, dir_fd=directory)
    try:
        # Only the bounded stat prefix needed for pid/(comm)/state/ppid.
        # No cmdline, environ, credentials, or metadata are retained or logged.
        prefix = os.read(fd, 256)
    finally:
        os.close(fd)
    fields = prefix[prefix.rindex(b")") + 1:].split()
    return int(fields[1])


def _children(deadline: float | None = None) -> list[int]:
    deadline = time.monotonic() + 1 if deadline is None else deadline
    try:
        with open(f"/proc/self/task/{os.getpid()}/children", "rb") as stream:
            raw = stream.read(65537)
        if len(raw) > 65536:
            raise OwnershipError("child_list_limit")
        return [int(item) for item in raw.split()]
    except FileNotFoundError:
        # CONFIG_PROC_CHILDREN is optional. Root approved this read-only fallback:
        # PPID is only a candidate filter, NEVER signal/wait authority.
        found = []
        with os.scandir("/proc") as entries:
            for entry in entries:
                if time.monotonic() >= deadline:
                    raise OwnershipError("child_scan_deadline")
                if not entry.name.isdecimal():
                    continue
                directory = None
                try:
                    directory = _proc_directory(int(entry.name))
                    if _ppid_at(directory) == os.getpid():
                        found.append(int(entry.name))
                except (FileNotFoundError, ProcessLookupError, PermissionError, ValueError):
                    # An unrelated process may exit or be unreadable. ECHILD,
                    # never this scan's completeness, is final closure proof.
                    continue
                finally:
                    if directory is not None:
                        os.close(directory)
        return found


def _single_thread() -> bool:
    return len(list(Path("/proc/self/task").iterdir())) == 1


def subreaper_state() -> int:
    libc = ctypes.CDLL(None, use_errno=True)
    prctl = libc.prctl
    prctl.argtypes = [ctypes.c_int] + [ctypes.c_ulong] * 4
    prctl.restype = ctypes.c_int
    value = ctypes.c_int()
    if prctl(37, ctypes.cast(ctypes.byref(value), ctypes.c_void_p).value, 0, 0, 0) != 0:
        raise Unsupported("get_subreaper")
    return value.value


@dataclass
class Leader:
    name: str
    _process: subprocess.Popen = field(repr=False)
    term_due: float | None = None
    kill_sent: bool = False
    reaped: bool = False
    reservation_lost: bool = False

    @property
    def pid(self) -> int:
        return self._process.pid

    @property
    def returncode(self) -> int | None:
        return self._process.returncode


@dataclass
class Adopted:
    serial: int
    pid: int
    pidfd: int
    term_due: float | None = None
    kill_sent: bool = False


@dataclass(frozen=True)
class Closure:
    closed: bool
    retain_data: bool
    reason: str
    leaders_reaped: int
    adopted_reaped: int
    echild_observed: bool


class LinuxProcessOwner:
    def __init__(self):
        raise TypeError("use activate_in_dedicated_cli")

    @classmethod
    def activate_in_dedicated_cli(cls) -> LinuxProcessOwner:
        required = ("pidfd_open", "P_PIDFD", "waitid", "WNOWAIT", "WEXITED", "WNOHANG", "killpg")
        if sys.platform != "linux" or os.getpid() <= 1 or any(not hasattr(os, name) for name in required):
            raise Unsupported("linux_process_api")
        try:
            exclusive = _single_thread() and not _children()
        except (OSError, OwnershipError) as error:
            raise Unsupported("direct_child_discovery") from error
        if not hasattr(signal, "pidfd_send_signal") or not exclusive:
            raise Unsupported("exclusive_cli_required")
        if subreaper_state() != 0:
            raise Unsupported("already_subreaper")
        # Probe actual kernel support before enabling subreaping or spawning.
        try:
            fd = os.pidfd_open(os.getpid(), 0)
        except OSError as error:
            raise Unsupported("pidfd_kernel_api") from error
        try:
            signal.pidfd_send_signal(fd, 0, None, 0)
            try:
                os.waitid(os.P_PIDFD, fd, os.WEXITED | os.WNOHANG | os.WNOWAIT)
            except ChildProcessError:
                pass  # A supported waitid correctly says that self is not a child.
            else:
                raise Unsupported("self_became_waitable")
        except OSError as error:
            raise Unsupported("pidfd_kernel_api") from error
        finally:
            os.close(fd)
        # Dedicated CLI only. No SIG_IGN/SA_NOCLDWAIT or asynchronous reapers.
        signal.signal(signal.SIGCHLD, signal.SIG_DFL)
        libc = ctypes.CDLL(None, use_errno=True)
        prctl = libc.prctl
        prctl.argtypes = [ctypes.c_int] + [ctypes.c_ulong] * 4
        prctl.restype = ctypes.c_int
        if prctl(36, 1, 0, 0, 0) != 0 or subreaper_state() != 1:
            raise Unsupported("set_subreaper")
        result = object.__new__(cls)
        result.pid = os.getpid()
        result.leaders: dict[str, Leader] = {}
        result.adopted: dict[int, Adopted] = {}
        result.adopted_reaped = 0
        result.serial = 0
        result.closing = False
        result.closed = False
        result.in_cleanup = False
        result.cancelled = False
        result.fault = ""
        result.group_signals: list[tuple[str, int]] = []
        result.adopted_signals: list[tuple[int, int]] = []
        for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(number, result._cancel)
        return result

    def _cancel(self, _number, _frame):
        # Cancellation requests admission shutdown; never interrupts cleanup.
        self.cancelled = True

    def _context(self):
        if os.getpid() != self.pid or not _single_thread():
            raise OwnershipError("exclusive_cli_lost")

    @staticmethod
    def _budget(timeout: float, grace: float):
        if not (math.isfinite(timeout) and 0 <= timeout <= 20 and math.isfinite(grace) and 0 <= grace <= 2):
            raise ValueError("cleanup_budget")

    def spawn(self, name: str, argv: list[str], *, pass_fds: tuple[int, ...] = ()) -> Leader:
        self._context()
        if self.closing or self.cancelled or self.fault or name in self.leaders or len(self.leaders) >= 16:
            raise OwnershipError("admission_closed")
        if not argv or not all(isinstance(arg, str) and arg for arg in argv):
            raise ValueError("argv")
        # Keep every Popen strongly referenced. Never context-manage, poll, or
        # communicate with these objects: those operations may reap the PID.
        process = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                   stderr=subprocess.DEVNULL, start_new_session=True, pass_fds=pass_fds)
        leader = Leader(name, process)
        self.leaders[name] = leader
        return leader

    def peek(self, leader: Leader):
        self._context()
        if self.leaders.get(leader.name) is not leader or leader.reaped or leader.reservation_lost:
            raise OwnershipError("leader_not_reserved")
        try:
            return os.waitid(os.P_PID, leader.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        except ChildProcessError as error:
            leader.reservation_lost = True
            self.fault = "leader_reservation_lost"
            raise OwnershipError(self.fault) from error

    def _group_signal(self, leader: Leader, number: int):
        self.peek(leader)  # Alive child or WNOWAIT zombie still reserves PID/PGID.
        try:
            os.killpg(leader.pid, number)
        except ProcessLookupError:
            pass  # No remaining group, but the original leader is still reserved.
        self.group_signals.append((leader.name, int(number)))

    def _finish_groups(self, leaders: list[Leader], deadline: float, grace: float) -> bool:
        for leader in leaders:
            if not leader.reaped and leader.term_due is None:
                if time.monotonic() >= deadline:
                    return False
                self._group_signal(leader, signal.SIGTERM)
                leader.term_due = time.monotonic() + grace
        while time.monotonic() < deadline:
            pending = False
            for leader in leaders:
                if leader.reaped:
                    continue
                pending = True
                now = time.monotonic()
                if not leader.kill_sent and now >= leader.term_due:
                    self._group_signal(leader, signal.SIGKILL)
                    leader.kill_sent = True
                if leader.kill_sent and self.peek(leader) is not None:
                    # The only consuming wait for a Popen leader, after the LAST
                    # numeric group signal. WNOWAIT already established exit.
                    leader._process.wait(timeout=max(0, min(0.05, deadline - time.monotonic())))
                    leader.reaped = True
            if not pending or all(leader.reaped for leader in leaders):
                return True
            time.sleep(min(0.005, max(0, deadline - time.monotonic())))
        return all(leader.reaped for leader in leaders)

    def stop_group(self, leader: Leader, *, timeout: float = 2, grace: float = 0.05) -> bool:
        """Signal protocol + leader wait only; group/tree emptiness NOT certified."""
        self._budget(timeout, grace)
        self._context()
        if self.in_cleanup or self.leaders.get(leader.name) is not leader or self.fault:
            return False
        self.in_cleanup = True
        try:
            return self._finish_groups([leader], time.monotonic() + timeout, grace)
        except (OSError, OwnershipError, subprocess.TimeoutExpired):
            return False
        finally:
            self.in_cleanup = False

    @staticmethod
    def _peek_adopted(child: Adopted):
        return os.waitid(os.P_PIDFD, child.pidfd, os.WEXITED | os.WNOHANG | os.WNOWAIT)

    def _capture_direct_children(self, deadline: float):
        # Called ONLY after ALL leaders were actually waited and global teardown
        # began. Under the exclusive-CLI premise every direct child is ours,
        # including descendants which escaped all original sessions.
        for pid in _children(deadline):
            if time.monotonic() >= deadline:
                raise OwnershipError("child_capture_deadline")
            if pid in self.adopted:
                continue
            directory = _proc_directory(pid)
            fd = None
            try:
                if _ppid_at(directory) != self.pid:
                    raise OwnershipError("candidate_reparented_before_pin")
                fd = os.pidfd_open(pid, 0)
                # Read through the SAME proc directory opened before pidfd_open.
                # If the old PID died/recycled, that old proc inode cannot retarget
                # the replacement. Reject failure or changed parent before acting.
                if _ppid_at(directory) != self.pid:
                    raise OwnershipError("candidate_reparented_after_pin")
                child = Adopted(self.serial, pid, fd)
                self._peek_adopted(child)  # Must really be this CLI's waitable child.
            except BaseException:
                if fd is not None:
                    os.close(fd)
                raise
            finally:
                os.close(directory)
            self.serial += 1
            self.adopted[pid] = child

    def _drain(self, deadline: float, grace: float) -> bool:
        while time.monotonic() < deadline:
            self._capture_direct_children(deadline)
            for pid, child in list(self.adopted.items()):
                if time.monotonic() >= deadline:
                    return False
                status = self._peek_adopted(child)
                if status is not None:
                    waited = os.waitid(os.P_PIDFD, child.pidfd, os.WEXITED | os.WNOHANG)
                    if waited is None or waited.si_pid != pid:
                        raise OwnershipError("adopted_wait_not_confirmed")
                    os.close(child.pidfd)
                    del self.adopted[pid]
                    self.adopted_reaped += 1
                    continue
                now = time.monotonic()
                number = (signal.SIGTERM if child.term_due is None else
                          signal.SIGKILL if not child.kill_sent and now >= child.term_due else None)
                if number is not None:
                    # Stable process identity, never os.kill(pid) or killpg for adoptees.
                    try:
                        signal.pidfd_send_signal(child.pidfd, number, None, 0)
                    except ProcessLookupError:
                        # Exit may race the signal. The next exact-fd wait must
                        # still reap it; ESRCH is never taken as closure proof.
                        pass
                    self.adopted_signals.append((child.serial, int(number)))
                    if number == signal.SIGTERM:
                        child.term_due = now + grace
                    else:
                        child.kill_sent = True
            try:
                os.waitid(os.P_ALL, 0, os.WEXITED | os.WNOHANG | os.WNOWAIT)
            except ChildProcessError:
                if self.adopted:
                    raise OwnershipError("adopted_reservation_lost")
                return True  # ECHILD: no live OR zombie direct children remain.
            time.sleep(min(0.005, max(0, deadline - time.monotonic())))
        return False

    def _result(self, reason: str) -> Closure:
        # This capsule never grants data-deletion authority, including on closure.
        # The current SF08 HOLD policy retains owned work on success as well.
        return Closure(self.closed, True, reason, sum(x.reaped for x in self.leaders.values()),
                       self.adopted_reaped, self.closed)

    def close_all(self, *, timeout: float = 3, grace: float = 0.05) -> Closure:
        self._budget(timeout, grace)
        if os.getpid() != self.pid or not _single_thread():
            return Closure(False, True, "exclusive_cli_lost", 0, 0, False)
        if self.closed:
            return self._result("closed")
        if self.in_cleanup:
            return self._result("cleanup_in_progress")
        self.closing = True  # No spawn once global teardown starts, even on timeout.
        self.in_cleanup = True
        try:
            self._context()
            if self.fault:
                return self._result(self.fault)
            deadline = time.monotonic() + timeout
            if not self._finish_groups(list(self.leaders.values()), deadline, grace):
                return self._result("group_deadline")
            if not self._drain(deadline, grace):
                return self._result("descendant_deadline")
            self.closed = True
            return self._result("closed")
        except (OSError, OwnershipError, subprocess.TimeoutExpired) as error:
            # Keep all ownership records and live pidfds for a later bounded attempt.
            return self._result(type(error).__name__)
        finally:
            self.in_cleanup = False
