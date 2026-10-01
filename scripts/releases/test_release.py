"""Regression tests for release selection, source isolation, and immutable assets."""
import hashlib
import json
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from zipfile import ZipFile

import package
import plan
import publish


class ReleasePlanTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        self.change("Cargo.toml", '[workspace.package]\nversion = "0.1.0"\n')
        self.change("editors/vscode-nagi/package.json", '{"version":"0.1.5"}\n')
        self.first = self.commit()
        self.root_patch = patch.object(plan, "ROOT", self.root)
        self.root_patch.start()

    def tearDown(self):
        self.root_patch.stop()
        self.temporary.cleanup()

    def change(self, file, text):
        path = self.root / file
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def commit(self):
        subprocess.run(["git", "add", "."], cwd=self.root, check=True)
        subprocess.run(["git", "-c", "user.name=Release Test", "-c", "user.email=test@example.invalid",
                        "commit", "-qm", "test"], cwd=self.root, check=True)
        return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=self.root).decode().strip()

    def test_docs_only_does_not_package_or_release(self):
        self.change("docs/example.md", "Updated documentation")
        result = plan.plan(self.first, self.commit())
        for key in ("release_nagi", "release_vscode", "package_nagi", "package_vscode"):
            self.assertEqual(result[key], "false")

    def test_multicommit_push_detects_both_versions_before_final_docs_commit(self):
        self.change("Cargo.toml", '[workspace.package]\nversion = "0.1.1"\n')
        self.change("editors/vscode-nagi/package.json", '{"version":"0.1.6"}\n')
        self.commit()
        self.change("docs/example.md", "Docs follow the version bump")
        result = plan.plan(self.first, self.commit())
        self.assertEqual(result["release_nagi"], "true")
        self.assertEqual(result["release_vscode"], "true")

    def test_extension_version_does_not_release_compiler(self):
        self.change("editors/vscode-nagi/package.json", '{"version":"0.1.6"}\n')
        result = plan.plan(self.first, self.commit())
        self.assertEqual(result["release_vscode"], "true")
        self.assertEqual(result["release_nagi"], "false")
        self.assertEqual(result["package_nagi"], "false")

    def test_pipeline_changes_verify_archives_without_publishing_unchanged_versions(self):
        self.change("scripts/releases/package.py", "# Packaging change")
        result = plan.plan(self.first, self.commit())
        self.assertEqual(result["package_nagi"], "true")
        self.assertEqual(result["package_vscode"], "true")
        self.assertEqual(result["release_nagi"], "false")
        self.assertEqual(result["release_vscode"], "false")

    def test_initial_push_does_not_invent_release(self):
        result = plan.plan("0" * 40, self.first)
        self.assertEqual(result["release_nagi"], "false")
        self.assertEqual(result["release_vscode"], "false")

    def test_downgrade_or_nonformal_version_is_rejected(self):
        self.change("editors/vscode-nagi/package.json", '{"version":"0.1.4"}\n')
        with self.assertRaisesRegex(ValueError, "must increase"):
            plan.plan(self.first, self.commit())
        self.change("editors/vscode-nagi/package.json", '{"version":"0.1.6-beta.1"}\n')
        with self.assertRaisesRegex(ValueError, "x.y.z"):
            plan.plan(self.first, self.commit())

    def test_package_uses_committed_source_and_excludes_untracked_files(self):
        self.change("runtime/Cargo.toml", '[package]\nname="nagi-runtime"\n')
        self.change("compiler/Cargo.toml", '[package]\nname="nagic"\n')
        self.change("README.md", "Committed documentation")
        commit = self.commit()
        self.change("README.md", "Uncommitted private notes")
        self.change("private-note.txt", "Must never be in a release")
        binary = self.root / "compiler-test"
        binary.write_bytes(b"prebuilt compiler fixture")
        output = self.root / "build/distribution"
        with patch.object(package, "ROOT", self.root):
            for platform in ("linux-x86_64", "windows-x86_64", "macos-arm64", "macos-x86_64"):
                archive = package.package(binary, "0.1.0", platform, output, commit)
                if platform != "windows-x86_64":
                    with tarfile.open(archive) as source:
                        names = source.getnames()
                        read = lambda name: source.extractfile(name).read()
                        executable = next(name for name in names if name.endswith("/target/release/nagic"))
                        self.assertEqual(source.getmember(executable).mode, 0o755)
                        self.assertEqual(read(executable), binary.read_bytes())
                        self.check_source(names, read, commit, platform)
                else:
                    with ZipFile(archive) as source:
                        self.assertIsNone(source.testzip())
                        self.assertIn(b"prebuilt compiler", source.read(next(name for name in source.namelist() if name.endswith("/nagic.exe"))))
                        self.check_source(source.namelist(), source.read, commit, platform)
                checksum = archive.with_name(archive.name + ".sha256").read_text().strip()
                self.assertEqual(checksum, f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}")

    def check_source(self, names, read, commit, platform):
        self.assertFalse(any(name.endswith("private-note.txt") or "/.git/" in name for name in names))
        self.assertEqual(read(next(name for name in names if name.endswith("/README.md"))), b"Committed documentation")
        metadata = json.loads(read(next(name for name in names if name.endswith("/release.json"))))
        self.assertEqual(metadata, {"commit": commit, "platform": platform, "version": "0.1.0"})


class FakeGitHub:
    def __init__(self, sha=None, draft=None, latest=None):
        self.sha = sha
        self.release = None if draft is None else {
            "id": 1, "tag_name": "vscode-v0.1.6", "draft": draft, "assets": []}
        self.calls = []
        self.api_calls = []
        self.files = {}
        self.latest = latest

    def api(self, resource):
        self.api_calls.append(resource)
        if resource.startswith("git/ref/"):
            return {"object": {"type": "commit", "sha": self.sha}} if self.sha else None
        if resource == "releases/latest":
            return {"tag_name": self.latest} if self.latest else None
        if resource.startswith("releases/tags/"):
            # GitHub's tag lookup exposes published releases, not drafts.
            if self.release and not self.release["draft"] and resource.endswith(self.release["tag_name"]):
                return self.release
            return None
        if resource == "releases?per_page=100&page=1":
            return [self.release] if self.release else []
        if resource == "releases/1":
            return self.release
        raise AssertionError(f"Unexpected GitHub resource: {resource}")

    def create_tag(self, tag, sha):
        self.calls.append(("tag", tag, sha))
        self.sha = sha

    def command(self, *args):
        self.calls.append(args)
        if args[0] == "create":
            self.release = {"id": 1, "tag_name": args[1], "draft": True, "assets": []}
        elif args[0] == "upload":
            file = Path(args[2])
            self.add(file.name, file.read_bytes())
        elif args[0] == "edit":
            self.release["draft"] = False

    def add(self, name, data):
        asset = {"name": name, "id": len(self.files) + 1}
        self.release["assets"].append(asset)
        self.files[asset["id"]] = data

    def content(self, asset):
        return self.files[asset["id"]]

    def digest(self, asset):
        return hashlib.sha256(self.content(asset)).hexdigest()


class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary.name)
        self.filename = "nagi-language-0.1.6.vsix"
        self.data = b"tested VSIX bytes"
        (self.directory / self.filename).write_bytes(self.data)
        self.checksum = f"{hashlib.sha256(self.data).hexdigest()}  {self.filename}\n".encode()
        (self.directory / (self.filename + ".sha256")).write_bytes(self.checksum)

    def tearDown(self):
        self.temporary.cleanup()

    def publish(self, client):
        publish.publish(client, "vscode", "0.1.6", "a" * 40, self.directory)

    def test_new_release_is_draft_until_assets_are_uploaded_and_verified(self):
        client = FakeGitHub()
        self.publish(client)
        self.assertEqual([call[0] for call in client.calls], ["tag", "create", "upload", "upload", "edit"])
        self.assertFalse(client.release["draft"])
        self.assertIn("--latest=false", client.calls[-1])
        self.assertNotIn("--clobber", str(client.calls))
        self.assertIn("releases/1", client.api_calls)

    def test_published_same_commit_is_idempotent_even_if_new_build_bytes_differ(self):
        client = FakeGitHub("a" * 40, draft=False)
        client.add(self.filename, b"original tested build")
        checksum = f"{hashlib.sha256(b'original tested build').hexdigest()}  {self.filename}\n".encode()
        client.add(self.filename + ".sha256", checksum)
        self.publish(client)
        self.assertEqual(client.calls, [])

    def test_existing_tag_at_other_commit_is_never_changed(self):
        client = FakeGitHub("b" * 40)
        with self.assertRaisesRegex(ValueError, "different commit"):
            self.publish(client)
        self.assertEqual(client.calls, [])

    def test_missing_or_invalid_published_asset_is_never_repaired_silently(self):
        client = FakeGitHub("a" * 40, draft=False)
        with self.assertRaisesRegex(ValueError, "incomplete"):
            self.publish(client)
        client.add(self.filename, self.data)
        client.add(self.filename + ".sha256", b"invalid checksum")
        with self.assertRaisesRegex(ValueError, "checksum"):
            self.publish(client)
        self.assertEqual(client.calls, [])

    def test_draft_retry_keeps_matching_assets_and_uploads_only_missing_files(self):
        client = FakeGitHub("a" * 40, draft=True)
        client.add(self.filename, self.data)
        self.publish(client)
        self.assertEqual([call[0] for call in client.calls], ["upload", "edit"])

    def test_draft_can_be_found_after_the_first_release_page(self):
        client = FakeGitHub("a" * 40, draft=True)
        original = client.api

        def api(resource):
            if resource.startswith("releases?per_page=100&page="):
                client.api_calls.append(resource)
                if resource.endswith("page=1"):
                    return [{"tag_name": f"other-v{i}"} for i in range(100)]
                if resource.endswith("page=2"):
                    return [client.release]
                raise AssertionError(resource)
            return original(resource)

        with patch.object(client, "api", side_effect=api):
            self.publish(client)
        self.assertEqual([call[0] for call in client.calls], ["upload", "upload", "edit"])
        self.assertIn("releases?per_page=100&page=2", client.api_calls)

    def test_created_draft_missing_from_listing_stops_before_upload(self):
        client = FakeGitHub()
        original = client.api

        def api(resource):
            if resource.startswith("releases?per_page="):
                return []
            return original(resource)

        with patch.object(client, "api", side_effect=api):
            with self.assertRaisesRegex(RuntimeError, "Created draft release could not be found"):
                self.publish(client)
        self.assertEqual([call[0] for call in client.calls], ["tag", "create"])
        self.assertTrue(client.release["draft"])

    def test_disappearing_release_cannot_be_published(self):
        client = FakeGitHub("a" * 40, draft=True)
        original = client.api

        def api(resource):
            return None if resource == "releases/1" else original(resource)

        with patch.object(client, "api", side_effect=api):
            with self.assertRaisesRegex(RuntimeError, "disappeared before upload verification"):
                self.publish(client)
        self.assertEqual([call[0] for call in client.calls], ["upload", "upload"])
        self.assertTrue(client.release["draft"])

    def test_corrupt_uploaded_asset_keeps_the_release_draft(self):
        client = FakeGitHub("a" * 40, draft=True)
        original = client.content

        def content(asset):
            return b"corrupted upload" if asset["name"] == self.filename else original(asset)

        with patch.object(client, "content", side_effect=content):
            with self.assertRaisesRegex(ValueError, "upload verification failed"):
                self.publish(client)
        self.assertEqual([call[0] for call in client.calls], ["upload", "upload"])
        self.assertTrue(client.release["draft"])

    def test_draft_with_different_asset_is_not_overwritten(self):
        client = FakeGitHub("a" * 40, draft=True)
        client.add(self.filename, b"different build")
        with self.assertRaisesRegex(ValueError, "not be overwritten"):
            self.publish(client)
        self.assertEqual(client.calls, [])

    def test_older_nagi_completion_cannot_replace_a_newer_latest_release(self):
        for version, flag in (("0.1.1", "--latest=false"), ("0.2.1", "--latest")):
            with self.subTest(version=version):
                for platform, suffix in (("linux-x86_64", ".tar.gz"), ("windows-x86_64", ".zip"),
                                         ("macos-arm64", ".tar.gz"), ("macos-x86_64", ".tar.gz")):
                    filename = f"nagi-{version}-{platform}{suffix}"
                    (self.directory / filename).write_bytes(self.data)
                    (self.directory / (filename + ".sha256")).write_text(f"{hashlib.sha256(self.data).hexdigest()}  {filename}\n")
                client = FakeGitHub(latest="nagi-v0.2.0")
                publish.publish(client, "nagi", version, "a" * 40, self.directory)
                self.assertIn(flag, client.calls[-1])
                uploads = [Path(call[2]).name for call in client.calls if call[0] == "upload"]
                self.assertEqual(len(uploads), 8)
                for platform, suffix in (("linux-x86_64", ".tar.gz"), ("windows-x86_64", ".zip"),
                                         ("macos-arm64", ".tar.gz"), ("macos-x86_64", ".tar.gz")):
                    filename = f"nagi-{version}-{platform}{suffix}"
                    self.assertIn(filename, uploads)
                    self.assertIn(filename + ".sha256", uploads)

    def test_missing_macos_archive_stops_before_creating_a_release(self):
        for platform, suffix in (("linux-x86_64", ".tar.gz"), ("windows-x86_64", ".zip")):
            filename = f"nagi-0.1.1-{platform}{suffix}"
            (self.directory / filename).write_bytes(self.data)
            (self.directory / (filename + ".sha256")).write_text(
                f"{hashlib.sha256(self.data).hexdigest()}  {filename}\n")
        client = FakeGitHub()
        with self.assertRaisesRegex(FileNotFoundError, "macos-arm64"):
            publish.publish(client, "nagi", "0.1.1", "a" * 40, self.directory)
        self.assertEqual(client.calls, [])


if __name__ == "__main__":
    unittest.main()
