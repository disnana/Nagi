"""Recheck the complete selected CI plan before any release token is used."""
from __future__ import annotations

import json
import os
import re

import gate


def errors(needs, event, ref, sha):
    failures = []
    if event != "push" or ref != "refs/heads/main":
        failures.append("publication requires a main push")
    try:
        if not isinstance(needs, dict):
            raise ValueError("invalid needs object")
        failures.extend(gate.errors(needs))
        if needs.get("ready", {}).get("result") != "success":
            failures.append("ready: required successful aggregate")
        if needs.get("ci-plan", {}).get("outputs", {}).get("level") != "3":
            failures.append("publication requires level 3")
        plan = needs.get("release-plan", {}).get("outputs", {})
        flags = [plan.get("release_" + component) for component in ("nagi", "vscode", "jetbrains")]
        if any(flag not in ("true", "false") for flag in flags) or "true" not in flags:
            failures.append("missing or invalid formal release plan")
        for component in ("nagi", "vscode", "jetbrains"):
            if plan.get("release_" + component) == "true" and plan.get("package_" + component) != "true":
                failures.append(component + ": release requires a validated package")
        if not isinstance(sha, str) or not re.fullmatch(r"[0-9a-f]{40}", sha) or plan.get("sha") != sha:
            failures.append("release source must equal this workflow commit")
    except (ValueError, TypeError, AttributeError, KeyError):
        failures.append("malformed publication inputs")
    return failures


def main():
    try:
        needs = json.loads(os.environ.get("NAGI_CI_NEEDS", ""))
    except (ValueError, TypeError):
        raise SystemExit("malformed publication inputs") from None
    failures = errors(needs, os.environ.get("GITHUB_EVENT_NAME"),
                      os.environ.get("GITHUB_REF"), os.environ.get("GITHUB_SHA"))
    if failures:
        raise SystemExit("\n".join(failures))
    print("Exact-source level-3 release checks passed; publication may proceed.")


if __name__ == "__main__":
    main()
