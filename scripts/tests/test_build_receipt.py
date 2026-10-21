import copy
import hashlib
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

    def configure_schema2(self):
        self.receipt["schema_version"] = 2
        body_manifest_bytes = b'[package]\nname = "nagi-body-test"\nversion = "0.1.0"\n'
        body_main_bytes = b"pub fn generated_body() {}\n"
        for name, contents in [("body-Cargo.toml", body_manifest_bytes),
                               ("src/entry.rs", b"fn main() {}\n"),
                               ("src/main.rs", body_main_bytes)]:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(contents)
            self.receipt["artifacts"][name] = MODULE.identity(path)

        framed = hashlib.sha256()
        framed.update(b"nagi-generated-body-v1\0")
        for contents in (body_manifest_bytes, body_main_bytes):
            framed.update(len(contents).to_bytes(8, "big"))
            framed.update(contents)
        digest = framed.hexdigest()
        self.receipt["generated_body_sha256"] = digest
        app_suffix = self.app.rsplit("-", 1)[-1]
        cargo = ("[[bin]]\nname = \"nagi-app-test\"\npath = \"src/entry.rs\"\n\n"
                 "[dependencies.nagi-runtime]\npath = \"../../code/nagi-runtime\"\n\n"
                 "[package]\nautobins = false\nedition = \"2021\"\n"
                 "name = \"nagi-app-test\"\nversion = \"0.1.0\"\n\n"
                 "[profile.release]\ncodegen-units = 1\nlto = false\n"
                 "opt-level = 3\npanic = \"unwind\"\n\n"
                 "[workspace]\nexclude = [\"../../code/ignored\"]\n\n"
                 "[dependencies.nagi-app-body]\n"
                 f'path = "../../code/{digest}"\n'
                 f'package = "nagi-body-{app_suffix}"\n').encode()
        cargo_path = self.root / "Cargo.toml"
        cargo_path.write_bytes(cargo)
        self.receipt["artifacts"]["Cargo.toml"] = MODULE.identity(cargo_path)

        cas_root = self.root.parent.parent / "code" / digest
        (cas_root / "src").mkdir(parents=True, exist_ok=True)
        (cas_root / "Cargo.toml").write_bytes(body_manifest_bytes)
        (cas_root / "src" / "lib.rs").write_bytes(body_main_bytes)
        self.write()
        return digest

    def test_valid_and_corrupt_bytes(self):
        MODULE.verify(self.path, self.compiler)
        (self.root / "src/main.rs").write_bytes(b"wrong")
        with self.assertRaisesRegex(ValueError, "integrity mismatch"):
            MODULE.verify(self.path)

    def test_schema2_valid_and_final_blank_line_is_accepted(self):
        self.configure_schema2()
        MODULE.verify(self.path, self.compiler)

        cargo_path = self.root / "Cargo.toml"
        cargo = cargo_path.read_text(encoding="utf-8")
        cargo = cargo.replace('path = "../../code/nagi-runtime"',
                              'path = "../../code/runtime-\\u2603"')
        cargo = cargo.replace('name = "nagi-app-test"', 'name = "nagi-app-Δ"')
        cargo_path.write_text(cargo, encoding="utf-8")
        self.receipt["artifacts"]["Cargo.toml"] = MODULE.identity(cargo_path)
        self.write()
        MODULE.verify(self.path)

        cargo_path.write_text(cargo_path.read_text(encoding="utf-8") + "\n", encoding="utf-8")
        self.receipt["artifacts"]["Cargo.toml"] = MODULE.identity(cargo_path)
        self.write()
        MODULE.verify(self.path)

    def test_schema2_snapshot_and_cache_tampering_are_rejected(self):
        self.configure_schema2()
        main = self.root / "src/main.rs"
        main.write_bytes(b"changed source\n")
        with self.assertRaisesRegex(ValueError, "integrity mismatch"):
            MODULE.verify(self.path)

        self.configure_schema2()
        main = self.root / "src/main.rs"
        main.write_bytes(b"x" * main.stat().st_size)
        self.receipt["artifacts"]["src/main.rs"] = MODULE.identity(main)
        self.write()
        with self.assertRaisesRegex(ValueError, "generated body digest mismatch"):
            MODULE.verify(self.path)

        self.configure_schema2()
        digest = self.receipt["generated_body_sha256"]
        cas_root = self.root.parent.parent / "code" / digest
        cas_manifest = cas_root / "Cargo.toml"
        cas_manifest.write_bytes(b"x" * cas_manifest.stat().st_size)
        with self.assertRaisesRegex(ValueError, "cache Cargo.toml mismatch"):
            MODULE.verify(self.path)

        self.configure_schema2()
        digest = self.receipt["generated_body_sha256"]
        cas_lib = self.root.parent.parent / "code" / digest / "src/lib.rs"
        cas_lib.write_bytes(b"x" * cas_lib.stat().st_size)
        with self.assertRaisesRegex(ValueError, "cache src/lib.rs mismatch"):
            MODULE.verify(self.path)

    def test_schema2_digest_and_dependency_path_are_strict(self):
        self.configure_schema2()
        self.receipt["generated_body_sha256"] = "A" * 64
        self.write()
        with self.assertRaisesRegex(ValueError, "invalid generated body digest"):
            MODULE.verify(self.path)

        digest = self.configure_schema2()
        self.receipt["generated_body_sha256"] = "0" * 64
        self.write()
        with self.assertRaisesRegex(ValueError, "generated body digest mismatch"):
            MODULE.verify(self.path)

        digest = self.configure_schema2()
        cargo_path = self.root / "Cargo.toml"
        cargo = cargo_path.read_text(encoding="utf-8").replace(
            f'path = "../../code/{digest}"', 'path = "../../../../outside"')
        cargo_path.write_text(cargo, encoding="utf-8")
        self.receipt["artifacts"]["Cargo.toml"] = MODULE.identity(cargo_path)
        self.write()
        with self.assertRaisesRegex(ValueError, "dependency table mismatch"):
            MODULE.verify(self.path)

    def test_schema2_dependency_table_rejects_duplicates_and_extra_keys(self):
        self.configure_schema2()
        cargo_path = self.root / "Cargo.toml"
        cargo = cargo_path.read_text(encoding="utf-8")
        cargo_path.write_text(cargo + "\n[dependencies.\"nagi-app-body\"]\n", encoding="utf-8")
        self.receipt["artifacts"]["Cargo.toml"] = MODULE.identity(cargo_path)
        self.write()
        with self.assertRaisesRegex(ValueError, "unexpected generated Cargo.toml table|duplicate"):
            MODULE.verify(self.path)

        self.configure_schema2()
        cargo_path = self.root / "Cargo.toml"
        cargo = cargo_path.read_text(encoding="utf-8").replace(
            f'package = "nagi-body-{self.app.rsplit("-", 1)[-1]}"',
            f'package = "nagi-body-{self.app.rsplit("-", 1)[-1]}"\nother = "key"')
        cargo_path.write_text(cargo, encoding="utf-8")
        self.receipt["artifacts"]["Cargo.toml"] = MODULE.identity(cargo_path)
        self.write()
        with self.assertRaisesRegex(ValueError, "generated Cargo.toml key|dependency table mismatch"):
            MODULE.verify(self.path)

    def test_schema2_rejects_missing_and_extra_artifact_names(self):
        self.configure_schema2()
        del self.receipt["artifacts"]["src/entry.rs"]
        self.write()
        with self.assertRaisesRegex(ValueError, "unexpected, missing"):
            MODULE.verify(self.path)

        self.configure_schema2()
        extra = self.root / "extra.txt"
        extra.write_text("extra", encoding="utf-8")
        self.receipt["artifacts"]["extra.txt"] = MODULE.identity(extra)
        self.write()
        with self.assertRaisesRegex(ValueError, "unexpected, missing"):
            MODULE.verify(self.path)

    def test_incomplete_schema2_receipt_is_rejected(self):
        self.configure_schema2()
        del self.receipt["generated_body_sha256"]
        self.write()
        with self.assertRaisesRegex(ValueError, "invalid generated body digest"):
            MODULE.verify(self.path)

    def test_schema2_rejects_cache_file_links(self):
        digest = self.configure_schema2()
        cas_lib = self.root.parent.parent / "code" / digest / "src/lib.rs"
        cas_lib.unlink()
        try:
            cas_lib.symlink_to(self.root / "src/main.rs")
        except OSError as error:
            self.skipTest(f"OS does not permit test symlinks: {error}")
        with self.assertRaisesRegex(ValueError, "symbolic link"):
            MODULE.verify(self.path)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "Unix named pipes")
    def test_schema2_rejects_cache_fifos_without_blocking(self):
        digest = self.configure_schema2()
        cas_lib = self.root.parent.parent / "code" / digest / "src/lib.rs"
        cas_lib.unlink()
        os.mkfifo(cas_lib)
        result = subprocess.run([sys.executable, str(SPEC.origin), str(self.path)],
                                capture_output=True, text=True, timeout=5)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not a regular file", result.stderr)

    def test_schema2_rejects_symlinked_code_parent(self):
        self.configure_schema2()
        code = self.root.parent.parent / "code"
        moved = code.with_name("code-moved")
        code.rename(moved)
        try:
            code.symlink_to(moved, target_is_directory=True)
        except OSError as error:
            self.skipTest(f"OS does not permit test symlinks: {error}")
        with self.assertRaisesRegex(ValueError, "symbolic link"):
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
