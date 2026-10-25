"""Package the prebuilt compiler and only the matching runtime sources."""
from __future__ import annotations

import argparse
import gzip
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
import third_party_notices


PLATFORMS = ("linux-x86_64", "windows-x86_64", "macos-arm64", "macos-x86_64")
PROJECT_TOOLS = (
    ("scripts/generate_project_ci.py", "tools/generate_project_ci.py"),
    ("scripts/verify_build_receipt.py", "tools/verify_build_receipt.py"),
)


def zip_entry(name: str, data: bytes, mode: int = 0o644) -> tuple[ZipInfo, bytes]:
    # ZIP's epoch avoids checkout times and makes interrupted releases retryable.
    info = ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
    info.compress_type = ZIP_DEFLATED
    info.create_system = 3
    info.external_attr = (0o100000 | mode) << 16
    return info, data


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
    return name in {"LICENSE", "runtime/Cargo.toml", "runtime-core/Cargo.toml", "runtime-core/build.rs"} or (
        name.startswith("runtime/src/") and not name.endswith(("/tests.rs", "_tests.rs"))
    )


SEED_DIRECTORY = "runtime-seed"
SEED_INPUTS = ("runtime/Cargo.toml", "runtime-core/Cargo.toml", "runtime-core/build.rs", "runtime/src/lib.rs")


def seed_payload(source: bytes, workspace: dict) -> dict[str, bytes]:
    """Bind the reviewed lock to the exact distributed runtime manifests/source."""
    lock = _archive_member(source, "Cargo.lock")
    payload = {f"{SEED_DIRECTORY}/Cargo.lock": lock}
    inputs = dict(payload)
    for name in SEED_INPUTS:
        data = _archive_member(source, name)
        if name.endswith("Cargo.toml"):
            data = runtime_manifest(data.decode("utf-8"), workspace)
        inputs[name] = data
    descriptor = {"schema_version": 1, "runtime_version": workspace["version"],
                  "files": {name: {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
                            for name, data in sorted(inputs.items())}}
    payload[f"{SEED_DIRECTORY}/seed.json"] = (json.dumps(descriptor, sort_keys=True, indent=2) + "\n").encode("utf-8")
    return payload


def project_tool_payload(source: bytes) -> dict[str, bytes]:
    """Read only the explicitly mapped regular tool files from the tested commit."""
    with tarfile.open(fileobj=io.BytesIO(source), mode="r:") as archive:
        members = {member.name: member for member in archive.getmembers()}
        payload = {}
        for source_name, destination_name in PROJECT_TOOLS:
            member = members.get(source_name)
            if member is None:
                if source_name + "/" in members:
                    raise ValueError(f"Release tool is not a regular file: {source_name}")
                raise ValueError(f"Release source is missing required tool {source_name}")
            if not member.isfile() or member.type not in (tarfile.REGTYPE, tarfile.AREGTYPE):
                raise ValueError(f"Release tool is not a regular file: {source_name}")
            contents = archive.extractfile(member)
            if contents is None:
                raise ValueError(f"Could not read release tool {source_name}")
            payload[destination_name] = contents.read()
        return payload


def validate_project_tool_sources(root: Path, commit: str) -> None:
    """Require each mapped source path to be a regular Git blob in the commit."""
    for source_name, _ in PROJECT_TOOLS:
        listing = subprocess.check_output(
            ["git", "ls-tree", "-z", commit, "--", source_name], cwd=root)
        entry = next((row for row in listing.split(b"\0") if row), None)
        if entry is None:
            raise ValueError(f"Release source is missing required tool {source_name}")
        metadata, encoded_path = entry.split(b"\t", 1)
        mode, kind, _ = metadata.split(b" ", 2)
        path = encoded_path.decode("utf-8").rstrip("/")
        if path != source_name:
            raise ValueError(f"Release source is missing required tool {source_name}")
        if kind != b"blob" or mode not in (b"100644", b"100755"):
            raise ValueError(f"Release tool is not a regular file: {source_name}")


def readme(version: str, executable: str) -> bytes:
    return f"""Nagi {version}

このフォルダーをPATHに追加し、{executable} --version で確認してください。
runtime/を含むフォルダー構成を保って使います。NAGI_ROOTは通常不要です。
アプリのビルドにはRust/CargoとCのビルド環境が必要です。
使い方: {executable} run --project /path/to/nagi.toml
Third-party notices: runtime/THIRD-PARTY-NOTICES/README.md
Docs: https://nagi.disnana.com/
任意のtoolsスクリプトにはPython 3.10以降が必要です。
CI workflow生成: python tools/generate_project_ci.py > nagi-ci.yml
build receipt照合: python tools/verify_build_receipt.py path/to/build-receipt.json --compiler path/to/nagic

Add this folder to PATH and run {executable} --version.
Keep runtime/ beside the compiler. NAGI_ROOT is normally unnecessary.
Building applications requires Rust/Cargo and a C build environment.
Usage: {executable} run --project /path/to/nagi.toml
Third-party notices: runtime/THIRD-PARTY-NOTICES/README.md
Docs: https://nagi.disnana.com/en/
Source: https://github.com/disnana/Nagi
Optional tools scripts require Python 3.10 or newer.
Generate a CI workflow: python tools/generate_project_ci.py > nagi-ci.yml
Verify a build receipt: python tools/verify_build_receipt.py path/to/build-receipt.json --compiler path/to/nagic
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
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip()
    if head != sha:
        raise ValueError("Release package source and compiler binary must use the checked-out commit")
    manifest = subprocess.check_output(["git", "show", f"{sha}:Cargo.toml"], cwd=ROOT).decode()
    workspace = tomllib.loads(manifest)["workspace"]["package"]
    if workspace["version"] != version:
        raise ValueError("Archive version must match the tested commit's Cargo.toml")
    target = third_party_notices.PLATFORM_TARGETS[platform]
    validate_project_tool_sources(ROOT, sha)
    source = subprocess.check_output(
        ["git", "archive", "--format=tar", sha, "--", "Cargo.toml", "Cargo.lock",
         "runtime/Cargo.toml", "runtime/src", "runtime-core", "LICENSE",
         *(source_name for source_name, _ in PROJECT_TOOLS)], cwd=ROOT)
    tool_payload = project_tool_payload(source)
    bundled_seed = seed_payload(source, workspace)
    core_manifest = tomllib.loads(_archive_member(source, "runtime-core/Cargo.toml").decode())
    if (core_manifest.get("package", {}).get("name") != "nagi-runtime-core"
            or core_manifest.get("lib", {}).get("path") != "../runtime/src/lib.rs"
            or core_manifest.get("package", {}).get("build") != "build.rs"):
        raise ValueError("Core runtime must use the fixed shared runtime source and build script")
    _archive_member(source, "runtime/src/lib.rs")
    _archive_member(source, "runtime-core/build.rs")
    binary_data = binary.read_bytes()
    metadata = third_party_notices.cargo_metadata(ROOT, target)
    third_party_notices.validate_workspace_inputs(ROOT, sha, metadata,
                                                   _archive_member(source, "Cargo.lock"))
    graphs = {name: third_party_notices.cargo_tree(ROOT, name, target)
              for name in third_party_notices.ROOT_ROLES}
    notice_files = third_party_notices.build_notice_payload(
        _archive_member(source, "Cargo.lock"), metadata, graphs, target, sha,
        hashlib.sha256(binary_data).hexdigest())
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
                        data = sources.extractfile(member).read()
                        if member.name in {"runtime/Cargo.toml", "runtime-core/Cargo.toml"}:
                            data = runtime_manifest(data.decode(), workspace)
                        archive.writestr(*zip_entry(f"{stem}/{member.name}", data, member.mode))
                    elif not member.isdir():
                        raise ValueError(f"Unsupported tracked source entry: {member.name}")
                for name, data in (tool_payload | bundled_seed).items():
                    archive.writestr(*zip_entry(f"{stem}/{name}", data))
                for name, data in notice_files.items():
                    archive.writestr(*zip_entry(f"{stem}/{name}", data))
                archive.writestr(*zip_entry(binary_path, binary_data, 0o755))
                archive.writestr(*zip_entry(f"{stem}/release.json", provenance))
                archive.writestr(*zip_entry(f"{stem}/README.txt", readme(version, executable)))
        else:
            # Suppress the gzip filename and clock; tar source metadata comes
            # from git archive at the immutable release commit.
            with destination.open("wb") as file, gzip.GzipFile(
                fileobj=file, filename="", mode="wb", mtime=0
            ) as compressed, tarfile.open(fileobj=compressed, mode="w:") as archive:
                for member in sources.getmembers():
                    if member.isdir():
                        continue
                    if not included(member.name):
                        continue
                    if not member.isfile():
                        raise ValueError(f"Unsupported tracked source entry: {member.name}")
                    data = sources.extractfile(member).read()
                    if member.name in {"runtime/Cargo.toml", "runtime-core/Cargo.toml"}:
                        data = runtime_manifest(data.decode(), workspace)
                    member.size = len(data)
                    member.name = f"{stem}/{member.name}"
                    archive.addfile(member, io.BytesIO(data))
                for name, data in (tool_payload | bundled_seed).items():
                    member = tarfile.TarInfo(f"{stem}/{name}")
                    member.size = len(data)
                    member.mode = 0o644
                    member.mtime = 0
                    archive.addfile(member, io.BytesIO(data))
                for name, data in notice_files.items():
                    member = tarfile.TarInfo(f"{stem}/{name}")
                    member.size = len(data)
                    member.mode = 0o644
                    archive.addfile(member, io.BytesIO(data))
                for name, data, mode in ((binary_path, binary_data, 0o755), (f"{stem}/release.json", provenance, 0o644), (f"{stem}/README.txt", readme(version, executable), 0o644)):
                    member = tarfile.TarInfo(name)
                    member.size = len(data)
                    member.mode = mode
                    archive.addfile(member, io.BytesIO(data))
    _verify_notice_archive(destination, platform, stem, notice_files,
                           _archive_member(source, "Cargo.lock"), target, sha,
                           hashlib.sha256(binary_data).hexdigest())
    _verify_seed_archive(destination, platform, stem, bundled_seed)
    checksum = hashlib.sha256(destination.read_bytes()).hexdigest()
    destination.with_name(destination.name + ".sha256").write_text(f"{checksum}  {destination.name}\n", encoding="utf-8")
    return destination


def _archive_member(source: bytes, name: str) -> bytes:
    try:
        with tarfile.open(fileobj=io.BytesIO(source), mode="r:") as archive:
            member = archive.getmember(name)
            if not member.isfile():
                raise ValueError("Cargo.lock archive member is not a file")
            return archive.extractfile(member).read()
    except (KeyError, OSError, tarfile.TarError) as error:
        raise ValueError(f"Release source is missing {name}") from error


def _verify_notice_archive(archive_path: Path, platform: str, stem: str,
                           expected: dict[str, bytes], lock_data: bytes,
                           target: str, commit: str, binary_sha256: str) -> None:
    prefix = f"{stem}/"
    notice_prefix = prefix + third_party_notices.NOTICE_ROOT + "/"
    actual = {}
    if platform == "windows-x86_64":
        with ZipFile(archive_path) as archive:
            for name in archive.namelist():
                if name.startswith(notice_prefix):
                    actual[name.removeprefix(prefix)] = archive.read(name)
    else:
        with tarfile.open(archive_path, mode="r:gz") as archive:
            for member in archive.getmembers():
                if member.isfile() and member.name.startswith(notice_prefix):
                    actual[member.name.removeprefix(prefix)] = archive.extractfile(member).read()
    if actual != expected:
        raise ValueError("Release archive third-party notice files differ from the validated payload")
    third_party_notices.validate_notice_payload(
        actual, lock_data, target, commit, binary_sha256)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--platform", required=True, choices=PLATFORMS)
    parser.add_argument("--out", type=Path, default=ROOT / "build/distribution")
    args = parser.parse_args()
    print(package(args.binary, args.version, args.platform, args.out))


def _verify_seed_archive(path: Path, platform: str, stem: str, expected: dict[str, bytes]) -> None:
    prefix = f"{stem}/runtime-seed/"
    actual = {}
    if platform == "windows-x86_64":
        with ZipFile(path) as archive:
            for member in archive.infolist():
                if member.filename.startswith(prefix):
                    key = member.filename.removeprefix(stem + "/")
                    if member.is_dir() or key in actual or (member.external_attr >> 16) & 0o170000 not in (0, 0o100000):
                        raise ValueError("Bundled runtime seed requires unique regular files")
                    actual[key] = archive.read(member)
    else:
        with tarfile.open(path, "r:gz") as archive:
            for member in archive.getmembers():
                if member.name.startswith(prefix):
                    key = member.name.removeprefix(stem + "/")
                    if not member.isfile() or key in actual:
                        raise ValueError("Bundled runtime seed requires unique regular files")
                    actual[key] = archive.extractfile(member).read()
    if actual != expected:
        raise ValueError("Bundled runtime seed differs from the reviewed payload")


if __name__ == "__main__":
    main()
