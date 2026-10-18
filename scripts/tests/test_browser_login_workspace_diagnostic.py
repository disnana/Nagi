"""Focused fake-process checks for the one-shot workspace comparison."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "browser_login_workspace_diagnostic",
    ROOT / "scripts/ci/browser_login_workspace_diagnostic.py",
)
DIAG = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DIAG)


def _fixture(root):
    (root / "runtime").mkdir(parents=True)
    sqlite_dir = root / "fixture-rusqlite"
    sqlite_dir.mkdir()
    (root / "Cargo.toml").write_text(
        '[workspace]\nmembers = ["runtime"]\n'
        '[workspace.package]\nversion = "0.1.0"\n', encoding="utf-8"
    )
    (root / "runtime/Cargo.toml").write_text(
        '[package]\nname = "nagi-runtime"\nversion.workspace = true\n',
        encoding="utf-8",
    )
    (sqlite_dir / "Cargo.toml").write_text(
        '[package]\nname = "rusqlite"\nversion = "0.40.0"\n',
        encoding="utf-8",
    )
    exe = root / "target/debug/deps/browser_login_public_api-fixture"
    exe.parent.mkdir(parents=True)
    exe.write_bytes(b"fixed fake test executable\n")
    runtime_id = "path+file://" + str(root / "runtime") + "#0.1.0"
    sqlite_id = "registry+https://example.invalid/rusqlite#rusqlite@0.40.0"
    messages = [
        {
            "reason": "compiler-artifact", "package_id": runtime_id,
            "manifest_path": str(root / "runtime/Cargo.toml"),
            "target": {"name": DIAG.TEST_TARGET, "kind": ["test"], "test": True},
            "executable": str(exe),
        },
        {
            "reason": "compiler-artifact", "package_id": sqlite_id,
            "manifest_path": str(sqlite_dir / "Cargo.toml"),
            "target": {"name": "rusqlite", "kind": ["lib"]},
            "features": ["bundled", "hooks", "limits"],
        },
        {"reason": "build-finished", "success": True},
    ]
    build = b"".join(json.dumps(message).encode() + b"\n" for message in messages)
    return exe, build


def _test_output(expected, failed=(), diagnostics=(), line_ending="\n", extra=""):
    failed = set(failed)
    count = len(expected)
    filtered = len(DIAG.ALL_TESTS) - count
    rows = ["running " + str(count) + (" test" if count == 1 else " tests")]
    rows.extend("test " + name + " ... " + ("FAILED" if name in failed else "ok")
                for name in expected)
    if failed:
        rows.append("")
        rows.append("failures:")
        for name, sample in diagnostics:
            rows.extend(("---- " + name + " stdout ----", sample))
        rows.extend(("", "failures:"))
        rows.extend("    " + name for name in expected if name in failed)
        rows.append("")
        rows.append(
            "test result: FAILED. " + str(count - len(failed)) + " passed; "
            + str(len(failed)) + " failed; 0 ignored; 0 measured; "
            + str(filtered) + " filtered out; finished in 0.01s"
        )
    else:
        rows.append(
            "test result: ok. " + str(count) + " passed; 0 failed; 0 ignored; "
            + "0 measured; " + str(filtered) + " filtered out; finished in 0.00s"
        )
    text = line_ending.join(rows) + line_ending
    return (text + extra).encode("utf-8")


def _stock_sample(**changes):
    values = {
        "a_status": "504", "b_status": "403", "a_auth_cookie": "false",
        "b_auth_cookie": "false", "a_deletion": "false", "b_deletion": "false",
        "verifier_calls": "0", "handler_calls": "0", "claim_committed": "false",
    }
    values.update(changes)
    return "browser_login_stock_cause case=concurrent_once " + " ".join(
        name + "=" + value for name, value in values.items()
    )


def _json_sample(**changes):
    values = {
        "status": "504", "body_bytes": "15", "expected_body_bytes": "31",
        "body_matches": "false", "verifier_calls": "2", "handler_calls": "2",
        "claim_committed": "false",
    }
    values.update(changes)
    return "browser_login_json_bound_diag phase=exact_bounds " + " ".join(
        name + "=" + value for name, value in values.items()
    )


class BrowserLoginWorkspaceDiagnosticTest(unittest.TestCase):
    def test_workflow_accepts_only_the_dedicated_branch_create_event(self):
        workflow = (ROOT / ".github/workflows/browser-login-workspace-diagnostic.yml").read_text()
        self.assertIn("on:\n  create:\n", workflow)
        self.assertNotIn("\n  push:", workflow)
        self.assertNotIn("\n  pull_request:", workflow)
        self.assertNotIn("\n  workflow_dispatch:", workflow)
        self.assertIn("github.event.ref_type == 'branch'", workflow)
        self.assertIn("github.event.ref == 'diag/session-workspace-executable-20261018'", workflow)
        self.assertIn("github.ref == 'refs/heads/diag/session-workspace-executable-20261018'", workflow)
        self.assertIn("contents: read", workflow)
        self.assertIn("persist-credentials: false", workflow)

        def matches(ref_type, ref, github_ref):
            return (ref_type == "branch" and ref == "diag/session-workspace-executable-20261018"
                    and github_ref == "refs/heads/diag/session-workspace-executable-20261018")

        self.assertTrue(matches("branch", "diag/session-workspace-executable-20261018",
                                "refs/heads/diag/session-workspace-executable-20261018"))
        self.assertFalse(matches("branch", "other", "refs/heads/other"))
        self.assertFalse(matches("tag", "diag/session-workspace-executable-20261018",
                                 "refs/tags/diag/session-workspace-executable-20261018"))

    def test_cargo_json_binds_expected_target_and_workspace_feature_union(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            exe, raw = _fixture(root)
            info, state = DIAG.parse_build(raw, 0, root)
            self.assertEqual("passed", state)
            self.assertEqual(exe.resolve(), info["executable"])
            self.assertEqual("nagi-runtime@0.1.0", info["package"])
            self.assertEqual(("bundled", "hooks", "limits"), info["rusqlite_features"])
            self.assertEqual((None, "invalid"), DIAG.parse_build(raw + b"{malformed}\n", 0, root))

    def test_actual_main_runs_two_single_cases_then_default_parallel_once(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            exe, build = _fixture(root)
            commands = []
            all_tests = DIAG.ALL_TESTS
            stock = _stock_sample()
            exact = _json_sample()

            def fake_run(command, cwd, max_bytes, timeout_seconds=None):
                command = tuple(map(str, command))
                cwd = Path(cwd)
                commands.append((command, cwd))
                if "--message-format=json" in command:
                    self.assertEqual(root, cwd)
                    self.assertNotIn("--offline", command)
                    self.assertIn("--workspace", command)
                    self.assertIn("--locked", command)
                    return 0, build, None
                self.assertEqual(root / "runtime", cwd)
                if command[1:] == ("--list",):
                    rows = [name + ": test" for name in all_tests]
                    return 0, ("\n".join(rows) + "\n10 tests, 0 benchmarks\n").encode(), None
                if command[-1:] == ("--exact",):
                    name = command[1]
                    sample = stock if name == DIAG.TARGET_TESTS[0] else exact
                    return 101, _test_output((name,), (name,), ((name, sample),), "\r\n"), None
                self.assertEqual((str(exe),), command)
                diagnostics = ((DIAG.TARGET_TESTS[0], stock), (DIAG.TARGET_TESTS[1], exact))
                return 101, _test_output(all_tests, DIAG.TARGET_TESTS, diagnostics), None

            stdout = io.StringIO()
            with mock.patch.object(DIAG, "ROOT", root), mock.patch.object(
                DIAG, "run_bounded", side_effect=fake_run
            ), mock.patch.dict(os.environ, {}, clear=True), contextlib.redirect_stdout(stdout):
                result = DIAG.main()

            self.assertEqual(1, result)
            self.assertEqual(5, len(commands))  # build, list, two singles, one parallel
            self.assertEqual((str(exe), DIAG.TARGET_TESTS[0], "--exact"), commands[2][0])
            self.assertEqual((str(exe), DIAG.TARGET_TESTS[1], "--exact"), commands[3][0])
            self.assertEqual((str(exe),), commands[4][0])
            output = stdout.getvalue()
            self.assertIn("profile=single-concurrent state=test_failed", output)
            self.assertIn("profile=single-json-bounds state=test_failed", output)
            self.assertIn("profile=parallel-ten state=test_failed", output)
            self.assertIn("sf07_workspace_failure_diagnostic case=concurrent_once", output)
            self.assertIn("sf07_workspace_failure_diagnostic case=exact_bounds", output)
            self.assertEqual(3, output.count("sf07_workspace_cases state=complete"))
            self.assertEqual(12, output.count("sf07_browser_case name="))
            self.assertEqual(3, output.count("sf07_workspace_run profile="))
            self.assertEqual(4, output.count("executable_unchanged=true"))
            self.assertNotIn("browser_login_stock_cause", output)
            self.assertNotIn("browser_login_json_bound_diag", output)

    def test_actual_main_success_runs_each_profile_once(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            exe, build = _fixture(root)
            commands = []

            def fake_run(command, cwd, max_bytes, timeout_seconds=None):
                command = tuple(map(str, command))
                cwd = Path(cwd)
                commands.append((command, cwd))
                if "--message-format=json" in command:
                    return 0, build, None
                if command[1:] == ("--list",):
                    rows = [name + ": test" for name in DIAG.ALL_TESTS]
                    return 0, ("\n".join(rows) + "\n10 tests, 0 benchmarks\n").encode(), None
                if command[-1:] == ("--exact",):
                    expected = (command[1],)
                else:
                    expected = DIAG.ALL_TESTS
                self.assertEqual(root / "runtime", cwd)
                self.assertEqual(str(exe), command[0])
                return 0, _test_output(expected), None

            stdout = io.StringIO()
            with mock.patch.object(DIAG, "ROOT", root), mock.patch.object(
                DIAG, "run_bounded", side_effect=fake_run
            ), mock.patch.dict(os.environ, {}, clear=True), contextlib.redirect_stdout(stdout):
                result = DIAG.main()

            self.assertEqual(0, result)
            self.assertEqual(5, len(commands))
            self.assertEqual((str(exe), DIAG.TARGET_TESTS[0], "--exact"), commands[2][0])
            self.assertEqual((str(exe), DIAG.TARGET_TESTS[1], "--exact"), commands[3][0])
            self.assertEqual((str(exe),), commands[4][0])
            output = stdout.getvalue()
            self.assertIn("sf07_workspace_diagnostic state=passed", output)
            self.assertEqual(3, output.count("sf07_workspace_cases state=complete"))
            self.assertEqual(12, output.count("sf07_browser_case name="))
            self.assertEqual(3, output.count("sf07_workspace_run profile="))
            self.assertEqual(3, output.count("sf07_workspace_diagnostics state=absent"))

    def test_invalid_collection_stops_before_any_profile(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, build = _fixture(root)
            calls = []

            def fake_run(command, cwd, max_bytes, timeout_seconds=None):
                command = tuple(map(str, command))
                calls.append(command)
                if "--message-format=json" in command:
                    return 0, build, None
                return 0, b"one_missing_case: test\n9 tests, 0 benchmarks\n", None

            with mock.patch.object(DIAG, "ROOT", root), mock.patch.object(
                DIAG, "run_bounded", side_effect=fake_run
            ), mock.patch.dict(os.environ, {}, clear=True), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(1, DIAG.main())
            self.assertEqual(2, len(calls))

    def test_malformed_cargo_json_stops_before_inventory(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _fixture(root)
            calls = []

            def fake_run(command, cwd, max_bytes, timeout_seconds=None):
                calls.append(tuple(map(str, command)))
                return 0, b'{"reason":\n', None

            with mock.patch.object(DIAG, "ROOT", root), mock.patch.object(
                DIAG, "run_bounded", side_effect=fake_run
            ), mock.patch.dict(os.environ, {}, clear=True), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(1, DIAG.main())
            self.assertEqual(1, len(calls))

    def test_secret_tainted_failure_output_is_never_echoed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            exe, _ = _fixture(root)
            name = DIAG.TARGET_TESTS[0]
            raw = _test_output((name,), (name,), ((name, _stock_sample()),),
                               extra="secret=DO_NOT_PRINT\n")
            stdout = io.StringIO()
            with mock.patch.object(DIAG, "run_bounded", return_value=(101, raw, None)), \
                    contextlib.redirect_stdout(stdout):
                result = DIAG._run_profile(
                    exe, DIAG._hash_file(exe), root, "single-concurrent", (name,), (name, "--exact")
                )
            self.assertFalse(result)
            self.assertNotIn("DO_NOT_PRINT", stdout.getvalue())
            self.assertIn("sf07_workspace_diagnostics state=unavailable", stdout.getvalue())

    def test_executable_mutation_invalidates_a_profile(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            exe, _ = _fixture(root)
            name = DIAG.TARGET_TESTS[0]
            raw = _test_output((name,), (name,), ((name, _stock_sample()),))

            def mutate_executable(*_args, **_kwargs):
                exe.write_bytes(b"changed fake executable\n")
                return 101, raw, None

            stdout = io.StringIO()
            with mock.patch.object(DIAG, "run_bounded", side_effect=mutate_executable), \
                    contextlib.redirect_stdout(stdout):
                result = DIAG._run_profile(
                    exe, DIAG._hash_file(exe), root, "single-concurrent", (name,), (name, "--exact")
                )
            self.assertFalse(result)
            self.assertIn("state=invalid", stdout.getvalue())
            self.assertIn("executable_unchanged=false", stdout.getvalue())
            self.assertNotIn("state=complete", stdout.getvalue())

    def test_diagnostics_are_closed_and_absent_is_explicit(self):
        cases = ((DIAG.TARGET_TESTS[0], "failed"), (DIAG.TARGET_TESTS[1], "failed"))
        stock = _stock_sample()
        exact = _json_sample()
        for line_ending in ("\n", "\r\n"):
            state, samples = DIAG.project_diagnostics(
                (stock + line_ending + exact + line_ending).encode(), cases
            )
            self.assertEqual("present", state)
            self.assertEqual(2, len(samples))
        self.assertEqual(("absent", ()), DIAG.project_diagnostics(b"ordinary test output\n", cases))
        rejected = (
            stock + "\n" + stock,
            stock + "\nunknown browser_login_stock_cause sample",
            stock + "\nsecret=DO_NOT_PRINT",
            _stock_sample(a_status="9999"),
            _stock_sample(verifier_calls="9" * 5000),
            "browser_login_stock_cause case=unknown status=504",
        )
        for text in rejected:
            self.assertEqual(("unavailable", ()), DIAG.project_diagnostics(text.encode(), cases))
        self.assertEqual((None, "invalid"), DIAG.parse_run(b"x" * (DIAG.MAX_TEST_BYTES + 1), (DIAG.TARGET_TESTS[0],)))
        failed_with_error = _test_output((DIAG.TARGET_TESTS[0],), (DIAG.TARGET_TESTS[0],), extra="error: raw diagnostic\n")
        self.assertEqual((None, "invalid"), DIAG.parse_run(failed_with_error, (DIAG.TARGET_TESTS[0],)))


if __name__ == "__main__":
    unittest.main()
