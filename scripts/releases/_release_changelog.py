"""Shared formal changelog section parsing for planning and publication notes."""
from __future__ import annotations

import re

VERSION_PATTERN = r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
HEADING = re.compile(
    rf"## (?:(?:Nagi (?P<nagi>{VERSION_PATTERN}))(?: / VS Code (?P<vscode>{VERSION_PATTERN}))?"
    rf"|VS Code (?P<extension>{VERSION_PATTERN})|JetBrains (?P<jetbrains>{VERSION_PATTERN}))"
    rf"(?: — [0-9]{{4}}-[0-9]{{2}}-[0-9]{{2}})?"
)


class MissingChangelogEntryError(ValueError):
    """No formal section matches the requested component version."""


class InvalidChangelogEntryError(ValueError):
    """A matching formal section is empty or duplicated."""


def changelog_entry_from_text(text: str, component: str, version: str, source: str) -> str:
    sections = []
    lines = text.splitlines()
    fence = None
    headings = []
    for index, line in enumerate(lines):
        marker = re.match(r"^\s{0,3}(`{3,}|~{3,})", line)
        if marker:
            run = marker.group(1)
            if fence is None:
                fence = run
            elif run[0] == fence[0] and len(run) >= len(fence) and not line[marker.end():].strip(" \t"):
                fence = None
        elif fence is None and line.startswith("## "):
            headings.append((index, line))
    for position, (start, heading) in enumerate(headings):
        match = HEADING.fullmatch(heading)
        if not match:
            continue
        selected = {
            "nagi": match.group("nagi"),
            "vscode": match.group("vscode") or match.group("extension"),
            "jetbrains": match.group("jetbrains"),
        }[component]
        if selected == version:
            end = headings[position + 1][0] if position + 1 < len(headings) else len(lines)
            sections.append("\n".join(lines[start + 1:end]).strip())
    message = f"CHANGELOG.md at {source} must contain exactly one nonempty {component} {version} entry"
    if not sections:
        raise MissingChangelogEntryError(message)
    if len(sections) != 1 or not sections[0]:
        raise InvalidChangelogEntryError(message)
    return sections[0]
