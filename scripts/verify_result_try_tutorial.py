"""Verify the Result/try tutorial as docs, Python, High, and saved Low."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

from native_artifacts import native_executable

ROOT = Path(__file__).resolve().parents[1]
EXE = ".exe" if os.name == "nt" else ""
TUTORIAL = ROOT / "test-nagi-code/result-try-tutorial"
PROMPT = "金額を入力してください > "


def run(command: list[str], **kwargs) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, text=True, encoding="utf-8", capture_output=True,
                          timeout=600, **kwargs)


def check_python() -> list[dict[str, object]]:
    rows = []
    expected = {
        "21": (0, PROMPT + "21\n42\n完了\n"),
        "abc": (0, PROMPT + "invalid literal for int() with base 10: 'abc'\n"
                "入力を直して再実行してください\n"),
        "-1": (0, PROMPT + "金額は0以上で入力してください\n"
                "入力を直して再実行してください\n"),
    }
    python_env = dict(os.environ, PYTHONIOENCODING="utf-8")
    for value, (code, stdout) in expected.items():
        result = run([sys.executable, str(TUTORIAL / "companion.py")],
                     input=value + "\n", cwd=ROOT, env=python_env)
        if result.returncode != code or result.stdout != stdout or result.stderr:
            raise AssertionError(f"Python input {value}: {result}")
        rows.append({"input": value, "exit": code, "stdout": result.stdout})
    return rows


def check_published_files() -> None:
    required = {
        "main.nagi": ["Pythonでは", "Nagiでは", "Nagi独自の仕様として", "try read_line()"],
        "clean.nagi": ["try read_line()", "try make_quote"],
        "companion.py": ["Pythonでは", "input(\"金額を入力してください > \")"],
        "README.md": ["Nagi 0.2.1", "nagic run --project"],
        "README.en.md": ["Nagi 0.2.1", "nagic run --project"],
    }
    for name, fragments in required.items():
        text = (TUTORIAL / name).read_text(encoding="utf-8")
        for fragment in fragments:
            if fragment not in text:
                raise AssertionError(f"{name} must contain {fragment!r}")


def compile_step(compiler: Path, stage: str, entry: Path, destination: Path,
                 cwd: Path, env: dict[str, str], output: Path, log_name: str) -> None:
    result = run([str(compiler), stage, str(entry), "--out", str(destination)],
                 cwd=cwd, env=env)
    (output / log_name).write_text(result.stdout + result.stderr, encoding="utf-8")
    if result.returncode:
        raise RuntimeError(f"{stage} failed for {entry.name}: {result.stdout}{result.stderr}")


def assert_main_case(form: str, executable: Path, value: str,
                     cwd: Path, env: dict[str, str]) -> dict[str, object]:
    result = run([str(executable)], input=value + "\n", cwd=cwd, env=env)
    if not result.stdout.startswith(PROMPT):
        raise AssertionError(f"{form} input {value}: {result}")
    lines = result.stdout.removeprefix(PROMPT).splitlines()
    if value == "21":
        valid = lines == ["21", "42", "完了"]
    elif value == "abc":
        valid = (len(lines) == 2 and bool(lines[0].strip())
                 and lines[1] == "入力を直して再実行してください")
    else:
        valid = lines == ["金額は0以上で入力してください", "入力を直して再実行してください"]
    if result.returncode != 0 or result.stderr or not valid:
        raise AssertionError(f"{form} input {value}: {result}")
    return {"form": form, "input": value, "exit": result.returncode,
            "stdout": result.stdout, "stderr": result.stderr}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", default=str(
        Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
        / "release" / ("nagic" + EXE)))
    parser.add_argument("--docs-only", action="store_true",
                        help="Check published files and the Python companion without native builds")
    args = parser.parse_args()

    check_published_files()
    python_rows = check_python()
    if args.docs_only:
        print("Result/try tutorial: published files and 3 Python cases passed")
        return

    compiler = Path(shutil.which(args.compiler) or args.compiler).resolve()
    output = ROOT / "build/result-try-tutorial-verification"
    output.mkdir(parents=True, exist_ok=True)
    target = Path(os.environ.get("NAGI_NATIVE_TARGET_DIR", ROOT / "native-target")).resolve()
    env = dict(os.environ, NAGI_ROOT=str(ROOT), NAGI_NATIVE_TARGET_DIR=str(target))
    rows = []

    with tempfile.TemporaryDirectory(prefix="nagi-result-try-", dir=output) as temporary:
        project = Path(temporary)
        high = project / "main.nagi"
        clean = project / "clean.nagi"
        shutil.copyfile(TUTORIAL / "main.nagi", high)
        shutil.copyfile(TUTORIAL / "clean.nagi", clean)

        lowered = project / "lowered"
        compile_step(compiler, "lower", high, lowered, project, env, output, "lower.log")
        saved = project / "saved.low"
        shutil.copyfile(lowered / "generated.low", saved)

        for form, entry in [("high", high), ("saved-low", saved)]:
            if form == "saved-low":
                high.unlink()
            generated = project / (form + "-build")
            compile_step(compiler, "check", entry, generated, project, env, output,
                         form + "-check.log")
            compile_step(compiler, "build", entry, generated, project, env, output,
                         form + "-build.log")
            executable = native_executable(generated, target)
            for value in ["21", "abc", "-1"]:
                rows.append(assert_main_case(form, executable, value, project, env))

        clean_output = project / "clean-build"
        compile_step(compiler, "check", clean, clean_output, project, env, output,
                     "clean-check.log")
        compile_step(compiler, "build", clean, clean_output, project, env, output,
                     "clean-build.log")
        clean_executable = native_executable(clean_output, target)
        success = run([str(clean_executable)], input="21\n", cwd=project, env=env)
        if success.returncode or success.stderr or success.stdout != PROMPT + "21\n42\n完了\n":
            raise AssertionError(f"clean input 21: {success}")
        rows.append({"form": "clean", "input": "21", "exit": success.returncode,
                     "stdout": success.stdout, "stderr": success.stderr})
        for value, error in [("abc", "Invalid: invalid digit found in string\n"),
                             ("-1", "Invalid: 金額は0以上で入力してください\n")]:
            failure = run([str(clean_executable)], input=value + "\n", cwd=project, env=env)
            if failure.returncode != 1 or failure.stdout != PROMPT or failure.stderr != error:
                raise AssertionError(f"clean input {value}: {failure}")
            rows.append({"form": "clean", "input": value, "exit": failure.returncode,
                         "stdout": failure.stdout, "stderr": failure.stderr})

    report = {"python_cases": python_rows, "native_cases": rows}
    (output / "results.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print("Result/try tutorial: Python 3 cases; High/saved Low 6 cases; clean 3 cases passed")


if __name__ == "__main__":
    main()
