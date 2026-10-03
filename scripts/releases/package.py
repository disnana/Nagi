"""Package the prebuilt compiler and only the matching runtime sources."""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import re
import subprocess
import tarfile
import tomllib
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

from plan import ROOT, version_tuple


PLATFORMS = ("linux-x86_64", "windows-x86_64", "macos-arm64", "macos-x86_64")


def runtime_manifest(text: str, workspace: dict) -> bytes:
    # The distributed runtime is independent of the development workspace.
    for key in ("version", "edition", "license"):
        text = re.sub(rf"(?m)^{key}\.workspace\s*=\s*true\s*$",
                      f"{key} = {json.dumps(workspace[key])}", text)
    text = re.sub(r"(?ms)^\[dev-dependencies\].*?(?=^\[|\Z)", "", text)
    parsed = tomllib.loads(text)
    if any(isinstance(value, dict) and value.get("workspace") for value in parsed["package"].values()):
        raise ValueError("Distributed runtime still inherits a workspace value")
    return (text.rstrip() + "\n\n[workspace]\n").encode()


def included(name: str) -> bool:
    return name == "LICENSE" or name == "runtime/Cargo.toml" or (
        name.startswith("runtime/src/") and not name.endswith(("/tests.rs", "_tests.rs"))
    )


def readme(version: str, executable: str) -> bytes:
    return f"""Nagi {version}

このフォルダーをPATHに追加し、{executable} --version で確認してください。
runtime/を含むフォルダー構成を保って使います。NAGI_ROOTは通常不要です。
アプリのビルドにはRust/CargoとCのビルド環境が必要です。
使い方: {executable} run --project /path/to/nagi.toml
Docs: https://disnana.github.io/Nagi/

Add this folder to PATH and run {executable} --version.
Keep runtime/ beside the compiler. NAGI_ROOT is normally unnecessary.
Building applications requires Rust/Cargo and a C build environment.
Usage: {executable} run --project /path/to/nagi.toml
Docs: https://disnana.github.io/Nagi/en/
Source: https://github.com/disnana/Nagi
""".encode()


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
    workspace = tomllib.loads(manifest)["workspace"]["package"]
    if workspace["version"] != version:
        raise ValueError("Archive version must match the tested commit's Cargo.toml")
    source = subprocess.check_output(["git", "archive", "--format=tar", sha, "--", "runtime/Cargo.toml", "runtime/src", "LICENSE"], cwd=ROOT)
    binary_data = binary.read_bytes()
    output.mkdir(parents=True, exist_ok=True)
    stem = f"nagi-{version}-{platform}"
    executable = "nagic.exe" if platform == "windows-x86_64" else "nagic"
    binary_path = f"{stem}/{executable}"
    provenance = json.dumps({"version": version, "platform": platform, "commit": sha}, indent=2).encode() + b"\n"
    windows = platform == "windows-x86_64"
    destination = output / filename
    with tarfile.open(fileobj=io.BytesIO(source), mode="r:") as sources:
        if windows:
            with ZipFile(destination, "w", ZIP_DEFLATED) as archive:
                for member in sources.getmembers():
                    if member.isdir():
                        continue
                    if not included(member.name):
                        continue
                    if member.isfile():
                        info = ZipInfo(f"{stem}/{member.name}")
                        info.compress_type = ZIP_DEFLATED
                        info.external_attr = (0o100000 | member.mode) << 16
                        data = sources.extractfile(member).read()
                        if member.name == "runtime/Cargo.toml":
                            data = runtime_manifest(data.decode(), workspace)
                        archive.writestr(info, data)
                    elif not member.isdir():
                        raise ValueError(f"Unsupported tracked source entry: {member.name}")
                archive.writestr(binary_path, binary_data)
                archive.writestr(f"{stem}/release.json", provenance)
                archive.writestr(f"{stem}/README.txt", readme(version, executable))
        else:
            with tarfile.open(destination, "w:gz") as archive:
                for member in sources.getmembers():
                    if member.isdir():
                        continue
                    if not included(member.name):
                        continue
                    if not member.isfile():
                        raise ValueError(f"Unsupported tracked source entry: {member.name}")
                    data = sources.extractfile(member).read()
                    if member.name == "runtime/Cargo.toml":
                        data = runtime_manifest(data.decode(), workspace)
                    member.size = len(data)
                    member.name = f"{stem}/{member.name}"
                    archive.addfile(member, io.BytesIO(data))
                for name, data, mode in ((binary_path, binary_data, 0o755), (f"{stem}/release.json", provenance, 0o644), (f"{stem}/README.txt", readme(version, executable), 0o644)):
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
