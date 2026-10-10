"""Keep native test execution tied to the selected build generation."""
import tempfile
import importlib.util
import json
import sys
from pathlib import Path
import unittest
from unittest.mock import patch

import verify_application_examples as applications


SPEC = importlib.util.spec_from_file_location(
    "quote_smoke", applications.PROJECTS / "quote-api/smoke.py",
)
quote_smoke = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(quote_smoke)


class ApplicationFailureEvidenceTests(unittest.TestCase):
    def test_receive_failure_records_live_child_before_cleanup_and_propagates(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            failure = ConnectionAbortedError("owned fixture receive failed")
            child = unittest.mock.Mock(pid=123, returncode=None)
            child.poll.return_value = None

            def stop(**kwargs):
                child.poll.return_value = 0
                child.returncode = 0

            child.wait.side_effect = stop

            def start(*args, **kwargs):
                # Popen passes file descriptors; children emit raw bytes without
                # the parent's TextIOWrapper translating LF to CRLF on Windows.
                kwargs["stdout"].buffer.write(b"owned child stdout\n")
                kwargs["stderr"].buffer.write(b"owned child stderr\n")
                return child

            ready = unittest.mock.Mock(status=200)
            ready.getheaders.return_value = []
            ready.read.return_value = b""
            connection = unittest.mock.Mock()
            connection.getresponse.side_effect = [ready, failure]
            with patch.object(quote_smoke.subprocess, "Popen", side_effect=start), \
                    patch.object(quote_smoke.http.client, "HTTPConnection", return_value=connection):
                with self.assertRaises(ConnectionAbortedError) as raised:
                    quote_smoke.verify(directory / "unused-executable", {}, directory)
            self.assertIs(raised.exception, failure)
            child.wait.assert_called_once_with(timeout=5)
            report = json.loads((directory / "smoke-failure.json").read_text(encoding="utf-8"))
            self.assertIsNone(report["child_returncode_before_cleanup"])
            self.assertEqual(report["request"]["case"], "health")
            self.assertEqual(report["request"]["stage"], "response-status-and-headers")
            self.assertEqual(report["exception_type"], "ConnectionAbortedError")
            self.assertEqual(report["stdout"]["tail"], "owned child stdout\n")
            self.assertEqual(report["stderr"]["tail"], "owned child stderr\n")
            self.assertEqual(report["stdout"]["bytes"], len(b"owned child stdout\n"))
            self.assertEqual(report["stderr"]["bytes"], len(b"owned child stderr\n"))
            self.assertFalse(report["stdout"]["truncated"])

    def test_log_capture_preserves_child_lf_and_crlf_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "child.stdout"
            for payload in (b"owned child\n", b"owned child\r\n"):
                with self.subTest(payload=payload):
                    path.write_bytes(payload)
                    report = quote_smoke.failure_log(path)
                    self.assertEqual(report["tail"], payload.decode("utf-8"))
                    self.assertEqual(report["bytes"], len(payload))
                    self.assertFalse(report["truncated"])

    def test_log_capture_bounds_bytes_and_marks_truncation(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "child.stdout"
            path.write_bytes(b"prefix" + b"x" * quote_smoke.FAILURE_LOG_BYTES)
            report = quote_smoke.failure_log(path)
            self.assertEqual(report["bytes"], 6 + quote_smoke.FAILURE_LOG_BYTES)
            self.assertTrue(report["truncated"])
            self.assertEqual(report["tail"], "x" * quote_smoke.FAILURE_LOG_BYTES)

    def test_main_retains_smoke_failure_in_existing_ci_artifact_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            failure_directory = root / "compiler-failures"
            failure = RuntimeError("owned fixture failed")
            details = {"child_returncode_before_cleanup": None, "stdout": {"tail": "child output"}}

            def verify(executable, env, directory):
                directory.mkdir(parents=True, exist_ok=True)
                (directory / "smoke-failure.json").write_text(json.dumps(details), encoding="utf-8")
                raise failure

            with patch.object(applications, "ROOT", root), \
                    patch.object(applications, "verifier", return_value=verify), \
                    patch.object(applications, "run"), \
                    patch.object(applications, "native_executable", return_value=root / "app"), \
                    patch.object(applications.shutil, "copy2"), \
                    patch.dict(applications.os.environ, {"NAGI_FAILURE_DIR": str(failure_directory)}), \
                    patch.object(sys, "argv", ["verify_application_examples.py", "--only", "quote-api"]):
                with self.assertRaises(RuntimeError) as raised:
                    applications.main()
            self.assertIs(raised.exception, failure)
            report = json.loads((failure_directory / "application-quote-api-high.json").read_text(encoding="utf-8"))
            self.assertEqual(report["project"], "quote-api")
            self.assertEqual(report["source"], "high")
            self.assertEqual(report["exception_type"], "RuntimeError")
            self.assertIn("owned fixture failed", report["traceback"])
            self.assertEqual(report["smoke"], details)

    def test_capture_io_failure_does_not_replace_original_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            blocked = root / "not-a-directory"
            blocked.write_text("owned fixture")
            failure = ConnectionResetError("owned fixture reset")
            with self.assertRaises(ConnectionResetError) as raised:
                applications.verify_smoke(
                    unittest.mock.Mock(side_effect=failure), root / "app",
                    {"NAGI_FAILURE_DIR": str(blocked)}, root, "quote-api", "low",
                )
            self.assertIs(raised.exception, failure)
            self.assertIn("Unable to save application failure evidence", failure.__notes__[0])


class AxumNativeTests(unittest.TestCase):
    def setUp(self):
        self.fixture = tempfile.TemporaryDirectory()
        self.addCleanup(self.fixture.cleanup)
        self.root = Path(self.fixture.name)
        self.generation = self.root / "generations" / "g-accepted"
        self.generation.mkdir(parents=True)
        self.published = self.generation / "app-generation"
        self.published.touch()
        self.manifest = self.generation / "Cargo.toml"
        self.manifest.write_text('[[bin]]\nname = "app-generation"\n', encoding="utf-8")
        self.target = self.root / "dependency-cache"
        self.log = self.root / "native-tests.log"

    def verify(self, output):
        def run(arguments, env, log):
            self.arguments = arguments
            self.environment = env
            log.write_text(output, encoding="utf-8")
        with patch.object(applications, "run", side_effect=run):
            return applications.verify_axum_native_tests(
                self.published, self.target, {"TEST_MARKER": "yes"}, self.log,
            )

    def test_uses_successful_generation_manifest_and_actual_bin(self):
        (self.root / "Cargo.toml").write_text('[[bin]]\nname = "unrelated"\n', encoding="utf-8")
        result = self.verify("test result: ok. 8 passed; 0 failed; 0 ignored; 0 filtered out\n")
        self.assertEqual(result, {"passed": 8, "failed": 0, "ignored": 0})
        self.assertEqual(self.arguments, [
            "cargo", "test", "--locked", "--release", "--manifest-path", self.manifest,
            "--target-dir", self.target, "--bin", "app-generation", "native::tests::",
            "--", "--nocapture",
        ])
        self.assertEqual(self.environment, {"TEST_MARKER": "yes"})

    def test_missing_generation_manifest_is_not_replaced_by_projection(self):
        self.manifest.unlink()
        (self.root / "Cargo.toml").write_text('[[bin]]\nname = "unrelated"\n', encoding="utf-8")
        with self.assertRaises(FileNotFoundError):
            self.verify("test result: ok. 8 passed; 0 failed; 0 ignored;\n")

    def test_rejects_zero_ignored_failed_missing_and_ambiguous_results(self):
        outputs = (
            "test result: ok. 0 passed; 0 failed; 0 ignored;\n",
            "test result: ok. 7 passed; 0 failed; 1 ignored;\n",
            "test result: ok. 7 passed; 1 failed; 0 ignored;\n",
            "compiler exited without a test summary\n",
            "test result: ok. 8 passed; 0 failed; 0 ignored;\n" * 2,
        )
        for output in outputs:
            with self.subTest(output=output), self.assertRaises(RuntimeError):
                self.verify(output)

    def test_command_failure_propagates(self):
        with patch.object(applications, "run", side_effect=RuntimeError("cargo failed")):
            with self.assertRaisesRegex(RuntimeError, "cargo failed"):
                applications.verify_axum_native_tests(
                    self.published, self.target, {}, self.log,
                )

    def test_rejects_missing_or_multiple_bins(self):
        for content in ('[package]\nname = "no-bin"\n', '[[bin]]\nname = "a"\n[[bin]]\nname = "b"\n'):
            self.manifest.write_text(content, encoding="utf-8")
            with self.subTest(content=content), self.assertRaises(ValueError):
                self.verify("test result: ok. 8 passed; 0 failed; 0 ignored;\n")


if __name__ == "__main__":
    unittest.main()
