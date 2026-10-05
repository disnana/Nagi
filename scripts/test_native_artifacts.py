"""Successful generation metadata is authoritative over a shared Cargo cache."""
from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

from native_artifacts import native_executable


class NativeArtifactsTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.generated = self.root / "generated"
        self.generated.mkdir()
        (self.generated / "Cargo.toml").write_text(
            "[package]\nname='nagi-main-0123456789abcdef'\nversion='0.1.0'\n",
            encoding="utf-8",
        )
        self.target = self.root / "cache"
        (self.target / "release").mkdir(parents=True)
        self.app = self.generated / ".nagi/apps/nagi-main-0123456789abcdef"

    def publish(self, generation="g-one"):
        exe = self.app / "generations" / generation / "application.exe"
        exe.parent.mkdir(parents=True)
        exe.write_bytes(b"successful generation")
        (self.app / "latest.json").write_text(json.dumps({
            "schema_version": 1,
            "app_id": "nagi-main-0123456789abcdef",
            "generation": generation,
            "executable": exe.relative_to(self.app).as_posix(),
        }), encoding="utf-8")
        return exe

    def test_successful_generation_is_selected_instead_of_mutable_package_cache(self):
        exe = self.publish()
        result = native_executable(self.generated, self.target)
        self.assertEqual(result.resolve(), exe.resolve())
        self.assertEqual(result.read_bytes(), b"successful generation")

    def test_latest_update_selects_new_generation_and_preserves_previous_file(self):
        old = self.publish()
        new = self.publish("g-two")
        self.assertEqual(native_executable(self.generated, self.target).resolve(), new.resolve())
        self.assertEqual(old.read_bytes(), b"successful generation")

    def test_invalid_success_metadata_does_not_fall_back_to_shared_cache(self):
        self.publish()
        (self.app / "latest.json").write_text("{", encoding="utf-8")
        with self.assertRaises((ValueError, RuntimeError)):
            native_executable(self.generated, self.target)

    def test_missing_published_binary_does_not_select_an_unrelated_cached_executable(self):
        exe = self.publish()
        exe.unlink()
        with self.assertRaises((FileNotFoundError, RuntimeError)):
            native_executable(self.generated, self.target)

    def test_new_namespace_without_success_does_not_use_legacy_cache(self):
        self.app.mkdir(parents=True)
        import os
        suffix = ".exe" if os.name == "nt" else ""
        cached = self.target / "release" / ("nagi-main-0123456789abcdef" + suffix)
        cached.write_bytes(b"caller-owned unrelated cache executable")
        with self.assertRaises((FileNotFoundError, RuntimeError)):
            native_executable(self.generated, self.target)
        self.assertEqual(cached.read_bytes(), b"caller-owned unrelated cache executable")

    def test_existing_legacy_generated_project_retains_explicit_fallback(self):
        # Compatibility for pre-Phase2 build folders without success metadata.
        import os
        suffix = ".exe" if os.name == "nt" else ""
        expected = self.target / "release" / ("nagi-main-0123456789abcdef" + suffix)
        expected.write_bytes(b"valid pre-Phase2 executable")
        self.assertEqual(native_executable(self.generated, self.target), expected)
        self.assertEqual(expected.read_bytes(), b"valid pre-Phase2 executable")


if __name__ == "__main__":
    unittest.main()
