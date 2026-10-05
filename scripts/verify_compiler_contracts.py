#!/usr/bin/env python3
"""外部conformance corpusと必須統合harnessの接続を検査する。

--run-linked は各Cargo/HTTP/SQL harnessを順番に実行する。登録だけでは保証しない。
"""
import argparse
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def verify():
    corpus = json.loads((ROOT / "tests/conformance/corpus.json").read_text(encoding="utf-8"))
    names = set()
    for case in corpus:
        assert case["name"] not in names, case["name"]
        names.add(case["name"])
        source = ROOT / "tests/conformance" / case["source"]
        assert source.is_file() and source.read_text(encoding="utf-8"), source
        assert case["expected"] in {"run-pass", "compile-pass", "reject:high-parse", "reject:high-check", "reject:low-parse", "reject:low-check"}, case
        assert isinstance(case["high"], bool), case
        if case["expected"].startswith("reject:"):
            assert case["diagnostic"] and 0 < case["line"] <= len(source.read_text(encoding="utf-8").splitlines()), case
            assert case["expected"].split(":")[1].startswith("high" if case["high"] else "low"), case
        if case["expected"] == "run-pass":
            assert case["oracle"], case
    harnesses = json.loads((ROOT / "tests/conformance/harnesses.json").read_text(encoding="utf-8"))
    for harness in harnesses:
        text = "\n".join((ROOT / file).read_text(encoding="utf-8") for file in harness["files"])
        for name in harness["test_names"]:
            assert re.search(r"\b(?:async\s+)?fn\s+" + re.escape(name) + r"\s*\(", text), name
        assert harness["command"][:2] == ["cargo", "test"], harness
    return corpus, harnesses


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-linked", action="store_true")
    args = parser.parse_args()
    corpus, harnesses = verify()
    print(f"conformance corpus: {len(corpus)}, linked harnesses: {len(harnesses)} (registration checked)", flush=True)
    if args.run_linked:
        for harness in harnesses:
            print(f"running {harness['name']}: {' '.join(harness['command'])}", flush=True)
            subprocess.run(harness["command"], cwd=ROOT, check=True)


if __name__ == "__main__":
    main()
