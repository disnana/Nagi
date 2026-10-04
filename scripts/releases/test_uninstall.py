"""Exercise uninstall.sh against local releases and isolated homes only."""
import os
import subprocess
import tarfile
import unittest
from pathlib import Path

import test_install as fixtures


@unittest.skipIf(os.name == "nt", "PowerShell uninstaller has separate Windows tests")
class ShellUninstallTests(unittest.TestCase):
    # Reuse release creation/download mocking, not the installer's test methods.
    setUp = fixtures.ShellInstallTests.setUp
    tearDown = fixtures.ShellInstallTests.tearDown
    bundle = fixtures.ShellInstallTests.bundle
    install = fixtures.ShellInstallTests.install

    def uninstall(self, *options):
        return subprocess.run(
            ["bash", str(fixtures.ROOT / "scripts/uninstall.sh"),
             "--prefix", str(self.root / "versions"), "--bin-dir", str(self.root / "bin"),
             "--profile", str(self.root / "profile"), *options],
            env={**self.environment, "HOME": str(self.root)}, capture_output=True, text=True)

    def prepare(self):
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        # Uninstall must never execute the installed or downloaded compiler.
        (self.root / "executed").unlink()
        return self.root / "versions" / self.stem

    def tree(self, directory):
        entries = {}
        for path in directory.rglob("*"):
            name = str(path.relative_to(directory))
            if path.is_symlink():
                entries[name] = ("link", os.readlink(path))
            elif path.is_dir():
                entries[name] = ("directory",)
            else:
                entries[name] = ("file", path.read_bytes())
        return entries

    def test_verified_uninstall_repeat_preserves_foreign_content_and_does_not_execute(self):
        self.prepare()
        unrelated = self.root / "versions/my-project"
        unrelated.mkdir()
        (unrelated / "main.nagi").write_text("user project")
        command = self.root / "bin/other-command"
        command.write_text("user command")
        profile = self.root / "profile"
        profile.write_bytes(b"# user configuration\n" + profile.read_bytes() + b"# trailing configuration")
        for _ in range(2):
            result = self.uninstall()
            self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.root / "versions" / self.stem).exists())
        self.assertFalse((self.root / "bin/nagic").is_symlink())
        self.assertFalse((self.root / "executed").exists())
        self.assertEqual(profile.read_bytes(), b"# user configuration\n\n# trailing configuration")
        self.assertEqual((unrelated / "main.nagi").read_text(), "user project")
        self.assertEqual(command.read_text(), "user command")
        self.assertTrue((self.root / "versions").is_dir())
        self.assertTrue((self.root / "bin").is_dir())

    def test_dry_run_preserves_every_installed_file_profile_and_command(self):
        self.prepare()
        before = self.tree(self.root)
        result = self.uninstall("--dry-run")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Would remove distribution", result.stdout)
        self.assertIn("Would remove command", result.stdout)
        self.assertEqual(self.tree(self.root), before)

    def test_no_path_preserves_the_exact_profile_bytes(self):
        self.prepare()
        original = (self.root / "profile").read_bytes()
        result = self.uninstall("--no-path")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "profile").read_bytes(), original)

    def test_modified_added_hidden_empty_and_symlink_content_preserve_complete_tree(self):
        for kind in ("modified", "added", "hidden", "empty", "symlink"):
            with self.subTest(kind=kind):
                old = self.prepare()
                outside = self.root / "outside"
                outside.write_text("external user content")
                if kind == "modified":
                    (old / "runtime/src/lib.rs").write_text("user changes")
                elif kind == "added":
                    (old / "notes.txt").write_text("user notes")
                elif kind == "hidden":
                    (old / ".user-notes").write_text("hidden user notes")
                elif kind == "empty":
                    (old / "project").mkdir()
                else:
                    (old / "outside-link").symlink_to(outside)
                before = self.tree(old)
                result = self.uninstall()
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("Kept changed or unverifiable", result.stderr)
                self.assertEqual(self.tree(old), before)
                self.assertFalse((self.root / "bin/nagic").is_symlink())
                self.assertEqual(outside.read_text(), "external user content")
                # Restore the fixture for the next subtest, never an external tree.
                import shutil
                shutil.rmtree(old)

    def test_unavailable_corrupt_unsafe_and_mismatched_archive_preserve_tree(self):
        old = self.prepare()
        before = self.tree(old)
        for kind in ("unavailable", "corrupt", "traversal", "link", "metadata"):
            with self.subTest(kind=kind):
                self.bundle(corrupt=kind == "corrupt", unsafe=kind if kind in ("traversal", "link") else None,
                            metadata_platform="foreign-platform" if kind == "metadata" else None)
                if kind == "unavailable":
                    (self.assets / (self.stem + ".tar.gz")).unlink()
                result = self.uninstall()
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("Kept changed or unverifiable", result.stderr)
                self.assertEqual(self.tree(old), before)
                self.assertFalse((self.root / "executed").exists())
                self.assertFalse((self.root / "versions/escaped").exists())

    def test_legacy_and_multiple_verified_versions_are_removed(self):
        self.bundle(version=self.next_version)
        versions = self.root / "versions"
        versions.mkdir()
        for release in (fixtures.VERSION, self.next_version):
            archive = self.assets / f"nagi-{release}-{self.platform}.tar.gz"
            with tarfile.open(archive) as bundle:
                bundle.extractall(versions, filter="data")
        result = self.uninstall()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(list(versions.iterdir()), [])
        self.assertFalse((self.root / "executed").exists())

    def test_foreign_command_file_and_link_are_preserved(self):
        for kind in ("file", "link"):
            with self.subTest(kind=kind):
                self.prepare()
                command = self.root / "bin/nagic"
                command.unlink()
                foreign = self.root / "foreign-command"
                foreign.write_text("external command")
                if kind == "file":
                    command.write_text("user command")
                else:
                    command.symlink_to(foreign)
                result = self.uninstall()
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("Kept", result.stderr)
                if kind == "file":
                    self.assertEqual(command.read_text(), "user command")
                else:
                    self.assertEqual(os.readlink(command), str(foreign))
                self.assertEqual(foreign.read_text(), "external command")
                command.unlink()

    def test_profile_removes_only_the_exact_matching_stanza_and_preserves_binary_profile(self):
        self.prepare()
        profile = self.root / "profile"
        line = profile.read_bytes().splitlines()[1]
        similar = line.replace(b"# Nagi installer", b"# Nagi installer (user modified)")
        profile.write_bytes(b"# leading\n" + line + b"\n" + similar + b"\n" + line)
        result = self.uninstall()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(profile.read_bytes(), b"# leading\n" + similar + b"\n")
        self.prepare()
        binary = b"user\x00content\n" + profile.read_bytes()
        profile.write_bytes(binary)
        result = self.uninstall()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(profile.read_bytes(), binary)
        self.assertIn("Kept binary shell profile", result.stderr)

    def test_linked_profile_is_never_followed(self):
        self.prepare()
        profile = self.root / "profile"
        outside = self.root / "outside-profile"
        outside.write_bytes(profile.read_bytes())
        original = outside.read_bytes()
        profile.unlink()
        profile.symlink_to(outside)
        result = self.uninstall()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Kept linked shell profile", result.stderr)
        self.assertTrue(profile.is_symlink())
        self.assertEqual(outside.read_bytes(), original)

    def test_linked_root_or_ancestor_and_command_directory_are_rejected(self):
        self.prepare()
        before = self.tree(self.root / "versions")
        alias = self.root / "alias"
        alias.symlink_to(self.root, target_is_directory=True)
        result = self.uninstall("--prefix", str(alias / "versions"))
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.tree(self.root / "versions"), before)
        result = self.uninstall("--bin-dir", str(alias / "bin"))
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.tree(self.root / "versions"), before)
        self.assertTrue((self.root / "bin/nagic").is_symlink())

    def test_installer_lock_prevents_changes_and_is_preserved(self):
        self.prepare()
        lock = self.root / "versions/.install-lock"
        lock.mkdir()
        before = self.tree(self.root)
        result = self.uninstall()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Another installer/uninstaller", result.stderr)
        self.assertEqual(self.tree(self.root), before)

    def test_linked_version_preserves_external_release_tree(self):
        old = self.prepare()
        outside = self.root / "external-release"
        old.rename(outside)
        old.symlink_to(outside, target_is_directory=True)
        before = self.tree(outside)
        result = self.uninstall()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.tree(outside), before)
        self.assertTrue(old.is_symlink())
        self.assertIn("Kept linked installation", result.stderr)

    def test_default_profile_candidates_include_old_shell_login_files(self):
        self.prepare()
        stanza = (self.root / "profile").read_bytes()
        names = (".profile", ".bash_profile", ".bash_login", ".bashrc", ".zprofile", ".zshrc")
        for name in names:
            (self.root / name).write_bytes(b"# keep\n" + stanza)
        result = subprocess.run(
            ["bash", str(fixtures.ROOT / "scripts/uninstall.sh"), "--prefix", str(self.root / "versions"),
             "--bin-dir", str(self.root / "bin")],
            env={**self.environment, "HOME": str(self.root)}, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in names:
            self.assertEqual((self.root / name).read_bytes(), b"# keep\n\n")

    def test_missing_install_does_not_create_directories(self):
        before = self.tree(self.root)
        result = self.uninstall()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.tree(self.root), before)


if __name__ == "__main__":
    unittest.main()
