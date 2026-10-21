#!/usr/bin/env python3
"""Verify observed published bytes, never authorize executing a restored cache."""
from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path

FIXED = {"generated.low", "src/main.rs", "Cargo.toml", "Cargo.lock", "sources.json", "provenance.json", "build-analysis.json"}
HEX = re.compile(r"[0-9a-f]{64}")
GENERATION = re.compile(r"g-[0-9a-f]+-[0-9a-f]+-[0-9a-f]+")


def identity(path: Path) -> dict:
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as source:
        while block := source.read(65536):
            size += len(block)
            digest.update(block)
    return {"sha256": digest.hexdigest(), "bytes": size}


def valid_identity(value: object) -> bool:
    return (isinstance(value, dict) and set(value) == {"sha256", "bytes"}
            and isinstance(value["sha256"], str) and HEX.fullmatch(value["sha256"]) is not None
            and type(value["bytes"]) is int and value["bytes"] >= 0)


def verify(receipt_path: Path, compiler: Path | None = None) -> dict:
    if receipt_path.is_symlink() or receipt_path.parent.is_symlink():
        raise ValueError("receipt and generation must not be symbolic links")
    with receipt_path.open("rb") as source:
        raw = source.read(1024 * 1024 + 1)
    if len(raw) > 1024 * 1024:
        raise ValueError("receipt exceeds 1 MiB")
    receipt = json.loads(raw)
    if (not isinstance(receipt, dict) or type(receipt.get("schema_version")) is not int
            or receipt["schema_version"] != 1):
        raise ValueError("unsupported receipt schema")
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
    if (not isinstance(artifacts, dict)
            or set(artifacts) not in (FIXED | {executable}, FIXED | {executable + ".exe"})):
        raise ValueError("unexpected, missing, or escaping artifact path")
    if not valid_identity(receipt.get("compiler")):
        raise ValueError("invalid compiler identity")
    for name, expected in artifacts.items():
        if not valid_identity(expected):
            raise ValueError(f"invalid identity: {name}")
        path = receipt_path.parent
        for component in name.split("/"):
            path = path / component
            if path.is_symlink():
                raise ValueError(f"symbolic link artifact: {name}")
        if identity(path) != expected:
            raise ValueError(f"artifact integrity mismatch: {name}")
    if compiler is not None and identity(compiler) != receipt["compiler"]:
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
