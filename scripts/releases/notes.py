"""Build release notes from an immutable CHANGELOG and component history."""
from __future__ import annotations

import re
import subprocess
from pathlib import Path
from urllib.parse import quote

from _release_changelog import changelog_entry_from_text
from plan import VERSION, version_tuple

ROOT = Path(__file__).resolve().parents[2]


def changelog_entry(sha: str, component: str, version: str) -> str:
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise ValueError("Release source must be an immutable 40-character commit SHA")
    commit = subprocess.run(["git", "rev-parse", "--verify", f"{sha}^{{commit}}"],
                            cwd=ROOT, capture_output=True)
    if commit.returncode or commit.stdout.decode().strip() != sha:
        raise ValueError(f"Release source is not a commit: {sha}")
    result = subprocess.run(["git", "show", f"{sha}:CHANGELOG.md"], cwd=ROOT,
                            capture_output=True)
    if result.returncode:
        raise RuntimeError(f"Could not read CHANGELOG.md at release commit {sha}")
    text = result.stdout.decode("utf-8")
    return changelog_entry_from_text(text, component, version, sha)


def tag_commit(client, tag: str) -> str | None:
    ref = client.api(f"git/ref/tags/{tag}")
    if not ref:
        return None
    obj = ref["object"]
    for _ in range(10):
        if obj["type"] == "commit":
            if not re.fullmatch(r"[0-9a-f]{40}", obj["sha"]):
                raise ValueError(f"Release tag has an invalid commit SHA: {tag}")
            return obj["sha"]
        if obj["type"] != "tag":
            break
        annotated = client.api(f"git/tags/{obj['sha']}")
        if not annotated:
            raise RuntimeError(f"Could not resolve release tag: {tag}")
        obj = annotated["object"]
    raise ValueError(f"Release tag does not resolve to a commit: {tag}")


def previous_release(client, component: str, version: str):
    previous = None
    previous_version = None
    prefix = f"{component}-v"
    page = 1
    while True:
        releases = client.api(f"releases?per_page=100&page={page}")
        if not isinstance(releases, list):
            raise RuntimeError("Could not retrieve release history for release notes")
        for release in releases:
            tag = release.get("tag_name", "")
            if not tag.startswith(prefix) or release.get("draft") is not False or release.get("prerelease") is not False:
                continue
            value = tag.removeprefix(prefix)
            if not VERSION.fullmatch(value):
                continue
            parsed = version_tuple(value)
            if parsed < version_tuple(version) and (previous_version is None or parsed > previous_version):
                previous, previous_version = release, parsed
        if len(releases) < 100:
            return previous
        page += 1


def release_notes(client, component: str, version: str, sha: str, installation: str) -> str:
    changes = changelog_entry(sha, component, version)
    previous = previous_release(client, component, version)
    base = f"https://github.com/{client.repository}"
    source = f"{base}/blob/{sha}/CHANGELOG.md"
    body = f"## Installation\n\n{installation.strip()}\n\n## Changes\n\n{changes}\n\n[CHANGELOG at the released commit]({source})\n\n## Release comparison\n\n"
    if previous:
        tag = previous["tag_name"]
        commit = tag_commit(client, tag)
        if not commit:
            raise RuntimeError(f"Published previous release has no commit tag: {tag}")
        encoded_tag = quote(tag, safe="")
        body += f"Previous {component} release: [{tag}]({base}/releases/tag/{encoded_tag}) at [{commit}]({base}/commit/{commit}).\n\n"
        body += f"- [Previous tag → released commit]({base}/compare/{encoded_tag}...{sha})\n- [Previous commit → released commit]({base}/compare/{commit}...{sha})\n"
        generated = client.generate_notes(f"{component}-v{version}", sha, tag)
        if not generated or not isinstance(generated.get("body"), str) or not generated["body"].strip():
            raise RuntimeError("GitHub did not return generated release notes")
        body += f"\n## Pull requests and contributors\n\n{generated['body'].strip()}\n"
    else:
        name = {"nagi": "Nagi", "vscode": "VS Code extension", "jetbrains": "JetBrains plugin"}[component]
        body += f"First {name} release: no previous published {component} release tag is available. [Source snapshot]({base}/tree/{sha}).\n"
    body += f"\nBuilt from commit [{sha}]({base}/commit/{sha}) after the required release checks succeeded. SHA-256 files accompany every download.\n"
    return body
