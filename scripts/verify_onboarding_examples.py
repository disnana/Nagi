"""Check the bilingual first-app tutorial against High and saved-Low execution."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

from native_artifacts import native_executable

ROOT = Path(__file__).resolve().parents[1]
EXE = ".exe" if os.name == "nt" else ""
CASES = [
    ("700", 0, "within budget", ""),
    ("1000", 0, "within budget", ""),
    ("1001", 0, "over budget", ""),
    ("abc", 1, None, "Invalid:"),
    ("-1", 1, None, "amount must be non-negative"),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", default=str(
        Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
        / "release" / ("nagic" + EXE)))
    parser.add_argument("--docs-only", action="store_true",
                        help="Check published code against the native-tested source without building")
    args = parser.parse_args()
    compiler = Path(shutil.which(args.compiler) or args.compiler).resolve()
    source = ROOT / "examples/tutorial/first_app.nagi"
    expected = source.read_text(encoding="utf-8").strip()
    for doc in [ROOT / "docs/first-app.md", ROOT / "docs/en/first-app.md"]:
        blocks = re.findall(r"^```nagi\n(.*?)^```", doc.read_text(encoding="utf-8"),
                            re.MULTILINE | re.DOTALL)
        if sum(block.strip() == expected for block in blocks) != 1:
            raise AssertionError(f"Tutorial code differs from the runnable example: {doc}")

    if args.docs_only:
        print("Onboarding: Japanese/English tutorial code matches the native-tested source")
        return

    output = ROOT / "build/onboarding-example-verification"
    output.mkdir(parents=True, exist_ok=True)
    target = Path(os.environ.get("NAGI_NATIVE_TARGET_DIR", ROOT / "native-target")).resolve()
    env = dict(os.environ, NAGI_ROOT=str(ROOT), NAGI_NATIVE_TARGET_DIR=str(target))
    rows = []
    # Saved Low must still build after its High source disappears. Each run owns
    # its workspace; native_executable selects the successful app generation.
    with tempfile.TemporaryDirectory(prefix="nagi-onboarding-", dir=output) as temporary:
        project = Path(temporary)
        high = project / "first_app.nagi"
        shutil.copyfile(source, high)

        def compile_step(stage, entry, destination, log_name):
            result = subprocess.run(
                [str(compiler), stage, str(entry), "--out", str(destination)], cwd=project, env=env,
                text=True, capture_output=True, timeout=600,
            )
            (output / log_name).write_text(result.stdout + result.stderr, encoding="utf-8")
            if result.returncode:
                raise RuntimeError(f"{stage} failed: {result.stdout}{result.stderr}")

        lowered = project / "lowered"
        compile_step("lower", high, lowered, "lower.log")
        saved = project / "saved.low"
        shutil.copyfile(lowered / "generated.low", saved)
        for form, entry in [("high", high), ("saved-low", saved)]:
            if form == "saved-low":
                high.unlink()
            generated = project / (form + "-build")
            compile_step("check", entry, generated, form + "-check.log")
            compile_step("build", entry, generated, form + "-build.log")
            executable = native_executable(generated, target)
            for value, code, answer, error in CASES:
                result = subprocess.run(
                    [str(executable)], input=value + "\n", cwd=project, env=env,
                    text=True, capture_output=True, timeout=10,
                )
                lines = ["Enter a whole-number amount:"]
                if answer is not None:
                    lines += [answer, "done"]
                if (result.returncode != code or result.stdout.splitlines() != lines
                        or (error and error not in result.stderr)
                        or (not error and result.stderr)):
                    raise AssertionError(f"{form} input {value}: {result}")
                rows.append({"form": form, "input": value, "exit": result.returncode,
                             "stdout": result.stdout, "stderr": result.stderr})
    report = {"docs_code": "Japanese/English match", "native_cases": rows}
    (output / "results.json").write_text(json.dumps(report, ensure_ascii=False, indent=2),
                                       encoding="utf-8")
    print("Onboarding: bilingual code matches; High/saved Low check and build; 10 native cases passed")


if __name__ == "__main__":
    main()
