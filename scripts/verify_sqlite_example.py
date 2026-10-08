"""Verify the public SQLite tutorial through High and independent saved Low."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
from native_artifacts import native_executable

ROOT = Path(__file__).resolve().parents[1]
EXE = ".exe" if os.name == "nt" else ""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", default=str(
        Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "release" / ("nagic" + EXE)))
    parser.add_argument("--docs-only", action="store_true")
    args = parser.parse_args()
    compiler = Path(shutil.which(args.compiler) or args.compiler).resolve()
    source = ROOT / "examples/sqlite_pool.nagi"
    expected = source.read_text(encoding="utf-8").strip()
    for doc in [ROOT / "docs/sqlite-pool.md", ROOT / "docs/en/sqlite-pool.md"]:
        blocks = re.findall(r"^```nagi\n(.*?)^```", doc.read_text(encoding="utf-8"), re.M | re.S)
        if sum(block.strip() == expected for block in blocks) != 1:
            raise AssertionError(f"SQLite tutorial differs from executable source: {doc}")
    if args.docs_only:
        print("SQLite: Japanese/English tutorial code matches the executable source")
        return
    output = ROOT / "build/sqlite-example-verification"
    output.mkdir(parents=True, exist_ok=True)
    target = Path(os.environ.get("NAGI_NATIVE_TARGET_DIR", ROOT / "native-target")).resolve()
    env = dict(os.environ, NAGI_ROOT=str(ROOT), NAGI_NATIVE_TARGET_DIR=str(target))
    rows = []
    (output / "results.json").write_text("[]\n", encoding="utf-8")
    with tempfile.TemporaryDirectory(prefix="sqlite-", dir=output) as directory:
        project = Path(directory)
        high = project / "sqlite_pool.nagi"
        shutil.copyfile(source, high)

        def compile_step(stage, entry, destination, log):
            completed = subprocess.run(
                [str(compiler), stage, str(entry), "--out", str(destination)],
                cwd=project, env=env, text=True, capture_output=True, timeout=600)
            (output / log).write_text(completed.stdout + completed.stderr, encoding="utf-8")
            if completed.returncode:
                raise RuntimeError(f"{stage} failed: {completed.stdout}{completed.stderr}")

        lowered = project / "lowered"
        compile_step("lower", high, lowered, "lower.log")
        saved = project / "saved.low"
        shutil.copyfile(lowered / "generated.low", saved)
        for form, entry in [("high", high), ("saved-low", saved)]:
            if form == "saved-low":
                high.unlink()
            generated = project / (form + "-build")
            compile_step("check", entry, generated, form + "-check.log")
            compile_step("build", entry, generated, form + "-build.log")
            completed = subprocess.run([str(native_executable(generated, target))],
                                       cwd=project, env=env, text=True, capture_output=True, timeout=30)
            if completed.returncode or completed.stdout.splitlines() != ["7", "closed"] or completed.stderr:
                raise AssertionError(f"{form}: {completed}")
            rows.append({"form": form, "check": "passed", "build": "passed", "exit": completed.returncode,
                         "stdout": completed.stdout, "stderr": completed.stderr})
            (output / "results.json").write_text(json.dumps(rows, indent=2) + "\n", encoding="utf-8")
    print("SQLite: bilingual source matches; High and independent saved Low check/build/native passed")


if __name__ == "__main__":
    main()
