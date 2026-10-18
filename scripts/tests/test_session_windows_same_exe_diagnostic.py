"""Focused protocol and subprocess fakes for the Windows diagnostic runner."""
import contextlib
import hashlib
import io
import json
import pathlib
import re
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

from scripts.ci import session_windows_same_exe_diagnostic as diag


class Parser:
    CASE_ENTRY = re.compile(r"([A-Za-z_][A-Za-z0-9_:]*): test\Z")
    CASE_FOOTER = re.compile(r"([0-9]{1,4}) tests, 0 benchmarks\Z")
    CASE_NAME_LIMIT = 256
    CASE_LIMIT = 1024


def inventory(names=diag.CASE_NAMES):
    return ("".join(f"{name}: test\n" for name in names) + "\n"
            + f"{len(names)} tests, 0 benchmarks\n").encode()


def test_output(names, failing=(), *, total=10, taint=""):
    failing = set(failing)
    lines = [f"running {len(names)} test" + ("s" if len(names) != 1 else "")]
    for name in names:
        lines.append(f"test {name} ... {'FAILED' if name in failing else 'ok'}")
    if failing:
        lines.extend(["failures:"])
        for name in names:
            if name in failing:
                source = diag.CASE_SOURCE[name]
                source_line = 73 if source == "observer_tests.rs" else 456
                lines.extend([f"---- {name} stdout ----",
                              f"thread '{name}' (1234) panicked at C:\\runner\\repo\\runtime\\src\\auth\\prelogin\\{source}:{source_line}:5:",
                              taint or "assertion failed"])
        lines.append("failures:")
        lines.extend(f"    {name}" for name in names if name in failing)
    passed = len(names) - len(failing)
    lines.append(
        f"test result: {'FAILED.' if failing else 'ok.'} {passed} passed; "
        f"{len(failing)} failed; 0 ignored; 0 measured; {total-len(names)} filtered out; "
        "finished in 0.01s"
    )
    return ("\n".join(lines) + "\n").encode()


def write_runtime_manifests(root, workspace_version="0.1.11"):
    (root / "Cargo.toml").write_text(
        "[workspace]\nmembers = ['runtime']\n\n"
        f"[workspace.package]\nversion = '{workspace_version}'\n"
    )
    package_manifest = root / "runtime" / "Cargo.toml"
    package_manifest.parent.mkdir(parents=True, exist_ok=True)
    package_manifest.write_text(
        "[package]\nname = 'nagi-runtime'\nversion.workspace = true\n"
    )
    return package_manifest


class CaptureTests(unittest.TestCase):
    def test_python_child_capture_and_oversize_are_bounded(self):
        with tempfile.TemporaryDirectory() as temp:
            env = dict(__import__("os").environ)
            out = diag.capture(
                [sys.executable, "-c", "import sys; sys.stdout.buffer.write(b'bounded-child\\n')"], temp, env, 128, timeout=5
            )
            self.assertEqual((out[0], out[2], out[3], out[4]), (b"bounded-child\n", 0, False, False))
            big = diag.capture(
                [sys.executable, "-c", "import sys; sys.stdout.write('x' * 200)"],
                temp, env, 64, timeout=5,
            )
            self.assertEqual(len(big[0]), 65)
            self.assertTrue(big[3])

    def test_windows_timeout_uses_tree_kill_and_taskkill_shape(self):
        calls = []

        class Process:
            pid = 4321
            stdout = io.BytesIO(b"")
            stderr = io.BytesIO(b"")
            waits = 0

            def wait(self, timeout=None):
                self.waits += 1
                if self.waits == 1:
                    raise subprocess.TimeoutExpired("fake", timeout)
                return -9

        def popen(_command, **_kwargs):
            return Process()

        def kill(pid, platform):
            calls.append((pid, platform))

        got = diag.capture(["fake"], ".", {}, 64, timeout=0.001, platform="nt",
                           popen=popen, kill_tree=kill)
        self.assertEqual(calls, [(4321, "nt")])
        self.assertTrue(got[4])
        self.assertEqual(got[2], -9)
        commands = []

        def run(command, **kwargs):
            commands.append((command, kwargs))

        diag._kill_tree(4321, "nt", run=run)
        self.assertEqual(commands[0][0], ["taskkill", "/PID", "4321", "/T", "/F"])


class CollectorTests(unittest.TestCase):
    def test_source_identity_accepts_git_lf_or_crlf_but_binary_hash_stays_raw(self):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            relative = "runtime/src/auth/prelogin/tests.rs"
            source = root / relative
            source.parent.mkdir(parents=True)
            canonical = "# checked source\nname = 'Nagi'\n".encode("utf-8")
            expected = hashlib.sha256(canonical).hexdigest()
            with mock.patch.dict(diag.SOURCE_BINDINGS,
                                 {"tests.rs": (relative, expected)}, clear=True):
                source.write_bytes(canonical)
                self.assertTrue(diag._validate_source(root))
                source.write_bytes(canonical.replace(b"\n", b"\r\n"))
                self.assertTrue(diag._validate_source(root))
                self.assertNotEqual(diag.file_sha256(source), expected)
                for invalid in (canonical + b"x", b"bare\rreturn\n", b"\xff",
                                b"x" * (64 * 1024 + 1)):
                    source.write_bytes(invalid)
                    self.assertFalse(diag._validate_source(root))

    def test_alias_ancestry_accepts_only_canonical_target_without_skipping_links(self):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp).resolve()
            target = root / "target"
            executable = target / "debug" / "deps" / "owned.exe"
            executable.parent.mkdir(parents=True)
            executable.write_bytes(b"owned alias fixture")
            chain = (executable, executable.parent, executable.parent.parent, target)
            digest = hashlib.sha256(executable.read_bytes()).hexdigest()

            class Alias:
                # Model a Windows short-name spelling whose resolved paths are
                # identical but whose lexical spelling differs from root.resolve().
                def __init__(self, index=0, link=None, outside=False):
                    self.index, self.link, self.outside = index, link, outside

                def resolve(self, strict=True):
                    return root / "outside.exe" if self.outside else chain[self.index]

                def is_symlink(self):
                    return self.index == self.link

                @property
                def parent(self):
                    return Alias(min(3, self.index + 1), self.link, self.outside)

                def __eq__(self, other):
                    return isinstance(other, Alias) and self.index == other.index

            with mock.patch.object(diag, "file_sha256", return_value=digest):
                self.assertEqual(diag._bound_executable_sha256(Alias(), root), digest)
                self.assertIsNone(diag._bound_executable_sha256(Alias(link=3), root))
                self.assertIsNone(diag._bound_executable_sha256(Alias(outside=True), root))

    def test_exact_five_case_inventory_and_rejects_mutants(self):
        self.assertEqual(diag._inventory(inventory(), Parser), list(diag.CASE_NAMES))
        tiny_libtest_shape = (
            b"alpha_case: test\nbeta_case: test\ngamma_case: test\n\n"
            b"3 tests, 0 benchmarks\n"
        )
        self.assertEqual(
            diag._inventory(tiny_libtest_shape, Parser, ("alpha_case", "beta_case", "gamma_case")),
            ["alpha_case", "beta_case", "gamma_case"],
        )
        self.assertIsNone(diag._inventory(inventory(diag.CASE_NAMES[:-1]), Parser))
        duplicate = inventory(diag.CASE_NAMES[:-1] + (diag.CASE_NAMES[0],))
        self.assertIsNone(diag._inventory(duplicate, Parser))
        self.assertIsNone(diag._inventory(inventory() + b"secret token\n", Parser))
        self.assertIsNone(diag._inventory(inventory()[:-1], Parser))
        self.assertIsNone(diag._inventory(b"0 tests, 0 benchmarks\n", Parser))
        self.assertIsNone(diag._inventory(b"x" * (diag.TEST_LIMIT + 1), Parser))

    def test_profiles_require_exact_rows_and_summary(self):
        single = test_output((diag.CASE_NAMES[0],))
        parsed = diag._test_profile(single, (diag.CASE_NAMES[0],))
        self.assertIsNotNone(parsed)
        self.assertEqual(parsed["states"], {diag.CASE_NAMES[0]: "ok"})
        self.assertIsNone(diag._test_profile(
            single + f"test {diag.CASE_NAMES[0]} ... ok\n".encode(), (diag.CASE_NAMES[0],)
        ))
        self.assertIsNone(diag._test_profile(
            single.replace(b"running 1 test", b"running 0 tests"),
            (diag.CASE_NAMES[0],),
        ))
        self.assertIsNone(diag._test_profile(
            single.replace(b"0 measured", b"1 measured"),
            (diag.CASE_NAMES[0],),
        ))
        five = diag._test_profile(test_output(diag.CASE_NAMES), diag.CASE_NAMES)
        self.assertEqual(len(five["states"]), 5)

    def test_failure_location_is_source_hash_bound_range_and_closed(self):
        case = diag.CASE_NAMES[0]
        line = (f"thread '{case}' panicked at "
                r"C:\runner\repo\runtime\src\auth\prelogin\tests.rs:456:5:")
        locations, invalid = diag._panic_locations([line], (case,))
        self.assertFalse(invalid)
        self.assertEqual(locations[case], ("tests", 456, 5))
        with_thread_id = line.replace(f"thread '{case}'", f"thread '{case}' (1234)")
        locations, invalid = diag._panic_locations([with_thread_id], (case,))
        self.assertFalse(invalid)
        self.assertEqual(locations[case], ("tests", 456, 5))
        unknown = line.replace("tests.rs:456", "unknown.rs:456")
        locations, invalid = diag._panic_locations([unknown], (case,))
        self.assertTrue(invalid)
        self.assertIsNone(locations[case])
        out_of_range = line.replace(":456:5:", ":900:5:")
        locations, invalid = diag._panic_locations([out_of_range], (case,))
        self.assertTrue(invalid)
        self.assertIsNone(locations[case])

    def test_cargo_artifact_requires_exact_package_manifest_finished_target(self):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            manifest = write_runtime_manifests(root)
            executable = root / "target" / "debug" / "deps" / "nagi_runtime.exe"
            executable.parent.mkdir(parents=True)
            executable.write_bytes(b"bound executable")
            artifact = {
                "reason": "compiler-artifact",
                "package_id": "path+file:///repo/runtime#nagi-runtime@0.1.11",
                "manifest_path": str(manifest),
                "target": {"name": "nagi_runtime", "kind": ["lib"], "test": True},
                "profile": {"test": True},
                "executable": str(executable),
            }

            def output(message=artifact, finish=True):
                return (json.dumps(message) + "\n"
                        + json.dumps({"reason": "build-finished", "success": finish}) + "\n").encode()

            self.assertEqual(diag._cargo_artifact(output(), root), executable)
            wrong_version = dict(artifact, package_id="path+file:///repo/runtime#nagi-runtime@0.2.0")
            self.assertIsNone(diag._cargo_artifact(output(wrong_version), root))
            wrong_manifest = dict(artifact, manifest_path=str(root / "other" / "Cargo.toml"))
            self.assertIsNone(diag._cargo_artifact(output(wrong_manifest), root))
            wrong_executable = dict(artifact, executable=str(root / "outside.exe"))
            self.assertIsNone(diag._cargo_artifact(output(wrong_executable), root))
            self.assertIsNone(diag._cargo_artifact(output(artifact, finish=False), root))
            missing_finish = json.dumps(artifact).encode() + b"\n"
            self.assertIsNone(diag._cargo_artifact(missing_finish, root))
            duplicate_finish = output() + b'{"reason":"build-finished","success":true}\n'
            self.assertIsNone(diag._cargo_artifact(duplicate_finish, root))

    def test_main_fake_success_uses_one_bound_executable_and_default_parallelism(self):
        with tempfile.TemporaryDirectory() as temp:
            base = pathlib.Path(temp)
            (base / "alias-component").mkdir()
            root = base / "alias-component" / ".."
            write_runtime_manifests(root)
            exe = root / "target" / "debug" / "deps" / "fake-test.exe"
            exe.parent.mkdir(parents=True)
            exe.write_bytes(b"same executable for all profiles")
            expected_hash = hashlib.sha256(exe.read_bytes()).hexdigest()
            artifact = {
                "reason": "compiler-artifact",
                "package_id": "path+file:///repo/runtime#nagi-runtime@0.1.11",
                "manifest_path": str(root / "runtime" / "Cargo.toml"),
                "target": {"name": "nagi_runtime", "kind": ["lib"], "test": True},
                "profile": {"test": True},
                "executable": str(exe),
            }
            calls = []

            def fake_capture(command, cwd, env, limit):
                calls.append((command, pathlib.Path(cwd), dict(env), limit))
                if command[0] == "fake-cargo":
                    return (json.dumps(artifact).encode() + b"\n"
                            + b'{"reason":"build-finished","success":true}\n',
                            b"", 0, False, False)
                if command[-1] == "--list":
                    return inventory(), b"", 0, False, False
                names = (command[1],) if len(command) == 4 else tuple(command[1:-2])
                return test_output(names), b"", 0, False, False

            output = io.StringIO()
            with (mock.patch.object(diag, "_validate_source", return_value=True),
                  mock.patch.object(diag, "_parser", return_value=Parser),
                  mock.patch.object(diag, "capture", side_effect=fake_capture),
                  contextlib.redirect_stdout(output)):
                result = diag.main(["--root", str(root), "--cargo", "fake-cargo"])
            commands = [call[0] for call in calls]
            self.assertEqual(result, 0)
            self.assertEqual(len(commands), 4)
            self.assertEqual(commands[0][1:], [
                "test", "--locked", "-p", "nagi-runtime", "--lib", "--no-run",
                "--message-format=json",
            ])
            self.assertEqual(commands[1], [str(exe), *diag.CASE_NAMES, "--exact", "--list"])
            self.assertEqual(commands[2], [str(exe), diag.CASE_NAMES[0], "--exact", "--show-output"])
            self.assertEqual(commands[3], [str(exe), *diag.CASE_NAMES, "--exact", "--show-output"])
            self.assertTrue(all("--test-threads" not in command for command in commands))
            self.assertTrue(all(call[2]["RUSTFLAGS"] == diag.RUSTFLAGS for call in calls))
            self.assertTrue(all("CARGO_ENCODED_RUSTFLAGS" not in call[2] for call in calls))
            self.assertTrue(all(call[1] == root.resolve() / "runtime" for call in calls[1:]))
            rendered = output.getvalue()
            self.assertIn(f"executable_sha256={expected_hash}", rendered)
            self.assertIn("profile=single state=pass exit=0 selected=1", rendered)
            self.assertIn("profile=parallel state=pass exit=0 selected=5", rendered)
            self.assertEqual(rendered.count("result=pass\n"), 1)

    def test_main_fake_build_same_artifact_and_closed_nonzero_projection(self):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            write_runtime_manifests(root)
            exe = root / "target" / "debug" / "deps" / "fake-test.exe"
            exe.parent.mkdir(parents=True)
            exe.write_bytes(b"immutable fake executable")
            artifact = {
                "reason": "compiler-artifact",
                "package_id": "path+file:///repo/runtime#nagi-runtime@0.1.11",
                "manifest_path": str(root / "runtime" / "Cargo.toml"),
                "target": {"name": "nagi_runtime", "kind": ["lib"], "test": True},
                "profile": {"test": True},
                "executable": str(exe),
            }
            calls = []

            def fake_capture(command, cwd, env, limit):
                calls.append((command, pathlib.Path(cwd), env["RUSTFLAGS"], limit))
                if command[0] == "fake-cargo":
                    return (json.dumps(artifact).encode() + b"\n"
                            + b'{"reason":"build-finished","success":true}\n',
                            b"", 0, False, False)
                if command[-1] == "--list":
                    return inventory(), b"", 0, False, False
                if len(command) == 4:
                    return test_output((diag.CASE_NAMES[0],)), b"", 0, False, False
                return test_output(diag.CASE_NAMES, failing=(diag.CASE_NAMES[1],),
                                   taint="owned-secret-value"), b"", 101, False, False

            output = io.StringIO()
            with (mock.patch.object(diag, "_validate_source", return_value=True),
                  mock.patch.object(diag, "_parser", return_value=Parser),
                  mock.patch.object(diag, "capture", side_effect=fake_capture),
                  contextlib.redirect_stdout(output)):
                result = diag.main(["--root", str(root), "--cargo", "fake-cargo"])
            rendered = output.getvalue()
            self.assertEqual(result, 1)
            self.assertEqual(len(calls), 4)
            self.assertEqual(calls[0][2], diag.RUSTFLAGS)
            self.assertIn("cfg=nagi_auth_stage_diag+nagi_browser_login_diag", rendered)
            self.assertIn(
                f"case={diag.CASE_NAMES[1]} state=fail source=observer_tests line=73 column=5",
                rendered,
            )
            self.assertIn("result=fail", rendered)
            self.assertNotIn("owned-secret-value", rendered)
            self.assertNotIn("runtime/src/auth/prelogin/tests.rs", rendered)

    def test_main_stops_before_tests_when_list_or_build_is_invalid(self):
        for stage in ("build", "list"):
            with self.subTest(stage=stage), tempfile.TemporaryDirectory() as temp:
                root = pathlib.Path(temp)
                write_runtime_manifests(root)
                (root / "target").mkdir()
                exe = root / "target" / "fake.exe"
                exe.write_bytes(b"stable")
                artifact = {
                    "reason": "compiler-artifact",
                    "package_id": "path+file:///repo/runtime#nagi-runtime@0.1.11",
                    "manifest_path": str(root / "runtime" / "Cargo.toml"),
                    "target": {"name": "nagi_runtime", "kind": ["lib"], "test": True},
                    "profile": {"test": True},
                    "executable": str(exe),
                }
                calls = []

                def fake_capture(command, _cwd, _env, _limit):
                    calls.append(command)
                    if command[0] == "fake-cargo":
                        if stage == "build":
                            return b"untrusted cargo output", b"", 1, False, False
                        return (json.dumps(artifact).encode() + b"\n"
                                + b'{"reason":"build-finished","success":true}\n',
                                b"", 0, False, False)
                    return b"not a valid inventory\n", b"", 0, False, False

                with (mock.patch.object(diag, "_validate_source", return_value=True),
                      mock.patch.object(diag, "_parser", return_value=Parser),
                      mock.patch.object(diag, "capture", side_effect=fake_capture),
                      contextlib.redirect_stdout(io.StringIO())):
                    result = diag.main(["--root", str(root), "--cargo", "fake-cargo"])
                self.assertEqual(result, 1)
                self.assertEqual(len(calls), 1 if stage == "build" else 2)

    def test_main_rejects_overrides_and_sanitizes_spawn_errors(self):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            (root / "runtime").mkdir()
            output = io.StringIO()
            with (mock.patch.object(diag, "_validate_source", return_value=True),
                  mock.patch.object(diag, "_parser", return_value=Parser),
                  mock.patch.dict(diag.os.environ, {"RUST_TEST_THREADS": "7"}),
                  contextlib.redirect_stdout(output)):
                result = diag.main(["--root", str(root), "--cargo", "fake-cargo"])
            self.assertEqual(result, 2)
            self.assertEqual(output.getvalue(), "result=unavailable stage=environment\n")

        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            (root / "runtime").mkdir()
            output = io.StringIO()
            with (mock.patch.object(diag, "_validate_source", return_value=True),
                  mock.patch.object(diag, "_parser", return_value=Parser),
                  mock.patch.object(diag, "capture", side_effect=OSError("owned-secret-error")),
                  contextlib.redirect_stdout(output)):
                result = diag.main(["--root", str(root), "--cargo", "fake-cargo"])
            self.assertEqual(result, 2)
            self.assertEqual(output.getvalue(), "result=unavailable stage=internal\n")
            self.assertNotIn("owned-secret-error", output.getvalue())

    def test_workflow_is_create_only_for_the_fixed_windows_branch(self):
        workflow = pathlib.Path(__file__).resolve().parents[2] / ".github/workflows/session-windows-same-exe-diagnostic.yml"
        text = workflow.read_text(encoding="utf-8")
        self.assertRegex(text, r"(?m)^on:\n  create:\s*$")
        for event in ("push:", "pull_request:", "workflow_dispatch:", "schedule:"):
            self.assertNotIn(event, text)
        self.assertIn("diag/session-windows-executable-r5-20261018", text)
        self.assertIn("github.event.ref_type == 'branch'", text)
        self.assertIn("github.ref == 'refs/heads/diag/session-windows-executable-r5-20261018'", text)


if __name__ == "__main__":
    unittest.main()
