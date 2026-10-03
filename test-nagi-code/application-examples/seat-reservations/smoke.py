"""The Nagi application asserts replies, restarts, and independent state."""

import json
from pathlib import Path
import subprocess


def verify(executable: Path, env: dict, output: Path) -> dict:
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    process = subprocess.run(
        [str(executable.resolve())], text=True, encoding="utf-8", capture_output=True,
        env=env, cwd=output, timeout=15,
    )
    result = {
        "exit_code": process.returncode,
        "stdout": process.stdout, "stderr": process.stderr,
    }
    (output / "result.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
    assert process.returncode == 0, result
    assert process.stdout == (
        "reservations: validated duplicates, capacity, restart and sibling isolation\n"
    ), result
    assert process.stderr == "", result
    return {"cases": 1, "result": "passed"}
