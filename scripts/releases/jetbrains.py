"""Verify and name a JetBrains plugin ZIP for release distribution."""
from __future__ import annotations

import argparse
import hashlib
import io
import shutil
import xml.etree.ElementTree as ET
from pathlib import Path
from zipfile import BadZipFile, ZipFile

from plan import version_tuple

PLUGIN_ID = "com.disnana.nagi"


def archive_name(version: str) -> str:
    version_tuple(version)
    return f"nagi-jetbrains-{version}.zip"


def _descriptor_version(data: bytes, archive: Path) -> tuple[str, str]:
    try:
        root = ET.fromstring(data)
    except ET.ParseError as error:
        raise ValueError(f"Invalid plugin.xml in {archive}: {error}") from error
    if root.tag != "idea-plugin":
        raise ValueError(f"Invalid plugin.xml root in {archive}: {root.tag}")

    values = {}
    for name in ("id", "version"):
        matches = root.findall(name)
        if len(matches) != 1 or not matches[0].text or not matches[0].text.strip():
            raise ValueError(f"plugin.xml must contain exactly one nonempty {name}: {archive}")
        values[name] = matches[0].text.strip()
    return values["id"], values["version"]


def verify_archive(path: Path, version: str) -> None:
    """Check the packaged descriptor from the plugin JAR, not Gradle source."""
    archive_name(version)
    try:
        with ZipFile(path) as distribution:
            bad_entry = distribution.testzip()
            if bad_entry:
                raise ValueError(f"Corrupt entry in JetBrains plugin ZIP: {bad_entry}")
            descriptors = []
            for entry in distribution.infolist():
                if not entry.filename.lower().endswith(".jar"):
                    continue
                try:
                    with ZipFile(io.BytesIO(distribution.read(entry))) as plugin_jar:
                        descriptor_names = [
                            name for name in plugin_jar.namelist()
                            if name == "META-INF/plugin.xml"
                        ]
                        for name in descriptor_names:
                            descriptors.append(_descriptor_version(plugin_jar.read(name), path))
                except BadZipFile as error:
                    raise ValueError(f"Invalid plugin JAR in JetBrains ZIP: {entry.filename}") from error
    except BadZipFile as error:
        raise ValueError(f"Invalid JetBrains plugin ZIP: {path}") from error

    if len(descriptors) != 1:
        raise ValueError(f"Expected one plugin JAR descriptor, found {len(descriptors)}: {path}")
    plugin_id, plugin_version = descriptors[0]
    if plugin_id != PLUGIN_ID:
        raise ValueError(f"Unexpected JetBrains plugin id in {path}: {plugin_id!r}")
    if plugin_version != version:
        raise ValueError(f"JetBrains plugin version mismatch in {path}: {plugin_version!r} != {version!r}")


def prepare_archive(source: Path, output: Path, version: str) -> tuple[Path, Path]:
    verify_archive(source, version)
    output.mkdir(parents=True, exist_ok=True)
    destination = output / archive_name(version)
    if source.resolve() != destination.resolve():
        shutil.copyfile(source, destination)
    digest = hashlib.sha256(destination.read_bytes()).hexdigest()
    checksum = destination.with_name(destination.name + ".sha256")
    checksum.write_text(f"{digest}  {destination.name}\n", encoding="utf-8")
    return destination, checksum


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    for path in prepare_archive(args.archive, args.output, args.version):
        print(path)


if __name__ == "__main__":
    main()
