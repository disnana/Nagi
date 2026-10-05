"""Record rebuild costs with the shared dependency cache; never claims a clean build."""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import resource
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--generated", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    rows = []
    for run in range(3):
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        started = time.monotonic()
        completed = subprocess.run([str(args.compiler.resolve()),"build","--project",str(args.project),"--out",str(args.generated)],capture_output=True,text=True,timeout=180)
        elapsed = time.monotonic() - started
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        (args.output / f"build-{run}.log").write_text(completed.stdout+completed.stderr)
        assert completed.returncode == 0, completed.stderr
        rows.append({"run":run,"wall_seconds":elapsed,"child_user_seconds":after.ru_utime-before.ru_utime,"child_sys_seconds":after.ru_stime-before.ru_stime})
    report={"kind":"rebuild of identical generated project using existing Cargo dependency cache; first run may compile edited source; no target deletion; both policy modes reside in one binary","runs":rows}
    (args.output / "build-times.json").write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
