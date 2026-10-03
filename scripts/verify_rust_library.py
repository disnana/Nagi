"""Build the local Rust library example from another working directory."""
import argparse
import os
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXPECTED = """Workshop notebook
Subtotal cents:
3000
Discount cents:
300
Service fee cents:
0
Total cents:
2700
unit price must be nonnegative and quantity must be positive
Local Rust library verified.
"""


def verify(compiler: Path) -> None:
    compiler = compiler.resolve()
    project = ROOT / "test-nagi-code/rust-library/nagi.toml"
    with tempfile.TemporaryDirectory(prefix="nagi library 凪 ") as directory:
        folder = Path(directory)
        result = subprocess.run(
            [str(compiler), "run", "--project", str(project), "--out", str(folder / "generated")],
            cwd=folder, text=True, capture_output=True, encoding="utf-8",
        )
        if result.returncode:
            raise RuntimeError(result.stdout + result.stderr)
        expected_lines = EXPECTED.splitlines()
        assert result.stdout.splitlines()[-len(expected_lines):] == expected_lines, result.stdout
        assert "unused import" not in result.stderr, result.stderr
    print("Passed: rust-library (local crate, package alias, feature selection, separate cwd/output)")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    executable = "nagic.exe" if os.name == "nt" else "nagic"
    parser.add_argument("--compiler", type=Path, default=target / "release" / executable)
    verify(parser.parse_args().compiler)
