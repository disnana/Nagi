#!/usr/bin/env python3
"""Run migrated HTTP/SQLite business cases on small owned loopback fixtures."""
from __future__ import annotations
import argparse
import json
import os
import re
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from urllib.error import HTTPError, URLError
from urllib.request import ProxyHandler, Request, build_opener

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from native_artifacts import native_executable

COMPILER = Path(os.environ["CARGO_TARGET_DIR"]) / "debug/nagic"
TARGET = Path(os.environ["NAGI_NATIVE_TARGET_DIR"])
OPENER = build_opener(ProxyHandler({}))


def run(command: list[str], directory: Path, label: str) -> str:
    result = subprocess.run(command, cwd=directory, text=True, capture_output=True, timeout=180)
    (directory / (label + ".log")).write_text(result.stdout + result.stderr)
    assert result.returncode == 0, (command, result.stdout, result.stderr)
    assert "warning:" not in result.stderr, result.stderr
    return result.stdout


def request(base: str, method: str, path: str, expected: int, payload=None):
    body = None if payload is None else json.dumps(payload, ensure_ascii=False).encode()
    req = Request(base + path, data=body, method=method)
    if body is not None:
        req.add_header("Content-Type", "application/json")
    try:
        response = OPENER.open(req, timeout=3)
    except HTTPError as error:
        response = error
    with response:
        text = response.read().decode()
        assert response.status == expected, (method, path, response.status, text)
        value = json.loads(text) if text and response.headers.get_content_type() == "application/json" else text
        print(json.dumps({"method": method, "path": path, "status": response.status, "body": value}, ensure_ascii=False), flush=True)
        return value


def business(base: str, name: str) -> int:
    collection, text, scalar = {
        "crud": ("/users", "name", "age"),
        "inventory": ("/items", "name", "quantity"),
        "tasks": ("/api/tasks", "title", "done"),
    }[name]
    assert request(base, "GET", collection, 200) == []
    request(base, "GET", collection + "/999", 404)
    original = {text: "small fixture", scalar: False if name == "tasks" else 18}
    created = request(base, "POST", collection, 200, original)
    assert created == {"id": 1, **original}, created
    item = collection + "/1"
    assert request(base, "GET", item, 200) == created
    changed = {text: "変更済み", scalar: True if name == "tasks" else 19}
    assert request(base, "PUT", item, 200, changed) == {"id": 1, **changed}
    request(base, "PUT", collection + "/999", 404, original)
    deleted = request(base, "DELETE", item, 200)
    assert deleted == ({"id": 1, "deleted": True} if name == "inventory" else 1), deleted
    request(base, "GET", item, 404)
    assert request(base, "GET", collection, 200) == []
    return 9


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=ROOT / "build/sf05-http-examples")
    parser.add_argument("--only", nargs="+", choices=("crud", "inventory", "tasks", "result-api"))
    args = parser.parse_args()
    output = args.out.resolve()
    output.mkdir(parents=True, exist_ok=False)
    specs = [("crud", ROOT / "examples/crud.nagi", 8080),
             ("inventory", ROOT / "test-nagi-code/inventory.nagi", None),
             ("tasks", ROOT / "test-nagi-code/web-demo/tasks.nagi", None),
             ("result-api", ROOT / "test-nagi-code/result-api/server.nagi", None)]
    if args.only:
        specs = [spec for spec in specs if spec[0] in args.only]
    records = []
    for name, source, fixed_port in specs:
        with tempfile.TemporaryDirectory(prefix=name + "-", dir=output) as folder:
            project = Path(folder)
            high = project / source.name
            # Copy the entire physical import closure, not just the entry. Low
            # later runs after all these original High modules are removed.
            pending = [source]
            copied = set()
            while pending:
                original = pending.pop().resolve()
                if original in copied:
                    continue
                copied.add(original)
                relative = original.relative_to(source.parent)
                destination = project / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(original, destination)
                for imported in re.findall(r'^import "([^"]+)"', original.read_text(), re.M):
                    pending.append(original.parent / imported)
            lowered = project / "lowered"
            run([str(COMPILER), "lower", str(high), "--no-project", "--out", str(lowered)], project, "lower")
            saved = project / "saved.low"
            shutil.copyfile(lowered / "generated.low", saved)
            for form, entry in [("high", high), ("saved-low", saved)]:
                if form == "saved-low":
                    for original in copied:
                        file = project / original.relative_to(source.parent)
                        file.unlink()
                        assert not file.exists()
                    assert not [p for p in project.rglob("*.nagi") if p.is_file()]
                generated = project / form
                run([str(COMPILER), "check", str(entry), "--no-project", "--out", str(generated)], project, form + "-check")
                run([str(COMPILER), "build", str(entry), "--no-project", "--out", str(generated)], project, form + "-build")
                with socket.socket() as probe:
                    probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                    probe.bind(("127.0.0.1", fixed_port or 0))
                    port = probe.getsockname()[1]
                env = dict(os.environ, NAGI_DB=":memory:", NAGI_PORT=str(port))
                base = f"http://127.0.0.1:{port}"
                with (project / (form + "-server.log")).open("w") as log:
                    process = subprocess.Popen([str(native_executable(generated, TARGET))], cwd=project, env=env, stdout=log, stderr=log)
                    try:
                        for _ in range(100):
                            assert process.poll() is None, (name, form, "server exited")
                            try:
                                with OPENER.open(base + "/health", timeout=0.2) as response:
                                    assert response.status == 200
                                break
                            except URLError:
                                time.sleep(0.03)
                        else:
                            raise AssertionError("server did not become ready")
                        if name == "result-api":
                            result = run([sys.executable, str(source.parent / "smoke_api.py"), "--base-url", base], project, form + "-smoke")
                            assert "PASS: 15 HTTP checks" in result
                            count = 15
                        else:
                            count = business(base, name)
                    finally:
                        process.terminate()
                        process.wait(timeout=5)
                records.append({"example": name, "form": form, "checks": count, "high_removed": form == "saved-low", "status": "passed"})
                # Keep raw command/server logs; generated packages remain governed
                # by the compiler's own artifact lifecycle, not hand-edited here.
                record_dir = output / f"{name}-{form}"
                record_dir.mkdir(exist_ok=False)
                for log in project.glob("*.log"):
                    shutil.copyfile(log, record_dir / log.name)
                shutil.copyfile(entry, record_dir / entry.name)
                shutil.copyfile(generated / "src/main.rs", record_dir / "generated.rs")
                (output / "results.json").write_text(json.dumps(records, indent=2) + "\n")
                print(f"Passed: {name} ({form}), {count} business HTTP checks", flush=True)
    (output / "results.json").write_text(json.dumps(records, indent=2) + "\n")
    runs = 2 * len(specs)
    checks = sum(30 if name == "result-api" else 18 for name, _source, _port in specs)
    assert len(records) == runs and sum(row["checks"] for row in records) == checks
    print(f"HTTP examples: {runs} native runs, {checks} business checks, 0 failures", flush=True)


if __name__ == "__main__":
    main()
