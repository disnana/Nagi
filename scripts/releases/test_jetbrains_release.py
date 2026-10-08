"""JetBrains common ZIP identity and immutable publication tests."""
import hashlib
import io
import tempfile
import unittest
from pathlib import Path
from zipfile import ZipFile
from unittest.mock import patch

import jetbrains
import publish
from test_release import FakeGitHub


def plugin_zip(path: Path, *, plugin_id="com.disnana.nagi", version="0.1.8", extra=b"payload") -> Path:
    descriptor = f"<idea-plugin><id>{plugin_id}</id><version>{version}</version></idea-plugin>"
    plugin_jar = io.BytesIO()
    with ZipFile(plugin_jar, "w") as archive:
        archive.writestr("META-INF/plugin.xml", descriptor)
        archive.writestr("payload.dat", extra)
    with ZipFile(path, "w") as distribution:
        distribution.writestr("lib/nagi-jetbrains.jar", plugin_jar.getvalue())
    return path


class JetBrainsArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_archive_reads_plugin_id_and_version_from_the_nested_jar(self):
        path = plugin_zip(self.root / "plugin.zip")
        jetbrains.verify_archive(path, "0.1.8")

    def test_archive_rejects_wrong_plugin_id_version_and_corrupt_zip(self):
        wrong_id = plugin_zip(self.root / "wrong-id.zip", plugin_id="com.example.other")
        with self.assertRaisesRegex(ValueError, "plugin id"):
            jetbrains.verify_archive(wrong_id, "0.1.8")
        wrong_version = plugin_zip(self.root / "wrong-version.zip", version="0.1.7")
        with self.assertRaisesRegex(ValueError, "version mismatch"):
            jetbrains.verify_archive(wrong_version, "0.1.8")
        corrupt = self.root / "corrupt.zip"
        corrupt.write_bytes(b"not a zip")
        with self.assertRaisesRegex(ValueError, "Invalid JetBrains plugin ZIP"):
            jetbrains.verify_archive(corrupt, "0.1.8")

    def test_preparation_uses_one_common_name_and_checksum(self):
        source = plugin_zip(self.root / "gradle-output.zip")
        output = self.root / "release-assets"
        archive, checksum = jetbrains.prepare_archive(source, output, "0.1.8")
        self.assertEqual(archive.name, "nagi-jetbrains-0.1.8.zip")
        self.assertEqual(checksum.read_text(encoding="utf-8"),
                         f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n")


class JetBrainsPublicationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        build = self.directory / "build"
        build.mkdir()
        source = plugin_zip(build / "common.zip")
        jetbrains.prepare_archive(source, self.directory, "0.1.8")
        note_builder = patch.object(publish, "release_notes", return_value="JetBrains fixture notes")
        note_builder.start()
        self.addCleanup(note_builder.stop)

    def publish(self, client, version="0.1.8"):
        publish.publish(client, "jetbrains", version, "a" * 40, self.directory)

    def test_publish_requires_one_common_zip_and_checksum(self):
        client = FakeGitHub()
        self.publish(client)
        self.assertEqual(client.release["tag_name"], "jetbrains-v0.1.8")
        self.assertFalse(client.release["draft"])
        self.assertEqual([call[0] for call in client.calls],
                         ["tag", "create", "upload", "upload", "edit"])
        self.assertEqual(len(client.release["assets"]), 2)
        self.assertEqual(client.calls[-1], ("edit", 1, False))

    def test_missing_common_archive_or_checksum_stops_before_release_mutation(self):
        client = FakeGitHub()
        (self.directory / "nagi-jetbrains-0.1.8.zip").unlink()
        with self.assertRaises(FileNotFoundError):
            self.publish(client)
        self.assertEqual(client.calls, [])

        source = plugin_zip(self.directory / "common.zip")
        jetbrains.prepare_archive(source, self.directory, "0.1.8")
        (self.directory / "nagi-jetbrains-0.1.8.zip.sha256").unlink()
        with self.assertRaises(FileNotFoundError):
            self.publish(client)
        self.assertEqual(client.calls, [])

    def test_invalid_local_plugin_identity_stops_before_github_reads(self):
        source = plugin_zip(self.directory / "wrong.zip", version="0.1.7")
        destination = self.directory / "nagi-jetbrains-0.1.8.zip"
        destination.write_bytes(source.read_bytes())
        client = FakeGitHub()
        with self.assertRaisesRegex(ValueError, "version mismatch"):
            self.publish(client)
        self.assertEqual(client.api_calls, [])
        self.assertEqual(client.calls, [])

    def test_draft_common_asset_with_different_bytes_is_never_overwritten(self):
        client = FakeGitHub("a" * 40, draft=True, component="jetbrains", version="0.1.8")
        client.add("nagi-jetbrains-0.1.8.zip", b"different build")
        with self.assertRaisesRegex(ValueError, "will not be overwritten"):
            self.publish(client)
        self.assertEqual(client.calls, [])

    def test_matching_draft_common_assets_are_kept(self):
        client = FakeGitHub("a" * 40, draft=True, component="jetbrains", version="0.1.8")
        client.release["body"] = "JetBrains fixture notes"
        for name in ("nagi-jetbrains-0.1.8.zip", "nagi-jetbrains-0.1.8.zip.sha256"):
            path = self.directory / name
            client.add(name, path.read_bytes())
        self.publish(client)
        self.assertEqual([call[0] for call in client.calls], ["edit"])
        self.assertFalse(client.release["draft"])

    def test_published_legacy_two_zip_release_is_never_rewritten(self):
        source = plugin_zip(self.directory / "legacy-tag-future-common.zip", version="0.1.1")
        jetbrains.prepare_archive(source, self.directory, "0.1.1")
        client = FakeGitHub("a" * 40, draft=False, component="jetbrains", version="0.1.1")
        client.add("nagi-jetbrains-IC-0.1.1.zip", b"historical IDEA ZIP")
        client.add("nagi-jetbrains-PC-0.1.1.zip", b"historical PyCharm ZIP")
        with self.assertRaisesRegex(ValueError, "incomplete; it will not be changed"):
            self.publish(client, "0.1.1")
        self.assertEqual(client.calls, [])

    def test_identical_published_common_release_is_idempotent(self):
        client = FakeGitHub("a" * 40, draft=False, component="jetbrains", version="0.1.8")
        for path in sorted(self.directory.glob("nagi-jetbrains-*")):
            client.add(path.name, path.read_bytes())
        self.publish(client)
        self.assertEqual(client.calls, [])


if __name__ == "__main__":
    unittest.main()
