#!/usr/bin/env python3
"""Compare generated and manual Query calls using the existing bounded fixture."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--out", type=Path, default=Path("/tmp/nagi-sf05-query-cost"))
OUTPUT = parser.parse_args().out.resolve()
TARGET = Path(os.environ["NAGI_NATIVE_TARGET_DIR"])
assert not OUTPUT.exists(), OUTPUT
result = subprocess.run(["cargo", "run", "--locked", "-p", "nagic", "--example", "sqlite-public-cost", "--", str(OUTPUT)],
                        cwd=ROOT, text=True, capture_output=True, timeout=180)
assert result.returncode == 0, result.stdout + result.stderr
print(result.stdout + result.stderr, flush=True)
records = {}
workspace_packages = {(p["name"], p["version"]): p.get("checksum") for p in tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]}
for form in ("generated", "manual"):
    package = OUTPUT / form
    result = subprocess.run(["cargo", "build", "--offline", "--release", "--manifest-path", str(package / "Cargo.toml"), "--target-dir", str(TARGET)],
                            cwd=ROOT, text=True, capture_output=True, timeout=180)
    (OUTPUT / (form + "-build.log")).write_text(result.stdout + result.stderr)
    assert result.returncode == 0 and "warning:" not in result.stderr, result.stdout + result.stderr
    packages = tomllib.loads((package / "Cargo.lock").read_text())["package"]
    for p in packages:
        if p.get("checksum"):
            assert workspace_packages.get((p["name"], p["version"])) == p["checksum"], p
    binary = TARGET / "release" / ("sqlite-cost-" + form)
    result = subprocess.run([str(binary)], cwd=ROOT, text=True, capture_output=True, timeout=10)
    (OUTPUT / (form + "-run.log")).write_text(result.stdout + result.stderr)
    assert result.returncode == 0 and not result.stderr
    rows = [json.loads(line) for line in result.stdout.splitlines()]
    assert [row["name"] for row in rows] == ["future_and_open", "literal", "selected", "close_after_rollback"]
    assert all(len(row["samples"]) == 32 for row in rows[1:3])
    records[form] = {"binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "measurements": rows}
    print(form + ": 32 literal + 32 selected samples; rollback and actual close passed", flush=True)
(OUTPUT / "results.json").write_text(json.dumps(records, indent=2) + "\n")
print("Cost: generated and manual Query measurements passed; no performance threshold", flush=True)
