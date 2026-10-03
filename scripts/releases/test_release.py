"""Regression tests for release selection, source isolation, and immutable assets."""
import hashlib
import json
import subprocess
import tarfile
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest.mock import patch
from urllib.parse import parse_qs, urlparse
from zipfile import ZipFile

import package
import plan
import publish


class ReleasePlanTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        self.change("Cargo.toml", '[workspace.package]\nversion = "0.1.0"\nedition = "2021"\nlicense = "MIT"\n')
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

    def test_publisher_change_packages_the_extension_without_releasing(self):
        self.change("editors/vscode-nagi/package.json", '{"version":"0.1.5","publisher":"Disnana"}\n')
        result = plan.plan(self.first, self.commit())
        self.assertEqual(result["package_vscode"], "true")
        for key in ("package_nagi", "release_nagi", "release_vscode"):
            self.assertEqual(result[key], "false")

    def test_extension_code_and_packaging_changes_produce_only_a_vsix(self):
        for file in ("editors/vscode-nagi/src/extension.js", "editors/vscode-nagi/scripts/package_vsix.py"):
            with self.subTest(file=file):
                base = plan.git("rev-parse", "HEAD").strip()
                self.change(file, "# Changed extension")
                result = plan.plan(base, self.commit())
                self.assertEqual(result["package_vscode"], "true")
                for key in ("package_nagi", "release_nagi", "release_vscode"):
                    self.assertEqual(result[key], "false")

    def test_extension_docs_and_tests_do_not_package(self):
        for file in ("editors/vscode-nagi/README.md", "editors/vscode-nagi/test/host.js"):
            with self.subTest(file=file):
                base = plan.git("rev-parse", "HEAD").strip()
                self.change(file, "# Documentation or test change")
                result = plan.plan(base, self.commit())
                for key in ("package_nagi", "package_vscode", "release_nagi", "release_vscode"):
                    self.assertEqual(result[key], "false")

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

    def test_installer_only_changes_run_platform_checks_without_releasing(self):
        for file in ("scripts/install.sh", "scripts/install.ps1"):
            with self.subTest(file=file):
                base = plan.git("rev-parse", "HEAD").strip()
                self.change(file, "# Installer update")
                result = plan.plan(base, self.commit())
                self.assertEqual(result["package_nagi"], "true")
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
        self.change("runtime/Cargo.toml", '[package]\nname="nagi-runtime"\nversion.workspace=true\nedition.workspace=true\nlicense.workspace=true\n[dev-dependencies]\nunused="1"\n')
        self.change("runtime/src/lib.rs", "// committed runtime\n")
        self.change("runtime/src/http/tests.rs", "// development test\n")
        self.change("runtime/src/actor/lifecycle_adversarial_tests.rs", "// development lifecycle tests\n")
        self.change("runtime/examples/bench.rs", "// development benchmark\n")
        self.change("LICENSE", "MIT license fixture\n")
        self.change("compiler/Cargo.toml", '[package]\nname="nagic"\n')
        self.change("README.md", "Committed documentation")
        commit = self.commit()
        self.change("README.md", "Uncommitted private notes")
        self.change("runtime/src/lib.rs", "// uncommitted runtime\n")
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
                        executable = f"nagi-0.1.0-{platform}/nagic"
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
        prefix = f"nagi-0.1.0-{platform}/"
        self.assertEqual(set(names), {prefix + name for name in (
            "nagic.exe" if platform == "windows-x86_64" else "nagic",
            "runtime/Cargo.toml", "runtime/src/lib.rs", "LICENSE", "release.json", "README.txt")})
        self.assertEqual(read(prefix + "runtime/src/lib.rs"), b"// committed runtime\n")
        manifest = tomllib.loads(read(prefix + "runtime/Cargo.toml").decode())
        self.assertEqual(manifest["package"], {"name": "nagi-runtime", "version": "0.1.0", "edition": "2021", "license": "MIT"})
        self.assertIn("workspace", manifest)
        self.assertNotIn("dev-dependencies", manifest)
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

    def create_release(self, tag, sha, title, notes):
        self.calls.append(("create", tag, sha, title, notes))
        self.release = {"id": 1, "tag_name": tag, "draft": True, "assets": []}
        return self.release

    def upload_asset(self, release_id, file):
        self.calls.append(("upload", release_id, str(file)))
        assert release_id == self.release["id"]
        self.add(file.name, file.read_bytes())

    def make_public(self, release_id, latest):
        self.calls.append(("edit", release_id, latest))
        assert release_id == self.release["id"]
        self.release["draft"] = False
        return self.release

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
        self.assertEqual(client.calls[-1], ("edit", 1, False))
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

    def test_created_draft_can_be_published_before_it_appears_in_listing(self):
        client = FakeGitHub()
        original = client.api

        def api(resource):
            if resource.startswith("releases?per_page="):
                return []
            return original(resource)

        with patch.object(client, "api", side_effect=api):
            self.publish(client)
        self.assertEqual([call[0] for call in client.calls], ["tag", "create", "upload", "upload", "edit"])
        self.assertFalse(client.release["draft"])

    def test_creation_response_without_draft_identity_stops_before_upload(self):
        client = FakeGitHub()
        with patch.object(client, "create_release", return_value={"draft": True}):
            with self.assertRaisesRegex(RuntimeError, "did not return the expected draft"):
                self.publish(client)
        self.assertEqual([call[0] for call in client.calls], ["tag"])

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

    def test_publication_must_be_confirmed_by_the_release_response(self):
        client = FakeGitHub("a" * 40, draft=True)
        with patch.object(client, "make_public", return_value=client.release):
            with self.assertRaisesRegex(RuntimeError, "did not confirm publication"):
                self.publish(client)
        self.assertTrue(client.release["draft"])

    def test_draft_with_different_asset_is_not_overwritten(self):
        client = FakeGitHub("a" * 40, draft=True)
        client.add(self.filename, b"different build")
        with self.assertRaisesRegex(ValueError, "not be overwritten"):
            self.publish(client)
        self.assertEqual(client.calls, [])

    def test_older_nagi_completion_cannot_replace_a_newer_latest_release(self):
        for version, latest in (("0.1.1", False), ("0.2.1", True)):
            with self.subTest(version=version):
                for platform, suffix in (("linux-x86_64", ".tar.gz"), ("windows-x86_64", ".zip"),
                                         ("macos-arm64", ".tar.gz"), ("macos-x86_64", ".tar.gz")):
                    filename = f"nagi-{version}-{platform}{suffix}"
                    (self.directory / filename).write_bytes(self.data)
                    (self.directory / (filename + ".sha256")).write_text(f"{hashlib.sha256(self.data).hexdigest()}  {filename}\n")
                client = FakeGitHub(latest="nagi-v0.2.0")
                publish.publish(client, "nagi", version, "a" * 40, self.directory)
                self.assertEqual(client.calls[-1], ("edit", 1, latest))
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


class GitHubProtocolTests(unittest.TestCase):
    def response(self, value):
        return subprocess.CompletedProcess([], 0, stdout=json.dumps(value).encode(), stderr=b"")

    def test_create_returns_the_api_identity_without_a_followup_lookup(self):
        client = publish.GitHub("owner/repo")
        release = {"id": 42, "tag_name": "vscode-v0.1.7", "draft": True, "assets": []}
        with patch.object(publish.subprocess, "run", return_value=self.response(release)) as run:
            self.assertEqual(client.create_release("vscode-v0.1.7", "a" * 40, "Nagi", "notes\n凪"), release)
        run.assert_called_once()
        command = run.call_args.args[0]
        self.assertEqual(command[:5], ["gh", "api", "--method", "POST", "repos/owner/repo/releases"])
        self.assertEqual(json.loads(run.call_args.kwargs["input"]), {
            "tag_name": "vscode-v0.1.7", "target_commitish": "a" * 40,
            "name": "Nagi", "body": "notes\n凪", "draft": True})

    def test_upload_uses_the_release_id_and_a_raw_binary_file(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / "凪 & bot.vsix"
            file.write_bytes(b"PK\x00\xff\n")
            with patch.object(publish.subprocess, "run", return_value=self.response({"id": 9})) as run:
                publish.GitHub("owner/repo").upload_asset(42, file)
            command = run.call_args.args[0]
            self.assertEqual(command[:4], ["gh", "api", "--method", "POST"])
            url = urlparse(command[4])
            self.assertEqual(url.netloc, "uploads.github.com")
            self.assertEqual(url.path, "/repos/owner/repo/releases/42/assets")
            self.assertEqual(parse_qs(url.query), {"name": [file.name]})
            self.assertEqual(command[command.index("--input") + 1], str(file))
            self.assertIn("Content-Type: application/octet-stream", command)
            self.assertIsNone(run.call_args.kwargs["input"])

    def test_publication_patches_the_same_id_and_preserves_latest_selection(self):
        for latest in (False, True):
            with self.subTest(latest=latest):
                with patch.object(publish.subprocess, "run", return_value=self.response({"draft": False})) as run:
                    publish.GitHub("owner/repo").make_public(42, latest)
                self.assertEqual(run.call_args.args[0][:5], [
                    "gh", "api", "--method", "PATCH", "repos/owner/repo/releases/42"])
                self.assertEqual(json.loads(run.call_args.kwargs["input"]), {
                    "draft": False, "make_latest": "true" if latest else "false"})

    def test_failed_write_is_not_treated_as_a_missing_read_result(self):
        response = subprocess.CompletedProcess([], 1, stdout=b"", stderr=b"HTTP 404: Not Found")
        with patch.object(publish.subprocess, "run", return_value=response):
            with self.assertRaisesRegex(RuntimeError, "HTTP 404"):
                publish.GitHub("owner/repo").create_release("vscode-v0.1.7", "a" * 40, "Nagi", "notes")


if __name__ == "__main__":
    unittest.main()
