"""Strict native inventory and semantic RED classification negative controls."""
import importlib.util
import contextlib
import io
import json
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

EXPECTED_NATIVE_REQUIRED = frozenset((
    "http_server::csrf_http::deadline_tests::guarded_preparation_pending_uses_actual_session_deadline",
    "http_server::csrf_http::deadline_tests::guarded_preparation_expiry_precedes_late_grant_denial_and_panic",
    "http_server::csrf_http::deadline_tests::guarded_preparation_same_poll_late_grant_is_504",
    "http_server::csrf_http::deadline_tests::guarded_preparation_same_poll_late_denied_is_504",
    "http_server::csrf_http::deadline_tests::guarded_preparation_same_poll_late_panic_is_504",
    "http_server::csrf_http::deadline_tests::guarded_preparation_initially_expired_session_still_challenges_401",
    "http_server::csrf_http::deadline_tests::guarded_body_wire_preserves_absolute_deadline_and_strict_precedence",
    "http_server::csrf_http::deadline_tests::body_selection_tests::existing_budget_tuples_select_absolute_deadline_and_status",
    "http_server::csrf_http::deadline_tests::body_selection_tests::preparation_spends_original_budget_and_never_restarts_common",
    "http_server::csrf_http::deadline_tests::body_selection_tests::original_credential_common_can_only_shorten_body_window",
    "http_server::csrf_http::deadline_tests::body_selection_tests::exact_tie_and_one_nanosecond_neighbors_are_strict",
    "http_server::csrf_http::deadline_tests::body_selection_tests::unrepresentable_body_deadline_remains_rejected",
    "http_server::csrf_http::deadline_tests::pool_readiness_tests::cold_worker2_startup_spends_original_guarded_http_budget",
    "http_server::csrf_http::deadline_tests::pool_readiness_tests::guarded_busy_mapper_returns_503_retires_owner_and_recovers_capacity",
    "http_server::csrf_http::deadline_tests::pool_readiness_tests::ready_worker2_setup_preserves_same_guarded_http_budget",
))


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
        return ("running 15 tests\n" + "".join("test " + name + " ... ok\n" for name in sorted(EXPECTED_NATIVE_REQUIRED))
                + "test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 500 filtered out; finished in 1.00s\n")

    def auth_stock(self, required=None):
        required = guard.AUTH_REQUIRED if required is None else required
        count = len(required)
        return (f"running {count} tests\n" + "".join("test " + name + " ... ok\n" for name in sorted(required))
                + f"test result: ok. {count} passed; 0 failed; 0 ignored; 0 measured; 500 filtered out; finished in 1.00s\n")

    @staticmethod
    def crlf(raw):
        return raw.replace(b"\r\n", b"\n").replace(b"\n", b"\r\n")

    def red(self, case, marker):
        name = guard.test_name(case)
        return ("running 1 test\n" + "test " + name + " ... FAILED\n"
                + "failures:\n---- " + name + " stdout ----\n"
                + "thread '" + name + "' panicked at private/path.rs:1:1:\n" + marker + "\n"
                + "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 500 filtered out; finished in 1.00s\n")

    def test_source_inventory_is_exact(self):
        source = ROOT / "runtime/src/http_server"
        names = set()
        for filename, prefix in (("csrf_deadline_tests.rs", ""),
                                 ("csrf_body_deadline_tests.rs", "body_selection_tests::"),
                                 ("csrf_pool_readiness_tests.rs", "pool_readiness_tests::")):
            text = (source / filename).read_text()
            names.update(guard.BASE + prefix + name for name in
                         re.findall(r"#\[(?:tokio::)?test\]\s*(?:async )?fn ([a-z0-9_]+)", text))
        self.assertEqual(names, EXPECTED_NATIVE_REQUIRED)
        self.assertEqual(names, guard.REQUIRED)
        self.assertEqual(len(names), 15)

    def test_auth_csrf_source_inventory_is_exact(self):
        source = ROOT / "runtime/src/auth/csrf/tests.rs"
        source_text = source.read_text()
        self.assertIn("mod fixed_wire_owner_tests;", source_text)
        outer = {guard.AUTH_BASE + name for name in
                 re.findall(r"#\[(?:tokio::)?test\]\s*(?:async )?fn ([a-z0-9_]+)", source_text)}
        expected_outer = {guard.AUTH_BASE + name for name in guard.AUTH_CASES}
        self.assertEqual(outer, expected_outer)
        self.assertEqual(len(outer), 14)
        self.assertEqual(len(guard.AUTH_CASES), 14)

        nested_source = source.with_name("fixed_wire_owner_tests.rs")
        nested = {"fixed_wire_owner_tests::" + name for name in
                  re.findall(r"#\[(?:tokio::)?test\]\s*(?:async )?fn ([a-z0-9_]+)", nested_source.read_text())}
        self.assertEqual(nested, set(guard.AUTH_ADDITIONAL))
        self.assertEqual(len(nested), 1)
        self.assertEqual(outer | {guard.AUTH_BASE + name for name in nested}, guard.AUTH_REQUIRED)
        self.assertEqual(len(guard.AUTH_REQUIRED), 15)
        self.assertIn("same_queue_reopen_reuses_data_context_without_refreshing_token_expiry",
                      guard.AUTH_CASES)
        self.assertIn("reopened_context_preserves_original_token_expiry_and_rejects_after_deadline",
                      guard.AUTH_CASES)

    def test_auth_inventory_refuses_old_missing_duplicate_ignored_foreign_extra_and_nonzero(self):
        raw = self.auth_stock()
        self.assertEqual(len(guard.AUTH_REQUIRED), 15)
        first = "test " + sorted(guard.AUTH_REQUIRED)[0] + " ... ok\n"
        nested_name = guard.AUTH_BASE + guard.AUTH_ADDITIONAL[0]
        nested = "test " + nested_name + " ... ok\n"
        old_required = frozenset(guard.AUTH_BASE + name for name in guard.AUTH_CASES)
        old_fourteen = self.auth_stock(old_required)
        missing_name = sorted(guard.AUTH_REQUIRED)[0]
        missing = (raw.replace("test " + missing_name + " ... ok\n", "")
                   .replace("running 15 tests", "running 14 tests")
                   .replace("15 passed", "14 passed"))
        duplicate = raw + first
        ignored = (raw.replace(nested, "test " + nested_name + " ... ignored\n")
                   .replace("15 passed; 0 failed; 0 ignored", "14 passed; 0 failed; 1 ignored"))
        foreign = raw.replace(nested_name, "foreign::" + nested_name.split("::", 2)[-1])
        extra = (raw.replace("running 15 tests", "running 16 tests")
                 .replace("15 passed", "16 passed")
                 + "test auth::csrf::tests::unexpected_extra_case ... ok\n")
        wrong_count = raw.replace("15 passed", "14 passed")
        for label, candidate, code in (
            ("old fourteen", old_fourteen, 0),
            ("missing", missing, 0),
            ("duplicate", duplicate, 0),
            ("ignored", ignored, 0),
            ("foreign", foreign, 0),
            ("extra", extra, 0),
            ("wrong summary count", wrong_count, 0),
            ("nonzero exit", raw, 101),
        ):
            with self.subTest(case=label):
                self.assertFalse(guard.inventory(candidate, code, guard.AUTH_REQUIRED))

    def test_stock_green_and_crlf(self):
        self.assertTrue(guard.inventory(self.stock(), 0, guard.REQUIRED))
        self.assertTrue(guard.inventory(self.stock().replace("\n", "\r\n"), 0, guard.REQUIRED))
        self.assertTrue(guard.inventory(self.auth_stock(), 0, guard.AUTH_REQUIRED))
        self.assertTrue(guard.inventory(self.auth_stock().replace("\n", "\r\n"), 0, guard.AUTH_REQUIRED))

    def test_zero_missing_duplicate_ignored_other_stage_and_nonzero_fail(self):
        row = "test " + sorted(guard.REQUIRED)[0] + " ... ok\n"
        old_twelve = frozenset(name for name in EXPECTED_NATIVE_REQUIRED if "pool_readiness_tests::" not in name)
        old_stock = ("running 12 tests\n" + "".join("test " + name + " ... ok\n" for name in sorted(old_twelve))
                     + "test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 500 filtered out; finished in 1.00s\n")
        self.assertFalse(guard.inventory(old_stock, 0, guard.REQUIRED))
        for raw, code in ((self.stock().replace(row, ""), 0), (self.stock() + row, 0),
                          (self.stock().replace(row, row.replace("... ok", "... ignored")), 0),
                          (self.stock().replace(row, row.replace(guard.BASE, "other::")), 0),
                          (self.stock().replace("15 passed", "0 passed"), 0), (self.stock(), 101)):
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

    def test_auth_semantic_red_requires_exact_case_summary_and_unique_marker(self):
        cases = [item for item in guard.MUTANTS if item[4].startswith(guard.AUTH_BASE)]
        self.assertEqual(len(cases), 4)
        for label, _, _, _, case, marker in cases:
            with self.subTest(mutant=label):
                good = self.red(case, marker)
                self.assertTrue(guard.semantic_red(good, 101, case, marker))
                for raw, code in (
                    ("error: rustc rejected mutation\n", 101),
                    (good.replace("running 1 test", "running 0 tests"), 101),
                    (good.replace("... FAILED", "... ignored"), 101),
                    (good.replace(case, "different::test"), 101),
                    (good.replace(marker, "unrelated_assertion"), 101),
                    (good + marker + "\n", 101),
                    (marker + "\n" + good, 101),
                    (good, 0),
                    (good, 124),
                ):
                    self.assertFalse(guard.semantic_red(raw, code, case, marker))

    def test_auth_fixture_stock_failure_stops_before_any_mutant(self):
        with mock.patch.object(guard, "build", return_value=(ROOT / "native", "compiled")), \
                mock.patch.object(guard, "command", side_effect=[
                    (0, self.stock(), False), (0, "running 0 tests\n", False)]) as command:
            record = guard.run_checkout(ROOT, {})
        self.assertEqual(record["auth_fixture"], "native_inventory_failed")
        self.assertEqual(record["stock"], "auth_fixture_native_inventory_failed")
        self.assertEqual(record["mutants"], {})
        self.assertEqual(command.call_count, 2)

    def test_auth_mutant_runner_wiring_restores_source_and_uses_exact_target(self):
        mutants = [item for item in guard.MUTANTS if item[4].startswith(guard.AUTH_BASE)]
        for mutant in mutants:
            label, path, old, new, case, marker = mutant
            with self.subTest(mutant=label):
                original = (ROOT / path).read_bytes()
                fake_source = mock.Mock()
                fake_source.read_bytes.return_value = original
                calls = [(0, self.stock(), False), (0, self.auth_stock(), False),
                         (101, self.red(case, marker), False)]
                with mock.patch.object(guard, "MUTANTS", (mutant,)), \
                        mock.patch.object(guard, "build", return_value=(ROOT / "native", "compiled")) as build, \
                        mock.patch.object(guard, "command", side_effect=calls) as command, \
                        mock.patch.object(pathlib.Path, "__truediv__", return_value=fake_source):
                    record = guard.run_checkout(ROOT, {})
                self.assertEqual(record["auth_fixture"], "compiled_native_green")
                self.assertTrue(record["mutants"][label]["semantic_red"])
                self.assertEqual(command.call_args_list[-1].args[0],
                                 [str(ROOT / "native"), "--exact", case])
                self.assertEqual(command.call_count, 3)
                self.assertEqual(build.call_count, 2)
                writes = fake_source.write_bytes.call_args_list
                self.assertEqual(len(writes), 2)
                self.assertNotEqual(writes[0].args[0], original)
                self.assertEqual(writes[1].args[0], original)

    def test_approved_auth_mutant_mapping_and_crlf_anchors(self):
        expected = {
            "csrf_compatible_checkpoint_always_denied":
                (guard.AUTH_BASE + guard.AUTH_CASES[0],
                 "same_queue_fixture phase=PositiveCheckpoint kind=Some(Denied) outcome=Some(RolledBack)"),
            "csrf_reopen_refreshes_stored_expiry":
                (guard.AUTH_BASE + guard.AUTH_CASES[1], "same_queue_fixture phase=OriginalRowUnchanged"),
            "csrf_checkpoint_renews_expired_row":
                (guard.AUTH_BASE + guard.AUTH_CASES[1], "same_queue_fixture phase=ExpiredCheckpointDeniedRolledBack"),
            "csrf_delivery_refreshes_original_deadline":
                (guard.AUTH_BASE + guard.AUTH_CASES[0], "same_queue_fixture phase=OriginalDeliveryDeadlineDenied"),
        }
        mutants = {label: (case, marker) for label, _, _, _, case, marker in guard.MUTANTS
                   if case.startswith(guard.AUTH_BASE)}
        self.assertEqual(mutants, expected)
        for label, path, old, new, _, _ in guard.MUTANTS:
            if not label.startswith("csrf_"):
                continue
            source = (ROOT / path).read_text(encoding="utf-8")
            crlf = source.replace("\n", "\r\n")
            changed = guard.replacement(crlf, old, new)
            self.assertNotEqual(changed, crlf)
            self.assertNotIn("\n", changed.replace("\r\n", ""))
            with self.assertRaises(ValueError):
                guard.replacement(crlf + "\n", old, new)

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
                    (0, self.stock(), False), (0, self.auth_stock(), False),
                    (101, self.red(case, marker), False)]), \
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
                mock.patch.object(guard, "command", side_effect=[
                    (0, self.stock(), False), (0, self.auth_stock(), False)]) as command, \
                mock.patch.object(pathlib.Path, "__truediv__", return_value=fake):
            result = guard.run_checkout(ROOT, {})
        build.assert_called_once()
        self.assertEqual(command.call_count, 2)
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
                mock.patch.object(guard, "command", side_effect=[
                    (0, self.stock(), False), (0, self.auth_stock(), False)]), \
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
                mock.patch.object(guard, "command", side_effect=[
                    (0, self.stock(), False), (0, self.auth_stock(), False)]), \
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
                mock.patch.object(guard, "command", side_effect=[
                    (0, self.stock(), False), (0, self.auth_stock(), False)]), \
                mock.patch.object(pathlib.Path, "__truediv__", return_value=fake):
            with self.assertRaises(guard.RunCancelled):
                guard.run_checkout(ROOT, {}, control)
        self.assertEqual(fake.write_bytes.call_count, 1)
        self.assertNotEqual(fake.write_bytes.call_args.args[0], fake.read_bytes.return_value)

    def summary_from_execute(self, evidence):
        captured = io.StringIO()
        with mock.patch.object(guard, "enable_subreaper", return_value=True), \
                mock.patch.object(guard.tempfile, "mkdtemp", return_value="/owned-summary-test"), \
                mock.patch.object(guard, "command", return_value=(0, "", False)) as command, \
                mock.patch.object(guard, "run_checkout", return_value=evidence), \
                mock.patch.object(guard.shutil, "rmtree"), \
                mock.patch.object(pathlib.Path, "write_text") as write, \
                contextlib.redirect_stdout(captured):
            code = guard.execute(ROOT, pathlib.Path("/closed-summary.json"), guard.Cancellation())
        self.assertEqual(command.call_count, 2)
        self.assertEqual(json.loads(write.call_args.args[0]), evidence)
        self.assertEqual(code, 0 if evidence["status"] == "passed" else 1)
        summary = captured.getvalue()
        self.assertEqual(len(summary.splitlines()), 3 + len(guard.MUTANTS))
        self.assertLessEqual(len(summary.encode("ascii")), 2048)
        return summary

    def test_closed_summary_fake_compile_inventory_assertion_and_red_failures(self):
        label = guard.MUTANTS[0][0]
        for stock, auth, result in (
            ("build_failed", "unavailable", None),
            ("build_timeout", "unavailable", None),
            ("build_artifact_missing", "unavailable", None),
            ("native_inventory_failed", "unavailable", None),
            ("native_timeout", "unavailable", None),
            ("auth_fixture_native_inventory_failed", "native_inventory_failed", None),
            ("auth_fixture_native_timeout", "native_timeout", None),
            ("compiled_native_green", "compiled_native_green",
             {"build": "build_failed", "native": "unavailable", "semantic_red": False}),
            ("compiled_native_green", "compiled_native_green",
             {"build": "unavailable", "native": "invalid_source_or_execution", "semantic_red": False}),
            ("compiled_native_green", "compiled_native_green",
             {"build": "compiled", "native": "completed", "native_exit": 101, "semantic_red": False}),
            ("compiled_native_green", "compiled_native_green",
             {"build": "compiled", "native": "process_timeout", "native_exit": -1, "semantic_red": False}),
            ("compiled_native_green", "compiled_native_green",
             {"build": "compiled", "native": "completed", "native_exit": 101, "semantic_red": True}),
        ):
            with self.subTest(stock=stock, result=result):
                evidence = {"schema": "nagi-csrf-deadline-mutations-v1", "status": "failed",
                            "stock": stock, "auth_fixture": auth,
                            "mutants": {} if result is None else {label: result}}
                summary = self.summary_from_execute(evidence)
                self.assertTrue(summary.startswith("csrf_deadline_compiled_mutations=failed\n"))
                self.assertIn("csrf_deadline_mutation_stock=" + stock + "\n", summary)
                self.assertIn("csrf_deadline_mutation_auth_fixture=" + auth + "\n", summary)
                expected = ("build=unavailable native=unavailable native_exit=unavailable semantic_red=unavailable"
                            if result is None else
                            "build=" + result["build"] + " native=" + result["native"]
                            + " native_exit=" + str(result.get("native_exit", "unavailable"))
                            + " semantic_red=" + str(result["semantic_red"]).lower())
                self.assertIn("csrf_deadline_mutant=" + label + " " + expected + "\n", summary)

    def test_closed_summary_success_preserves_passed_gate_and_all_mutant_rows(self):
        result = {"build": "compiled", "native": "completed", "native_exit": 101, "semantic_red": True}
        evidence = {"schema": "nagi-csrf-deadline-mutations-v1", "status": "passed",
                    "stock": "compiled_native_green", "auth_fixture": "compiled_native_green",
                    "mutants": {item[0]: dict(result) for item in guard.MUTANTS}}
        summary = self.summary_from_execute(evidence)
        self.assertTrue(summary.startswith("csrf_deadline_compiled_mutations=passed\n"))
        self.assertEqual([line.split()[0] for line in summary.splitlines()[3:]],
                         ["csrf_deadline_mutant=" + item[0] for item in guard.MUTANTS])
        self.assertEqual(summary.count("semantic_red=true\n"), len(guard.MUTANTS))

    def test_closed_summary_unknown_values_and_labels_never_reach_stdout(self):
        payload = "private/path\\secret\n::error::" + "X" * 10000 + "\u2603"
        for result in (payload, [], None,
                       {"build": payload, "native": payload, "native_exit": payload, "semantic_red": payload},
                       {"build": ["compiled"], "native": {"completed": True}, "native_exit": True, "semantic_red": 1}):
            with self.subTest(kind=type(result).__name__):
                evidence = {"status": "failed", "stock": payload, "auth_fixture": payload,
                            "mutants": {payload: payload, guard.MUTANTS[0][0]: result}, "raw": payload}
                summary = self.summary_from_execute(evidence)
                self.assertNotIn("private", summary)
                self.assertNotIn("::error::", summary)
                self.assertNotIn("X", summary)
                self.assertIn("csrf_deadline_mutation_stock=unavailable\n", summary)
                self.assertIn("csrf_deadline_mutation_auth_fixture=unavailable\n", summary)
                self.assertTrue(all(line.endswith("native_exit=unavailable semantic_red=unavailable")
                                    for line in summary.splitlines()[3:]))

    def test_closed_summary_native_exit_is_bounded_and_boolean_is_not_an_integer(self):
        for code, expected in ((-1, "-1"), (0, "0"), (101, "101"), (124, "other"),
                               (10 ** 100, "other"), (True, "unavailable")):
            with self.subTest(code=code):
                result = {"build": "compiled", "native": "completed", "native_exit": code, "semantic_red": False}
                evidence = {"status": "failed", "stock": "compiled_native_green",
                            "auth_fixture": "compiled_native_green", "mutants": {guard.MUTANTS[0][0]: result}}
                summary = self.summary_from_execute(evidence)
                self.assertIn("native_exit=" + expected + " semantic_red=false\n", summary)

    def test_closed_summary_rejects_nonmapping_mutants_and_unexpected_aggregate_status(self):
        for mutants in (None, [], "::error::private"):
            evidence = {"status": "::error::private", "stock": ["build_failed"],
                        "auth_fixture": {"native_timeout": True}, "mutants": mutants}
            summary = self.summary_from_execute(evidence)
            self.assertTrue(summary.startswith("csrf_deadline_compiled_mutations=failed\n"))
            self.assertNotIn("private", summary)
            self.assertNotIn("::error::", summary)
            self.assertEqual(summary.count("build=unavailable native=unavailable native_exit=unavailable semantic_red=unavailable\n"),
                             len(guard.MUTANTS))


if __name__ == "__main__":
    unittest.main()
