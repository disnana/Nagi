"""Focused fakes for the one-build, same-executable full-lib comparison."""
import contextlib
import hashlib
import io
import json
import pathlib
import re
import tempfile
import unittest
from unittest import mock

from scripts.ci import auth_stage_diagnostic as auth_parser
from scripts.ci import session_windows_same_exe_diagnostic as base
from scripts.ci import session_windows_full_lib_diagnostic as diag


class Parser:
    RECORD = auth_parser.RECORD
    CASE_ENTRY = re.compile(r"([A-Za-z_][A-Za-z0-9_:]*): test\Z")
    CASE_FOOTER = re.compile(r"([0-9]{1,4}) tests, 0 benchmarks\Z")
    CASE_NAME_LIMIT = 256
    CASE_LIMIT = 1024


OTHER_CASES = ("other::normal", "other::existing_ignore")
FULL_NAMES = (*base.CASE_NAMES, *OTHER_CASES)
RECORD = (b"op=Claim stage=Return event=Success elapsed_us=None "
          b"utc_remaining_ms=None mono=None kind=None outcome=None\n")
SLOT1 = (b"auth-stage slot=1 (test-only; scheduling perturbed)\n"
         b"lost=false\n")
SLOT2 = (b"auth-stage slot=2 (test-only; scheduling perturbed)\n"
         + RECORD + b"lost=false\n")


def listing(names):
    return ("".join(f"{name}: test\n" for name in names) + "\n"
            + f"{len(names)} tests, 0 benchmarks\n").encode()


def test_output(names, states=None, *, filtered=0, fail_location=True):
    states = dict(states or {})
    lines = [f"running {len(names)} test" + ("s" if len(names) != 1 else "")]
    for name in names:
        state = states.get(name, "ok")
        if state == "ignored_reason":
            lines.append(f"test {name} ... ignored, protocol shape only, not a Nagi skipped test")
        else:
            lines.append(f"test {name} ... {state}")
    failed = [name for name in names if states.get(name) == "FAILED"]
    if failed:
        lines.append("failures:")
        for name in failed:
            if fail_location and name in base.CASE_NAMES:
                source = base.CASE_SOURCE[name]
                source_name = "tests.rs" if source == "tests.rs" else "observer_tests.rs"
                line = 456 if source_name == "tests.rs" else 73
                lines.extend([
                    f"---- {name} stdout ----",
                    f"thread '{name}' (321) panicked at C:\\ci\\repo\\runtime\\src\\auth\\prelogin\\{source_name}:{line}:5:",
                    "assertion failed",
                ])
        lines.append("failures:")
        lines.extend(f"    {name}" for name in failed)
    passed = sum(states.get(name, "ok") == "ok" for name in names)
    ignored = sum(states.get(name, "ok").startswith("ignored") for name in names)
    outcome = "FAILED." if failed else "ok."
    lines.append(
        f"test result: {outcome} {passed} passed; {len(failed)} failed; {ignored} ignored; "
        f"0 measured; {filtered} filtered out; finished in 0.01s"
    )
    return ("\n".join(lines) + "\n").encode()


def write_manifests(root):
    (root / "Cargo.toml").write_text(
        "[workspace]\nmembers = ['runtime']\n\n[workspace.package]\nversion = '0.1.11'\n"
    )
    runtime = root / "runtime" / "Cargo.toml"
    runtime.parent.mkdir(parents=True, exist_ok=True)
    runtime.write_text("[package]\nname = 'nagi-runtime'\nversion.workspace = true\n")
    return runtime


class FullProtocolTests(unittest.TestCase):
    def test_full_inventory_is_exact_bounded_and_contains_the_five_controls(self):
        self.assertEqual(diag._full_inventory(listing(FULL_NAMES), Parser, base.CASE_NAMES), list(FULL_NAMES))
        self.assertEqual(
            diag._full_inventory(listing(FULL_NAMES).replace(b"\n", b"\r\n"), Parser, base.CASE_NAMES),
            list(FULL_NAMES),
        )
        self.assertIsNone(diag._full_inventory(listing(FULL_NAMES[1:]), Parser, base.CASE_NAMES))
        self.assertIsNone(diag._full_inventory(listing((*FULL_NAMES, FULL_NAMES[0])), Parser, base.CASE_NAMES))
        self.assertIsNone(diag._full_inventory(listing(OTHER_CASES), Parser, base.CASE_NAMES))
        self.assertIsNone(diag._full_inventory(listing(FULL_NAMES) + b"unexpected\n", Parser, base.CASE_NAMES))
        self.assertIsNone(diag._full_inventory(b"x" * (diag.LIST_LIMIT + 1), Parser, base.CASE_NAMES))

    def test_two_guard_blocks_allow_empty_records_and_reject_partial_unknown_or_lost(self):
        raw = SLOT1 + SLOT2
        self.assertEqual(diag._full_stage_stderr(raw, Parser), {1: 0, 2: 1})
        self.assertEqual(diag._full_stage_stderr(raw.replace(b"\n", b"\r\n"), Parser), {1: 0, 2: 1})
        for invalid in (
            SLOT2,
            SLOT1 + SLOT1,
            SLOT1 + SLOT2.replace(b"lost=false", b"lost=true"),
            SLOT1 + SLOT2[:-1],
            SLOT1 + b"unknown output\n" + SLOT2,
            SLOT1 + SLOT2 + SLOT1,
            SLOT1 + (b"auth-stage slot=2 (test-only; scheduling perturbed)\n"
                     + RECORD * 65 + b"lost=false\n"),
            b"x" * (diag.FULL_STDERR_LIMIT + 1),
        ):
            self.assertIsNone(diag._full_stage_stderr(invalid, Parser))

    def test_full_profile_sanitizes_reasoned_ignore_and_requires_unfiltered_summary(self):
        raw = test_output(FULL_NAMES, {OTHER_CASES[1]: "ignored_reason"})
        parsed = diag._profile(raw, FULL_NAMES, 0, base, diag.FULL_OUTPUT_LIMIT)
        self.assertIsNotNone(parsed)
        self.assertEqual(parsed["ignored"], 1)
        self.assertEqual(parsed["states"][OTHER_CASES[1]], "ignored")
        self.assertIsNone(diag._profile(
            test_output(FULL_NAMES, {OTHER_CASES[1]: "ignored_reason"}, filtered=1),
            FULL_NAMES, 0, base, diag.FULL_OUTPUT_LIMIT,
        ))
        self.assertIsNone(diag._profile(
            raw.replace(b"ignored, protocol shape only, not a Nagi skipped test", b""),
            FULL_NAMES, 0, base, diag.FULL_OUTPUT_LIMIT,
        ))


class EndToEndFakes(unittest.TestCase):
    def test_one_build_reuses_r6_single_and_five_then_runs_full_once_same_executable(self):
        self._run_one_build()

    def test_executable_change_after_focused_hash_check_refuses_full(self):
        self._run_one_build(swap_after_focused=True)

    def _run_one_build(self, swap_after_focused=False):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            runtime_manifest = write_manifests(root)
            exe = root / "target" / "debug" / "deps" / "nagi_runtime-test.exe"
            exe.parent.mkdir(parents=True)
            exe.write_bytes(b"one stable diagnostic executable")
            exe_hash = hashlib.sha256(exe.read_bytes()).hexdigest()
            artifact = {
                "reason": "compiler-artifact",
                "package_id": "path+file:///repo/runtime#nagi-runtime@0.1.11",
                "manifest_path": str(runtime_manifest),
                "target": {"name": "nagi_runtime", "kind": ["lib"], "test": True},
                "profile": {"test": True},
                "executable": str(exe),
            }
            commands = []

            def fake_capture(command, cwd, env, limit, timeout=base.CAPTURE_TIMEOUT, **_kwargs):
                commands.append((list(command), pathlib.Path(cwd), dict(env), limit, timeout))
                if command[0] == "fake-cargo":
                    return (json.dumps(artifact).encode() + b"\n"
                            + b'{"reason":"build-finished","success":true}\n', b"", 0, False, False)
                if command[-1] == "--list" and len(command) > 2:
                    return listing(base.CASE_NAMES), b"", 0, False, False
                if command[-1] == "--list":
                    return listing(FULL_NAMES), b"", 0, False, False
                if len(command) == 4:
                    return test_output((base.CASE_NAMES[0],), filtered=len(FULL_NAMES) - 1), SLOT2, 0, False, False
                if command[-2:] == ["--exact", "--show-output"]:
                    return test_output(base.CASE_NAMES, filtered=len(FULL_NAMES) - len(base.CASE_NAMES)), SLOT2, 0, False, False
                return test_output(FULL_NAMES, {OTHER_CASES[1]: "ignored_reason"}), SLOT1 + SLOT2, 0, False, False

            original_build_calls = diag._build_calls

            def build_calls(*args):
                result = original_build_calls(*args)
                if swap_after_focused:
                    exe.write_bytes(b"changed between focused and full")
                return result

            stdout = io.StringIO()
            with (mock.patch.object(diag, "_load_r6", return_value=base),
                  mock.patch.object(base, "_validate_source", return_value=True),
                  mock.patch.object(base, "_parser", return_value=Parser),
                  mock.patch.object(base, "capture", side_effect=fake_capture),
                  mock.patch.object(diag, "_build_calls", side_effect=build_calls),
                  mock.patch.dict(base.os.environ, {}, clear=True),
                  contextlib.redirect_stdout(stdout)):
                result = diag.main(["--root", str(root), "--cargo", "fake-cargo"])

            rendered = stdout.getvalue()
            if swap_after_focused:
                self.assertEqual(result, 1)
                self.assertEqual(len(commands), 4)
                self.assertNotIn("full_inventory=pass", rendered)
                self.assertNotIn("same_executable_sha256=", rendered)
                self.assertIn("result=fail stage=artifact", rendered)
                return
            self.assertEqual(result, 0)
            self.assertEqual(sum(command[0][0] == "fake-cargo" for command in commands), 1)
            self.assertEqual(len(commands), 6)
            executable_commands = [command[0] for command in commands if command[0][0] == str(exe)]
            self.assertEqual(executable_commands, [
                [str(exe), *base.CASE_NAMES, "--exact", "--list"],
                [str(exe), base.CASE_NAMES[0], "--exact", "--show-output"],
                [str(exe), *base.CASE_NAMES, "--exact", "--show-output"],
                [str(exe), "--list"],
                [str(exe), "--show-output"],
            ])
            self.assertTrue(all("--test-threads" not in call[0] for call in commands))
            self.assertTrue(all(call[2]["RUSTFLAGS"] == base.RUSTFLAGS for call in commands))
            self.assertTrue(all(call[1] == root / "runtime" for call in commands[1:]))
            self.assertIn(f"same_executable_sha256={exe_hash}", rendered)
            self.assertIn("focused_profile=single", rendered)
            self.assertIn("focused_profile=parallel", rendered)
            self.assertIn("full_inventory=pass tests=7", rendered)
            self.assertIn("full=observed exit=0 tests=7 passed=6 failed=0 ignored=1 filtered=0", rendered)
            self.assertIn("ignored_case=other::existing_ignore", rendered)
            self.assertIn("result=pass_with_ignored", rendered)
            self.assertNotIn("protocol shape only", rendered)

    def test_full_failure_keeps_exit_nonzero_and_projects_known_source_only(self):
        # The source position parser is hash-bound by the reused R6 module.
        failure = test_output(
            FULL_NAMES,
            {base.CASE_NAMES[0]: "FAILED", OTHER_CASES[1]: "ignored_reason"},
        )
        profile = diag._profile(failure, FULL_NAMES, 0, base, diag.FULL_OUTPUT_LIMIT)
        self.assertIsNotNone(profile)
        locations, malformed = base._panic_locations(profile["panics"], (base.CASE_NAMES[0],))
        self.assertFalse(malformed)
        self.assertEqual(locations[base.CASE_NAMES[0]], ("tests", 456, 5))

    def test_unknown_guard_stderr_does_not_suppress_valid_failure_names_or_source(self):
        known = base.CASE_NAMES[0]
        profile = diag._profile(
            test_output(FULL_NAMES, {known: "FAILED"}), FULL_NAMES, 0, base, diag.FULL_OUTPUT_LIMIT
        )
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            success, ignored = diag._emit_full_evidence(
                profile, FULL_NAMES, 101, True, None, True, base
            )
        rendered = output.getvalue()
        self.assertFalse(success)
        self.assertEqual(ignored, [])
        self.assertIn("stdout_valid=true stderr_valid=false", rendered)
        self.assertIn(f"failed_case={known}", rendered)
        self.assertIn(f"known_case={known} state=fail source=tests line=456 column=5", rendered)
        self.assertNotIn("panicked at", rendered)


class WorkflowTests(unittest.TestCase):
    def test_workflow_is_create_only_for_exact_private_branch(self):
        workflow = pathlib.Path(__file__).resolve().parents[2] / ".github/workflows/session-windows-full-lib-diagnostic.yml"
        text = workflow.read_text(encoding="utf-8")
        self.assertRegex(text, r"(?m)^on:\n  create:\s*$")
        for event in ("push:", "pull_request:", "workflow_dispatch:", "schedule:"):
            self.assertNotIn(event, text)
        branch = "diag/session-windows-full-lib-20261018"
        self.assertIn(f"github.event.ref == '{branch}'", text)
        self.assertIn(f"github.ref == 'refs/heads/{branch}'", text)


if __name__ == "__main__":
    unittest.main()
