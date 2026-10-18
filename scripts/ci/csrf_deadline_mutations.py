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
AUTH_BASE = "auth::csrf::tests::"
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
AUTH_CASES = (
    "same_queue_reopen_reuses_data_context_without_refreshing_token_expiry",
    "reopened_context_preserves_original_token_expiry_and_rejects_after_deadline",
    "bytes_limit_is_independent_and_expired_cleanup_spends_one_batch",
    "expired_token_reclaims_byte_capacity_without_relaxing_live_token_cap",
    "checkpoint_accepts_before_and_refuses_at_or_after_original_token_expiry",
    "owner_only_seal_is_destructive_foreign_and_replay_are_refused",
    "unpolled_and_dropped_pending_issue_do_not_deliver_and_native_is_joined",
    "actual_unknown_and_committed_cleanup_never_make_ready",
    "actual_reserve_wait_rechecks_live_gate_before_any_business_enqueue",
    "partial_schema_and_wrong_actual_source_are_refused_without_adoption",
    "aggregate_declared_budgets_are_checked_in_both_open_orders",
    "cancelled_after_actual_business_commit_retires_material_and_preserves_row",
    "issuance_caps_original_absolute_and_checkpoint_keeps_original_idle_boundary",
    "nondecreasing_clock_refusal_preserves_live_row_and_never_delivers",
)
# The U2 actual issuer fixture is a nested Rust test module, kept separate from the frozen outer tuple.
AUTH_ADDITIONAL = (
    "fixed_wire_owner_tests::actual_csrf_wire_fixed_owner_outlives_issuer_until_last_bytes_clone",
)
AUTH_REQUIRED = (frozenset(AUTH_BASE + name for name in AUTH_CASES)
                 | frozenset(AUTH_BASE + name for name in AUTH_ADDITIONAL))
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
    ("csrf_compatible_checkpoint_always_denied", "runtime/src/auth/csrf.rs",
     """            if !bool::from(matched) || fresh.ceil_ms>=expiry {
                #[cfg(all(test, nagi_auth_stage_diag))]""",
     """            if true {
                #[cfg(all(test, nagi_auth_stage_diag))]""",
     AUTH_BASE + AUTH_CASES[0],
     "same_queue_fixture phase=PositiveCheckpoint kind=Some(Denied) outcome=Some(RolledBack)"),
    ("csrf_reopen_refreshes_stored_expiry", "runtime/src/auth/csrf.rs",
     """            session::PreloginContext::track_sample(&mut wall,c,m.last_wall).map_err(Failure::session)?;
            if m.options!=options || m.origin!=origin { return Err(Failure::unavailable()); }
            m.incarnation.try_into().map_err(|_|Failure::unavailable())""",
     """            let c=session::PreloginContext::track_sample(&mut wall,c,m.last_wall).map_err(Failure::session)?;
            if m.options!=options || m.origin!=origin { return Err(Failure::unavailable()); }
            let refreshed_expiry = c.floor_ms.checked_add(options.ttl_ms)
                .and_then(|expiry| expiry.checked_add(1))
                .ok_or_else(Failure::unavailable)?;
            let p = sqlite::bind_i64(sqlite::parameters(), refreshed_expiry);
            sqlite::exec(
                &tx,
                sqlite::literal("UPDATE __nagi_csrf_tokens SET expiry_ms=max(expiry_ms+1,?)"),
                p,
            )
                .await
                .map_err(Failure::database)?;
            m.incarnation.try_into().map_err(|_|Failure::unavailable())""",
     AUTH_BASE + AUTH_CASES[1], "same_queue_fixture phase=OriginalRowUnchanged"),
    ("csrf_checkpoint_renews_expired_row", "runtime/src/auth/csrf.rs",
     """            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::Cleanup);
            self.maintenance(&tx,c.ceil_ms,true).await?;
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::ReadTokens);""",
     """            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::Cleanup);
            let refreshed_expiry = c.ceil_ms.checked_add(self.options.ttl_ms)
                .ok_or_else(Failure::unavailable)?;
            let p = sqlite::bind_i64(sqlite::parameters(), refreshed_expiry);
            let p = sqlite::bind_bytes(p, digest.to_vec());
            let p = sqlite::bind_i64(p, c.ceil_ms);
            sqlite::exec(
                &tx,
                sqlite::literal("UPDATE __nagi_csrf_tokens SET expiry_ms=? WHERE digest=? AND expiry_ms<=?"),
                p,
            ).await.map_err(Failure::database)?;
            self.maintenance(&tx,c.ceil_ms,true).await?;
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::ReadTokens);""",
     AUTH_BASE + AUTH_CASES[1], "same_queue_fixture phase=ExpiredCheckpointDeniedRolledBack"),
    ("csrf_delivery_refreshes_original_deadline", "runtime/src/auth/csrf.rs",
     "            let deadline = c.deadline.ok_or_else(Failure::internal)?;",
     """            let deadline = Instant::now()
                .checked_add(Duration::from_millis(self.options.ttl_ms as u64))
                .ok_or_else(Failure::unavailable)?;""",
     AUTH_BASE + AUTH_CASES[0], "same_queue_fixture phase=OriginalDeliveryDeadlineDenied"),
)
SUMMARY_MUTANT_LABELS = tuple(item[0] for item in MUTANTS)


def closed_summary(evidence: dict) -> tuple[str, ...]:
    # Project only fixed labels and finite values. Never render source paths,
    # assertion text, process output, unknown keys, or arbitrary exit integers.
    def stage(value, allowed):
        return value if type(value) is str and value in allowed else "unavailable"

    stock = stage(evidence.get("stock"), (
        "unavailable", "checkout_failed", "subreaper_unavailable",
        "process_group_unconfirmed", "cancelled_or_execution_unavailable",
        "compiled_native_green", "build_timeout", "build_failed", "build_artifact_missing",
        "native_timeout", "native_inventory_failed",
        "auth_fixture_native_timeout", "auth_fixture_native_inventory_failed",
    ))
    auth = stage(evidence.get("auth_fixture"), (
        "unavailable", "compiled_native_green", "native_timeout", "native_inventory_failed",
    ))
    status = "passed" if evidence.get("status") == "passed" else "failed"
    lines = ["csrf_deadline_compiled_mutations=" + status,
             "csrf_deadline_mutation_stock=" + stock,
             "csrf_deadline_mutation_auth_fixture=" + auth]
    mutants = evidence.get("mutants")
    mutants = mutants if type(mutants) is dict else {}
    for label in SUMMARY_MUTANT_LABELS:
        result = mutants.get(label)
        result = result if type(result) is dict else {}
        build_stage = stage(result.get("build"), (
            "unavailable", "compiled", "build_timeout", "build_failed", "build_artifact_missing",
        ))
        native = stage(result.get("native"), (
            "unavailable", "completed", "process_timeout", "invalid_source_or_execution",
        ))
        code = result.get("native_exit")
        native_exit = (str(code) if code in (-1, 0, 101) else "other") if type(code) is int else "unavailable"
        red = result.get("semantic_red")
        semantic = ("true" if red else "false") if type(red) is bool else "unavailable"
        lines.append("csrf_deadline_mutant=" + label + " build=" + build_stage
                     + " native=" + native + " native_exit=" + native_exit
                     + " semantic_red=" + semantic)
    return tuple(lines)


def test_name(case: str) -> str:
    return case if case.startswith(AUTH_BASE) else BASE + case


def inventory(raw: str, exit_code: int, required: frozenset[str]) -> bool:
    text = raw.replace("\r\n", "\n")
    rows, running, summaries = ROW.findall(text), RUNNING.findall(text), SUMMARY.findall(text)
    return (exit_code == 0 and running == [str(len(required))]
            and summaries == [("ok", str(len(required)), "0", "0")]
            and len(rows) == len(required) and set(rows) == {(name, "ok") for name in required})


def semantic_red(raw: str, exit_code: int, case: str, marker: str) -> bool:
    text = raw.replace("\r\n", "\n")
    name = test_name(case)
    headers = re.findall(r"^---- ([A-Za-z_][A-Za-z0-9_:]*) stdout ----$", text, re.M)
    section = text.split("---- " + name + " stdout ----\n", 1)
    stdout = re.split(r"(?m)^failures:\n|^test result:", section[1], maxsplit=1)[0] if len(section) == 2 else ""
    marker_line = r"^" + re.escape(marker) + r"$"
    return (exit_code == 101 and RUNNING.findall(text) == ["1"]
            and SUMMARY.findall(text) == [("FAILED", "0", "1", "0")]
            and ROW.findall(text) == [(name, "FAILED")] and headers == [name]
            and len(re.findall(marker_line, text, re.M)) == 1
            and len(re.findall(marker_line, stdout, re.M)) == 1)


def _newline_style(text: str) -> str:
    crlf = text.count("\r\n")
    bare_lf = text.count("\n") - crlf
    bare_cr = text.count("\r") - crlf
    if bare_cr or (crlf and bare_lf):
        raise ValueError("mixed_source_newlines")
    return "\r\n" if crlf else "\n"


def replacement(source: str, old: str, new: str) -> str:
    newline = _newline_style(source)
    _newline_style(old)
    _newline_style(new)
    source = source.replace("\r\n", "\n")
    old = old.replace("\r\n", "\n")
    new = new.replace("\r\n", "\n")
    if source.count(old) != 1:
        raise ValueError("mutation_anchor_not_unique")
    changed = source.replace(old, new, 1)
    return changed.replace("\n", newline) if newline == "\r\n" else changed


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
                "stock": "unavailable", "auth_fixture": "unavailable", "mutants": {}}
    binary, stage = build(root, env, control)
    if binary is None:
        evidence["stock"] = stage
        return evidence
    code, raw, timed_out = command([str(binary), BASE], root, env, 90, control)
    if timed_out or not inventory(raw, code, REQUIRED):
        evidence["stock"] = "native_timeout" if timed_out else "native_inventory_failed"
        return evidence
    evidence["stock"] = "compiled_native_green"
    code, raw, timed_out = command([str(binary), AUTH_BASE], root, env, 90, control)
    if timed_out or not inventory(raw, code, AUTH_REQUIRED):
        evidence["auth_fixture"] = "native_timeout" if timed_out else "native_inventory_failed"
        evidence["stock"] = "auth_fixture_" + evidence["auth_fixture"]
        return evidence
    evidence["auth_fixture"] = "compiled_native_green"
    for label, path, old, new, case, marker in MUTANTS:
        source = root / path
        original = source.read_bytes()
        result = {"build": "unavailable", "native": "unavailable", "semantic_red": False}
        evidence["mutants"][label] = result
        retain = False
        try:
            if control is not None:
                control.check()
            changed = replacement(original.decode("utf-8"), old, new).encode("utf-8")
            source.write_bytes(changed)
            binary, result["build"] = build(root, env, control)
            if binary is None:
                continue
            code, raw, timed_out = command([str(binary), "--exact", test_name(case)], root, env, 30, control)
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
                "stock": "checkout_failed", "auth_fixture": "unavailable", "mutants": {}}
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
    print("\n".join(closed_summary(evidence)))
    return 0 if evidence["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
