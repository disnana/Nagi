"""Run the shell installer with local release assets; never use a user's HOME."""
import hashlib
import io
import json
import os
import platform
import re
import subprocess
import tarfile
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]


class InstallerVersionTests(unittest.TestCase):
    def test_defaults_install_the_current_compiler_release(self):
        bash = (ROOT / "scripts/install.sh").read_text()
        powershell = (ROOT / "scripts/install.ps1").read_text()
        self.assertEqual(re.search(r"(?m)^version=(\d+\.\d+\.\d+)$", bash)[1], VERSION)
        self.assertEqual(re.search(r"\[string\]\$Version = '(\d+\.\d+\.\d+)'", powershell)[1], VERSION)


@unittest.skipIf(os.name == "nt", "PowerShell installer has a separate Windows test")
class ShellInstallTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="nagi installer 凪 ' $() ")
        self.root = Path(self.temporary.name)
        system = "macos" if platform.system() == "Darwin" else "linux"
        architecture = "arm64" if platform.machine() in ("arm64", "aarch64") else "x86_64"
        self.platform = f"{system}-{architecture}"
        self.stem = f"nagi-{VERSION}-{self.platform}"
        self.assets = self.root / "assets"
        self.assets.mkdir()
        self.tools = self.root / "tools"
        self.tools.mkdir()
        curl = self.tools / "curl"
        curl.write_text('''#!/bin/sh
output=''
url=''
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o) output=$2; shift 2 ;;
        https://*) url=$1; shift ;;
        *) shift ;;
    esac
done
/bin/cp "$NAGI_TEST_ASSETS/${url##*/}" "$output"
''')
        curl.chmod(0o755)
        self.environment = {**os.environ, "PATH": str(self.tools) + os.pathsep + os.environ["PATH"],
                            "NAGI_TEST_ASSETS": str(self.assets), "NAGI_INSTALL_MARKER": str(self.root / "executed")}
        self.bundle()

    def tearDown(self):
        self.temporary.cleanup()

    def bundle(self, *, corrupt=False, unsafe=None, metadata_platform=None):
        archive = self.assets / (self.stem + ".tar.gz")
        metadata = {"version": VERSION, "platform": metadata_platform or self.platform, "commit": "a" * 40}
        files = {"nagic": f'#!/bin/sh\ntouch "$NAGI_INSTALL_MARKER"\necho "nagic {VERSION}"\n'.encode(),
                 "runtime/Cargo.toml": b"runtime fixture", "runtime/src/lib.rs": b"runtime fixture",
                 "release.json": (json.dumps(metadata, indent=2) + "\n").encode(),
                 "README.txt": b"installation notes", "LICENSE": b"license fixture"}
        with tarfile.open(archive, "w:gz") as output:
            for name, content in files.items():
                entry = tarfile.TarInfo(f"{self.stem}/{name}")
                entry.size = len(content)
                entry.mode = 0o755 if name == "nagic" else 0o644
                output.addfile(entry, io.BytesIO(content))
            if unsafe == "traversal":
                entry = tarfile.TarInfo(f"{self.stem}/../escaped")
                output.addfile(entry, io.BytesIO(b""))
            elif unsafe == "link":
                entry = tarfile.TarInfo(f"{self.stem}/runtime/link")
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
        result = subprocess.run(["bash", "--noprofile", "--norc", "-c", '. "$1"; nagic --version', "test", str(profile)],
                                env=environment, capture_output=True, text=True, check=True)
        self.assertEqual(result.stdout.strip(), f"nagic {VERSION}")
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


if __name__ == "__main__":
    unittest.main()
