"""Verify a distribution by using PATH from an unrelated project directory."""
import argparse
import hashlib
import json
import os
import subprocess
import tarfile
import tempfile
from pathlib import Path
from zipfile import ZipFile


def verify(archive: Path, version: str, platform: str, target: Path | None = None) -> None:
    archive = archive.resolve()
    target = target.resolve() if target is not None else None
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    assert archive.with_name(archive.name + ".sha256").read_text().strip() == f"{digest}  {archive.name}"
    stem = f"nagi-{version}-{platform}"
    with tempfile.TemporaryDirectory(prefix="nagi distribution ") as folder:
        folder = Path(folder)
        if archive.suffix == ".zip":
            with ZipFile(archive) as source:
                source.extractall(folder)
        else:
            with tarfile.open(archive) as source:
                source.extractall(folder, filter="data")
        root = folder / stem
        metadata = json.loads((root / "release.json").read_text())
        assert metadata["version"] == version and metadata["platform"] == platform, metadata
        exe = root / ("nagic.exe" if os.name == "nt" else "nagic")
        environment = dict(os.environ)
        environment.pop("NAGI_ROOT", None)
        environment.pop("NAGI_NATIVE_TARGET_DIR", None)
        if target is not None:
            environment["NAGI_NATIVE_TARGET_DIR"] = str(target)
        for flag in ("--version", "-V", "version", "--help"):
            result = subprocess.run([str(exe), flag], cwd=folder, env={**environment, "PATH": ""},
                                    check=True, capture_output=True, text=True, encoding="utf-8")
            assert not result.stderr, result.stderr
            if flag == "--help":
                assert "--version" in result.stdout and "--project" in result.stdout, result.stdout
            else:
                assert result.stdout.strip() == f"nagic {version}", result.stdout
        project = folder / "outside project 凪"
        project.mkdir()
        (project / "nagi.toml").write_text("entry='main.nagi'\n", encoding="utf-8")
        marker = folder.name
        (project / "main.nagi").write_text(
            'def main():\n    print("Hello, Nagi!")\n    print(2 + 2)\n'
            f'    print({json.dumps(marker)})\n', encoding="utf-8")
        environment["PATH"] = str(root) + os.pathsep + environment["PATH"]
        # Windows resolves an executable using the parent's PATH. Update this
        # verification process too, so the bare command tests PATH on every OS.
        previous_path = os.environ["PATH"]
        os.environ["PATH"] = environment["PATH"]
        try:
            result = subprocess.run(["nagic", "run", "--project", str(project / "nagi.toml")],
                                    cwd=folder, env=environment, check=True, capture_output=True, text=True, encoding="utf-8")
            assert result.stdout.splitlines()[-3:] == ["Hello, Nagi!", "4", marker], result.stdout
            assert not (root / "native-target").exists(), "Build wrote into the installed distribution"
            print(f"Verified {version} {platform}: PATH, external project, version, help")
        finally:
            os.environ["PATH"] = previous_path


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--target", type=Path, help="Reuse native dependencies in this build directory")
    args = parser.parse_args()
    verify(args.archive, args.version, args.platform, args.target)
