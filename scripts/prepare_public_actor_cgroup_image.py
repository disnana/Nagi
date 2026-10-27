#!/usr/bin/env python3
"""Prepare a network-free scratch Docker context for a trusted Nagi binary."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys


MIN_FREE = 8 * 1024**3


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_ldd(output: str) -> list[Path]:
    paths: list[Path] = []
    for raw in output.splitlines():
        line = raw.strip()
        if not line or line.startswith("linux-vdso"):
            continue
        if "not found" in line:
            raise RuntimeError(f"unresolved dynamic dependency: {line}")
        target = line.split("=>", 1)[1].strip() if "=>" in line else line
        value = target.split(" ", 1)[0]
        path = Path(value)
        if not path.is_absolute() or ".." in path.parts:
            raise RuntimeError(f"unexpected ldd dependency: {line}")
        if path not in paths:
            paths.append(path)
    if not paths:
        raise RuntimeError("ldd returned no filesystem dependencies")
    return paths


def capacity(path: Path) -> int:
    free = shutil.disk_usage(path).free
    if free < MIN_FREE:
        raise RuntimeError(f"disk guard: {free} bytes free, need {MIN_FREE}")
    return free


def copy_dependency(root: Path, source: Path) -> tuple[Path, str]:
    if not source.is_absolute() or ".." in source.parts:
        raise RuntimeError(f"unsafe dynamic dependency path: {source}")
    resolved = source.resolve(strict=True)
    if not resolved.is_file():
        raise RuntimeError(f"dynamic dependency is not a file: {source}")
    target = root / source.relative_to("/")
    target.relative_to(root)
    if target.exists() or target.is_symlink():
        raise RuntimeError(f"duplicate image path: /{source.relative_to('/')}")
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(resolved, target)
    target.chmod(0o555)
    return target, sha256(target)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--context", type=Path, required=True)
    args = parser.parse_args()
    if platform.system() != "Linux" or platform.architecture()[0] != "64bit":
        parser.error("Linux64 only")
    binary = args.binary.resolve(strict=True)
    if not binary.is_file() or not os.access(binary, os.X_OK):
        parser.error("--binary must be an executable regular file")
    context = args.context.absolute()
    capacity(context.parent)
    context.mkdir()
    root = context / "rootfs"
    destination = root / "app" / "nagi"
    destination.parent.mkdir(parents=True)
    shutil.copy2(binary, destination)
    destination.chmod(0o555)
    output = subprocess.check_output(["ldd", str(binary)], text=True,
                                     stderr=subprocess.STDOUT)
    dependencies = parse_ldd(output)
    files = {"/app/nagi": sha256(destination)}
    for source in dependencies:
        _, copied_hash = copy_dependency(root, source)
        files[f"/{source.relative_to('/')}"] = copied_hash
    (context / "Dockerfile").write_text(
        'FROM scratch\nCOPY rootfs/ /\nENTRYPOINT ["/app/nagi"]\n')
    manifest = {
        "schema": 1,
        "binary_source": str(binary),
        "binary_sha256": sha256(binary),
        "ldd_output": output.splitlines(),
        "files_sha256": files,
        "disk_free_before": capacity(context),
    }
    (context / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps({"context": str(context), "files": len(files),
                      "binary_sha256": manifest["binary_sha256"]}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
