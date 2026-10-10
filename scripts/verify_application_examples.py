"""Exercise High projects with saved Low, and projects written directly in Low."""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import tomllib
import traceback
from native_artifacts import native_executable

ROOT = Path(__file__).resolve().parents[1]
PROJECTS = ROOT / "test-nagi-code/application-examples"
NAMES = ("stock-report", "device-settings", "seat-reservations", "file-json", "quote-api", "supervised-worker", "byte-inspector", "axum-service", "auth-boundary", "order-quote")
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


def verify_smoke(verify, executable, env, directory, name, mode):
    try:
        return verify(executable, env, directory)
    except Exception as error:
        report = {
            "project": name, "source": mode, "executable": str(executable),
            "exception_type": type(error).__name__, "exception": str(error)[:4096],
            "traceback": traceback.format_exc()[-16384:],
        }
        try:
            details = directory / "smoke-failure.json"
            if details.exists():
                with details.open("rb") as source:
                    # quote-api's bounded log tails fit this evidence budget.
                    content = source.read(512 * 1024 + 1)
                if len(content) > 512 * 1024:
                    raise ValueError("smoke evidence exceeds 512 KiB")
                report["smoke"] = json.loads(content)
            failure_directory = Path(env.get("NAGI_FAILURE_DIR", ROOT / "build/compiler-failures"))
            failure_directory.mkdir(parents=True, exist_ok=True)
            (failure_directory / f"application-{name}-{mode}.json").write_text(
                json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8",
            )
        except (OSError, ValueError) as capture_error:
            error.add_note(f"Unable to save application failure evidence: {capture_error}")
        raise


def verify_axum_native_tests(published, target, env, log):
    # Use the successful generation, rather than a mutable compatibility manifest.
    manifest = published.parent / "Cargo.toml"
    with manifest.open("rb") as source:
        bins = tomllib.load(source).get("bin", [])
    if len(bins) != 1 or not isinstance(bins[0].get("name"), str):
        raise ValueError(f"Expected one generated Axum executable: {manifest}")
    run([
        "cargo", "test", "--locked", "--release", "--manifest-path", manifest,
        "--target-dir", target, "--bin", bins[0]["name"], "native::tests::",
        "--", "--nocapture",
    ], env, log)
    summaries = re.findall(
        r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;",
        log.read_text(encoding="utf-8"),
    )
    if len(summaries) != 1:
        raise RuntimeError(f"Missing or ambiguous Axum native test result: {log}")
    passed, failed, ignored = map(int, summaries[0])
    if passed == 0 or failed != 0 or ignored != 0:
        raise RuntimeError(f"Axum native tests did not all execute successfully: {log}")
    return {"passed": passed, "failed": failed, "ignored": ignored}


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
            published = native_executable(directory, target)
            executable = directory / published.name
            shutil.copy2(published, executable)
            native_tests = None
            if name == "axum-service":
                native_tests = verify_axum_native_tests(
                    published, target, env, generated / f"{mode}-native-tests.log",
                )
            facts = verify_smoke(verify, executable, env, directory, name, mode)
            if native_tests is not None:
                facts["native_tests"] = native_tests
            rows.append({"project": name, "source": mode, "status": "passed", **facts})
            (output / "results.json").write_text(
                json.dumps(rows, ensure_ascii=False, indent=2) + "\n", encoding="utf-8",
            )
            print(f"Passed: {name} ({mode})", flush=True)
    print(json.dumps({"projects": len({row["project"] for row in rows}), "runs": len(rows), "status": "passed"}), flush=True)


if __name__ == "__main__":
    main()
