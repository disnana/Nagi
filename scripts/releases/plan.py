"""Plan independent Nagi/VSIX releases from a push's complete commit range."""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERSION = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)")


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT).decode("utf-8")


def version_tuple(value: str) -> tuple[int, int, int]:
    match = VERSION.fullmatch(value)
    if not match:
        raise ValueError(f"Formal releases require an x.y.z version: {value!r}")
    return tuple(int(part) for part in match.groups())


def versions(ref: str) -> dict[str, str]:
    return {
        "nagi": tomllib.loads(git("show", f"{ref}:Cargo.toml"))["workspace"]["package"]["version"],
        "vscode": json.loads(git("show", f"{ref}:editors/vscode-nagi/package.json"))["version"],
    }


def plan(base: str, head: str) -> dict[str, str]:
    # Resolve to immutable commit IDs before passing revisions to later commands.
    head = git("rev-parse", "--verify", f"{head}^{{commit}}").strip()
    current = versions(head)
    if not base or set(base) == {"0"}:
        base = head  # Initial pushes do not invent an initial release.
    base = git("rev-parse", "--verify", f"{base}^{{commit}}").strip()
    previous = versions(base)
    changed = git("diff", "--name-only", base, head).splitlines()
    packaging_changed = any(p in (".github/workflows/ci.yml", "scripts/install.sh", "scripts/install.ps1",
                                  "scripts/uninstall.sh", "scripts/uninstall.ps1")
                            or p.startswith("scripts/releases/") for p in changed)
    # Validate all four native distributions when their build/test inputs
    # change. Packaging does not publish an unchanged component version.
    nagi_changed = any(p in ("Cargo.toml", "Cargo.lock")
                       or p.startswith(("compiler/", "runtime/")) for p in changed)
    extension_changed = any(p.startswith("editors/vscode-nagi/")
                            and not p.startswith("editors/vscode-nagi/test/")
                            and p != "editors/vscode-nagi/README.md" for p in changed)
    result = {"sha": head}
    for component, value in current.items():
        new = version_tuple(value)
        old = version_tuple(previous[component])
        release = value != previous[component]
        if release and new <= old:
            raise ValueError(f"{component} version must increase: {previous[component]} → {value}")
        result[f"{component}_version"] = value
        result[f"release_{component}"] = str(release).lower()
        package = (release or packaging_changed
                   or (component == "nagi" and nagi_changed)
                   or (component == "vscode" and extension_changed))
        result[f"package_{component}"] = str(package).lower()
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = plan(args.base, args.head)
    with args.output.open("a", encoding="utf-8") as output:
        for key, value in result.items():
            output.write(f"{key}={value}\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
