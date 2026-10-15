"""Compile/run CSRF deadline mutants in an owned checkout; emit closed evidence.

Only a successful Rust build plus an exact, non-ignored native test failure at
the designated assertion is semantic RED. Watchdog/process timeout is invalid.
"""
from __future__ import annotations

import json
import ctypes
import os
import pathlib
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time

BASE = "http_server::csrf_http::deadline_tests::"
CASES = (
    "guarded_preparation_pending_uses_actual_session_deadline",
    "guarded_preparation_expiry_precedes_late_grant_denial_and_panic",
    "guarded_preparation_same_poll_late_grant_is_504",
    "guarded_preparation_same_poll_late_denied_is_504",
    "guarded_preparation_same_poll_late_panic_is_504",
    "guarded_preparation_initially_expired_session_still_challenges_401",
    "guarded_body_wire_preserves_absolute_deadline_and_strict_precedence",
) + tuple("body_selection_tests::" + name for name in (
    "existing_budget_tuples_select_absolute_deadline_and_status",
    "preparation_spends_original_budget_and_never_restarts_common",
    "original_credential_common_can_only_shorten_body_window",
    "exact_tie_and_one_nanosecond_neighbors_are_strict",
    "unrepresentable_body_deadline_remains_rejected",
))
REQUIRED = frozenset(BASE + name for name in CASES)
ROW = re.compile(r"^test ([A-Za-z_][A-Za-z0-9_:]*) \.\.\. (ok|FAILED|ignored)$", re.M)
RUNNING = re.compile(r"^running ([0-9]+) tests?$", re.M)
SUMMARY = re.compile(
    r"^test result: (ok|FAILED)\. ([0-9]+) passed; ([0-9]+) failed; "
    r"([0-9]+) ignored; 0 measured; [0-9]+ filtered out; finished in [0-9.]+s$", re.M
)
SESSION = "runtime/src/auth/session.rs"
CORE = "runtime/src/http_server/csrf_http.rs"
PROJECTION_OLD = """let remaining = row
            .idle_ms
            .min(row.absolute_ms)
            .checked_sub(c.ceil_ms)"""
PROJECTION_NEW = """let remaining = row
            .absolute_ms
            .checked_sub(c.ceil_ms)"""
POST_OLD = """        let completion = future.as_mut().poll(cx);
        let deadline = match live_until(owner, until) {
            Ok(deadline) => deadline,
            Err(status) => return Poll::Ready(Err(status)),
        };
        if let Poll::Ready(value) = completion {
            return Poll::Ready(Ok(value));
        }"""
POST_NEW = """        let completion = future.as_mut().poll(cx);
        if let Poll::Ready(value) = completion {
            return Poll::Ready(Ok(value));
        }
        let deadline = match live_until(owner, until) {
            Ok(deadline) => deadline,
            Err(status) => return Poll::Ready(Err(status)),
        };"""
MUTANTS = (
    ("session_projection_omitted", SESSION, PROJECTION_OLD, PROJECTION_NEW,
     CASES[2], "projection_deadline_equality"),
    ("late_grant_priority_omitted", CORE, POST_OLD, POST_NEW,
     CASES[2], "late_grant_outer_504"),
    ("late_denied_priority_omitted", CORE, POST_OLD, POST_NEW,
     CASES[3], "late_denied_outer_504"),
    ("late_panic_priority_omitted", CORE, POST_OLD, POST_NEW,
     CASES[4], "late_panic_outer_504"),
    ("timer_fixed_to_original_until", CORE,
     ".reset(tokio::time::Instant::from_std(deadline));",
     ".reset(tokio::time::Instant::from_std(deadline.max(until)));",
     CASES[0], "pending_actual_timer_wake_watchdog"),
)


def inventory(raw: str, exit_code: int, required: frozenset[str]) -> bool:
    text = raw.replace("\r\n", "\n")
    rows, running, summaries = ROW.findall(text), RUNNING.findall(text), SUMMARY.findall(text)
    return (exit_code == 0 and running == [str(len(required))]
            and summaries == [("ok", str(len(required)), "0", "0")]
            and len(rows) == len(required) and set(rows) == {(name, "ok") for name in required})


def semantic_red(raw: str, exit_code: int, case: str, marker: str) -> bool:
    text = raw.replace("\r\n", "\n")
    name = BASE + case
    headers = re.findall(r"^---- ([A-Za-z_][A-Za-z0-9_:]*) stdout ----$", text, re.M)
    section = text.split("---- " + name + " stdout ----\n", 1)
    stdout = re.split(r"(?m)^failures:\n|^test result:", section[1], maxsplit=1)[0] if len(section) == 2 else ""
    marker_line = r"^" + re.escape(marker) + r"$"
    return (exit_code == 101 and RUNNING.findall(text) == ["1"]
            and SUMMARY.findall(text) == [("FAILED", "0", "1", "0")]
            and ROW.findall(text) == [(name, "FAILED")] and headers == [name]
            and len(re.findall(marker_line, text, re.M)) == 1
            and len(re.findall(marker_line, stdout, re.M)) == 1)


def replacement(source: str, old: str, new: str) -> str:
    if source.count(old) != 1:
        raise ValueError("mutation_anchor_not_unique")
    return source.replace(old, new, 1)


class ProcessRetentionRequired(RuntimeError):
    """An owned group could not be proved quiescent; retain all its source."""
    def __init__(self, owner):
        super().__init__("owned_process_group_unconfirmed")
        self.owner = owner


class RunCancelled(RuntimeError):
    pass


class Cancellation:
    # Dedicated single-thread Linux CLI only. Handlers record; they never raise
    # between fork/exec and owner publication, and no signal mask is inherited.
    def __init__(self):
        self.requested = False
        self.active = None
        self.handlers = {}

    def record(self, _signum, _frame):
        self.requested = True

    def check(self):
        if self.requested:
            raise RunCancelled("runner_cancelled")

    def __enter__(self):
        for sig in (signal.SIGINT, signal.SIGTERM):
            self.handlers[sig] = signal.signal(sig, self.record)
        return self

    def __exit__(self, *_):
        for sig, handler in self.handlers.items():
            signal.signal(sig, handler)


def enable_subreaper() -> bool:
    if sys.platform != "linux":
        return False
    try:
        children = pathlib.Path("/proc/self/task") / str(os.getpid()) / "children"
        if (not children.is_file() or
                int(pathlib.Path("/proc/self/stat").read_text().split()[0]) != os.getpid()):
            return False
        libc = ctypes.CDLL(None, use_errno=True)
        enabled = ctypes.c_int(0)
        # PR_SET/GET_CHILD_SUBREAPER: adopted Cargo/rustc descendants can be
        # actually waited, rather than calling a killed parent a joined tree.
        return (libc.prctl(36, 1, 0, 0, 0) == 0 and
                libc.prctl(37, ctypes.byref(enabled), 0, 0, 0) == 0 and enabled.value == 1)
    except (OSError, AttributeError, ValueError, IndexError):
        return False


class ProcessOwner:
    # Dedicated Linux git/Cargo/native group only. Keep the leader unreaped while
    # inspecting/reaping its descendants: its PID/PGID cannot be reused then.
    def __init__(self, control=None):
        self.control = control if control is not None else Cancellation()
        self.process = None
        self.pgid = None
        self.output = None
        self.launch_attempted = False
        self.reap_started = False
        self.confirmed = False

    def leader_exited(self):
        return os.waitid(os.P_PID, self.pgid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is not None

    def descendants(self):
        # This single-threaded CLI has exactly one spawned group at a time.
        # Once its leader exits, Linux reparents descendant roots to this
        # subreaper. Inspect only our own children, never unrelated processes.
        children = (pathlib.Path("/proc/self/task") / str(os.getpid()) / "children").read_text().split()
        members = []
        for value in children:
            pid = int(value)
            if pid == self.pgid:
                continue
            try:
                stat = (pathlib.Path("/proc") / value / "stat").read_text()
            except FileNotFoundError:
                continue
            # comm may contain spaces/parentheses. The following fields are
            # state, ppid, pgrp, session. An escaped/unknown child forces retain.
            fields = stat[stat.rfind(")") + 2:].split()
            if int(fields[2]) != self.pgid or int(fields[3]) != self.pgid:
                raise ProcessRetentionRequired(self)
            members.append(pid)
        return members

    def joined(self):
        if not self.leader_exited():
            return False
        for pid in self.descendants():
            try:
                os.waitpid(pid, os.WNOHANG)
            except ChildProcessError:
                pass  # Not yet adopted, or its own parent has just reaped it.
        if self.descendants():
            return False
        # Only now reap the reserved leader. No all-process wait is used.
        self.reap_started = True
        self.process.wait(timeout=1)
        self.confirmed = True
        return True

    def wait_joined(self, seconds, cancellable=False):
        deadline = time.monotonic() + seconds
        while True:
            if cancellable:
                self.control.check()
            if self.joined():
                return True
            if time.monotonic() >= deadline:
                return False
            time.sleep(0.02)

    def stop_and_wait(self):
        if self.confirmed:
            return
        if (self.process is None or type(self.pgid) is not int or self.pgid <= 0 or
                self.reap_started):
            raise ProcessRetentionRequired(self)
        for sig in (signal.SIGTERM, signal.SIGKILL):
            try:
                os.killpg(self.pgid, sig)
                if self.wait_joined(3):
                    return
            except BaseException:
                raise ProcessRetentionRequired(self)
        raise ProcessRetentionRequired(self)

    def run(self, args: list[str], root: pathlib.Path, env: dict[str, str], seconds: int):
        # Reserve this owner and anonymous output file before creating the child.
        # The handle stays owned through timeout and all finite termination waits.
        if self.control.active is not None:
            raise ProcessRetentionRequired(self.control.active)
        self.control.active = self
        try:
            self.control.check()
            self.output = tempfile.TemporaryFile()
            self.launch_attempted = True
            self.process = subprocess.Popen(args, cwd=root, env=env, stdout=self.output,
                                            stderr=subprocess.STDOUT, start_new_session=True)
            self.pgid = self.process.pid
            self.control.check()
            if not self.wait_joined(seconds, cancellable=True):
                self.stop_and_wait()
                self.output.close()
                return -1, "", True
            self.control.check()
            self.output.seek(0)
            raw = self.output.read()
            self.output.close()
            return self.process.returncode, raw.decode("utf-8", errors="replace"), False
        except ProcessRetentionRequired:
            raise
        except BaseException:
            if self.launch_attempted and not self.confirmed:
                self.stop_and_wait()
            if self.output is not None:
                self.output.close()
            raise
        finally:
            if self.confirmed or not self.launch_attempted:
                self.control.active = None


def command(args: list[str], root: pathlib.Path, env: dict[str, str], seconds: int, control=None):
    return ProcessOwner(control).run(args, root, env, seconds)


def build(root: pathlib.Path, env: dict[str, str], control=None):
    code, raw, timed_out = command(
        ["cargo", "test", "--locked", "-p", "nagi-runtime", "--lib", "--no-run",
         "--message-format=json"], root, env, 600, control,
    )
    binaries = set()
    for line in raw.splitlines():
        try:
            record = json.loads(line)
        except ValueError:
            continue
        if not isinstance(record, dict):
            continue
        if (record.get("reason") == "compiler-artifact" and
                record.get("target", {}).get("name") == "nagi_runtime" and
                record.get("profile", {}).get("test") is True and record.get("executable")):
            binaries.add(record["executable"])
    if timed_out or code != 0 or len(binaries) != 1:
        return None, "build_timeout" if timed_out else "build_failed"
    binary = pathlib.Path(binaries.pop())
    return (binary, "compiled") if binary.is_file() else (None, "build_artifact_missing")


def run_checkout(root: pathlib.Path, env: dict[str, str], control=None) -> dict:
    evidence = {"schema": "nagi-csrf-deadline-mutations-v1", "status": "failed",
                "stock": "unavailable", "mutants": {}}
    binary, stage = build(root, env, control)
    if binary is None:
        evidence["stock"] = stage
        return evidence
    code, raw, timed_out = command([str(binary), BASE], root, env, 90, control)
    if timed_out or not inventory(raw, code, REQUIRED):
        evidence["stock"] = "native_timeout" if timed_out else "native_inventory_failed"
        return evidence
    evidence["stock"] = "compiled_native_green"
    for label, path, old, new, case, marker in MUTANTS:
        source = root / path
        original = source.read_bytes()
        result = {"build": "unavailable", "native": "unavailable", "semantic_red": False}
        evidence["mutants"][label] = result
        retain = False
        try:
            if control is not None:
                control.check()
            source.write_text(replacement(original.decode("utf-8"), old, new), encoding="utf-8")
            binary, result["build"] = build(root, env, control)
            if binary is None:
                continue
            code, raw, timed_out = command([str(binary), "--exact", BASE + case], root, env, 30, control)
            result["native_exit"] = code
            result["native"] = "process_timeout" if timed_out else "completed"
            result["semantic_red"] = not timed_out and semantic_red(raw, code, case, marker)
        except ProcessRetentionRequired:
            retain = True
            raise
        except (OSError, ValueError):
            result["native"] = "invalid_source_or_execution"
        except BaseException:
            retain = True
            raise
        finally:
            if not retain:
                if control is not None:
                    control.check()
                source.write_bytes(original)
    if (len(evidence["mutants"]) == len(MUTANTS) and
            all(item["semantic_red"] for item in evidence["mutants"].values())):
        evidence["status"] = "passed"
    return evidence


def main(argv: list[str]) -> int:
    if len(argv) == 5 and argv[1] == "--coverage":
        try:
            with pathlib.Path(argv[2]).open("rb") as stream:
                raw = stream.read(256 * 1024 + 1)
            valid = (len(raw) <= 256 * 1024 and
                     inventory(raw.decode("utf-8"), int(argv[3]), REQUIRED))
        except (OSError, ValueError, UnicodeDecodeError):
            valid = False
        record = {"schema": "nagi-csrf-deadline-coverage-v1", "required_count": len(REQUIRED),
                  "status": "passed" if valid else "failed"}
        pathlib.Path(argv[4]).write_text(json.dumps(record, sort_keys=True) + "\n", encoding="ascii")
        print("csrf_deadline_native_coverage=" + record["status"])
        return 0 if valid else 1
    if len(argv) != 3:
        return 2
    repo, output = pathlib.Path(argv[1]).resolve(), pathlib.Path(argv[2]).resolve()
    with Cancellation() as control:
        return execute(repo, output, control)


def execute(repo, output, control):
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(repo / "target")
    evidence = {"schema": "nagi-csrf-deadline-mutations-v1", "status": "failed",
                "stock": "checkout_failed", "mutants": {}}
    # Unknown is the default before launch, throughout publication and cleanup.
    # Only a completed command and a clear cancellation record authorize cleanup.
    if not enable_subreaper():
        evidence["stock"] = "subreaper_unavailable"
    else:
        temp = pathlib.Path(tempfile.mkdtemp(prefix="nagi-csrf-mutants-"))
        checkout = temp / "checkout"
        added, retain = False, True
        retained_owner = None
        try:
            control.check()
            code, _, _ = command(["git", "worktree", "add", "--detach", str(checkout), "HEAD"], repo, env, 60, control)
            added = code == 0
            if added:
                evidence = run_checkout(checkout, env, control)
            control.check()
            retain = False
        except ProcessRetentionRequired as error:
            retain = True
            retained_owner = error.owner
            evidence["stock"] = "process_group_unconfirmed"
            evidence["status"] = "failed"
        except BaseException:
            retain = True
            retained_owner = control.active
            evidence["stock"] = "cancelled_or_execution_unavailable"
            evidence["status"] = "failed"
        finally:
            if not retain and (added or checkout.exists()):
                # Reserve uncertainty before the cleanup child is launched too.
                retain = True
                try:
                    control.check()
                    code, _, _ = command(["git", "worktree", "remove", "--force", str(checkout)], repo, env, 60, control)
                    control.check()
                    retain = code != 0
                except ProcessRetentionRequired as error:
                    retained_owner = error.owner
                except BaseException:
                    retained_owner = control.active
            if not retain:
                try:
                    control.check()
                    shutil.rmtree(temp)
                except BaseException:
                    retain = True
            if retain:
                evidence["status"] = "failed"
                evidence["workspace_retained"] = True
    if control.requested:
        evidence["status"] = "failed"
        evidence["cancelled"] = True
    output.write_text(json.dumps(evidence, sort_keys=True) + "\n", encoding="ascii")
    print("csrf_deadline_compiled_mutations=" + evidence["status"])
    return 0 if evidence["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
