#!/usr/bin/env python3
"""Alternate complete baseline/current editor requests on identical physical inputs."""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import statistics
import subprocess
import tempfile
import time
from pathlib import Path


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def measure(command: list[str], root: Path, request: Path, log: Path, launcher: Path | None) -> tuple[dict, bytes]:
    start = time.perf_counter()
    with request.open("rb") as source, log.open("wb") as output, log.with_suffix(".stderr").open("wb") as error:
        child = subprocess.Popen(([str(launcher)] if launcher else []) + command,
                                 cwd=root, stdin=source, stdout=output, stderr=error)
        if hasattr(os, "wait4"):
            _, status, usage = os.wait4(child.pid, 0)
            child.returncode = os.waitstatus_to_exitcode(status)
            cpu = usage.ru_utime + usage.ru_stime
            rss = usage.ru_maxrss / (1024 if platform.system() == "Darwin" else 1)
        else:
            child.wait()
            cpu = rss = None
    elapsed = time.perf_counter() - start
    payload = log.read_bytes()
    result = {"wall_s": elapsed, "cpu_s": cpu,
              "peak_child_rss_kib": rss, "exit_code": child.returncode,
              "stdout_sha256": digest(payload)}
    if launcher:
        records = [line.removeprefix("nagi-benchmark-rusage: ")
                   for line in log.with_suffix(".stderr").read_text().splitlines()
                   if line.startswith("nagi-benchmark-rusage: ")]
        assert len(records) == 1, "resource launcher did not produce exactly one record"
        record = json.loads(records[0])
        fields = {"wall_s", "cpu_s", "peak_child_rss_kib"}
        assert isinstance(record, dict) and set(record) == fields, "invalid resource fields"
        assert all(type(record[key]) in (int, float) and math.isfinite(record[key])
                   and record[key] >= 0 for key in fields), "invalid resource values"
        for key in fields:
            result[key] = record[key]
    return result, payload


def verify(case: str, value: dict, root: Path, count: int) -> None:
    if case.startswith("symbols"):
        assert value["definitions"] and value["references"]
        assert any(item["name"] == "helper_0" for item in value["definitions"])
        return
    assert value["format"] == "nagi-assist-v1"
    assert value["full_compile_checked"] is False
    if case.startswith("assist-no-query"):
        assert value["frontend_accepted"] is True
        assert value["symbols"]["definitions"]
        return
    items = {item["name"]: item for item in value["completion"]["items"]}
    if case == "assist-import":
        assert "helper_0" in items
        assert value["completion"]["context"] == "import-names"
    elif case == "assist-member":
        assert value["recovered"] is True
        assert set(items) == {"age", "name"}
        assert value["semantic_status"] == "editor-partial"
        assert len(value["diagnostics"]) == 1
    else:
        assert value["semantic_status"] == "editor-partial"
        assert value["frontend_accepted"] is False
        assert len(value["diagnostics"]) == 1
        assert "value0" in items and f"value{count - 1}" in items
        assert items["value0"]["type"] == "i64"
        assert items["value0"]["target"] == {"file": str(root / "main.nagi"), "line": 3, "column": 5, "length": 6}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeat", type=int, default=7)
    parser.add_argument("--baseline-revision", default="unknown")
    parser.add_argument("--compiler-revision", default="unknown")
    parser.add_argument("--profile-label", default="unknown")
    parser.add_argument("--resource-launcher", type=Path,
                        help="optional small Linux wait4 launcher for a separate RSS run")
    args = parser.parse_args()
    if not 3 <= args.repeat <= 30:
        parser.error("repeat must be 3..30")
    compilers = {"baseline": args.baseline.resolve(), "current": args.compiler.resolve()}
    launcher = args.resource_launcher.resolve() if args.resource_launcher else None
    args.work_dir.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="editor-queries-", dir=args.work_dir.resolve()))
    report = {"schema_version": 1, "complete": False, "platform": platform.platform(),
              "profile": args.profile_label, "baseline_revision": args.baseline_revision,
              "compiler_revision": args.compiler_revision,
              "compiler_sha256": {name: digest(path.read_bytes()) for name, path in compilers.items()},
              "resource_launcher_sha256": digest(launcher.read_bytes()) if launcher else None,
              "rss_scope": "small C launcher child wait4" if launcher else "Python launcher child wait4; fork RSS may mask compiler memory",
              "conditions": "sequential alternating order; warm OS cache; one-shot full request; identical source paths; no reusable checker facts; child high-water RSS",
              "work_directory": str(root), "cases": {}}

    def save() -> None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    for count in (20, 500):
        folder = root / str(count)
        folder.mkdir()
        helper = "".join(f"def helper_{n}(value: i64) -> i64:\n    return value + {n}\n" for n in range(count))
        (folder / "helper.nagi").write_text(helper, encoding="utf-8")
        original = 'import helper\nclass Person:\n    age: i64\n    name: str\ndef main():\n    print(helper.helper_0(7))\n'
        (folder / "main.nagi").write_text(original, encoding="utf-8")
        subprocess.run([str(compilers["baseline"]), "lower", "main.nagi", "--no-project", "--out", "out"], cwd=folder, check=True, capture_output=True)
        (folder / "saved.low").write_bytes((folder / "out/generated.low").read_bytes())
        # A separate synthetic lexical-load case keeps semantic work small:
        # one large string exercises scanning/allocation without 500 functions.
        lexical = ('def helper_0(value: i64) -> i64:\n    return value\n'
                   'def main():\n    payload = "' + "x" * (count * 1024)
                   + '"\n    print(helper_0(len(view(payload))))\n')
        (folder / "lexical.nagi").write_text(lexical, encoding="utf-8")
        locals_text = 'import helper\ndef main():\n' + "".join(f"    value{n} = {n}\n" for n in range(count)) + '    label = "🦀"\n    pr\n'
        member_text = original.replace('    print(helper.helper_0(7))\n', '    person = Person(age=7, name="🦀")\n    person.na\n').replace("\n", "\r\n")
        cases = {
            "symbols-high": ("symbols", "main.nagi", {}),
            "symbols-saved-low": ("symbols", "saved.low", {}),
            "symbols-lexical": ("symbols", "lexical.nagi", {}),
            "assist-no-query": ("assist", "main.nagi", {}),
            "assist-no-query-lexical": ("assist", "lexical.nagi", {}),
            "assist-locals": ("assist", "main.nagi", {"text": locals_text, "line": count + 4, "column": 7}),
            "assist-member": ("assist", "main.nagi", {"text": member_text, "line": 7, "column": 14}),
            "assist-import": ("assist", "main.nagi", {"text": "from helper import helper_", "line": 1, "column": 27}),
        }
        for case, (command, entry, overlay) in cases.items():
            request = {"files": []}
            if overlay:
                request = {"files": [{"file": entry, "text": overlay["text"]}], "query": {"file": entry, "line": overlay["line"], "column": overlay["column"]}}
            request_path = folder / f"{case}.json"
            request_path.write_text(json.dumps(request, ensure_ascii=False), encoding="utf-8")
            samples = {name: [] for name in compilers}
            data = {"functions": 1 if entry == "lexical.nagi" else count,
                    "lexical_string_bytes": count * 1024 if entry == "lexical.nagi" else 0,
                    "request_sha256": digest(request_path.read_bytes()), "samples": samples}
            report["cases"][f"{case}-{count}"] = data
            expected = None
            for sample in range(args.repeat):
                for name in (list(compilers) if sample % 2 == 0 else list(reversed(compilers))):
                    result, payload = measure([str(compilers[name]), command, entry, "--no-project", "--editor-input"], folder, request_path, folder / f"{case}-{name}-{sample}.json", launcher)
                    samples[name].append(result)
                    save()
                    assert result["exit_code"] == 0, f"request failed: {folder}, {case}, {name}"
                    value = json.loads(payload)
                    verify(case, value, folder, count)
                    expected = value if expected is None else expected
                    assert value == expected, f"JSON differs: {folder}, {case}, {name}"
            data["summary"] = {name: {metric: {"median": statistics.median(s[metric] for s in values), "min": min(s[metric] for s in values), "max": max(s[metric] for s in values)} for metric in ("wall_s", "cpu_s", "peak_child_rss_kib") if values[0][metric] is not None} for name, values in samples.items()}
            save()
    report["complete"] = True
    save()
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
