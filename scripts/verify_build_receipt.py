#!/usr/bin/env python3
"""Verify observed published bytes, never authorize executing a restored cache."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
from contextlib import contextmanager
from pathlib import Path

FIXED = {"generated.low", "src/main.rs", "Cargo.toml", "Cargo.lock", "sources.json", "provenance.json", "build-analysis.json"}
FIXED_V2 = FIXED | {"src/entry.rs", "body-Cargo.toml"}
HEX = re.compile(r"[0-9a-f]{64}")
GENERATION = re.compile(r"g-[0-9a-f]+-[0-9a-f]+-[0-9a-f]+")
GENERATED_BODY_DOMAIN = b"nagi-generated-body-v1\0"
MANIFEST_TABLE_KEYS = {
    "[[bin]]": {"name": "string", "path": "string"},
    "[dependencies.nagi-runtime]": {"path": "string"},
    "[package]": {"name": "string", "version": "string", "edition": "string", "autobins": "boolean"},
    "[profile.release]": {"opt-level": "integer", "lto": "boolean", "codegen-units": "integer", "panic": "string"},
    "[workspace]": {"exclude": "string-array"},
    "[dependencies.nagi-app-body]": {"path": "string", "package": "string"},
}
BODY_TABLE = "[dependencies.nagi-app-body]"


def reject_link(path: Path) -> os.stat_result:
    metadata = path.lstat()
    if (stat.S_ISLNK(metadata.st_mode)
            or getattr(metadata, "st_file_attributes", 0)
            & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0)):
        raise ValueError(f"symbolic link or reparse point: {path}")
    return metadata


@contextmanager
def regular_file(path: Path, *, follow_symlinks: bool = False):
    # Nonblocking prevents a substituted Unix FIFO from hanging at open. The
    # descriptor check also rejects devices/directories before reading bytes.
    # Concurrent hostile directory replacement is outside this local verifier.
    metadata = path.stat() if follow_symlinks else reject_link(path)
    if not stat.S_ISREG(metadata.st_mode):
        raise ValueError(f"not a regular file: {path}")
    flags = os.O_RDONLY | getattr(os, "O_BINARY", 0) | getattr(os, "O_NONBLOCK", 0)
    if not follow_symlinks:
        flags |= getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        if not stat.S_ISREG(os.fstat(descriptor).st_mode):
            raise ValueError(f"not a regular file: {path}")
        with os.fdopen(descriptor, "rb", closefd=False) as source:
            yield source
    finally:
        os.close(descriptor)


def _read_exact(source, amount: int) -> bytes:
    result = bytearray()
    while len(result) < amount:
        block = source.read(min(65536, amount - len(result)))
        if not block:
            break
        result.extend(block)
    return bytes(result)


def identity(path: Path, *, follow_symlinks: bool = False) -> dict:
    digest = hashlib.sha256()
    size = 0
    with regular_file(path, follow_symlinks=follow_symlinks) as source:
        while block := source.read(65536):
            size += len(block)
            digest.update(block)
    return {"sha256": digest.hexdigest(), "bytes": size}


def valid_identity(value: object) -> bool:
    return (isinstance(value, dict) and set(value) == {"sha256", "bytes"}
            and isinstance(value["sha256"], str) and HEX.fullmatch(value["sha256"]) is not None
            and type(value["bytes"]) is int and value["bytes"] >= 0)


def _check_directory(path: Path) -> None:
    if not stat.S_ISDIR(reject_link(path).st_mode):
        raise ValueError(f"not a directory: {path}")


def _framed_digest(files: list[tuple[Path, int]]) -> str:
    digest = hashlib.sha256()
    digest.update(GENERATED_BODY_DOMAIN)
    for path, length in files:
        if length >= 1 << 64:
            raise ValueError(f"artifact length cannot be framed: {path}")
        digest.update(length.to_bytes(8, "big"))
        with regular_file(path) as source:
            if os.fstat(source.fileno()).st_size != length:
                raise ValueError(f"artifact length mismatch: {path}")
            remaining = length
            while remaining:
                block = source.read(min(65536, remaining))
                if not block:
                    raise ValueError(f"artifact length mismatch: {path}")
                digest.update(block)
                remaining -= len(block)
            if source.read(1):
                raise ValueError(f"artifact length mismatch: {path}")
    return digest.hexdigest()


def _files_equal(left: Path, right: Path, expected_bytes: int) -> bool:
    with regular_file(left) as left_source, regular_file(right) as right_source:
        if (os.fstat(left_source.fileno()).st_size != expected_bytes
                or os.fstat(right_source.fileno()).st_size != expected_bytes):
            raise ValueError("generated body cache length mismatch")
        remaining = expected_bytes
        while remaining:
            amount = min(65536, remaining)
            left_block = _read_exact(left_source, amount)
            right_block = _read_exact(right_source, amount)
            if len(left_block) != amount or len(right_block) != amount:
                raise ValueError("generated body cache length mismatch")
            if left_block != right_block:
                return False
            remaining -= amount
        if left_source.read(1) or right_source.read(1):
            raise ValueError("generated body cache length mismatch")
        return True


def _valid_basic_string(value: str) -> bool:
    # Check only the one-line delimiter shape emitted by toml::to_string; the
    # Cargo build has already validated TOML value semantics.
    value = value.strip()
    if len(value) < 2 or value[0] != '"' or value[-1] != '"' or '"""' in value:
        return False
    escaped = False
    for char in value[1:-1]:
        if escaped:
            escaped = False
            continue
        if char == "\\":
            escaped = True
        elif char == '"':
            return False
        if ord(char) < 0x20:
            return False
    return not escaped


def _valid_manifest_value(kind: str, value: str) -> bool:
    value = value.strip()
    if kind == "string":
        return _valid_basic_string(value)
    if kind == "boolean":
        return value in ("true", "false")
    if kind == "integer":
        return re.fullmatch(r"[0-9]+", value) is not None
    if kind == "string-array":
        if not (value.startswith("[") and value.endswith("]")):
            return False
        item = value[1:-1].strip()
        if item.endswith(","):
            item = item[:-1].strip()
        return _valid_basic_string(item)
    return False


def _verify_manifest_table(path: Path, expected_bytes: int, digest: str, app_suffix: str) -> None:
    # The receipt length bounds this read. The generated root manifest has a
    # closed table/key set, so reject all other TOML shapes conservatively.
    with regular_file(path) as source:
        if os.fstat(source.fileno()).st_size != expected_bytes:
            raise ValueError("artifact length mismatch: Cargo.toml")
        raw = source.read(expected_bytes + 1)
    if len(raw) != expected_bytes:
        raise ValueError("artifact length mismatch: Cargo.toml")
    try:
        manifest = raw.decode("utf-8").replace("\r\n", "\n")
    except UnicodeDecodeError as error:
        raise ValueError("invalid UTF-8 in Cargo.toml") from error
    if "\r" in manifest:
        raise ValueError("invalid line endings in Cargo.toml")

    expected_lines = [
        f'path = "../../code/{digest}"',
        f'package = "nagi-body-{app_suffix}"',
    ]
    lines = manifest.split("\n")
    if lines and lines[-1] == "":
        lines.pop()  # A final newline is not an extra blank line in the body table.

    current_table: str | None = None
    seen_tables: set[str] = set()
    current_keys: set[str] = set()
    body_lines: list[str] = []

    def finish_table() -> None:
        if current_table is None:
            return
        if current_keys != set(MANIFEST_TABLE_KEYS[current_table]):
            raise ValueError(f"generated Cargo.toml table keys mismatch: {current_table}")
        if current_table == BODY_TABLE:
            table_body = body_lines[:]
            if table_body and table_body[-1] == "":
                table_body.pop()
            if table_body != expected_lines:
                raise ValueError("generated body dependency table mismatch")

    for line in lines:
        if line in MANIFEST_TABLE_KEYS:
            finish_table()
            if current_table == BODY_TABLE:
                raise ValueError("generated body dependency table must be final")
            if line in seen_tables:
                raise ValueError(f"duplicate generated Cargo.toml table: {line}")
            seen_tables.add(line)
            current_table = line
            current_keys = set()
            if current_table == BODY_TABLE:
                body_lines = []
            continue
        if line.startswith("["):
            raise ValueError(f"unexpected generated Cargo.toml table: {line}")
        if not line.strip():
            if current_table == BODY_TABLE:
                body_lines.append(line)
            continue
        if current_table is None:
            raise ValueError("generated Cargo.toml root prefix is not empty")
        if "=" not in line:
            raise ValueError(f"malformed generated Cargo.toml assignment: {line}")
        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip()
        if re.fullmatch(r"[A-Za-z0-9_-]+", key) is None or not value:
            raise ValueError(f"malformed generated Cargo.toml assignment: {line}")
        value_kind = MANIFEST_TABLE_KEYS[current_table].get(key)
        if value_kind is None or key in current_keys:
            raise ValueError(f"unexpected generated Cargo.toml key: {current_table}.{key}")
        if not _valid_manifest_value(value_kind, value):
            raise ValueError(f"malformed generated Cargo.toml value: {current_table}.{key}")
        current_keys.add(key)
        if current_table == BODY_TABLE:
            body_lines.append(line)

    finish_table()
    if BODY_TABLE not in seen_tables:
        raise ValueError("missing generated body dependency table")
    if seen_tables != set(MANIFEST_TABLE_KEYS):
        raise ValueError("missing generated Cargo.toml tables")


def _verify_generated_body(receipt_path: Path, receipt: dict, app_suffix: str) -> None:
    digest = receipt.get("generated_body_sha256")
    if not isinstance(digest, str) or HEX.fullmatch(digest) is None:
        raise ValueError("invalid generated body digest")
    artifacts = receipt["artifacts"]
    generation_root = receipt_path.parent
    body_manifest = generation_root / "body-Cargo.toml"
    body_main = generation_root / "src" / "main.rs"
    calculated = _framed_digest([
        (body_manifest, artifacts["body-Cargo.toml"]["bytes"]),
        (body_main, artifacts["src/main.rs"]["bytes"]),
    ])
    if calculated != digest:
        raise ValueError("generated body digest mismatch")

    app_root = receipt_path.parent.parent.parent
    code_root = app_root / "code"
    cas_root = code_root / digest
    _check_directory(code_root)
    _check_directory(cas_root)
    cas_src = cas_root / "src"
    _check_directory(cas_src)
    cas_manifest = cas_root / "Cargo.toml"
    cas_main = cas_src / "lib.rs"
    if not _files_equal(body_manifest, cas_manifest, artifacts["body-Cargo.toml"]["bytes"]):
        raise ValueError("generated body cache Cargo.toml mismatch")
    if not _files_equal(body_main, cas_main, artifacts["src/main.rs"]["bytes"]):
        raise ValueError("generated body cache src/lib.rs mismatch")
    _verify_manifest_table(generation_root / "Cargo.toml", artifacts["Cargo.toml"]["bytes"], digest, app_suffix)


def verify(receipt_path: Path, compiler: Path | None = None) -> dict:
    for directory in list(receipt_path.parents)[:3]:
        if not stat.S_ISDIR(reject_link(directory).st_mode):
            raise ValueError(f"not a directory: {directory}")
    with regular_file(receipt_path) as source:
        raw = source.read(1024 * 1024 + 1)
    if len(raw) > 1024 * 1024:
        raise ValueError("receipt exceeds 1 MiB")
    receipt = json.loads(raw)
    if (not isinstance(receipt, dict) or type(receipt.get("schema_version")) is not int
            or receipt["schema_version"] not in (1, 2)):
        raise ValueError("unsupported receipt schema")
    schema_version = receipt["schema_version"]
    generation = receipt.get("generation")
    app = receipt.get("app_id")
    if (not isinstance(generation, str) or GENERATION.fullmatch(generation) is None
            or generation != receipt_path.parent.name):
        raise ValueError("generation identity mismatch")
    if (not isinstance(app, str) or not app.startswith("nagi-")
            or "/" in app or "\\" in app or re.fullmatch(r"[0-9a-f]{16}", app.rsplit("-", 1)[-1]) is None
            or receipt_path.parent.parent.name != "generations"
            or receipt_path.parent.parent.parent.name != app):
        raise ValueError("application identity mismatch")
    executable = f"nagi-{app.rsplit('-', 1)[-1]}-{generation}"
    artifacts = receipt.get("artifacts")
    fixed = FIXED if schema_version == 1 else FIXED_V2
    if (not isinstance(artifacts, dict)
            or set(artifacts) not in (fixed | {executable}, fixed | {executable + ".exe"})):
        raise ValueError("unexpected, missing, or escaping artifact path")
    if not valid_identity(receipt.get("compiler")):
        raise ValueError("invalid compiler identity")
    for name, expected in artifacts.items():
        if not valid_identity(expected):
            raise ValueError(f"invalid identity: {name}")
        path = receipt_path.parent
        for component in name.split("/"):
            path = path / component
            reject_link(path)
        if identity(path) != expected:
            raise ValueError(f"artifact integrity mismatch: {name}")
    if schema_version == 2:
        _verify_generated_body(receipt_path, receipt, app.rsplit("-", 1)[-1])
    # Installer command paths may themselves be links to the installed exe.
    if compiler is not None and identity(compiler, follow_symlinks=True) != receipt["compiler"]:
        raise ValueError("compiler integrity mismatch")
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("receipt", type=Path)
    parser.add_argument("--compiler", type=Path)
    args = parser.parse_args()
    try:
        verify(args.receipt, args.compiler)
    except (OSError, ValueError, TypeError) as error:
        parser.exit(1, f"receipt verification failed: {error}\n")
    print("Published bytes verified; external dependencies and cache authenticity are not verified.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
