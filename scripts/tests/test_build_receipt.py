import copy
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("verify_build_receipt", Path(__file__).resolve().parents[1] / "verify_build_receipt.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ReceiptTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.app = "nagi-main-0123456789abcdef"
        self.generation = "g-1-2-3"
        self.root = Path(self.temporary.name) / self.app / "generations" / self.generation
        self.root.mkdir(parents=True)
        self.executable = "nagi-0123456789abcdef-g-1-2-3"
        self.compiler = Path(self.temporary.name) / "compiler"
        self.compiler.write_bytes(b"compiler")
        self.receipt = {"schema_version": 1, "app_id": self.app, "generation": self.generation,
                        "compiler": MODULE.identity(self.compiler), "artifacts": {}}
        for name in MODULE.FIXED | {self.executable}:
            path = self.root / name
            path.parent.mkdir(exist_ok=True)
            path.write_bytes(name.encode())
            self.receipt["artifacts"][name] = MODULE.identity(path)
        self.path = self.root / "build-receipt.json"
        self.write()

    def write(self):
        self.path.write_text(json.dumps(self.receipt), encoding="utf-8")

    def test_valid_and_corrupt_bytes(self):
        MODULE.verify(self.path, self.compiler)
        (self.root / "src/main.rs").write_bytes(b"wrong")
        with self.assertRaisesRegex(ValueError, "integrity mismatch"):
            MODULE.verify(self.path)

    def test_metadata_cannot_select_arbitrary_paths(self):
        initial = copy.deepcopy(self.receipt)
        for key, value in [("schema_version", True), ("schema_version", 2),
                           ("generation", "../g-1-2-3"), ("app_id", "other")]:
            self.receipt = copy.deepcopy(initial)
            self.receipt[key] = value
            self.write()
            with self.assertRaises(ValueError):
                MODULE.verify(self.path)
        self.receipt = initial
        self.receipt["artifacts"]["../../compiler"] = MODULE.identity(self.compiler)
        self.write()
        with self.assertRaises(ValueError):
            MODULE.verify(self.path)

    def test_missing_truncated_and_malformed(self):
        (self.root / self.executable).unlink()
        with self.assertRaises(OSError):
            MODULE.verify(self.path)
        for text in ["{", "[]", "null", "x" * (1024 * 1024 + 1)]:
            self.path.write_text(text)
            with self.assertRaises(ValueError):
                MODULE.verify(self.path)

    def test_invalid_digest_length_boolean_and_compiler(self):
        initial = copy.deepcopy(self.receipt)
        for field, value in [("sha256", "x" * 64), ("bytes", True), ("bytes", -1)]:
            self.receipt = copy.deepcopy(initial)
            self.receipt["artifacts"][self.executable][field] = value
            self.write()
            with self.assertRaises(ValueError):
                MODULE.verify(self.path)
        self.receipt = initial
        self.write()
        self.compiler.write_bytes(b"other compiler")
        with self.assertRaisesRegex(ValueError, "compiler"):
            MODULE.verify(self.path, self.compiler)

    def test_symlink_artifact_is_rejected(self):
        path = self.root / self.executable
        path.unlink()
        try:
            path.symlink_to(self.compiler)
        except OSError as error:
            self.skipTest(f"OS does not permit test symlinks: {error}")
        with self.assertRaisesRegex(ValueError, "symbolic link"):
            MODULE.verify(self.path)

    def test_directory_artifact_is_rejected(self):
        path = self.root / self.executable
        path.unlink()
        path.mkdir()
        with self.assertRaisesRegex(ValueError, "not a regular file"):
            MODULE.verify(self.path)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "Unix named pipes")
    def test_named_pipe_receipt_or_artifact_cannot_block_cli(self):
        for path in (self.path, self.root / self.executable):
            original = path.read_bytes()
            path.unlink()
            os.mkfifo(path)
            result = subprocess.run([sys.executable, str(SPEC.origin), str(self.path)],
                                    capture_output=True, text=True, timeout=5)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("not a regular file", result.stderr)
            path.unlink()
            path.write_bytes(original)

    def test_compiler_command_link_is_supported_but_source_directory_link_is_rejected(self):
        compiler_link = self.compiler.with_name("compiler-command")
        try:
            compiler_link.symlink_to(self.compiler)
        except OSError as error:
            self.skipTest(f"OS does not permit test symlinks: {error}")
        MODULE.verify(self.path, compiler_link)
        source = self.root / "src"
        moved = self.root / "original-src"
        source.rename(moved)
        source.symlink_to(moved, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symbolic link"):
            MODULE.verify(self.path)

    @unittest.skipUnless(os.name == "nt", "Windows directory junctions")
    def test_windows_junction_is_rejected_without_reading_target(self):
        source = self.root / "src"
        moved = self.root / "original-src"
        source.rename(moved)
        result = subprocess.run(["cmd", "/c", "mklink", "/J", str(source), str(moved)],
                                capture_output=True, text=True, timeout=5)
        self.assertEqual(result.returncode, 0, result.stderr)
        try:
            with self.assertRaisesRegex(ValueError, "reparse point"):
                MODULE.verify(self.path)
        finally:
            source.rmdir()


if __name__ == "__main__":
    unittest.main()
