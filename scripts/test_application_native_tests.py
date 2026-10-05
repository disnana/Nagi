"""Keep native test execution tied to the selected build generation."""
import tempfile
from pathlib import Path
import unittest
from unittest.mock import patch

import verify_application_examples as applications


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
