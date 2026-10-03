"""Verify UTF-8 JSON files and reject paths that already exist."""

import json
from pathlib import Path
import subprocess
import tempfile


def verify(executable: Path, env: dict, output: Path) -> dict:
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    results = []
    with tempfile.TemporaryDirectory(prefix="JSON 設定 ", dir=output) as temporary:
        working = Path(temporary)
        destination = working / "倉庫 settings.json"

        def invoke(path):
            process = subprocess.run(
                [str(executable.resolve())], env=dict(env, NAGI_SAMPLE_FILE=str(path)),
                cwd=working, text=True, encoding="utf-8", capture_output=True, timeout=10,
            )
            result = {
                "exit_code": process.returncode,
                "stdout": process.stdout, "stderr": process.stderr,
            }
            results.append(result)
            (output / "results.json").write_text(
                json.dumps(results, ensure_ascii=False, indent=2), encoding="utf-8",
            )
            return process, result

        expected = {"site": "東京", "enabled": True, "retries": 3}
        process, result = invoke(destination)
        assert process.returncode == 0 and process.stderr == "", result
        lines = process.stdout.splitlines()
        assert len(lines) == 3 and json.loads(lines[0]) == expected, result
        assert lines[1:] == [str(destination), "file-json: OK"], result
        stored = json.loads(destination.read_text(encoding="utf-8"))
        assert stored == expected and type(stored["enabled"]) is bool, stored
        assert type(stored["retries"]) is int, stored

        original = destination.read_bytes()
        process, result = invoke(destination)
        assert process.returncode == 1 and process.stdout == "", result
        assert process.stderr.startswith("Invalid: "), result
        assert destination.read_bytes() == original, "the original JSON was overwritten"

        existing = working / "既存 settings.json"
        previous = b'{"site":"existing","enabled":false,"retries":9}'
        existing.write_bytes(previous)
        process, result = invoke(existing)
        assert process.returncode == 1 and process.stdout == "", result
        assert process.stderr.startswith("Invalid: "), result
        assert existing.read_bytes() == previous, "the existing configuration was overwritten"

        directory = working / "設定 directory"
        directory.mkdir()
        process, result = invoke(directory)
        assert process.returncode == 1 and process.stdout == "", result
        assert process.stderr.startswith("Invalid: "), result
        assert directory.is_dir() and not list(directory.iterdir()), directory
    return {"cases": len(results), "result": "passed"}
