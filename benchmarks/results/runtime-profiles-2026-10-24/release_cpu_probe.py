#!/usr/bin/env python3
"""Default-release compiler regression probe; auto is deliberately not selected."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import tomllib

p = argparse.ArgumentParser()
for name in ("baseline", "compiler", "runtime-root", "work-dir", "output"):
    p.add_argument("--" + name, type=Path, required=True)
p.add_argument("--repeat", type=int, default=15)
a = p.parse_args()
if not 3 <= a.repeat <= 100 or a.output.exists():
    p.error("repeat must be3..100; output must be new")
sys.path.insert(0, str(a.runtime_root.resolve() / "scripts"))
import benchmark_smart_build_matrix as h
h.reject_overrides(dict(os.environ))
try:
    h.validate_measurement_paths(a.runtime_root.resolve(), a.work_dir, a.output)
except ValueError as error:
    p.error(str(error))
a.work_dir.mkdir(parents=True, exist_ok=True)
h.capacity([a.work_dir, Path(tempfile.gettempdir())], 3 * 1024 ** 3)
root = Path(tempfile.mkdtemp(prefix="nagi-default-release-cpu-", dir=a.work_dir.resolve()))
project, runtime, target = root / "project", root / "runtime-copy", root / "cargo/target"
project.mkdir()
copy = h.copy_runtime(a.runtime_root.resolve(), runtime)
source = ('@rust("native::iterations")\nextern def iterations() -> i64\n'
          'def main():\n    n = iterations()\n    i = 0\n    checksum = 0\n'
          '    while i < n:\n        checksum = (checksum + i) % 1000000007\n'
          '        i = i + 1\n    print(checksum)\n')
(project / "cpu.nagi").write_text(source)
(project / "native.rs").write_text('pub fn iterations() -> i64 { std::hint::black_box(5_000_000) }\n')
expected = str((5_000_000 * 4_999_999 // 2) % 1_000_000_007) + "\n"
environment = dict(os.environ, NAGI_ROOT=str(runtime), NAGI_NATIVE_TARGET_DIR=str(target),
                   CARGO_NET_OFFLINE="true", CARGO_TERM_COLOR="never", CARGO_BUILD_JOBS="2")
report = {"complete": False, "scope": "default release compiler regression; same current full runtime; no auto/core throughput claim",
          "conditions": "same source/runtime paths and seed lock; ordinary release profile; jobs2; warm registry/OS caches; shared Cargo target; alternating native execution order",
          "root": str(root), "platform": platform.platform(), "repeat": a.repeat,
          "runtime_copy": copy, "source": source, "expected": expected, "roles": {}}
a.output.parent.mkdir(parents=True, exist_ok=True)
with a.output.open("x") as f:
    json.dump(report, f, indent=2)

def save():
    a.output.write_text(json.dumps(report, indent=2) + "\n")

def run_native(role, label):
    row = report["roles"][role]
    binary = Path(row["native"])
    if h.digest(binary) != row["native_sha256"]:
        raise RuntimeError("published executable changed")
    sample = h.measure([str(binary)], project, environment, root / f"{role}-{label}.log")
    if sample["exit_code"] or Path(sample["log"]).read_text() != expected:
        raise RuntimeError("native checksum or exit mismatch")
    return sample

try:
    report["rustc"] = subprocess.check_output(["rustc", "-Vv"], text=True).strip()
    report["cargo"] = subprocess.check_output(["cargo", "-V"], text=True).strip()
    for role, compiler in (("baseline", a.baseline.resolve()), ("current", a.compiler.resolve())):
        folder = root / role
        out = folder / "out"
        out.mkdir(parents=True)
        shutil.copyfile(runtime / "Cargo.lock", out / "Cargo.lock")
        compiler_sha = h.digest(compiler)
        command = [str(compiler), "build", "cpu.nagi", "--no-project", "--rust", "native.rs",
                   "--out", str(out), "--profile", "release", "--build-stats"]
        print(role + ": build", flush=True)
        build = h.measure(command, project, environment, folder / "build.log")
        stats = build.get("stats") or {}
        if (build["exit_code"] or stats.get("profile") != "release" or stats.get("checks") != "full"
                or stats.get("cargo_always_invoked") is not True or h.digest(compiler) != compiler_sha):
            raise RuntimeError("default release build/check contract mismatch")
        if role == "current" and stats.get("runtime") != {"requested": "full", "selected": "full", "reason": "full_requested"}:
            raise RuntimeError("default changed its runtime configuration")
        binary = h.native_executable(out, target)
        h.verify_receipt(binary.parent / "build-receipt.json", compiler)
        manifest = tomllib.loads((binary.parent / "Cargo.toml").read_text())
        report["roles"][role] = {"compiler_sha256": compiler_sha, "build": build,
                                 "native": str(binary), "native_sha256": h.digest(binary),
                                 "release_profile": manifest["profile"]["release"], "samples": []}
        run_native(role, "check")
        save()
    base, current = report["roles"]["baseline"], report["roles"]["current"]
    if (base["build"]["stats"]["rust_sha256"] != current["build"]["stats"]["rust_sha256"]
            or base["release_profile"] != current["release_profile"]):
        raise RuntimeError("generated Rust or release configuration changed")
    report["generated_rust_equal"] = True
    for role in report["roles"]:
        for i in range(2):
            run_native(role, f"warmup-{i}")
    for i in range(a.repeat):
        order = ("baseline", "current") if i % 2 == 0 else ("current", "baseline")
        for role in order:
            report["roles"][role]["samples"].append(run_native(role, f"sample-{i}"))
        save()
    for row in report["roles"].values():
        row["summary"] = {key: h.distribution([s[key] for s in row["samples"]])
                          for key in ("wall_s", "cpu_s", "peak_child_rss_kib")}
    owned = {"target": str(target), "complete": True}
    h.prune_target(root, owned)
    for role in report["roles"]:
        run_native(role, "after-prune")
    report["owned_target_pruned"] = True
    report["complete"] = True
    save()
except BaseException as error:
    report["error"] = f"{type(error).__name__}: {error}"
    save()
    raise
print(a.output)
