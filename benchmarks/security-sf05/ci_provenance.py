#!/usr/bin/env python3
"""Save CI source identity and native/log hashes after the explicit SF05 guards."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from run_validation import ROOT, sources

directory = Path(sys.argv[1])
directory.mkdir(parents=True, exist_ok=True)
record = {
    "checkout_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
    "github_sha": os.environ.get("GITHUB_SHA"),
    "platform": os.environ.get("SF05_PLATFORM"),
    "rustc": subprocess.check_output(["rustc", "-Vv"], text=True),
    "cargo": subprocess.check_output(["cargo", "-V"], text=True).strip(),
    "source_sha256": sources(),
    "artifact_sha256": {str(path.relative_to(directory)): hashlib.sha256(path.read_bytes()).hexdigest()
                        for path in sorted(directory.rglob("*")) if path.is_file()},
}
(directory / "provenance.json").write_text(json.dumps(record, indent=2) + "\n")
