"""Exercise the actual VSIX packager without writing into the checkout."""
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from zipfile import ZipFile


ROOT = Path(__file__).resolve().parents[2]


class VsixPackagingTests(unittest.TestCase):
    def test_identical_sources_ignore_checkout_metadata_and_wall_clock(self):
        with tempfile.TemporaryDirectory(prefix="nagi vsix 凪 ") as directory:
            root = Path(directory)
            extension = root / "editors/vscode-nagi"
            script = extension / "scripts/package_vsix.py"
            script.parent.mkdir(parents=True)
            shutil.copyfile(ROOT / "editors/vscode-nagi/scripts/package_vsix.py", script)
            files = {
                "package.json": json.dumps({
                    "version": "0.1.12", "name": "nagi-lang", "publisher": "Disnana",
                    "displayName": "Nagi", "description": "Nagi language support",
                    "engines": {"vscode": "^1.85.0"},
                }),
                "README.md": "# Nagi\n",
                "language-configuration.json": "{}\n",
                "low-configuration.json": "{}\n",
                "src/extension.js": "exports.activate = () => {};\n",
                "syntaxes/nagi.tmLanguage.json": "{}\n",
                "snippets/nagi.json": "{}\n",
            }
            for name, contents in files.items():
                file = extension / name
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_text(contents, encoding="utf-8")
            (root / "LICENSE").write_text("MIT license fixture\n", encoding="utf-8")
            # Files outside the package allowlist must remain outside the VSIX.
            (extension / "private-note.txt").write_text("private fixture", encoding="utf-8")
            test = extension / "test/test.js"
            test.parent.mkdir()
            test.write_text("development test", encoding="utf-8")
            output = root / "build/distribution/nagi-language-0.1.12.vsix"
            snapshots = []
            for clock in (1700000000, 1800000000):
                for file in root.rglob("*"):
                    if file.is_file():
                        os.utime(file, (clock, clock))
                subprocess.run([
                    sys.executable, "-c",
                    "import runpy, sys\nfrom unittest.mock import patch\n"
                    "with patch('zipfile.time.time', return_value=int(sys.argv[2])):\n"
                    "    runpy.run_path(sys.argv[1], run_name='__main__')\n",
                    str(script), str(clock),
                ], check=True, capture_output=True)
                snapshots.append(output.read_bytes())
            self.assertEqual(hashlib.sha256(snapshots[0]).digest(),
                             hashlib.sha256(snapshots[1]).digest())
            with ZipFile(output) as archive:
                self.assertIsNone(archive.testzip())
                self.assertEqual(set(archive.namelist()), {
                    "extension.vsixmanifest", "[Content_Types].xml", "extension/LICENSE.txt",
                    *("extension/" + name for name in files),
                })
                self.assertTrue(all(entry.date_time == (1980, 1, 1, 0, 0, 0)
                                    and entry.external_attr >> 16 == 0o100644
                                    for entry in archive.infolist()))
                self.assertEqual(archive.read("extension/LICENSE.txt"), b"MIT license fixture\n")


if __name__ == "__main__":
    unittest.main()
