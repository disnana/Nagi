"""Publish verified assets without replacing existing tags or release files."""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path
from urllib.parse import quote

from package import PLATFORMS, archive_name
from plan import version_tuple
from jetbrains import archive_name as jetbrains_archive_name, verify_archive
from notes import release_notes, tag_commit


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

    def write_api(self, method: str, resource: str, *, data=None, file: Path | None = None):
        endpoint = resource if resource.startswith("https://") else f"repos/{self.repository}/{resource}"
        command = ["gh", "api", "--method", method, endpoint]
        if file is None:
            command.extend(["--input", "-", "-H", "Content-Type: application/json"])
            body = json.dumps(data).encode()
        else:
            command.extend(["--input", str(file), "-H", "Content-Type: application/octet-stream"])
            body = None
        result = subprocess.run(command, input=body, capture_output=True)
        if result.returncode:
            raise RuntimeError(result.stderr.decode(errors="replace"))
        return json.loads(result.stdout)

    def create_release(self, tag: str, sha: str, title: str, notes: str):
        return self.write_api("POST", "releases", data={
            "tag_name": tag, "target_commitish": sha, "name": title,
            "body": notes, "draft": True})

    def upload_asset(self, release_id: int, file: Path):
        endpoint = f"https://uploads.github.com/repos/{self.repository}/releases/{release_id}/assets?name={quote(file.name, safe='')}"
        return self.write_api("POST", endpoint, file=file)

    def make_public(self, release_id: int, latest: bool):
        return self.write_api("PATCH", f"releases/{release_id}", data={
            "draft": False, "make_latest": "true" if latest else "false"})

    def generate_notes(self, tag: str, sha: str, previous_tag: str):
        return self.write_api("POST", "releases/generate-notes", data={
            "tag_name": tag, "target_commitish": sha, "previous_tag_name": previous_tag})

    def update_notes(self, release_id: int, notes: str):
        return self.write_api("PATCH", f"releases/{release_id}", data={"body": notes})

    def create_tag(self, tag: str, sha: str) -> None:
        self.write_api("POST", "git/refs", data={"ref": f"refs/tags/{tag}", "sha": sha})

    def content(self, asset) -> bytes:
        return subprocess.check_output(["gh", "api", "-H", "Accept: application/octet-stream",
                                        f"repos/{self.repository}/releases/assets/{asset['id']}"])

    def digest(self, asset) -> str:
        digest = asset.get("digest")
        if digest and digest.startswith("sha256:"):
            return digest.removeprefix("sha256:")
        return hashlib.sha256(self.content(asset)).hexdigest()


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
        notes = "Archives include the prebuilt compiler and the matching runtime sources, license, and installation notes. Extract the whole archive and add its root folder to PATH. Run `nagic --version` to verify the installed version. The compiler finds the bundled runtime automatically; NAGI_ROOT is normally unnecessary. Building Nagi applications still requires Rust/Cargo and a C build environment. Linux x86_64, Windows x64, macOS Apple Silicon (arm64), and macOS Intel (x86_64) are included. macOS archives are verified on macOS 15.\n"
    elif component == "jetbrains":
        filenames = [jetbrains_archive_name(version)]
        title = f"Nagi for JetBrains IDEs {version}"
        notes = "Download the Nagi for JetBrains ZIP, then use Settings/Preferences → Plugins → Install Plugin from Disk. Restart the IDE afterward. Install the Nagi compiler separately for check and run commands.\n"
    else:
        raise ValueError(f"Unknown release component: {component}")
    assets = []
    for filename in filenames:
        path = directory / filename
        if component == "jetbrains":
            verify_archive(path, version)
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
    # Validate draft assets before changing its notes. Missing source/history or
    # note generation failures must precede tag creation and every release write.
    if release:
        remote = {asset["name"]: asset for asset in release["assets"]}
        for path in assets:
            if path.name in remote and client.digest(remote[path.name]) != hashlib.sha256(path.read_bytes()).hexdigest():
                raise ValueError(f"Existing asset differs; it will not be overwritten: {path.name}")
    notes = release_notes(client, component, version, sha, notes)
    if not existing_commit:
        client.create_tag(tag, sha)
    if not release:
        # Use the creation response immediately. Draft listings can lag behind
        # creation, so neither uploads nor publication should look up its tag.
        release = client.create_release(tag, sha, title, notes)
        if not release or release.get("tag_name") != tag or not release.get("draft") or not release.get("id"):
            raise RuntimeError(f"Create release did not return the expected draft: {tag}")
    elif release.get("body") != notes:
        current = client.api(f"releases/{release['id']}")
        if not current or current.get("id") != release["id"] or current.get("tag_name") != tag or current.get("draft") is not True:
            raise RuntimeError(f"Release is no longer the expected draft for note update: {tag}")
        updated = client.update_notes(release["id"], notes)
        if not updated or updated.get("id") != release["id"] or updated.get("tag_name") != tag or updated.get("draft") is not True:
            raise RuntimeError(f"Note update did not return the expected draft: {tag}")
        release = client.api(f"releases/{updated['id']}")
        if not release or release.get("id") != updated["id"] or release.get("tag_name") != tag or release.get("body") != notes or release.get("draft") is not True:
            raise RuntimeError(f"Draft note readback verification failed: {tag}")
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
            client.upload_asset(release["id"], path)
    # Read back all uploaded files before making the draft public.
    release_id = release["id"]
    release = client.api(f"releases/{release_id}")
    if release is None:
        raise RuntimeError(f"Release disappeared before upload verification: {tag}")
    if release.get("body") != notes:
        raise RuntimeError(f"Release note readback verification failed: {tag}")
    if release.get("id") != release_id or release.get("tag_name") != tag:
        raise RuntimeError(f"Release readback identity changed unexpectedly: {tag}")
    remote = {asset["name"]: asset for asset in release["assets"]}
    for path in assets:
        if path.name not in remote or client.digest(remote[path.name]) != hashlib.sha256(path.read_bytes()).hexdigest():
            raise ValueError(f"Release upload verification failed: {path.name}")
    if release["draft"]:
        latest = False
        if component == "nagi":
            previous_latest = client.api("releases/latest")
            previous_tag = previous_latest["tag_name"] if previous_latest else ""
            if not previous_tag.startswith("nagi-v") or version_tuple(version) > version_tuple(previous_tag.removeprefix("nagi-v")):
                latest = True
        published = client.make_public(release["id"], latest)
        if not published or published.get("id") != release["id"] or published.get("tag_name") != tag or published.get("draft") is not False:
            raise RuntimeError(f"Release response did not confirm publication: {tag}")
    print(f"Verified formal release: {tag} ({sha})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--component", choices=("nagi", "vscode", "jetbrains"), required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--assets", type=Path, required=True)
    args = parser.parse_args()
    publish(GitHub(args.repository), args.component, args.version, args.sha, args.assets)


if __name__ == "__main__":
    main()
