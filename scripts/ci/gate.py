"""Require the jobs selected by the change and package plans to succeed."""
from __future__ import annotations

import json
import os


def errors(needs: dict) -> list[str]:
    failures = []
    for job in ("changes", "release-plan"):
        if needs.get(job, {}).get("result") != "success":
            failures.append(f"{job}: {needs.get(job, {}).get('result', 'missing')}")
    full = needs.get("changes", {}).get("outputs", {}).get("full_checks")
    if full not in ("true", "false"):
        failures.append("linux: missing or invalid check plan")
    else:
        expected = "success" if full == "true" else "skipped"
        actual = needs.get("linux", {}).get("result", "missing")
        if actual != expected:
            failures.append(f"linux: {actual}, expected {expected}")
    jetbrains = needs.get("changes", {}).get("outputs", {}).get("jetbrains_checks")
    if jetbrains not in ("true", "false"):
        failures.append("jetbrains: missing or invalid check plan")
    else:
        expected = "success" if jetbrains == "true" else "skipped"
        actual = needs.get("jetbrains", {}).get("result", "missing")
        if actual != expected:
            failures.append(f"jetbrains: {actual}, expected {expected}")
    plan = needs.get("release-plan", {}).get("outputs", {})
    for component, job in (("vscode", "vscode-package"), ("nagi", "nagi-package")):
        planned = plan.get(f"package_{component}")
        if planned not in ("true", "false"):
            failures.append(f"{job}: missing or invalid package plan")
            continue
        expected = "success" if planned == "true" else "skipped"
        actual = needs.get(job, {}).get("result", "missing")
        if actual != expected:
            failures.append(f"{job}: {actual}, expected {expected}")
    return failures


def main() -> None:
    failures = errors(json.loads(os.environ["NAGI_CI_NEEDS"]))
    if failures:
        raise SystemExit("\n".join(failures))
    print("Required checks and planned packages passed.")


if __name__ == "__main__":
    main()
