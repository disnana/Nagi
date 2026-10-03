"""Run the stock report against real JSON input, including rejected records."""

import json
from pathlib import Path
import subprocess


def verify(executable: Path, env: dict, output: Path) -> dict:
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    cases = [
        (
            "unicode warehouse",
            {"warehouse": "東京倉庫", "items": [
                {"product_id": 1, "on_hand": 12, "reserved": 3},
                {"product_id": 2, "on_hand": 5, "reserved": 5},
            ]},
            {"warehouse": "東京倉庫", "products": 2, "available": 9, "reserved": 8},
        ),
        (
            "empty warehouse inventory",
            {"warehouse": "north", "items": []},
            {"warehouse": "north", "products": 0, "available": 0, "reserved": 0},
        ),
        (
            "maximum supported batch",
            {"warehouse": "north", "items": [
                {"product_id": index + 1, "on_hand": 1000000, "reserved": 0}
                for index in range(10000)
            ]},
            {"warehouse": "north", "products": 10000, "available": 10000000000, "reserved": 0},
        ),
        ("reserved exceeds stock", {"warehouse": "north", "items": [
            {"product_id": 1, "on_hand": 2, "reserved": 3},
        ]}, "counts must satisfy"),
        ("negative count", {"warehouse": "north", "items": [
            {"product_id": 1, "on_hand": -1, "reserved": 0},
        ]}, "counts must satisfy"),
        ("invalid product", {"warehouse": "north", "items": [
            {"product_id": 0, "on_hand": 2, "reserved": 1},
        ]}, "product_id must be positive"),
        ("missing field", {"warehouse": "north", "items": [
            {"on_hand": 2, "reserved": 1},
        ]}, "input must contain"),
        ("invalid JSON", "{", "input must contain"),
        ("wrong field type", {"warehouse": "north", "items": [
            {"product_id": 1, "on_hand": "two", "reserved": 0},
        ]}, "input must contain"),
        ("blank warehouse", {"warehouse": "", "items": []}, "warehouse must contain"),
        ("UTF-8 warehouse byte limit", {"warehouse": "凪" * 27, "items": []}, "warehouse must contain"),
        ("batch exceeds limit", {"warehouse": "north", "items": [
            {"product_id": index + 1, "on_hand": 0, "reserved": 0}
            for index in range(10001)
        ]}, "more than 10000 products"),
    ]
    results = []
    for name, value, expected in cases:
        text = value if isinstance(value, str) else json.dumps(value, ensure_ascii=False)
        process = subprocess.run(
            [str(executable.resolve())], input=text + "\n", text=True, encoding="utf-8",
            capture_output=True, env=env, cwd=output, timeout=10,
        )
        result = {
            "case": name, "exit_code": process.returncode,
            "stdout": process.stdout, "stderr": process.stderr,
        }
        results.append(result)
        (output / "results.json").write_text(
            json.dumps(results, ensure_ascii=False, indent=2), encoding="utf-8",
        )
        if isinstance(expected, dict):
            assert process.returncode == 0, result
            actual = json.loads(process.stdout)
            assert actual == expected, result
            assert all(type(actual[key]) is int for key in ("products", "available", "reserved")), result
            assert process.stderr == "", result
        else:
            assert process.returncode == 1, result
            assert process.stdout == "", result
            assert expected in process.stderr, result
    return {"cases": len(results), "result": "passed"}
