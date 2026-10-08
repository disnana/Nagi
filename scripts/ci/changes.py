"""Run full checks unless the complete range changes Docs/site or the independent JetBrains plugin."""
from __future__ import annotations

import argparse
import json
import re
import subprocess
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[2]
COMMIT = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
ROOT_DOCS = {
    "README.md", "README.en.md", "CHANGELOG.md", "PERFORMANCE.md",
    "CONTRIBUTING.md", "CONTRIBUTING.en.md", "SECURITY.md", "SECURITY.en.md",
    "DESIGN.md", "DESIGN.en.md",
    "AGENTS.md",
}
SITE_FILES = {
    "website/README.md", "website/README.en.md", "website/build.py", "website/requirements.txt",
    "website/CNAME", ".github/workflows/pages.yml",
}
ASSET_SUFFIXES = {".css", ".js", ".svg", ".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".woff", ".woff2"}
JETBRAINS_CHECK_INPUTS = {
    ".github/workflows/ci.yml", "scripts/ci/changes.py", "scripts/ci/gate.py",
    "scripts/ci/test_checks.py", "Cargo.toml", "Cargo.lock",
}
JETBRAINS_CHECK_PREFIXES = ("compiler/", "runtime/")


def valid_path(path: str) -> bool:
    # Git paths are POSIX paths; reject malformed input rather than normalizing it.
    return bool(path) and "\\" not in path and "\0" not in path and all(
        part not in ("", ".", "..") for part in path.split("/")
    )


def is_docs_path(path: str) -> bool:
    if not valid_path(path):
        return False
    value = PurePosixPath(path)
    if path in ROOT_DOCS or path in SITE_FILES or path == "editors/vscode-nagi/README.md":
        return True
    if path.startswith("docs/") and value.suffix == ".md":
        return True
    if path.startswith("ai/") and value.suffix == ".md":
        return True
    if path.startswith("test-nagi-code/") and value.name in ("README.md", "README.en.md"):
        return True
    if path.startswith("website/templates/") and value.suffix == ".html":
        return True
    return path.startswith("website/assets/") and value.suffix in ASSET_SUFFIXES


def is_jetbrains_path(path: str) -> bool:
    # The independent workflow checks this plugin. Do not include sibling
    # editors, the shared classifier, or the compiler's own CI workflow.
    return valid_path(path) and (
        path.startswith("editors/jetbrains-nagi/")
        or path == ".github/workflows/jetbrains.yml"
    )


def requires_jetbrains(path: str) -> bool:
    # Workflow, build, and compiler inputs exercise its integration gate.
    return valid_path(path) and (
        is_jetbrains_path(path)
        or path in JETBRAINS_CHECK_INPUTS
        or any(path.startswith(prefix) for prefix in JETBRAINS_CHECK_PREFIXES)
    )


def git(*args: str) -> bytes:
    return subprocess.check_output(["git", *args], cwd=ROOT, stderr=subprocess.PIPE)


def classify(event: str, base: str, head: str) -> dict[str, str]:
    result = {
        "full_checks": "true", "jetbrains_checks": "true",
        "reason": "No complete Docs/plugin-only comparison.",
    }
    if event not in ("push", "pull_request"):
        result["reason"] = "Manual, scheduled, or unrecognized event."
        return result
    if not COMMIT.fullmatch(base) or not COMMIT.fullmatch(head) or set(base) == {"0"} or set(head) == {"0"}:
        result["reason"] = "Missing or invalid commit range (including a new branch)."
        return result
    try:
        for ref in (base, head):
            if git("rev-parse", "--verify", f"{ref}^{{commit}}").decode("ascii").strip() != ref:
                return result
        # A force push or an unrelated range may omit relevant history. Run everything.
        git("merge-base", "--is-ancestor", base, head)
        # Disable rename detection so both deleted and added paths must be allowed.
        raw = git("diff", "--no-ext-diff", "--no-renames", "--name-only", "-z", base, head, "--")
        if not raw or not raw.endswith(b"\0"):
            return result
        paths = raw[:-1].decode("utf-8").split("\0")
    except (OSError, subprocess.SubprocessError, UnicodeError):
        return result
    if any(not valid_path(path) for path in paths):
        return result
    result["jetbrains_checks"] = str(any(requires_jetbrains(path) for path in paths)).lower()
    if all(is_docs_path(path) or is_jetbrains_path(path) for path in paths):
        result["full_checks"] = "false"
        result["reason"] = (
            "The complete range changes only the independently checked JetBrains plugin and/or Docs/site presentation."
            if any(is_jetbrains_path(path) for path in paths)
            else "The complete range changes only Docs/site presentation."
        )
    else:
        result["reason"] = "Code, configuration, build inputs, or an unrecognized path changed."
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event", required=True)
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = classify(args.event, args.base, args.head)
    with args.output.open("a", encoding="utf-8") as output:
        output.write(f"full_checks={result['full_checks']}\n")
        output.write(f"jetbrains_checks={result['jetbrains_checks']}\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
