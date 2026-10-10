#!/usr/bin/env python3
"""Execute every editor test file directly so Node reports actual test counts."""
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
total = 0
files = sorted((ROOT / "editors/vscode-nagi/test").glob("*.test.js"))
for file in files:
    result = subprocess.run(["node", str(file)], cwd=ROOT, text=True, capture_output=True)
    text = result.stdout + result.stderr
    print(text, flush=True)
    passed = re.search(r"(?:#|ℹ) pass (\d+)", text)
    failed = re.search(r"(?:#|ℹ) fail (\d+)", text)
    skipped = re.search(r"(?:#|ℹ) skipped (\d+)", text)
    assert result.returncode == 0 and passed and int(passed[1]) > 0, file
    assert failed and int(failed[1]) == 0 and skipped and int(skipped[1]) == 0, file
    total += int(passed[1])
assert len(files) == 22, len(files)
print(f"Editor: {len(files)} files, {total} tests executed, 0 failed, 0 skipped", flush=True)
