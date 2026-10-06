#!/usr/bin/env python3
"""S1先行fixtureの登録を検査する。Nagiのparse/check成功は検査しない。"""
import json
from pathlib import Path


def verify(root):
    cases = json.loads((root / "contracts.json").read_text(encoding="utf-8"))
    names = set()
    files = set()
    pairs = {}
    for case in cases:
        assert case["name"] not in names, case["name"]
        names.add(case["name"])
        path = root / case["source"]
        assert path.parent == root and path.is_file(), path
        files.add(path.name)
        text = path.read_text(encoding="utf-8")
        assert text.strip(), path
        assert isinstance(case["high"], bool), case
        assert path.suffix == (".nagi" if case["high"] else ".low"), case
        assert case["expected"] in {"check-pass", "check-fail"}, case
        assert case["contract"], case
        if case["expected"] == "check-fail":
            assert case["diagnostic"] and 0 < case["primary_line"] <= len(text.splitlines()), case
            marker = "# primary"
            assert marker in text.splitlines()[case["primary_line"] - 1], case
        else:
            assert case["primary_line"] == 0 and case["diagnostic"] == "", case
        pairs.setdefault(path.stem, set()).add(case["high"])
    assert all(pair == {True, False} for pair in pairs.values()), pairs
    assert files == {p.name for p in root.iterdir() if p.suffix in {".nagi", ".low"}}
    return len(cases), len(pairs)


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1] / "tests/task-handles"
    count, pairs = verify(root)
    print(f"Task future-contract inputs: {count} sources / {pairs} High-Low pairs (registration only)")
