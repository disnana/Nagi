"""Exercise the handwritten Low quote CLI with real JSON input."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess


def verify(executable: Path, env: dict, output: Path) -> dict:
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)

    def order(customer="north", items=None):
        return {"customer": customer, "items": items if items is not None else [
            {"product_id": 1, "quantity": 2, "unit_price_cents": 1250},
        ]}

    def summary(customer, lines, units, subtotal, discount):
        return {
            "customer": customer, "lines": lines, "units": units,
            "subtotal_cents": subtotal, "discount_cents": discount,
            "total_cents": subtotal - discount,
        }

    cases = [
        ("small order", order(), summary("north", 1, 2, 2500, 0)),
        ("unicode and discount", order("東京", [
            {"product_id": 1, "quantity": 3, "unit_price_cents": 2500},
            {"product_id": 2, "quantity": 5, "unit_price_cents": 500},
        ]), summary("東京", 2, 8, 10000, 500)),
        ("below discount threshold", order(items=[
            {"product_id": 1, "quantity": 1, "unit_price_cents": 9999},
        ]), summary("north", 1, 1, 9999, 0)),
        ("discount rounds down", order(items=[
            {"product_id": 1, "quantity": 1, "unit_price_cents": 10019},
        ]), summary("north", 1, 1, 10019, 500)),
        ("free item", order(items=[
            {"product_id": 3, "quantity": 1, "unit_price_cents": 0},
        ]), summary("north", 1, 1, 0, 0)),
        ("repeated product ID counts as separate lines", order(items=[
            {"product_id": 1, "quantity": 2, "unit_price_cents": 1250},
            {"product_id": 1, "quantity": 1, "unit_price_cents": 1250},
        ]), summary("north", 2, 3, 3750, 0)),
        ("maximum supported order", order("c" * 80, [
            {"product_id": 1, "quantity": 1000, "unit_price_cents": 1000000}
            for _ in range(1000)
        ]), summary("c" * 80, 1000, 1000000, 1000000000000, 50000000000)),
        ("escaped customer name", order("A\nB\"C"), summary("A\nB\"C", 1, 2, 2500, 0)),
        ("empty items", order(items=[]), "items must contain"),
        ("too many lines", order(items=[
            {"product_id": 1, "quantity": 1, "unit_price_cents": 0}
            for _ in range(1001)
        ]), "items must contain"),
        ("blank customer", order(""), "customer must contain"),
        ("UTF-8 customer byte limit", order("凪" * 27), "customer must contain"),
        ("zero product ID", order(items=[
            {"product_id": 0, "quantity": 1, "unit_price_cents": 100},
        ]), "product_id must be"),
        ("negative product ID", order(items=[
            {"product_id": -1, "quantity": 1, "unit_price_cents": 100},
        ]), "product_id must be"),
        ("zero quantity", order(items=[
            {"product_id": 1, "quantity": 0, "unit_price_cents": 100},
        ]), "quantity must be"),
        ("negative quantity", order(items=[
            {"product_id": 1, "quantity": -1, "unit_price_cents": 100},
        ]), "quantity must be"),
        ("quantity exceeds limit", order(items=[
            {"product_id": 1, "quantity": 1001, "unit_price_cents": 100},
        ]), "quantity must be"),
        ("negative price", order(items=[
            {"product_id": 1, "quantity": 1, "unit_price_cents": -1},
        ]), "unit_price_cents must be"),
        ("price exceeds limit", order(items=[
            {"product_id": 1, "quantity": 1, "unit_price_cents": 1000001},
        ]), "unit_price_cents must be"),
        ("invalid JSON", "{", "input must contain"),
        ("missing field", {"customer": "north", "items": [
            {"product_id": 1, "quantity": 1},
        ]}, "input must contain"),
        ("incorrect numeric type", order(items=[
            {"product_id": 1, "quantity": "two", "unit_price_cents": 100},
        ]), "input must contain"),
        ("fractional quantity", order(items=[
            {"product_id": 1, "quantity": 1.5, "unit_price_cents": 100},
        ]), "input must contain"),
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
            json.dumps(results, ensure_ascii=False, indent=2) + "\n", encoding="utf-8",
        )
        if isinstance(expected, dict):
            assert process.returncode == 0, result
            actual = json.loads(process.stdout)
            assert actual == expected, result
            assert all(type(actual[key]) is int for key in expected if key != "customer"), result
            assert process.stderr == "", result
        else:
            assert process.returncode == 1, result
            assert process.stdout == "", result
            assert expected in process.stderr, result
    return {"cases": len(results), "result": "passed"}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=Path("build/order-quote-smoke"))
    args = parser.parse_args()
    print(json.dumps(verify(args.executable, dict(os.environ), args.out)))
