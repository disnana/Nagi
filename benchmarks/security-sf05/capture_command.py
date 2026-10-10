#!/usr/bin/env python3
"""Save a command's raw output and source/cache identity; require exit and marker."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
from run_validation import ROOT, sources


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--marker", action="append", default=[])
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command
    if command and command[0] == "--":
        command = command[1:]
    if not command:
        parser.error("command is required")
    args.destination.mkdir(parents=True, exist_ok=False)
    log = args.destination / "raw.log"
    record = {
        "command": command,
        "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "source_sha256": sources(),
        "cache": {key: os.environ.get(key) for key in ("CARGO_TARGET_DIR", "NAGI_NATIVE_TARGET_DIR", "CARGO_NET_OFFLINE")},
        "compiler_sha256": hashlib.sha256((Path(os.environ["CARGO_TARGET_DIR"]) / "debug/nagic").read_bytes()).hexdigest(),
        "clean_build_claim": False,
    }
    started = time.monotonic()
    with log.open("wb") as output:
        result = subprocess.run(command, cwd=ROOT, stdout=output, stderr=subprocess.STDOUT)
    text = log.read_text(errors="replace")
    matched = {marker: marker in text for marker in args.marker}
    accepted = result.returncode == 0 and all(matched.values())
    record.update(exit=result.returncode, seconds=time.monotonic() - started,
                  markers=matched, accepted=accepted,
                  raw_log_sha256=hashlib.sha256(log.read_bytes()).hexdigest())
    (args.destination / "provenance.json").write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps({"exit": result.returncode, "accepted": accepted, "log": str(log)}), flush=True)
    return int(not accepted)


if __name__ == "__main__":
    raise SystemExit(main())
