"""Build lock-bound third-party source notices for Nagi release archives."""
from __future__ import annotations

import gzip
import hashlib
import io
import json
import os
import re
import stat
import subprocess
import tarfile
import tomllib
import unicodedata
import zlib
from pathlib import Path, PurePosixPath
from typing import Mapping, Sequence


PLATFORM_TARGETS = {
    "linux-x86_64": "x86_64-unknown-linux-gnu",
    "windows-x86_64": "x86_64-pc-windows-msvc",
    "macos-arm64": "aarch64-apple-darwin",
    "macos-x86_64": "x86_64-apple-darwin",
}
NOTICE_ROOT = "runtime/THIRD-PARTY-NOTICES"
MANIFEST_PATH = f"{NOTICE_ROOT}/manifest.json"
README_PATH = f"{NOTICE_ROOT}/README.md"
TEXT_ROOT = f"{NOTICE_ROOT}/texts/sha256/"
ROOT_ROLES = {
    "nagic": "nagic-binary",
    "nagi-runtime": "nagi-runtime-source",
}
SPDX_LICENSE_IDS = frozenset({
    "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "BSL-1.0", "CC0-1.0",
    "ISC", "LGPL-2.1-or-later", "MIT", "MIT-0", "Unicode-3.0",
    "Unlicense", "Zlib",
})
SPDX_EXCEPTION_IDS = frozenset({"LLVM-exception"})
SPDX_TOKEN = re.compile(r"\s*(AND\b|OR\b|WITH\b|\(|\)|[A-Za-z0-9][A-Za-z0-9.-]*)")
LICENSE_FILE = re.compile(r"(?i)^(?:license|licence|copying)(?:[._-].*)?$")
ATTRIBUTION_FILE = re.compile(r"(?i)^(?:authors?|notice|copyright|patents?)(?:[._-].*)?$")
TREE_PACKAGE = re.compile(r"^([A-Za-z0-9_-]+) v([^\s(]+)(?: \([^)]*\))?$ ".rstrip())
HASH = re.compile(r"^[0-9a-f]{64}$")
MAX_CRATE_ARCHIVE_BYTES = 64 * 1024 * 1024
MAX_CRATE_MEMBER_COUNT = 100_000
MAX_CRATE_UNPACKED_BYTES = 128 * 1024 * 1024
MAX_LICENSE_SOURCE_BYTES = 1024 * 1024
COMMIT = re.compile(r"^[0-9a-f]{40}$")


class NoticeError(ValueError):
    """A locked dependency graph cannot be represented safely in notices."""


class _BoundedDecompressedReader:
    def __init__(self, stream, maximum: int):
        self.stream = stream
        self.maximum = maximum
        self.bytes_read = 0

    def read(self, size: int = -1) -> bytes:
        remaining = self.maximum - self.bytes_read
        request = remaining + 1 if size is None or size < 0 else min(size, remaining + 1)
        data = self.stream.read(request)
        self.bytes_read += len(data)
        if self.bytes_read > self.maximum:
            raise NoticeError("Cargo crate archive exceeds unpacked size limit")
        return data


def cargo_metadata(root: Path, target: str) -> dict:
    if target not in PLATFORM_TARGETS.values():
        raise NoticeError("Unsupported release target for notice metadata")
    command = ["cargo", "metadata", "--locked", "--offline", "--format-version", "1",
               "--filter-platform", target, "--manifest-path", str(root / "Cargo.toml")]
    try:
        result = subprocess.run(command, cwd=root, check=False, capture_output=True, text=True)
    except OSError as error:
        raise NoticeError("Cargo metadata could not be started for notice generation") from error
    if result.returncode != 0:
        raise NoticeError("Cargo metadata failed for the locked release graph")
    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise NoticeError("Cargo metadata returned invalid JSON") from error
    if not isinstance(data, dict):
        raise NoticeError("Cargo metadata returned an invalid graph")
    return data


def parse_cargo_tree(output: str) -> list[tuple[str, str]]:
    packages = []
    seen = set()
    for raw_line in output.splitlines():
        line = raw_line.removesuffix(" (*)").strip()
        if not line:
            continue
        match = TREE_PACKAGE.fullmatch(line)
        if not match:
            raise NoticeError("Cargo tree output contained an unrecognized package row")
        item = (match.group(1), match.group(2))
        if item not in seen:
            seen.add(item)
            packages.append(item)
    if not packages:
        raise NoticeError("Cargo tree returned an empty release graph")
    return packages


def cargo_tree(root: Path, package: str, target: str) -> list[tuple[str, str]]:
    if package not in ROOT_ROLES or target not in PLATFORM_TARGETS.values():
        raise NoticeError("Unsupported release root or target for notice graph")
    command = ["cargo", "--color", "never", "tree", "--locked", "--offline", "--manifest-path", str(root / "Cargo.toml"),
               "--package", package, "--edges", "normal,build", "--target", target,
               "--format", "{p}", "--prefix", "none", "--quiet"]
    try:
        result = subprocess.run(command, cwd=root, check=False, capture_output=True, text=True)
    except OSError as error:
        raise NoticeError("Cargo tree could not be started for notice generation") from error
    if result.returncode != 0:
        raise NoticeError(f"Cargo tree failed for the {package} release graph")
    return parse_cargo_tree(result.stdout)


def parse_spdx_expression(expression: str) -> tuple[list[str], list[str]]:
    if not isinstance(expression, str) or not expression.strip():
        raise NoticeError("Selected dependency has no SPDX license expression")
    normalized = expression.strip()
    if normalized == "MIT/Apache-2.0":
        normalized = "MIT OR Apache-2.0"

    tokens = []
    offset = 0
    while offset < len(normalized):
        match = SPDX_TOKEN.match(normalized, offset)
        if not match:
            raise NoticeError("Selected dependency has an unrecognized SPDX expression")
        tokens.append(match.group(1))
        offset = match.end()
    index = 0
    licenses = set()
    exceptions = set()

    def take(expected: str | None = None) -> str:
        nonlocal index
        if index >= len(tokens):
            raise NoticeError("Selected dependency has a malformed SPDX expression")
        token = tokens[index]
        if expected is not None and token != expected:
            raise NoticeError("Selected dependency has a malformed SPDX expression")
        index += 1
        return token

    def parse_primary() -> None:
        token = take()
        if token == "(":
            parse_or()
            take(")")
            return
        if token in {"AND", "OR", "WITH", ")"}:
            raise NoticeError("Selected dependency has a malformed SPDX expression")
        if token not in SPDX_LICENSE_IDS:
            raise NoticeError("Selected dependency has an unknown SPDX license identifier")
        licenses.add(token)
        if index < len(tokens) and tokens[index] == "WITH":
            take("WITH")
            exception = take()
            if exception not in SPDX_EXCEPTION_IDS:
                raise NoticeError("Selected dependency has an unknown SPDX exception identifier")
            exceptions.add(exception)

    def parse_and() -> None:
        parse_primary()
        while index < len(tokens) and tokens[index] == "AND":
            take("AND")
            parse_primary()

    def parse_or() -> None:
        parse_and()
        while index < len(tokens) and tokens[index] == "OR":
            take("OR")
            parse_and()

    parse_or()
    if index != len(tokens) or not licenses:
        raise NoticeError("Selected dependency has a malformed SPDX expression")
    return sorted(licenses), sorted(exceptions)


def _safe_relative(path: str) -> str:
    if not isinstance(path, str) or not path or "\\" in path:
        raise NoticeError("License source path is invalid")
    pure = PurePosixPath(path)
    if pure.is_absolute() or any(part in {"", ".", ".."} for part in pure.parts):
        raise NoticeError("License source path is not package-relative")
    return pure.as_posix()


def _read_local_package_file(package_root: Path, relative: str) -> bytes:
    relative = _safe_relative(relative)
    current = package_root
    parts = PurePosixPath(relative).parts
    try:
        for index, part in enumerate(parts):
            current = current / part
            info = current.lstat()
            if stat.S_ISLNK(info.st_mode):
                raise NoticeError("License source uses a symlink")
            if index < len(parts) - 1 and not stat.S_ISDIR(info.st_mode):
                raise NoticeError("License source parent is not a directory")
        if not stat.S_ISREG(info.st_mode):
            raise NoticeError("License source is not a regular file")
        if info.st_size <= 0:
            raise NoticeError("License source is empty")
        if info.st_size > MAX_LICENSE_SOURCE_BYTES:
            raise NoticeError("License source exceeds size limit")
        data = current.read_bytes()
    except NoticeError:
        raise
    except OSError as error:
        raise NoticeError("License source path escapes or is missing from the package source") from error
    if not data or len(data) > MAX_LICENSE_SOURCE_BYTES or len(data) != info.st_size:
        raise NoticeError("License source changed or exceeds size limit while reading")
    return data


def _read_package_sources(package: dict, package_root: Path) -> list[dict]:
    try:
        resolved_root = package_root.resolve(strict=True)
    except OSError as error:
        raise NoticeError(f"License source root is missing for {package['name']}@{package['version']}") from error
    if not resolved_root.is_dir():
        raise NoticeError(f"License source root is invalid for {package['name']}@{package['version']}")

    documents: dict[str, tuple[str, bytes]] = {}

    def add(relative: str, role: str) -> None:
        relative = _safe_relative(relative)
        data = _read_local_package_file(resolved_root, relative)
        previous = documents.get(relative)
        if previous is not None and previous[0] != role:
            raise NoticeError("One package source file has conflicting notice roles")
        documents[relative] = (role, data)

    license_file = package.get("license_file")
    if license_file is not None:
        if not isinstance(license_file, str) or not license_file:
            raise NoticeError("Cargo metadata has an invalid license_file value")
        candidate = Path(license_file)
        if not candidate.is_absolute():
            candidate = resolved_root / candidate
        try:
            relative = candidate.relative_to(resolved_root).as_posix()
        except ValueError as error:
            raise NoticeError("License source path escapes or is missing from the package source") from error
        add(relative, "license")

    try:
        entries = list(resolved_root.iterdir())
    except OSError as error:
        raise NoticeError(f"License source root cannot be read for {package['name']}@{package['version']}") from error
    for entry in entries:
        if not entry.is_file() and not entry.is_symlink():
            continue
        if LICENSE_FILE.fullmatch(entry.name):
            add(entry.name, "license")
        elif ATTRIBUTION_FILE.fullmatch(entry.name):
            add(entry.name, "attribution")

    if not any(role == "license" for role, _ in documents.values()):
        raise NoticeError(f"No license text source for {package['name']}@{package['version']}")
    return [{"source_path": path, "role": role, "bytes": len(data), "data": data}
            for path, (role, data) in sorted(documents.items())]


def _crate_archive_path(package_root: Path, name: str, version: str) -> Path:
    if (not isinstance(name, str) or not re.fullmatch(r"[A-Za-z0-9_-]+", name)
            or not isinstance(version, str) or not re.fullmatch(r"[A-Za-z0-9.+-]+", version)):
        raise NoticeError("Registry package identity is invalid for crate archive lookup")
    try:
        resolved_root = package_root.resolve(strict=True)
    except OSError as error:
        raise NoticeError(f"Cargo source root is missing for {name}@{version}") from error
    if resolved_root.name != f"{name}-{version}":
        raise NoticeError(f"Cargo source directory identity differs for {name}@{version}")
    registry_id = resolved_root.parent
    src_directory = registry_id.parent
    registry_directory = src_directory.parent
    if src_directory.name != "src" or registry_directory.name != "registry":
        raise NoticeError(f"Cargo source directory is outside the registry cache for {name}@{version}")
    cache_root = registry_directory / "cache"
    cache_directory = cache_root / registry_id.name
    try:
        for directory in (registry_directory, cache_root, cache_directory):
            info = directory.lstat()
            if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
                raise NoticeError("Cargo registry cache path is not a real directory")
    except NoticeError:
        raise
    except OSError as error:
        raise NoticeError(f"Cargo crate archive cache is missing for {name}@{version}") from error
    archive = cache_directory / f"{name}-{version}.crate"
    try:
        info = archive.lstat()
    except OSError as error:
        raise NoticeError(f"Cargo crate archive is missing for {name}@{version}") from error
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISREG(info.st_mode):
        raise NoticeError("Cargo crate archive is not a regular file")
    if info.st_size <= 0 or info.st_size > MAX_CRATE_ARCHIVE_BYTES:
        raise NoticeError("Cargo crate archive exceeds size limit or is empty")
    return archive


def _read_locked_crate_archive(archive_path: Path, checksum: str) -> bytes:
    if not isinstance(checksum, str) or not HASH.fullmatch(checksum):
        raise NoticeError("Cargo.lock crate archive checksum is malformed")
    try:
        before = archive_path.lstat()
        if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
            raise NoticeError("Cargo crate archive is not a regular file")
        if before.st_size <= 0 or before.st_size > MAX_CRATE_ARCHIVE_BYTES:
            raise NoticeError("Cargo crate archive exceeds size limit or is empty")
        with archive_path.open("rb") as archive_file:
            opened = os.fstat(archive_file.fileno())
            if (not stat.S_ISREG(opened.st_mode) or opened.st_dev != before.st_dev
                    or opened.st_ino != before.st_ino):
                raise NoticeError("Cargo crate archive changed while being opened")
            data = archive_file.read(MAX_CRATE_ARCHIVE_BYTES + 1)
            if len(data) > MAX_CRATE_ARCHIVE_BYTES or archive_file.read(1):
                raise NoticeError("Cargo crate archive exceeds size limit")
        after = archive_path.lstat()
    except NoticeError:
        raise
    except OSError as error:
        raise NoticeError("Cargo crate archive cannot be read") from error
    if (len(data) != before.st_size or after.st_size != before.st_size
            or after.st_mtime_ns != before.st_mtime_ns or after.st_ino != before.st_ino):
        raise NoticeError("Cargo crate archive changed while being read")
    if hashlib.sha256(data).hexdigest() != checksum:
        raise NoticeError("Cargo crate archive checksum differs from Cargo.lock")
    return data


def _crate_member_relative(name: str, archive_root: str, is_directory: bool) -> str:
    if (not isinstance(name, str) or not name or "\\" in name or "\0" in name
            or name.startswith("/")):
        raise NoticeError("Unsafe crate archive member path")
    raw = name[:-1] if is_directory and name.endswith("/") else name
    parts = raw.split("/")
    if not raw or any(part in {"", ".", ".."} for part in parts) or parts[0] != archive_root:
        raise NoticeError("Unsafe crate archive member path")
    relative = "/".join(parts[1:])
    if not relative and not is_directory:
        raise NoticeError("Unsafe crate archive member path")
    return relative


def _verify_crate_source_bytes(name: str, version: str, source_root: Path, checksum: str,
                               source_files: Mapping[str, bytes]) -> None:
    archive_path = _crate_archive_path(source_root, name, version)
    archive_bytes = _read_locked_crate_archive(archive_path, checksum)
    archive_root = f"{name}-{version}"
    required = set(source_files)
    found: dict[str, bytes] = {}
    seen: set[str] = set()
    seen_platform_names: set[str] = set()
    total_members = 0
    total_unpacked = 0
    try:
        with gzip.GzipFile(fileobj=io.BytesIO(archive_bytes), mode="rb") as gzip_stream:
            bounded_stream = _BoundedDecompressedReader(gzip_stream, MAX_CRATE_UNPACKED_BYTES)
            with tarfile.open(fileobj=bounded_stream, mode="r|") as archive:
                for member in archive:
                    total_members += 1
                    if total_members > MAX_CRATE_MEMBER_COUNT:
                        raise NoticeError("Cargo crate archive has too many members")
                    if member.issym() or member.islnk():
                        raise NoticeError("Cargo crate archive contains links or unsupported member types")
                    is_directory = member.isdir()
                    relative = _crate_member_relative(member.name, archive_root, is_directory)
                    if relative in seen:
                        raise NoticeError("Cargo crate archive contains a duplicate crate archive member")
                    platform_name = unicodedata.normalize("NFC", relative).casefold()
                    if platform_name in seen_platform_names:
                        raise NoticeError("Cargo crate archive contains an ambiguous duplicate crate archive member")
                    seen.add(relative)
                    seen_platform_names.add(platform_name)
                    if not isinstance(member.size, int) or member.size < 0:
                        raise NoticeError("Cargo crate archive member has an invalid size")
                    if is_directory:
                        if member.size != 0:
                            raise NoticeError("Cargo crate archive directory has content")
                        continue
                    if not member.isfile():
                        raise NoticeError("Cargo crate archive contains links or unsupported member types")
                    total_unpacked += member.size
                    if total_unpacked > MAX_CRATE_UNPACKED_BYTES:
                        raise NoticeError("Cargo crate archive exceeds unpacked size limit")
                    if relative not in required:
                        continue
                    if member.size > MAX_LICENSE_SOURCE_BYTES:
                        raise NoticeError("Cargo crate source member exceeds size limit")
                    stream = archive.extractfile(member)
                    if stream is None:
                        raise NoticeError("Cargo crate source member cannot be read")
                    data = stream.read(MAX_LICENSE_SOURCE_BYTES + 1)
                    if len(data) != member.size or len(data) > MAX_LICENSE_SOURCE_BYTES:
                        raise NoticeError("Cargo crate source member is truncated or exceeds size limit")
                    found[relative] = data
                tar_reader = archive.fileobj
                if tar_reader is None:
                    raise NoticeError("Cargo crate archive reader is unavailable")
                while True:
                    trailing = tar_reader.read(64 * 1024)
                    if not trailing:
                        break
                    if any(trailing):
                        raise NoticeError("Cargo crate archive has non-zero trailing data")
    except NoticeError:
        raise
    except (OSError, EOFError, ValueError, tarfile.TarError, zlib.error) as error:
        raise NoticeError("Cargo crate archive is malformed or unreadable") from error
    missing = required - found.keys()
    if missing:
        raise NoticeError("Cargo crate archive is missing a selected source member")
    for relative, local_bytes in source_files.items():
        if found[relative] != local_bytes:
            raise NoticeError("Local source bytes differ from lock-verified crate")


def _lock_rows(lock_data: bytes) -> dict[tuple[str, str, str | None], dict]:
    try:
        document = tomllib.loads(lock_data.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise NoticeError("Cargo.lock is malformed") from error
    rows = document.get("package")
    if not isinstance(rows, list):
        raise NoticeError("Cargo.lock has no package rows")
    result = {}
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get("name"), str) or not isinstance(row.get("version"), str):
            raise NoticeError("Cargo.lock has a malformed package row")
        source = row.get("source")
        if source is not None and not isinstance(source, str):
            raise NoticeError("Cargo.lock has a malformed package source")
        key = (row["name"], row["version"], source)
        if key in result:
            raise NoticeError("Cargo.lock has a duplicate package identity")
        result[key] = row
    return result


def _metadata_packages(metadata: dict) -> tuple[dict[str, dict], set[str], set[str]]:
    packages = metadata.get("packages")
    members = metadata.get("workspace_members")
    resolve = metadata.get("resolve")
    nodes = resolve.get("nodes") if isinstance(resolve, dict) else None
    if not isinstance(packages, list) or not isinstance(members, list) or not isinstance(nodes, list):
        raise NoticeError("Cargo metadata is missing package, workspace, or resolution data")
    by_id = {}
    for package in packages:
        if not isinstance(package, dict) or not isinstance(package.get("id"), str):
            raise NoticeError("Cargo metadata has a malformed package record")
        if package["id"] in by_id:
            raise NoticeError("Cargo metadata has duplicate package IDs")
        by_id[package["id"]] = package
    node_ids = set()
    for node in nodes:
        if not isinstance(node, dict) or not isinstance(node.get("id"), str) or node["id"] in node_ids:
            raise NoticeError("Cargo metadata has malformed resolution nodes")
        node_ids.add(node["id"])
    if any(member not in by_id for member in members):
        raise NoticeError("Cargo metadata has an unknown workspace member")
    return by_id, set(members), node_ids


def _manifest_root(path: str, package_name: str, version: str) -> Path:
    if not isinstance(path, str) or not path:
        raise NoticeError(f"Cargo metadata has no source manifest for {package_name}@{version}")
    manifest = Path(path)
    try:
        resolved = manifest.resolve(strict=True)
    except OSError as error:
        raise NoticeError(f"Cargo source manifest is missing for {package_name}@{version}") from error
    if not resolved.is_file() or resolved.name != "Cargo.toml":
        raise NoticeError(f"Cargo source manifest is invalid for {package_name}@{version}")
    return resolved.parent


def _workspace_roots(metadata_by_id: Mapping[str, dict], workspace_members: set[str], node_ids: set[str], lock: dict) -> dict[str, dict]:
    result = {}
    for name in ROOT_ROLES:
        matches = [package for package in metadata_by_id.values()
                   if package.get("name") == name and package.get("id") in workspace_members]
        if len(matches) != 1:
            raise NoticeError(f"Cargo workspace must contain exactly one {name} package")
        package = matches[0]
        identity = (package.get("name"), package.get("version"), package.get("source"))
        if package.get("source") is not None or package["id"] not in node_ids:
            raise NoticeError(f"Cargo workspace root is not a local resolved package: {name}")
        if identity not in lock:
            raise NoticeError(f"Cargo.lock has no workspace package row for {name}")
        result[name] = package
    return result


def _byte_mismatch_details(left_name: str, left: bytes, right_name: str, right: bytes) -> str:
    first_difference_kind = "identical"
    for left_byte, right_byte in zip(left, right):
        if left_byte != right_byte:
            first_difference_kind = (
                "line-ending-byte" if {left_byte, right_byte} == {10, 13} else "content-byte")
            break
    else:
        if len(left) != len(right):
            first_difference_kind = "length-only"
    return (f"{left_name}_size={len(left)}; "
            f"{left_name}_sha256={hashlib.sha256(left).hexdigest()}; "
            f"{right_name}_size={len(right)}; "
            f"{right_name}_sha256={hashlib.sha256(right).hexdigest()}; "
            f"first_difference_kind={first_difference_kind}")


def validate_workspace_inputs(root: Path, source_commit: str, metadata: dict, lock_data: bytes) -> None:
    """Require Cargo's local graph inputs to be the exact committed files."""
    if not COMMIT.fullmatch(source_commit):
        raise NoticeError("Source commit for Cargo graph validation is invalid")
    root = root.resolve()
    workspace_root = metadata.get("workspace_root")
    try:
        resolved_workspace = Path(workspace_root).resolve(strict=True)
    except (TypeError, OSError) as error:
        raise NoticeError("Cargo metadata workspace root is invalid") from error
    if resolved_workspace != root:
        raise NoticeError("Cargo metadata belongs to a different workspace")
    try:
        current_lock = (root / "Cargo.lock").read_bytes()
    except OSError as error:
        raise NoticeError("Current Cargo.lock cannot be read") from error
    if current_lock != lock_data:
        raise NoticeError("Current Cargo.lock differs from the archived release source")

    metadata_by_id, workspace_members, _ = _metadata_packages(metadata)
    manifest_paths = {"Cargo.toml"}
    for member_id in workspace_members:
        package = metadata_by_id[member_id]
        manifest = _manifest_root(package.get("manifest_path"), package.get("name", "?"),
                                  package.get("version", "?")) / "Cargo.toml"
        try:
            relative = manifest.resolve(strict=True).relative_to(root).as_posix()
        except (OSError, ValueError) as error:
            raise NoticeError("Cargo workspace member manifest is outside the release checkout") from error
        manifest_paths.add(_safe_relative(relative))

    for relative in sorted(manifest_paths | {"Cargo.lock"}):
        try:
            current = (root / relative).read_bytes()
            committed = subprocess.check_output(
                ["git", "show", f"{source_commit}:{relative}"], cwd=root, stderr=subprocess.DEVNULL)
        except (OSError, subprocess.CalledProcessError) as error:
            raise NoticeError("Cargo graph input is absent from the selected source commit") from error
        if current != committed:
            details = _byte_mismatch_details("worktree", current, "commit", committed)
            raise NoticeError(
                f"Cargo graph input differs from the selected source commit ({relative}; {details})")
        if relative == "Cargo.lock" and committed != lock_data:
            details = _byte_mismatch_details("commit", committed, "archive", lock_data)
            raise NoticeError(
                f"Cargo.lock differs from the archived release source ({details})")


def _selected_registry_packages(lock_data: bytes, metadata: dict,
                                graph_packages: Mapping[str, Sequence[tuple[str, str]]]) -> tuple[dict, list[dict]]:
    lock = _lock_rows(lock_data)
    metadata_by_id, workspace_members, node_ids = _metadata_packages(metadata)
    roots = _workspace_roots(metadata_by_id, workspace_members, node_ids, lock)
    if set(graph_packages) != set(ROOT_ROLES):
        raise NoticeError("Cargo tree roots do not cover the compiler and runtime distributions")

    selected: dict[tuple[str, str, str], dict] = {}
    root_records = []
    for root_name, role in ROOT_ROLES.items():
        root = roots[root_name]
        tree = graph_packages[root_name]
        if not isinstance(tree, Sequence) or isinstance(tree, (str, bytes)):
            raise NoticeError(f"Cargo tree package rows are malformed for {root_name}")
        unique = set()
        for item in tree:
            if (not isinstance(item, (tuple, list)) or len(item) != 2
                    or not all(isinstance(value, str) and value for value in item)):
                raise NoticeError(f"Cargo tree package rows are malformed for {root_name}")
            unique.add((item[0], item[1]))
        if (root["name"], root["version"]) not in unique:
            raise NoticeError(f"Cargo tree omitted the {root_name} root package")

        registry_keys = set()
        for name, version in unique:
            matches = [package for package in metadata_by_id.values()
                       if package.get("name") == name and package.get("version") == version]
            if len(matches) != 1:
                raise NoticeError(f"Cargo metadata cannot uniquely bind {name}@{version}")
            package = matches[0]
            if package["id"] not in node_ids:
                raise NoticeError(f"Cargo metadata did not resolve {name}@{version}")
            source = package.get("source")
            if source is None and name in ROOT_ROLES:
                if package.get("id") not in workspace_members:
                    raise NoticeError(f"Unexpected local package in release graph: {name}@{version}")
                continue
            if not isinstance(source, str) or not source.startswith("registry+"):
                raise NoticeError(f"Unsupported dependency source for {name}@{version}")
            lock_key = (name, version, source)
            row = lock.get(lock_key)
            checksum = row.get("checksum") if row else None
            if not isinstance(checksum, str) or not HASH.fullmatch(checksum):
                raise NoticeError(f"Cargo.lock checksum is missing for {name}@{version}")
            key = (name, version, source)
            prior = selected.get(key)
            if prior is not None and prior["lock_checksum"] != checksum:
                raise NoticeError(f"Cargo.lock package identity conflicts for {name}@{version}")
            if prior is None:
                package_root = _manifest_root(package.get("manifest_path"), name, version)
                try:
                    manifest_bytes = _read_local_package_file(package_root, "Cargo.toml")
                    raw_manifest = tomllib.loads(manifest_bytes.decode("utf-8"))
                    raw_package = raw_manifest.get("package")
                except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
                    raise NoticeError(f"Cargo source manifest cannot be read for {name}@{version}") from error
                if (not isinstance(raw_package, dict) or raw_package.get("name") != name
                        or raw_package.get("version") != version):
                    raise NoticeError(f"Cargo source manifest identity differs for {name}@{version}")
                metadata_license = package.get("license")
                metadata_license_file = package.get("license_file")
                raw_license = raw_package.get("license")
                raw_license_file = raw_package.get("license-file")
                if metadata_license != raw_license:
                    raise NoticeError(f"Cargo SPDX metadata differs from source for {name}@{version}")
                if metadata_license is None:
                    raise NoticeError(f"No SPDX expression for {name}@{version}")
                if metadata_license_file is None and raw_license_file is not None:
                    raise NoticeError(f"Cargo license-file metadata is missing for {name}@{version}")
                if metadata_license_file is not None and raw_license_file is None:
                    raise NoticeError(f"Cargo license-file source is undeclared for {name}@{version}")
                if metadata_license_file is not None:
                    raw_path = Path(raw_license_file)
                    metadata_path = Path(metadata_license_file)
                    if not metadata_path.is_absolute():
                        metadata_path = package_root / metadata_path
                    if metadata_path.resolve() != (package_root / raw_path).resolve():
                        raise NoticeError(f"Cargo license-file path differs for {name}@{version}")
                license_ids, exception_ids = parse_spdx_expression(metadata_license)
                documents = _read_package_sources(package, package_root)
                source_files = {"Cargo.toml": manifest_bytes}
                source_files.update({document["source_path"]: document["data"] for document in documents})
                _verify_crate_source_bytes(name, version, package_root, checksum, source_files)
                selected[key] = {
                    "name": name,
                    "version": version,
                    "source": source,
                    "lock_checksum": checksum,
                    "declared_license_expression": metadata_license,
                    "license_ids": license_ids,
                    "exception_ids": exception_ids,
                    "license_file": metadata_license_file is not None,
                    "documents": documents,
                    "distribution_roles": set(),
                }
            selected[key]["distribution_roles"].add(role)
            registry_keys.add(key)
        root_records.append({
            "package": root_name,
            "version": root["version"],
            "distribution_role": role,
            "registry_packages": [
                {"name": name, "version": version,
                 "cargo_source": source,
                 "lock_checksum": selected[(name, version, source)]["lock_checksum"]}
                for name, version, source in sorted(registry_keys)
            ],
        })
    return selected, root_records


def build_notice_payload(lock_data: bytes, metadata: dict,
                         graph_packages: Mapping[str, Sequence[tuple[str, str]]], target: str,
                         source_commit: str, compiler_binary_sha256: str) -> dict[str, bytes]:
    if target not in PLATFORM_TARGETS.values():
        raise NoticeError("Unsupported target for notice manifest")
    if not isinstance(source_commit, str) or not COMMIT.fullmatch(source_commit):
        raise NoticeError("Source commit for notice manifest is invalid")
    if not isinstance(compiler_binary_sha256, str) or not HASH.fullmatch(compiler_binary_sha256):
        raise NoticeError("Compiler binary hash for notice manifest is invalid")

    packages, roots = _selected_registry_packages(lock_data, metadata, graph_packages)
    text_payloads: dict[str, bytes] = {}
    package_rows = []
    for key in sorted(packages):
        package = packages[key]
        notice_rows = []
        for document in package["documents"]:
            data = document["data"]
            digest = hashlib.sha256(data).hexdigest()
            path = f"{TEXT_ROOT}{digest}.txt"
            previous = text_payloads.get(path)
            if previous is not None and previous != data:
                raise NoticeError("Hash-addressed license text collision")
            text_payloads[path] = data
            notice_rows.append({"source_path": document["source_path"], "role": document["role"],
                                "sha256": digest, "bytes": len(data), "text_path": path})
        package_rows.append({
            "name": package["name"],
            "version": package["version"],
            "cargo_source": package["source"],
            "lock_checksum": package["lock_checksum"],
            "declared_license_expression": package["declared_license_expression"],
            "license_ids": package["license_ids"],
            "exception_ids": package["exception_ids"],
            "license_file_declared": package["license_file"],
            "distribution_roles": sorted(package["distribution_roles"]),
            "notice_texts": notice_rows,
        })

    files = [{"path": path, "sha256": path[len(TEXT_ROOT):-4], "bytes": len(data)}
             for path, data in sorted(text_payloads.items())]
    manifest = {
        "schema_version": 1,
        "scope": {
            "resolver": "cargo metadata and cargo tree; --locked --offline",
            "target": target,
            "dependency_edges": ["normal", "build"],
            "feature_selection": "Cargo default feature selection",
            "roots": roots,
            "limits": [
                "Cargo.lock is a verification input and is not included in the archive.",
                "Runtime source consumers may resolve a different future application graph.",
                "Non-default optional feature configurations may select dependencies not enumerated here.",
                "The compiler entry is its Cargo build graph, not a symbol-level link audit.",
            ],
        },
        "snapshot": {
            "source_commit": source_commit,
            "cargo_lock_sha256": hashlib.sha256(lock_data).hexdigest(),
            "target": target,
            "compiler_binary_sha256": compiler_binary_sha256,
        },
        "source_coverage": "all selected registry packages have at least one source notice",
        "legal_review": "not performed",
        "packages": package_rows,
        "files": files,
        "unresolved": [],
    }
    readme = (
        "Third-party license and attribution source inventory for this Nagi distribution.\n\n"
        f"Source commit: {source_commit}\nTarget: {target}\n"
        f"Cargo.lock SHA-256: {manifest['snapshot']['cargo_lock_sha256']}\n"
        f"Compiler binary SHA-256: {compiler_binary_sha256}\n\n"
        "The manifest distinguishes the nagic binary build graph from dependencies of the distributed runtime source. "
        "The runtime source archive does not include Cargo.lock; a later generated application may resolve a different graph. "
        "This inventory preserves selected package license expressions and observed source texts. It is not legal advice "
        "or a conclusion that any distribution is legally compliant.\n"
    ).encode("utf-8")
    payload = {path: data for path, data in text_payloads.items()}
    payload[README_PATH] = readme
    payload[MANIFEST_PATH] = (json.dumps(manifest, sort_keys=True, indent=2, ensure_ascii=False) + "\n").encode("utf-8")
    validate_notice_payload(payload, lock_data, target, source_commit, compiler_binary_sha256)
    return dict(sorted(payload.items()))


def validate_notice_payload(payload: Mapping[str, bytes], lock_data: bytes, target: str,
                             source_commit: str, compiler_binary_sha256: str) -> dict:
    if not isinstance(payload, Mapping) or MANIFEST_PATH not in payload or README_PATH not in payload:
        raise NoticeError("Generated third-party notice bundle is missing its manifest or README")
    try:
        manifest = json.loads(payload[MANIFEST_PATH].decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError, AttributeError) as error:
        raise NoticeError("Generated third-party notice manifest is invalid") from error
    if not isinstance(manifest, dict) or manifest.get("schema_version") != 1:
        raise NoticeError("Generated third-party notice manifest schema is unsupported")
    snapshot = manifest.get("snapshot")
    if not isinstance(snapshot, dict):
        raise NoticeError("Generated third-party notice manifest has no snapshot")
    if snapshot.get("cargo_lock_sha256") != hashlib.sha256(lock_data).hexdigest():
        raise NoticeError("Generated third-party notice manifest does not match Cargo.lock")
    if (snapshot.get("target") != target or snapshot.get("source_commit") != source_commit
            or snapshot.get("compiler_binary_sha256") != compiler_binary_sha256):
        raise NoticeError("Generated third-party notice manifest is bound to different release inputs")
    if manifest.get("legal_review") != "not performed" or manifest.get("unresolved") != []:
        raise NoticeError("Generated third-party notice manifest has unresolved review state")
    if manifest.get("source_coverage") != "all selected registry packages have at least one source notice":
        raise NoticeError("Generated third-party notice source coverage is not complete")

    roots = manifest.get("scope", {}).get("roots") if isinstance(manifest.get("scope"), dict) else None
    if not isinstance(roots, list) or {row.get("package") for row in roots if isinstance(row, dict)} != set(ROOT_ROLES):
        raise NoticeError("Generated third-party notice manifest omits a distribution graph root")
    expected_roles = {row: role for row, role in ROOT_ROLES.items()}
    package_rows = manifest.get("packages")
    if not isinstance(package_rows, list):
        raise NoticeError("Generated third-party notice manifest has no package rows")
    packages = {}
    referenced_paths = set()
    lock_rows = _lock_rows(lock_data)
    for row in package_rows:
        if not isinstance(row, dict):
            raise NoticeError("Generated third-party notice package row is malformed")
        name, version = row.get("name"), row.get("version")
        cargo_source, checksum = row.get("cargo_source"), row.get("lock_checksum")
        key = (name, version, cargo_source, checksum)
        if (not all(isinstance(value, str) and value for value in (name, version))
                or not isinstance(cargo_source, str) or not cargo_source.startswith("registry+")
                or not isinstance(checksum, str) or not HASH.fullmatch(checksum)
                or key in packages):
            raise NoticeError("Generated third-party notice package identity is malformed or duplicated")
        lock_row = lock_rows.get((name, version, cargo_source))
        if lock_row is None or lock_row.get("checksum") != checksum:
            raise NoticeError("Generated third-party notice package identity does not match Cargo.lock")
        expression = row.get("declared_license_expression")
        license_ids, exception_ids = parse_spdx_expression(expression)
        if row.get("license_ids") != license_ids or row.get("exception_ids") != exception_ids:
            raise NoticeError("Generated third-party notice SPDX identifiers do not match the expression")
        if not isinstance(row.get("license_file_declared"), bool):
            raise NoticeError("Generated third-party notice package source state is malformed")
        roles = row.get("distribution_roles")
        if not isinstance(roles, list) or not roles or not set(roles).issubset(set(ROOT_ROLES.values())):
            raise NoticeError("Generated third-party notice package roles are malformed")
        texts = row.get("notice_texts")
        if not isinstance(texts, list) or not any(isinstance(text, dict) and text.get("role") == "license" for text in texts):
            raise NoticeError("Generated third-party notice package has no license text mapping")
        for text in texts:
            if not isinstance(text, dict):
                raise NoticeError("Generated third-party notice text mapping is malformed")
            source_path = _safe_relative(text.get("source_path"))
            path = text.get("text_path")
            digest = text.get("sha256")
            if (not isinstance(digest, str) or not HASH.fullmatch(digest)
                    or path != f"{TEXT_ROOT}{digest}.txt"):
                raise NoticeError("Generated third-party notice text path or hash is invalid")
            referenced_paths.add(path)
        packages[key] = row

    for root in roots:
        if not isinstance(root, dict) or root.get("distribution_role") != expected_roles.get(root.get("package")):
            raise NoticeError("Generated third-party notice root role is invalid")
        root_packages = root.get("registry_packages")
        if not isinstance(root_packages, list):
            raise NoticeError("Generated third-party notice root package list is invalid")
        for identity in root_packages:
            if not isinstance(identity, dict):
                raise NoticeError("Generated third-party notice root package identity is invalid")
            key = (identity.get("name"), identity.get("version"),
                   identity.get("cargo_source"), identity.get("lock_checksum"))
            package = packages.get(key)
            if package is None or root["distribution_role"] not in package["distribution_roles"]:
                raise NoticeError("Generated third-party notice graph root and package roles do not match")
    expected_packages = {
        (identity.get("name"), identity.get("version"), identity.get("cargo_source"),
         identity.get("lock_checksum"))
        for root in roots for identity in root.get("registry_packages", []) if isinstance(identity, dict)
    }
    if expected_packages != set(packages):
        raise NoticeError("Generated third-party notice package rows do not match graph roots")

    files = manifest.get("files")
    if not isinstance(files, list):
        raise NoticeError("Generated third-party notice file table is malformed")
    declared_paths = set()
    for item in files:
        if not isinstance(item, dict):
            raise NoticeError("Generated third-party notice file row is malformed")
        path, digest, size = item.get("path"), item.get("sha256"), item.get("bytes")
        if (not isinstance(digest, str) or not HASH.fullmatch(digest)
                or path != f"{TEXT_ROOT}{digest}.txt" or path in declared_paths):
            raise NoticeError("Generated third-party notice file path is malformed")
        data = payload.get(path)
        if not isinstance(data, bytes) or len(data) != size or hashlib.sha256(data).hexdigest() != digest:
            raise NoticeError("Generated third-party notice text hash does not match payload")
        declared_paths.add(path)
    if declared_paths != referenced_paths:
        raise NoticeError("Generated third-party notice files and package mappings differ")
    expected_payload_paths = declared_paths | {MANIFEST_PATH, README_PATH}
    if set(payload) != expected_payload_paths:
        raise NoticeError("Generated third-party notice payload has unlisted or missing files")
    return manifest
