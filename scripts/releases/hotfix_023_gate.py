"""Validate the immutable Nagi 0.2.3 hotfix source run and its artifacts."""
from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any
from urllib.error import HTTPError
from urllib.request import Request, urlopen


REPOSITORY = "disnana/Nagi"
VERSION = "0.2.3"
SOURCE_SHA = "f6ec2130dc64d2c8c77aa357d0b89437dc9cad2f"
BASE_SHA = "7c92045a04519609ae287803a31ac0816c728086"
SOURCE_RUN_ID = 40990014206
CONFIRMATION = "publish-nagi-0.2.3"
TAG = f"nagi-v{VERSION}"

EXPECTED_JOBS = {
    "nagi-package (ubuntu-latest, linux-x86_64, nagic, x86_64-unknown-linux-gnu)",
    "nagi-package (windows-latest, windows-x86_64, nagic.exe, x86_64-pc-windows-msvc)",
    "nagi-package (macos-15, macos-arm64, nagic, aarch64-apple-darwin)",
    "nagi-package (macos-15-intel, macos-x86_64, nagic, x86_64-apple-darwin)",
    "nagi-regression (ubuntu-latest, linux-x86_64, nagic, x86_64-unknown-linux-gnu)",
    "nagi-regression (windows-latest, windows-x86_64, nagic.exe, x86_64-pc-windows-msvc)",
    "nagi-regression (macos-15, macos-arm64, nagic, aarch64-apple-darwin)",
    "nagi-regression (macos-15-intel, macos-x86_64, nagic, x86_64-apple-darwin)",
    "sf08-browser / browser-tls",
    "Ready to merge",
}

EXPECTED_ARTIFACTS = {
    "release-linux-x86_64": 12950086206,
    "release-windows-x86_64": 12952064098,
    "release-macos-arm64": 12949587439,
    "release-macos-x86_64": 12953964814,
}

EXPECTED_ARCHIVES = {
    "nagi-0.2.3-linux-x86_64.tar.gz":
        "3c5e12e31e1c25d5b8facbc0f4371b7772b0d6cc6c31a46247c57d1609100d8e",
    "nagi-0.2.3-windows-x86_64.zip":
        "e495a311b0beaa09aa59f4f43b5add7663d205ab7d8861aa2da4f25ec9303c00",
    "nagi-0.2.3-macos-arm64.tar.gz":
        "d0b74b80c1f04bda0b17fa21f89ce2d1d04d61a4fc473b760bc447fab45c31c1",
    "nagi-0.2.3-macos-x86_64.tar.gz":
        "c02f4b6f687b7b7c976185672203cbcff495803b2d154164c26e60be0be267cc",
}

EXPECTED_CHANGES = {
    ".github/workflows/compiler-contracts.yml",
    "CHANGELOG.md",
    "Cargo.lock",
    "Cargo.toml",
    "compiler/src/emit.rs",
    "scripts/ci/test_windows_gnu_target_path_workflow.py",
    "scripts/ci/windows_gnu_target_path.ps1",
    "tools/sf08-fixture-runtime/Cargo.lock",
}


def _require(value: bool, message: str) -> None:
    if not value:
        raise ValueError(message)


def _request(resource: str, *, accept: str = "application/vnd.github+json"):
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    headers = {
        "Accept": accept,
        "User-Agent": "nagi-hotfix-release-gate",
        "X-GitHub-Api-Version": "2022-11-28",
    }
    if token:
        headers["Authorization"] = f"Bearer {token}"
    return Request(f"https://api.github.com/repos/{REPOSITORY}/{resource}", headers=headers)


def _api(resource: str) -> Any:
    with urlopen(_request(resource), timeout=30) as response:
        return json.load(response)


def _api_optional(resource: str) -> Any | None:
    try:
        return _api(resource)
    except HTTPError as error:
        if error.code == 404:
            return None
        raise


def _api_bytes(resource: str) -> bytes:
    with urlopen(_request(resource, accept="application/octet-stream"), timeout=60) as response:
        return response.read()


def validate_source(run: dict, jobs_payload: dict, artifacts_payload: dict) -> None:
    _require(run.get("id") == SOURCE_RUN_ID, "release source run ID changed")
    _require(run.get("head_sha") == SOURCE_SHA, "release source run used another commit")
    _require(run.get("status") == "completed" and run.get("conclusion") == "success",
             "release source run is not a completed success")
    _require(run.get("event") == "workflow_dispatch", "release source run event changed")
    _require(run.get("path") == ".github/workflows/ci.yml", "release source workflow changed")
    _require(run.get("repository", {}).get("full_name") == REPOSITORY,
             "release source repository changed")

    jobs = jobs_payload.get("jobs", [])
    _require(jobs_payload.get("total_count") == len(jobs), "release job response was truncated")
    by_name: dict[str, list[dict]] = {}
    for job in jobs:
        by_name.setdefault(job.get("name", ""), []).append(job)
    for name in EXPECTED_JOBS:
        matches = by_name.get(name, [])
        _require(len(matches) == 1, f"required release job is missing or duplicated: {name}")
        _require(matches[0].get("status") == "completed"
                 and matches[0].get("conclusion") == "success",
                 f"required release job did not succeed: {name}")

    artifacts = artifacts_payload.get("artifacts", [])
    _require(artifacts_payload.get("total_count") == len(artifacts),
             "release artifact response was truncated")
    release_artifacts: dict[str, list[dict]] = {}
    for item in artifacts:
        if item.get("name") in EXPECTED_ARTIFACTS:
            release_artifacts.setdefault(item["name"], []).append(item)
    for name, artifact_id in EXPECTED_ARTIFACTS.items():
        matches = release_artifacts.get(name, [])
        _require(len(matches) == 1, f"release artifact is missing or duplicated: {name}")
        artifact = matches[0]
        _require(artifact.get("id") == artifact_id, f"release artifact ID changed: {name}")
        _require(artifact.get("expired") is False, f"release artifact expired: {name}")
        workflow_run = artifact.get("workflow_run") or {}
        _require(workflow_run.get("id") == SOURCE_RUN_ID
                 and workflow_run.get("head_sha") == SOURCE_SHA,
                 f"release artifact source changed: {name}")


def verify_assets(directory: Path) -> None:
    root = directory.resolve()
    _require(root.is_dir(), "release asset directory is missing")
    entries = list(root.iterdir())
    _require(all(path.is_file() and not path.is_symlink() for path in entries),
             "release asset directory contains a non-regular entry")
    files = {path.name: path for path in entries}
    expected_files = set(EXPECTED_ARCHIVES) | {name + ".sha256" for name in EXPECTED_ARCHIVES}
    _require(set(files) == expected_files, "downloaded release file set changed")
    for name, expected_digest in EXPECTED_ARCHIVES.items():
        digest = hashlib.sha256(files[name].read_bytes()).hexdigest()
        _require(digest == expected_digest, f"release archive digest changed: {name}")
        checksum = files[name + ".sha256"].read_text(encoding="utf-8").strip()
        _require(checksum == f"{expected_digest}  {name}",
                 f"release checksum file changed: {name}")


def _expected_file_digests(directory: Path) -> dict[str, str]:
    return {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
            for path in directory.iterdir() if path.is_file() and not path.is_symlink()}


def validate_release_state(tag_sha: str | None, release: dict | None,
                           remote_digests: dict[str, str], expected_digests: dict[str, str],
                           *, require_public: bool) -> None:
    _require(tag_sha in (None, SOURCE_SHA), "release tag points to another commit")
    if release is None:
        _require(not require_public, "published release is missing")
        _require(not remote_digests, "release assets exist without a release")
        return
    _require(release.get("tag_name") == TAG, "release tag name changed")
    draft = release.get("draft")
    _require(isinstance(draft, bool), "release draft state is missing")
    _require(release.get("prerelease") is False, "hotfix release must not be a prerelease")
    if require_public or draft is False:
        _require(tag_sha == SOURCE_SHA, "public release tag is missing")
    if require_public:
        _require(draft is False, "release is still a draft")
    if draft:
        _require(set(remote_digests) <= set(expected_digests),
                 "draft release contains an unexpected asset")
    else:
        _require(set(remote_digests) == set(expected_digests),
                 "published release does not contain exactly eight reviewed files")
    for name, digest in remote_digests.items():
        _require(digest == expected_digests[name],
                 f"existing release asset differs from the reviewed file: {name}")


def _tag_commit() -> str | None:
    reference = _api_optional(f"git/ref/tags/{TAG}")
    if reference is None:
        return None
    value = reference.get("object", {})
    for _ in range(10):
        if value.get("type") == "commit":
            return value.get("sha")
        _require(value.get("type") == "tag", "release tag does not resolve to a commit")
        annotated = _api_optional(f"git/tags/{value.get('sha', '')}")
        _require(annotated is not None, "annotated release tag disappeared")
        value = annotated.get("object", {})
    raise ValueError("release tag annotation chain is too deep")


def _release_for_tag() -> dict | None:
    matches = []
    page = 1
    while True:
        releases = _api(f"releases?per_page=100&page={page}")
        _require(isinstance(releases, list), "release listing response changed")
        matches.extend(release for release in releases if release.get("tag_name") == TAG)
        if len(releases) < 100:
            break
        page += 1
    _require(len(matches) <= 1, "multiple releases use the hotfix tag")
    return matches[0] if matches else None


def _remote_asset_digests(release: dict | None) -> dict[str, str]:
    if release is None:
        return {}
    payload = _api(f"releases/{release['id']}/assets?per_page=100")
    _require(isinstance(payload, list) and len(payload) < 100,
             "release asset response was truncated")
    values: dict[str, str] = {}
    for asset in payload:
        name = asset.get("name", "")
        _require(name not in values, f"release asset is duplicated: {name}")
        digest = asset.get("digest")
        if isinstance(digest, str) and digest.startswith("sha256:"):
            values[name] = digest.removeprefix("sha256:")
        else:
            values[name] = hashlib.sha256(
                _api_bytes(f"releases/assets/{asset['id']}")).hexdigest()
    return values


def release_gate(directory: Path, *, require_public: bool) -> None:
    verify_assets(directory)
    expected = _expected_file_digests(directory)
    release = _release_for_tag()
    validate_release_state(_tag_commit(), release, _remote_asset_digests(release), expected,
                           require_public=require_public)
    if require_public:
        latest = _api("releases/latest")
        _require(latest.get("tag_name") == TAG, "Nagi 0.2.3 was not marked latest")


def _git(root: Path, *arguments: str) -> str:
    return subprocess.check_output(["git", *arguments], cwd=root, text=True).strip()


def verify_candidate(root: Path) -> None:
    _require(_git(root, "rev-parse", "HEAD") == SOURCE_SHA, "candidate checkout SHA changed")
    ancestor = subprocess.run(["git", "merge-base", "--is-ancestor", BASE_SHA, SOURCE_SHA],
                              cwd=root, capture_output=True)
    _require(ancestor.returncode == 0, "candidate is not based on Nagi 0.2.2")
    changes = set(_git(root, "diff", "--name-only", BASE_SHA, SOURCE_SHA).splitlines())
    _require(changes == EXPECTED_CHANGES, "candidate diff no longer matches the reviewed hotfix")
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    _require(manifest["workspace"]["package"]["version"] == VERSION,
             "candidate workspace version changed")
    _require((root / "scripts/releases/publish.py").is_file(),
             "candidate publication implementation is missing")


def source_gate() -> None:
    _require(os.environ.get("GITHUB_REPOSITORY") == REPOSITORY,
             "workflow is running in another repository")
    _require(os.environ.get("GITHUB_REF") == "refs/heads/main",
             "hotfix publication must be dispatched from main")
    _require(os.environ.get("HOTFIX_CONFIRMATION") == CONFIRMATION,
             f"confirmation must equal {CONFIRMATION}")
    run = _api(f"actions/runs/{SOURCE_RUN_ID}")
    jobs = _api(f"actions/runs/{SOURCE_RUN_ID}/jobs?filter=latest&per_page=100")
    artifacts = _api(f"actions/runs/{SOURCE_RUN_ID}/artifacts?per_page=100")
    validate_source(run, jobs, artifacts)


def main() -> None:
    if len(sys.argv) != 3 or sys.argv[1] not in {"assets", "candidate", "release-pre", "release-post"}:
        if sys.argv[1:] == ["source"]:
            source_gate()
            return
        raise SystemExit("usage: hotfix_023_gate.py source | assets DIRECTORY | candidate DIRECTORY")
    if sys.argv[1] == "assets":
        verify_assets(Path(sys.argv[2]))
    elif sys.argv[1] == "candidate":
        verify_candidate(Path(sys.argv[2]))
    else:
        release_gate(Path(sys.argv[2]), require_public=sys.argv[1] == "release-post")


if __name__ == "__main__":
    main()
