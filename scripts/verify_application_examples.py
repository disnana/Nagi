"""Exercise High projects with saved Low, and projects written directly in Low."""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
PROJECTS = ROOT / "test-nagi-code/application-examples"
NAMES = ("stock-report", "device-settings", "seat-reservations", "file-json", "quote-api", "supervised-worker", "byte-inspector", "axum-service", "order-quote")
LOW_PROJECTS = {"order-quote": ROOT / "test-nagi-code/low-examples/order-quote"}
EXE = ".exe" if os.name == "nt" else ""


def run(args, env, log):
    process = subprocess.Popen(
        [str(arg) for arg in args], cwd=ROOT, env=env,
        text=True, encoding="utf-8", stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        start_new_session=os.name != "nt",
    )
    try:
        stdout, stderr = process.communicate(timeout=180)
    except subprocess.TimeoutExpired:
        if os.name == "nt":
            subprocess.run(
                ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                capture_output=True, timeout=10,
            )
        else:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        stdout, stderr = process.communicate(timeout=10)
        log.write_text(stdout + stderr + "\nTimed out after 180 seconds.\n", encoding="utf-8")
        raise RuntimeError(f"Command timed out: {args}; see {log}") from None
    log.write_text(stdout + stderr, encoding="utf-8")
    if process.returncode:
        raise RuntimeError(f"Command failed: {args}\n{stdout}{stderr}")
    if "warning:" in stderr:
        raise RuntimeError(f"Build warnings: {args}\n{stderr}")


def verifier(project):
    spec = importlib.util.spec_from_file_location(
        "application_" + project.name.replace("-", "_"), project / "smoke.py",
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.verify


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", default=str(
        Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "release" / ("nagic" + EXE)
    ))
    parser.add_argument("--only", choices=NAMES, help="Verify one project")
    args = parser.parse_args()
    compiler = Path(shutil.which(args.compiler) or args.compiler).resolve()
    target = Path(os.environ.get("NAGI_NATIVE_TARGET_DIR", ROOT / "native-target")).resolve()
    output = ROOT / "build/application-example-verification"
    output.mkdir(parents=True, exist_ok=True)
    (output / "results.json").write_text("[]\n", encoding="utf-8")
    env = dict(os.environ, NAGI_ROOT=str(ROOT), NAGI_NATIVE_TARGET_DIR=str(target))
    rows = []
    for name in (args.only,) if args.only else NAMES:
        project = LOW_PROJECTS.get(name, PROJECTS / name)
        verify = verifier(project)
        generated = output / name
        generated.mkdir(parents=True, exist_ok=True)
        # Low sources are checked and built directly; lower does not regenerate
        # a Low input. High sources also run from independently loaded saved Low.
        modes = ("low",) if name in LOW_PROJECTS else ("high", "low")
        for mode in modes:
            directory = generated / mode
            source = [] if mode == "high" or name in LOW_PROJECTS else [generated / "high/generated.low"]
            options = [*source, "--project", project, "--out", directory]
            run([compiler, "check", *options], env, generated / f"{mode}-check.log")
            run([compiler, "build", *options], env, generated / f"{mode}-build.log")
            with (directory / "Cargo.toml").open("rb") as manifest:
                package = tomllib.load(manifest)["package"]["name"]
            executable = directory / (package + EXE)
            shutil.copy2(target / "release" / (package + EXE), executable)
            facts = verify(executable, env, directory)
            rows.append({"project": name, "source": mode, "status": "passed", **facts})
            (output / "results.json").write_text(
                json.dumps(rows, ensure_ascii=False, indent=2) + "\n", encoding="utf-8",
            )
            print(f"Passed: {name} ({mode})", flush=True)
    print(json.dumps({"projects": len({row["project"] for row in rows}), "runs": len(rows), "status": "passed"}), flush=True)


if __name__ == "__main__":
    main()
