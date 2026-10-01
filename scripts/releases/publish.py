"""Publish verified assets without replacing existing tags or release files."""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

from package import PLATFORMS, archive_name
from plan import version_tuple


class GitHub:
    def __init__(self, repository: str):
        self.repository = repository

    def api(self, resource: str):
        result = subprocess.run(["gh", "api", f"repos/{self.repository}/{resource}"], capture_output=True)
        if result.returncode:
            if b"HTTP 404" in result.stderr:
                return None
            raise RuntimeError(result.stderr.decode(errors="replace"))
        return json.loads(result.stdout)

    def command(self, *args: str) -> None:
        subprocess.run(["gh", "release", *args, "--repo", self.repository], check=True)

    def create_tag(self, tag: str, sha: str) -> None:
        subprocess.run(["gh", "api", "--method", "POST", f"repos/{self.repository}/git/refs",
                        "-f", f"ref=refs/tags/{tag}", "-f", f"sha={sha}"], check=True)

    def content(self, asset) -> bytes:
        return subprocess.check_output(["gh", "api", "-H", "Accept: application/octet-stream",
                                        f"repos/{self.repository}/releases/assets/{asset['id']}"])

    def digest(self, asset) -> str:
        digest = asset.get("digest")
        if digest and digest.startswith("sha256:"):
            return digest.removeprefix("sha256:")
        return hashlib.sha256(self.content(asset)).hexdigest()


def tag_commit(client, tag: str) -> str | None:
    ref = client.api(f"git/ref/tags/{tag}")
    if not ref:
        return None
    obj = ref["object"]
    for _ in range(10):
        if obj["type"] == "commit":
            return obj["sha"]
        if obj["type"] != "tag":
            break
        obj = client.api(f"git/tags/{obj['sha']}")["object"]
    raise ValueError(f"Release tag does not resolve to a commit: {tag}")


def release_for_tag(client, tag: str):
    release = client.api(f"releases/tags/{tag}")
    if release:
        return release
    # The tag endpoint only returns published releases. Authenticated release
    # listings also include drafts, including one left by an interrupted run.
    page = 1
    while True:
        releases = client.api(f"releases?per_page=100&page={page}")
        if releases is None:
            raise RuntimeError(f"Could not list releases while looking for {tag}")
        for release in releases:
            if release["tag_name"] == tag:
                return release
        if len(releases) < 100:
            return None
        page += 1


def publish(client, component: str, version: str, sha: str, directory: Path) -> None:
    version_tuple(version)
    tag = f"{component}-v{version}"
    if component == "vscode":
        filenames = [f"nagi-language-{version}.vsix"]
        title = f"Nagi Language for VS Code {version}"
        notes = "Install the VSIX using Extensions: Install from VSIX in VS Code. Type checking and execution require the Nagi compiler separately; the VSIX does not include it.\n"
    elif component == "nagi":
        filenames = [archive_name(version, platform) for platform in PLATFORMS]
        title = f"Nagi {version}"
        notes = "Archives include the prebuilt compiler and the matching Git-tracked source, runtime, examples, and Docs. Extract the whole archive and keep its directory structure. The compiler is in target/release/. Building Nagi applications still requires Rust/Cargo and a C build environment. Linux x86_64, Windows x64, macOS Apple Silicon (arm64), and macOS Intel (x86_64) are included. macOS archives are verified on macOS 15.\n"
    else:
        raise ValueError(f"Unknown release component: {component}")
    assets = []
    for filename in filenames:
        path = directory / filename
        checksum = directory / (filename + ".sha256")
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if checksum.read_text(encoding="utf-8").strip() != f"{digest}  {filename}":
            raise ValueError(f"Incorrect asset checksum: {filename}")
        assets.extend((path, checksum))

    existing_commit = tag_commit(client, tag)
    if existing_commit and existing_commit != sha:
        raise ValueError(f"{tag} already points to a different commit; increase the version")
    release = release_for_tag(client, tag)
    if release and not release["draft"]:
        remote = {asset["name"]: asset for asset in release["assets"]}
        if existing_commit != sha:
            raise ValueError(f"Published release has no matching commit tag: {tag}")
        for filename in filenames:
            if filename not in remote or filename + ".sha256" not in remote:
                raise ValueError(f"Published release is incomplete; it will not be changed: {tag}")
            expected = f"{client.digest(remote[filename])}  {filename}"
            if client.content(remote[filename + ".sha256"]).decode().strip() != expected:
                raise ValueError(f"Published release checksum is invalid: {filename}")
        print(f"Already published and verified; keeping existing assets: {tag} ({sha})")
        return
    if not existing_commit:
        client.create_tag(tag, sha)
    if not release:
        notes += f"\nBuilt from commit {sha} after Nagi checks succeeded. SHA-256 files accompany every download.\n"
        with tempfile.TemporaryDirectory() as temporary:
            notes_file = Path(temporary) / "notes.md"
            notes_file.write_text(notes, encoding="utf-8")
            client.command("create", tag, "--target", sha, "--draft", "--title", title,
                           "--notes-file", str(notes_file))
        release = release_for_tag(client, tag)
        if release is None:
            raise RuntimeError(f"Created draft release could not be found: {tag}")
    if tag_commit(client, tag) != sha:
        raise ValueError(f"Release tag changed unexpectedly: {tag}")
    remote = {asset["name"]: asset for asset in release["assets"]}
    for path in assets:
        if path.name in remote:
            if client.digest(remote[path.name]) != hashlib.sha256(path.read_bytes()).hexdigest():
                raise ValueError(f"Existing asset differs; it will not be overwritten: {path.name}")
        elif not release["draft"]:
            raise ValueError(f"Published release is missing {path.name}; it will not be changed")
        else:
            client.command("upload", tag, str(path))
    # Read back all uploaded files before making the draft public.
    release = client.api(f"releases/{release['id']}")
    if release is None:
        raise RuntimeError(f"Release disappeared before upload verification: {tag}")
    remote = {asset["name"]: asset for asset in release["assets"]}
    for path in assets:
        if path.name not in remote or client.digest(remote[path.name]) != hashlib.sha256(path.read_bytes()).hexdigest():
            raise ValueError(f"Release upload verification failed: {path.name}")
    if release["draft"]:
        latest = "--latest=false"
        if component == "nagi":
            previous_latest = client.api("releases/latest")
            previous_tag = previous_latest["tag_name"] if previous_latest else ""
            if not previous_tag.startswith("nagi-v") or version_tuple(version) > version_tuple(previous_tag.removeprefix("nagi-v")):
                latest = "--latest"
        client.command("edit", tag, "--draft=false", latest)
    print(f"Verified formal release: {tag} ({sha})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--component", choices=("nagi", "vscode"), required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--assets", type=Path, required=True)
    args = parser.parse_args()
    publish(GitHub(args.repository), args.component, args.version, args.sha, args.assets)


if __name__ == "__main__":
    main()
