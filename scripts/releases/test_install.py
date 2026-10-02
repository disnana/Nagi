"""Run the shell installer with local release assets; never use a user's HOME."""
import hashlib
import io
import json
import os
import platform
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]


@unittest.skipIf(os.name == "nt", "PowerShell installer has a separate Windows test")
class ShellInstallTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="nagi installer 凪 ' $() ")
        self.root = Path(self.temporary.name)
        system = "macos" if platform.system() == "Darwin" else "linux"
        architecture = "arm64" if platform.machine() in ("arm64", "aarch64") else "x86_64"
        self.platform = f"{system}-{architecture}"
        self.stem = f"nagi-{VERSION}-{self.platform}"
        major, minor, patch = map(int, VERSION.split("."))
        self.next_version = f"{major}.{minor}.{patch + 1}"
        self.assets = self.root / "assets"
        self.assets.mkdir()
        self.tools = self.root / "tools"
        self.tools.mkdir()
        curl = self.tools / "curl"
        curl.write_text('''#!/bin/sh
output=''
url=''
head=0
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o) output=$2; shift 2 ;;
        --head) head=1; shift ;;
        https://*) url=$1; shift ;;
        *) shift ;;
    esac
done
if [ "$head" = 1 ]; then printf '%s' "$NAGI_TEST_LATEST_URL"; exit 0; fi
/bin/cp "$NAGI_TEST_ASSETS/${url##*/}" "$output"
''')
        curl.chmod(0o755)
        self.environment = {**os.environ, "PATH": str(self.tools) + os.pathsep + os.environ["PATH"],
                            "NAGI_TEST_ASSETS": str(self.assets), "NAGI_INSTALL_MARKER": str(self.root / "executed"),
                            "NAGI_TEST_LATEST_URL": f"https://github.com/disnana/Nagi/releases/tag/nagi-v{VERSION}"}
        self.bundle()

    def tearDown(self):
        self.temporary.cleanup()

    def bundle(self, *, version=VERSION, corrupt=False, unsafe=None, metadata_platform=None, activation_failure=False):
        stem = f"nagi-{version}-{self.platform}"
        archive = self.assets / (stem + ".tar.gz")
        metadata = {"version": version, "platform": metadata_platform or self.platform, "commit": "a" * 40}
        activation = 'case "$0" in */.install.*) ;; *) exit 73 ;; esac\n' if activation_failure else ''
        files = {"nagic": f'#!/bin/sh\n{activation}touch "$NAGI_INSTALL_MARKER"\necho "nagic {version}"\n'.encode(),
                 "runtime/Cargo.toml": b"runtime fixture", "runtime/src/lib.rs": b"runtime fixture",
                 "release.json": (json.dumps(metadata, indent=2) + "\n").encode(),
                 "README.txt": b"installation notes", "LICENSE": b"license fixture"}
        with tarfile.open(archive, "w:gz") as output:
            for name, content in files.items():
                entry = tarfile.TarInfo(f"{stem}/{name}")
                entry.size = len(content)
                entry.mode = 0o755 if name == "nagic" else 0o644
                output.addfile(entry, io.BytesIO(content))
            if unsafe == "traversal":
                entry = tarfile.TarInfo(f"{stem}/../escaped")
                output.addfile(entry, io.BytesIO(b""))
            elif unsafe == "link":
                entry = tarfile.TarInfo(f"{stem}/runtime/link")
                entry.type = tarfile.SYMTYPE
                entry.linkname = "../../../escaped"
                output.addfile(entry)
        digest = "0" * 64 if corrupt else hashlib.sha256(archive.read_bytes()).hexdigest()
        archive.with_name(archive.name + ".sha256").write_text(f"{digest}  {archive.name}\n")

    def install(self, *options):
        return subprocess.run(["bash", str(ROOT / "scripts/install.sh"), "--prefix", str(self.root / "versions"),
                               "--bin-dir", str(self.root / "bin"), "--profile", str(self.root / "profile"), *options],
                              env=self.environment, capture_output=True, text=True)

    def test_install_and_repeat_preserve_profile_and_existing_path(self):
        profile = self.root / "profile"
        profile.write_text("# existing user configuration\n")
        for _ in range(2):
            result = self.install()
            self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(profile.read_text().count("# Nagi installer"), 1)
        self.assertTrue(profile.read_text().startswith("# existing user configuration\n"))
        old_bin = self.root / "old-bin"
        old_bin.mkdir()
        old_command = old_bin / "nagic"
        old_command.write_text("#!/bin/sh\necho old compiler\n")
        old_command.chmod(0o755)
        environment = {**self.environment, "PATH": str(old_bin) + os.pathsep + self.environment["PATH"] + os.pathsep + str(self.root / "bin")}
        shells = ["bash"] + (["zsh"] if shutil.which("zsh") else [])
        for shell in shells:
            with self.subTest(shell=shell):
                flags = ["--noprofile", "--norc"] if shell == "bash" else ["-f"]
                result = subprocess.run([shell, *flags, "-c", '. "$1"; nagic --version', "test", str(profile)],
                                        env=environment, capture_output=True, text=True, check=True)
                self.assertEqual(result.stderr, "", profile.read_text())
                self.assertEqual(result.stdout.strip(), f"nagic {VERSION}", profile.read_text())
        self.assertTrue((self.root / "bin/nagic").is_symlink())

    def test_checksum_failure_never_executes_or_registers_the_payload(self):
        self.bundle(corrupt=True)
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("SHA-256 mismatch", result.stderr)
        self.assertFalse((self.root / "executed").exists())
        self.assertFalse((self.root / "bin").exists())
        self.assertFalse((self.root / "profile").exists())

    def test_unsafe_archives_are_rejected_before_extraction_or_execution(self):
        for unsafe in ("traversal", "link"):
            with self.subTest(unsafe=unsafe):
                self.bundle(unsafe=unsafe)
                result = self.install()
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((self.root / "executed").exists())
                self.assertFalse((self.root / "versions/escaped").exists())

    def test_wrong_platform_metadata_is_rejected(self):
        self.bundle(metadata_platform="different-os")
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("metadata mismatch", result.stderr)
        self.assertFalse((self.root / "executed").exists())

    def test_existing_command_is_not_overwritten(self):
        command = self.root / "bin/nagic"
        command.parent.mkdir()
        command.write_text("an existing command")
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(command.read_text(), "an existing command")
        self.assertFalse((self.root / "profile").exists())

    def test_no_path_leaves_shell_configuration_untouched(self):
        result = self.install("--no-path")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.root / "profile").exists())

    def activate_next_release(self, **options):
        self.bundle(version=self.next_version, **options)
        self.environment["NAGI_TEST_LATEST_URL"] = f"https://github.com/disnana/Nagi/releases/tag/nagi-v{self.next_version}"
        return self.install()

    def assert_active(self, version):
        command = self.root / "bin/nagic"
        self.assertEqual(command.resolve(), self.root / "versions" / f"nagi-{version}-{self.platform}" / "nagic")
        output = subprocess.check_output([str(command), "--version"], env=self.environment, text=True)
        self.assertEqual(output.strip(), f"nagic {version}")

    def test_upgrade_repeat_and_explicit_downgrade_keep_only_the_selected_version(self):
        self.assertEqual(self.install("--version", VERSION).returncode, 0)
        result = self.activate_next_release()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_active(self.next_version)
        self.assertFalse((self.root / "versions" / self.stem).exists())
        self.assertEqual(self.install().returncode, 0)
        self.assertEqual((self.root / "profile").read_text().count("# Nagi installer"), 1)
        result = self.install("--version", VERSION)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_active(VERSION)
        self.assertEqual([p.name for p in (self.root / "versions").iterdir()], [self.stem])

    def test_original_install_without_receipts_migrates(self):
        versions = self.root / "versions"
        versions.mkdir()
        with tarfile.open(self.assets / (self.stem + ".tar.gz")) as archive:
            archive.extractall(versions, filter="data")
        (self.root / "bin").mkdir()
        (self.root / "bin/nagic").symlink_to(versions / self.stem / "nagic")
        result = self.activate_next_release()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((versions / self.stem).exists())
        self.assert_active(self.next_version)

    def check_old_tree_is_preserved(self, change):
        self.assertEqual(self.install().returncode, 0)
        old = self.root / "versions" / self.stem
        change(old)
        result = self.activate_next_release()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Kept changed or unverifiable", result.stderr)
        self.assertTrue((old / "runtime/src/lib.rs").exists())
        self.assert_active(self.next_version)
        return old

    def test_modified_runtime_is_kept(self):
        old = self.check_old_tree_is_preserved(lambda p: (p / "runtime/src/lib.rs").write_text("user changes"))
        self.assertEqual((old / "runtime/src/lib.rs").read_text(), "user changes")

    def test_added_file_is_kept(self):
        old = self.check_old_tree_is_preserved(lambda p: (p / "notes.txt").write_text("user notes"))
        self.assertEqual((old / "notes.txt").read_text(), "user notes")

    def test_added_empty_directory_is_kept(self):
        old = self.check_old_tree_is_preserved(lambda p: (p / "my-project").mkdir())
        self.assertTrue((old / "my-project").is_dir())

    def test_added_symlink_never_follows_or_removes_the_target(self):
        outside = self.root / "outside"
        outside.write_text("keep")
        old = self.check_old_tree_is_preserved(lambda p: (p / "outside-link").symlink_to(outside))
        self.assertTrue((old / "outside-link").is_symlink())
        self.assertEqual(outside.read_text(), "keep")

    def test_unavailable_old_archive_does_not_prevent_upgrade_or_delete_old_files(self):
        self.assertEqual(self.install().returncode, 0)
        (self.assets / (self.stem + ".tar.gz")).unlink()
        result = self.activate_next_release()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.root / "versions" / self.stem).exists())
        self.assert_active(self.next_version)

    def test_failed_download_or_activation_preserves_current_command_and_profile(self):
        for failure in ("corrupt", "activation_failure"):
            with self.subTest(failure=failure):
                self.assertEqual(self.install("--version", VERSION).returncode, 0)
                profile = (self.root / "profile").read_bytes()
                result = self.activate_next_release(**{failure: True})
                self.assertNotEqual(result.returncode, 0)
                self.assert_active(VERSION)
                self.assertEqual((self.root / "profile").read_bytes(), profile)

    def test_profile_failure_rolls_back_the_active_command(self):
        self.assertEqual(self.install().returncode, 0)
        (self.root / "profile").unlink()
        (self.root / "profile").mkdir()
        result = self.activate_next_release()
        self.assertNotEqual(result.returncode, 0)
        self.assert_active(VERSION)
        self.assertTrue((self.root / "profile").is_dir())

    def test_latest_cannot_resolve_to_vsix_prerelease_or_another_repository(self):
        for url in ("https://github.com/disnana/Nagi/releases/tag/vscode-v0.1.8",
                    "https://github.com/disnana/Nagi/releases/tag/nagi-v1.0.0-beta",
                    "https://example.com/releases/tag/nagi-v1.0.0"):
            with self.subTest(url=url):
                self.environment["NAGI_TEST_LATEST_URL"] = url
                self.assertNotEqual(self.install().returncode, 0)
                self.assertFalse((self.root / "executed").exists())

    def test_concurrent_installer_is_rejected_without_changing_the_active_version(self):
        self.assertEqual(self.install().returncode, 0)
        (self.root / "versions/.install-lock").mkdir()
        result = self.activate_next_release()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Another installer", result.stderr)
        self.assert_active(VERSION)


if __name__ == "__main__":
    unittest.main()
