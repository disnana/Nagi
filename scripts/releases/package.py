"""Package Git-tracked Nagi sources with the matching prebuilt compiler."""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import subprocess
import tarfile
import tomllib
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

from plan import ROOT, version_tuple


PLATFORMS = ("linux-x86_64", "windows-x86_64", "macos-arm64", "macos-x86_64")


def archive_name(version: str, platform: str) -> str:
    if platform not in PLATFORMS:
        raise ValueError(f"Unsupported release platform: {platform}")
    suffix = ".zip" if platform == "windows-x86_64" else ".tar.gz"
    return f"nagi-{version}-{platform}{suffix}"


def package(binary: Path, version: str, platform: str, output: Path, commit: str = "HEAD") -> Path:
    version_tuple(version)
    filename = archive_name(version, platform)
    sha = subprocess.check_output(["git", "rev-parse", "--verify", f"{commit}^{{commit}}"], cwd=ROOT).decode().strip()
    manifest = subprocess.check_output(["git", "show", f"{sha}:Cargo.toml"], cwd=ROOT).decode()
    if tomllib.loads(manifest)["workspace"]["package"]["version"] != version:
        raise ValueError("Archive version must match the tested commit's Cargo.toml")
    source = subprocess.check_output(["git", "archive", "--format=tar", sha], cwd=ROOT)
    binary_data = binary.read_bytes()
    output.mkdir(parents=True, exist_ok=True)
    stem = f"nagi-{version}-{platform}"
    executable = "nagic.exe" if platform == "windows-x86_64" else "nagic"
    binary_path = f"{stem}/target/release/{executable}"
    provenance = json.dumps({"version": version, "platform": platform, "commit": sha}, indent=2).encode() + b"\n"
    windows = platform == "windows-x86_64"
    destination = output / filename
    with tarfile.open(fileobj=io.BytesIO(source), mode="r:") as sources:
        if windows:
            with ZipFile(destination, "w", ZIP_DEFLATED) as archive:
                for member in sources.getmembers():
                    if member.isfile():
                        info = ZipInfo(f"{stem}/{member.name}")
                        info.compress_type = ZIP_DEFLATED
                        info.external_attr = (0o100000 | member.mode) << 16
                        archive.writestr(info, sources.extractfile(member).read())
                    elif not member.isdir():
                        raise ValueError(f"Unsupported tracked source entry: {member.name}")
                archive.writestr(binary_path, binary_data)
                archive.writestr(f"{stem}/release.json", provenance)
        else:
            with tarfile.open(destination, "w:gz") as archive:
                for member in sources.getmembers():
                    stream = sources.extractfile(member) if member.isfile() else None
                    member.name = f"{stem}/{member.name}"
                    archive.addfile(member, stream)
                for name, data, mode in ((binary_path, binary_data, 0o755), (f"{stem}/release.json", provenance, 0o644)):
                    member = tarfile.TarInfo(name)
                    member.size = len(data)
                    member.mode = mode
                    archive.addfile(member, io.BytesIO(data))
    checksum = hashlib.sha256(destination.read_bytes()).hexdigest()
    destination.with_name(destination.name + ".sha256").write_text(f"{checksum}  {destination.name}\n", encoding="utf-8")
    return destination


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--platform", required=True, choices=PLATFORMS)
    parser.add_argument("--out", type=Path, default=ROOT / "build/distribution")
    args = parser.parse_args()
    print(package(args.binary, args.version, args.platform, args.out))


if __name__ == "__main__":
    main()
