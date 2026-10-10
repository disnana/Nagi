#!/usr/bin/env python3
"""Verify optional SQL checking from an unrelated directory without Cargo/runtime."""
import os
from pathlib import Path
import shutil
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts/releases"))
from verify import verify_sql

base = ROOT / "build/sf05-isolated-check"
base.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix="outside-", dir=base) as folder:
    project = Path(folder)
    compiler = project / "nagic"
    shutil.copy2(Path(os.environ["CARGO_TARGET_DIR"]) / "debug/nagic", compiler)
    verify_sql(compiler, project, dict(os.environ))
