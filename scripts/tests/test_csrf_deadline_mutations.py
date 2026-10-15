"""Strict native inventory and semantic RED classification negative controls."""
import importlib.util
import pathlib
import re
import linecache
import sys
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("csrf_mutants", ROOT / "scripts/ci/csrf_deadline_mutations.py")
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


class MutationEvidenceTests(unittest.TestCase):
    def setUp(self):
        # Process ownership runs only in the dedicated Linux CI CLI. Its mocked
        # negative controls also run on Windows/macOS without invoking syscalls.
        constants = mock.patch.multiple(guard.os, P_PID=1, WEXITED=4, WNOHANG=1,
                                        WNOWAIT=0x1000000, create=True)
        constants.start()
        self.addCleanup(constants.stop)
        kill = mock.patch.object(guard.signal, "SIGKILL", 9, create=True)
        kill.start()
        self.addCleanup(kill.stop)

    def stock(self):
        return ("running 12 tests\n" + "".join("test " + name + " ... ok\n" for name in sorted(guard.REQUIRED))
                + "test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 500 filtered out; finished in 1.00s\n")

    @staticmethod
    def crlf(raw):
        return raw.replace(b"\r\n", b"\n").replace(b"\n", b"\r\n")

    def red(self, case, marker):
        name = guard.BASE + case
        return ("running 1 test\n" + "test " + name + " ... FAILED\n"
                + "failures:\n---- " + name + " stdout ----\n"
                + "thread '" + name + "' panicked at private/path.rs:1:1:\n" + marker + "\n"
                + "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 500 filtered out; finished in 1.00s\n")

    def test_source_inventory_is_exact(self):
        source = ROOT / "runtime/src/http_server"
        names = set()
        for filename, prefix in (("csrf_deadline_tests.rs", ""), ("csrf_body_deadline_tests.rs", "body_selection_tests::")):
            text = (source / filename).read_text()
            names.update(guard.BASE + prefix + name for name in
                         re.findall(r"#\[(?:tokio::)?test\]\s*(?:async )?fn ([a-z0-9_]+)", text))
        self.assertEqual(names, guard.REQUIRED)
        self.assertEqual(len(names), 12)

    def test_stock_green_and_crlf(self):
        self.assertTrue(guard.inventory(self.stock(), 0, guard.REQUIRED))
        self.assertTrue(guard.inventory(self.stock().replace("\n", "\r\n"), 0, guard.REQUIRED))

    def test_zero_missing_duplicate_ignored_other_stage_and_nonzero_fail(self):
        row = "test " + sorted(guard.REQUIRED)[0] + " ... ok\n"
        for raw, code in ((self.stock().replace(row, ""), 0), (self.stock() + row, 0),
                          (self.stock().replace(row, row.replace("... ok", "... ignored")), 0),
                          (self.stock().replace(row, row.replace(guard.BASE, "other::")), 0),
                          (self.stock().replace("12 passed", "0 passed"), 0), (self.stock(), 101)):
            with self.subTest(code=code):
                self.assertFalse(guard.inventory(raw, code, guard.REQUIRED))

    def test_each_exact_semantic_red(self):
        for label, _, _, _, case, marker in guard.MUTANTS:
            with self.subTest(mutant=label):
                self.assertTrue(guard.semantic_red(self.red(case, marker), 101, case, marker))
                self.assertTrue(guard.semantic_red(self.red(case, marker).replace("\n", "\r\n"), 101, case, marker))

    def test_build_error_timeout_zero_ignored_wrong_case_and_wrong_assert_are_not_red(self):
        *_, case, marker = guard.MUTANTS[0]
        good = self.red(case, marker)
        for raw, code in (("error: rustc failed\n", 101), (good, -1), (good, 124), (good, 0),
                          (good.replace("running 1 test", "running 0 tests"), 101),
                          (good.replace("0 ignored", "1 ignored"), 101),
                          (good.replace(case, "different_stage"), 101),
                          (good.replace(marker, "unrelated_assertion"), 101),
                          (marker + "\n" + good.replace(marker, "unrelated_assertion"), 101),
                          (good + marker + "\n", 101)):
            with self.subTest(code=code):
                self.assertFalse(guard.semantic_red(raw, code, case, marker))

    def test_mutation_anchors_are_unique_and_all_sources_are_real(self):
        for _, path, old, new, _, _ in guard.MUTANTS:
            with self.subTest(path=path):
                source = (ROOT / path).read_text()
                self.assertEqual(source.count(old), 1)
                self.assertNotEqual(source, guard.replacement(source, old, new))
                with self.assertRaises(ValueError):
                    guard.replacement(source + old, old, new)

    def test_replacement_preserves_crlf_and_rejects_mixed_newlines(self):
        label, path, old, new, _, _ = guard.MUTANTS[0]
        source = (ROOT / path).read_text()
        crlf_source = source.replace("\n", "\r\n")
        changed = guard.replacement(crlf_source, old, new)
        self.assertNotEqual(changed, crlf_source)
        self.assertNotIn("\n", changed.replace("\r\n", ""))
        self.assertEqual(guard.replacement(source, old, new).count("\r\n"), 0)
        with self.assertRaises(ValueError):
            guard.replacement(crlf_source + "\n", old, new)

    def test_crlf_mutation_restores_exact_original_bytes_after_process_exit(self):
        label, path, old, new, case, marker = guard.MUTANTS[0]
        original = self.crlf((ROOT / path).read_bytes())
        fake = mock.Mock()
        fake.read_bytes.return_value = original
        with mock.patch.object(guard, "MUTANTS", ((label, path, old, new, case, marker),)), \
                mock.patch.object(guard, "build", return_value=(ROOT / "native", "compiled")), \
                mock.patch.object(guard, "command", side_effect=[
                    (0, self.stock(), False), (101, self.red(case, marker), False)]), \
                mock.patch.object(pathlib.Path, "__truediv__", return_value=fake):
            guard.run_checkout(ROOT, {})
        writes = fake.write_bytes.call_args_list
        self.assertEqual(len(writes), 2)
        mutated = writes[0].args[0]
        self.assertNotEqual(mutated, original)
        self.assertIn(b"\r\n", mutated)
        self.assertNotIn(b"\n", mutated.replace(b"\r\n", b""))
        self.assertEqual(writes[1].args[0], original)

    def test_mixed_newline_source_fails_closed_before_mutant_build(self):
        label, path, old, new, case, marker = guard.MUTANTS[0]
        original = self.crlf((ROOT / path).read_bytes()) + b"\n"
        fake = mock.Mock()
        fake.read_bytes.return_value = original
        with mock.patch.object(guard, "MUTANTS", ((label, path, old, new, case, marker),)), \
                mock.patch.object(guard, "build", return_value=(ROOT / "native", "compiled")) as build, \
                mock.patch.object(guard, "command", return_value=(0, self.stock(), False)) as command, \
                mock.patch.object(pathlib.Path, "__truediv__", return_value=fake):
            result = guard.run_checkout(ROOT, {})
        build.assert_called_once()
        command.assert_called_once()
        fake.write_bytes.assert_called_once_with(original)
        self.assertEqual(result["mutants"][label]["native"], "invalid_source_or_execution")

    def test_compiler_rejection_and_compile_watchdog_are_not_native_evidence(self):
        for outcome, expected in (((101, "error", False), "build_failed"),
                                  ((-1, "", True), "build_timeout"),
                                  ((0, "", False), "build_failed")):
            with mock.patch.object(guard, "command", return_value=outcome):
                binary, stage = guard.build(ROOT, {})
                self.assertIsNone(binary)
                self.assertEqual(stage, expected)

    def test_native_process_watchdog_never_calls_red_classifier(self):
        # The runner's timeout branch is separate from the fixed Rust watchdog
        # assertion. Only that compiled assertion is permitted to be RED.
        self.assertIn("pending_actual_timer_wake_watchdog", [m[-1] for m in guard.MUTANTS])
        with mock.patch.object(guard, "build", return_value=(ROOT / "native", "compiled")), \
                mock.patch.object(guard, "command", return_value=(-1, "", True)), \
                mock.patch.object(guard, "semantic_red") as red:
            record = guard.run_checkout(ROOT, {})
        self.assertEqual(record["stock"], "native_timeout")
        red.assert_not_called()

    def test_leader_is_observed_without_reaping_then_children_are_waited_before_leader(self):
        owner = guard.ProcessOwner()
        owner.pgid = 12345
        owner.process = mock.Mock()
        calls = mock.Mock()
        calls.attach_mock(owner.process.wait, "leader_wait")
        with mock.patch.object(guard.os, "waitid", return_value=object(), create=True) as waitid, \
                mock.patch.object(owner, "descendants", side_effect=[[12346], []]), \
                mock.patch.object(guard.os, "waitpid", return_value=(12346, 0), create=True) as child_wait:
            calls.attach_mock(child_wait, "child_wait")
            self.assertTrue(owner.joined())
        waitid.assert_called_once_with(guard.os.P_PID, 12345,
                                       guard.os.WEXITED | guard.os.WNOHANG | guard.os.WNOWAIT)
        self.assertEqual(calls.mock_calls, [mock.call.child_wait(12346, guard.os.WNOHANG),
                                           mock.call.leader_wait(timeout=1)])

    def test_timeout_terminates_only_owned_group_with_finite_waits(self):
        owner = guard.ProcessOwner()
        owner.pgid = 12345
        owner.process = mock.Mock()
        with mock.patch.object(guard.os, "killpg", create=True) as killpg, \
                mock.patch.object(owner, "wait_joined", side_effect=[False, True]) as wait:
            owner.stop_and_wait()
        self.assertEqual(killpg.call_args_list, [mock.call(12345, guard.signal.SIGTERM),
                                                mock.call(12345, guard.signal.SIGKILL)])
        self.assertEqual(wait.call_args_list, [mock.call(3), mock.call(3)])

    def test_unconfirmed_group_retains_owner_and_refuses_source_restore(self):
        owner = guard.ProcessOwner()
        owner.pgid = 12345
        owner.process = mock.Mock()
        with mock.patch.object(guard.os, "killpg", create=True), \
                mock.patch.object(owner, "wait_joined", return_value=False):
            with self.assertRaises(guard.ProcessRetentionRequired) as failure:
                owner.stop_and_wait()
        self.assertIs(failure.exception.owner, owner)
        # This branch must propagate beyond run_checkout's source restore.
        label, path, old, new, case, marker = guard.MUTANTS[0]
        original = (ROOT / path).read_text()
        fake_source = mock.Mock()
        fake_source.read_bytes.return_value = original.encode()
        with mock.patch.object(guard, "MUTANTS", ((label, path, old, new, case, marker),)), \
                mock.patch.object(guard, "build", side_effect=[(ROOT / "native", "compiled"), failure.exception]), \
                mock.patch.object(guard, "command", return_value=(0, self.stock(), False)), \
                mock.patch.object(pathlib.Path, "__truediv__", return_value=fake_source):
            with self.assertRaises(guard.ProcessRetentionRequired):
                guard.run_checkout(ROOT, {})
        fake_source.write_text.assert_not_called()
        fake_source.write_bytes.assert_called_once()
        self.assertNotEqual(fake_source.write_bytes.call_args.args[0], original.encode("utf-8"))

    def test_unconfirmed_group_refuses_worktree_remove_and_directory_delete(self):
        owner = guard.ProcessOwner()
        with mock.patch.object(guard, "enable_subreaper", return_value=True), \
                mock.patch.object(guard.tempfile, "mkdtemp", return_value="/owned-test-only"), \
                mock.patch.object(guard, "command", return_value=(0, "", False)) as command, \
                mock.patch.object(guard, "run_checkout", side_effect=guard.ProcessRetentionRequired(owner)), \
                mock.patch.object(guard.shutil, "rmtree") as remove, \
                mock.patch.object(pathlib.Path, "write_text") as write, \
                mock.patch("builtins.print"):
            self.assertEqual(guard.main(["runner", str(ROOT), "/closed-evidence.json"]), 1)
        self.assertEqual(command.call_count, 1)
        remove.assert_not_called()
        self.assertIn('"workspace_retained": true', write.call_args.args[0])

    def launch_main(self, popen_effect, publication_interrupt=False):
        owners = []
        def call(args, root, env, seconds, control=None):
            self.assertIn("add", args)
            owner = guard.ProcessOwner(control)
            owners.append(owner)
            return owner.run(args, root, env, seconds)
        def trace(frame, event, _arg):
            if (publication_interrupt and event == "line" and
                    frame.f_code is guard.ProcessOwner.run.__code__ and
                    linecache.getline(frame.f_code.co_filename, frame.f_lineno).strip() == "self.pgid = self.process.pid"):
                raise KeyboardInterrupt()
            return trace
        previous = sys.gettrace()
        try:
            with mock.patch.object(guard, "enable_subreaper", return_value=True), \
                    mock.patch.object(guard.tempfile, "mkdtemp", return_value="/audit-owned"), \
                    mock.patch.object(guard.tempfile, "TemporaryFile"), \
                    mock.patch.object(guard.subprocess, "Popen", side_effect=popen_effect), \
                    mock.patch.object(guard, "command", side_effect=call) as commands, \
                    mock.patch.object(guard.os, "killpg", create=True) as kill, \
                    mock.patch.object(guard.shutil, "rmtree") as remove, \
                    mock.patch.object(pathlib.Path, "write_text") as write, \
                    mock.patch("builtins.print"):
                if publication_interrupt:
                    sys.settrace(trace)
                status = guard.main(["runner", str(ROOT), "/closed-evidence.json"])
                self.assertEqual(status, 1)
                self.assertEqual(commands.call_count, 1)
                remove.assert_not_called()
                kill.assert_not_called()
                self.assertIn('"workspace_retained": true', write.call_args.args[0])
        finally:
            sys.settrace(previous)
        return owners

    def test_a1_trace_interrupt_between_launch_and_pgid_publication_retains(self):
        handle = mock.Mock(pid=12345)
        owners = self.launch_main(lambda *a, **k: handle, publication_interrupt=True)
        self.assertIs(owners[0].process, handle)
        self.assertIsNone(owners[0].pgid)
        self.assertIs(owners[0].control.active, owners[0])

    def test_ambiguous_forkexec_raise_with_no_published_handle_retains(self):
        for error in (KeyboardInterrupt(), OSError("forkexec_result_unknown")):
            with self.subTest(kind=type(error).__name__):
                owners = self.launch_main(mock.Mock(side_effect=error))
                self.assertTrue(owners[0].launch_attempted)
                self.assertIsNone(owners[0].process)

    def test_sigint_sigterm_only_record_through_launch_then_owned_stop(self):
        for sig in (guard.signal.SIGINT, guard.signal.SIGTERM):
            with self.subTest(signal=sig):
                control = guard.Cancellation()
                owner = guard.ProcessOwner(control)
                def launch(*_a, **_k):
                    control.record(sig, None)
                    self.assertIs(control.active, owner)
                    return mock.Mock(pid=12345)
                def joined_stop():
                    self.assertEqual(owner.pgid, 12345)
                    # Repeated cancellation stays recorded during finite wait.
                    control.record(sig, None)
                    owner.confirmed = True
                with mock.patch.object(guard.subprocess, "Popen", side_effect=launch), \
                        mock.patch.object(guard.tempfile, "TemporaryFile"), \
                        mock.patch.object(owner, "stop_and_wait", side_effect=joined_stop) as stop:
                    with self.assertRaises(guard.RunCancelled):
                        owner.run(["cargo"], ROOT, {}, 1)
                stop.assert_called_once()
                self.assertTrue(control.requested)
                self.assertIsNone(control.active)

    def test_handlers_are_installed_and_restored_without_signal_mask(self):
        prior = mock.Mock()
        with mock.patch.object(guard.signal, "signal", return_value=prior) as register:
            with guard.Cancellation() as control:
                control.record(guard.signal.SIGTERM, None)
                self.assertTrue(control.requested)
        self.assertEqual(register.call_args_list, [
            mock.call(guard.signal.SIGINT, control.record), mock.call(guard.signal.SIGTERM, control.record),
            mock.call(guard.signal.SIGINT, prior), mock.call(guard.signal.SIGTERM, prior)])

    def test_interrupt_during_finite_stop_is_retention_not_parent_only_kill(self):
        owner = guard.ProcessOwner()
        owner.process, owner.pgid = mock.Mock(), 12345
        with mock.patch.object(guard.os, "killpg", side_effect=KeyboardInterrupt(), create=True):
            with self.assertRaises(guard.ProcessRetentionRequired) as error:
                owner.stop_and_wait()
        self.assertIs(error.exception.owner, owner)

    def test_reap_publication_interrupt_never_signals_a_reusable_pid(self):
        owner = guard.ProcessOwner()
        owner.process, owner.pgid = mock.Mock(), 12345
        owner.reap_started = True
        with mock.patch.object(guard.os, "killpg", create=True) as kill:
            with self.assertRaises(guard.ProcessRetentionRequired):
                owner.stop_and_wait()
        kill.assert_not_called()

    def test_unknown_owner_blocks_every_subsequent_launch(self):
        control = guard.Cancellation()
        first = guard.ProcessOwner(control)
        control.active = first
        with mock.patch.object(guard.subprocess, "Popen") as launch:
            with self.assertRaises(guard.ProcessRetentionRequired) as error:
                guard.ProcessOwner(control).run(["git"], ROOT, {}, 1)
        self.assertIs(error.exception.owner, first)
        launch.assert_not_called()

    def test_cancel_during_cleanup_retains_and_never_rmtree(self):
        control = guard.Cancellation()
        with mock.patch.object(guard, "enable_subreaper", return_value=True), \
                mock.patch.object(guard.tempfile, "mkdtemp", return_value="/audit-owned"), \
                mock.patch.object(guard, "command", side_effect=[(0, "", False), KeyboardInterrupt()]) as command, \
                mock.patch.object(guard, "run_checkout", return_value={"status": "passed"}), \
                mock.patch.object(guard.shutil, "rmtree") as remove, \
                mock.patch.object(pathlib.Path, "write_text") as write, \
                mock.patch("builtins.print"):
            self.assertEqual(guard.execute(ROOT, pathlib.Path("/closed.json"), control), 1)
        self.assertEqual(command.call_count, 2)
        remove.assert_not_called()
        self.assertIn('"workspace_retained": true', write.call_args.args[0])

    def test_recorded_sigterm_during_cleanup_never_starts_directory_delete(self):
        control = guard.Cancellation()
        calls = []
        def command(args, *_a):
            calls.append(args)
            if "remove" in args:
                control.record(guard.signal.SIGTERM, None)
            return 0, "", False
        with mock.patch.object(guard, "enable_subreaper", return_value=True), \
                mock.patch.object(guard.tempfile, "mkdtemp", return_value="/audit-owned"), \
                mock.patch.object(guard, "command", side_effect=command), \
                mock.patch.object(guard, "run_checkout", return_value={"status": "passed"}), \
                mock.patch.object(guard.shutil, "rmtree") as remove, \
                mock.patch.object(pathlib.Path, "write_text") as write, \
                mock.patch("builtins.print"):
            self.assertEqual(guard.execute(ROOT, pathlib.Path("/closed.json"), control), 1)
        self.assertEqual(len(calls), 2)
        remove.assert_not_called()
        self.assertIn('"workspace_retained": true', write.call_args.args[0])

    def test_cancelled_mutant_never_restores_source(self):
        label, path, old, new, case, marker = guard.MUTANTS[0]
        fake = mock.Mock()
        fake.read_bytes.return_value = self.crlf((ROOT / path).read_bytes())
        with mock.patch.object(guard, "MUTANTS", ((label, path, old, new, case, marker),)), \
                mock.patch.object(guard, "build", side_effect=[(ROOT / "native", "compiled"), KeyboardInterrupt()]), \
                mock.patch.object(guard, "command", return_value=(0, self.stock(), False)), \
                mock.patch.object(pathlib.Path, "__truediv__", return_value=fake):
            with self.assertRaises(KeyboardInterrupt):
                guard.run_checkout(ROOT, {})
        self.assertEqual(fake.write_bytes.call_count, 1)
        self.assertNotEqual(fake.write_bytes.call_args.args[0], fake.read_bytes.return_value)

    def test_recorded_signal_at_mutant_finally_never_restores_source(self):
        control = guard.Cancellation()
        label, path, old, new, case, marker = guard.MUTANTS[0]
        fake = mock.Mock()
        fake.read_bytes.return_value = self.crlf((ROOT / path).read_bytes())
        native, builds = ROOT / "native", []
        def build(*_a):
            builds.append(None)
            if len(builds) == 1:
                return native, "compiled"
            control.record(guard.signal.SIGINT, None)
            return None, "build_failed"
        with mock.patch.object(guard, "MUTANTS", ((label, path, old, new, case, marker),)), \
                mock.patch.object(guard, "build", side_effect=build), \
                mock.patch.object(guard, "command", return_value=(0, self.stock(), False)), \
                mock.patch.object(pathlib.Path, "__truediv__", return_value=fake):
            with self.assertRaises(guard.RunCancelled):
                guard.run_checkout(ROOT, {}, control)
        self.assertEqual(fake.write_bytes.call_count, 1)
        self.assertNotEqual(fake.write_bytes.call_args.args[0], fake.read_bytes.return_value)


if __name__ == "__main__":
    unittest.main()
