"""Verify actor failures, task panic recovery and cleanup without fixed sleeps."""

import json
from pathlib import Path
import subprocess


def verify(executable: Path, env: dict, directory: Path) -> dict:
    directory = directory.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    expected = {
        "packing_total": 4,
        "audit_total": 9,
        "packing_starts": 2,
        "audit_starts": 1,
        "connector_starts": 2,
        "failed_events": 1,
        "panicked_events": 1,
        "restarts": 2,
        "stopped_children": 3,
        "shutdown_events": 1,
        "connector_active": 0,
        "connector_cleanups": 2,
    }
    child_env = dict(env, RUST_BACKTRACE="0")
    process = subprocess.Popen(
        [str(executable.resolve())], cwd=directory, env=child_env,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, encoding="utf-8",
    )
    timed_out = False
    try:
        stdout, stderr = process.communicate(timeout=15)
    except subprocess.TimeoutExpired:
        timed_out = True
        process.kill()
        stdout, stderr = process.communicate(timeout=5)
    (directory / "worker.stdout").write_text(stdout, encoding="utf-8")
    (directory / "worker.stderr").write_text(stderr, encoding="utf-8")
    assert not timed_out, "supervised worker did not finish within 15 seconds"
    assert process.returncode == 0, (process.returncode, stdout, stderr)
    lines = stdout.splitlines()
    assert lines[:-1] == [
        "job rejected: packing total stays 5",
        "packing restarted: total reset to 0; audit continued at 9",
        "connector panic recovered: generation 2 is active",
    ], lines
    result = json.loads(lines[-1])
    assert result == expected, result
    # Rust's panic hook still prints for a panic caught by the Supervisor.
    assert stderr.count("supervised-worker: controlled connector panic") == 1, stderr
    assert "panicked at" in stderr, stderr
    (directory / "results.json").write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8",
    )
    return {"cases": len(expected), "starts": 1, "result": "passed"}
