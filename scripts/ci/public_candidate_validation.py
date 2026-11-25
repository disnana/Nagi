"""Run the normal native gates with no publication command or credentials."""
import hashlib
import json
import os
from pathlib import Path
import platform as host_platform
import re
import shutil
import subprocess
import sys
import tarfile
import zipfile

BASE = "546b8eaa3b1d069473824d4a161a7f5421174763"
CANDIDATE = "d8fadadd1f64b7c4d7ee20585fbd4fa287b12a81"
TREE = "2e9837dc317828f9b73b2d27b11d64215063f25c"

NATIVE = {
    "linux-x86_64": ("x86_64-unknown-linux-gnu", 37, "Linux", {"x86_64", "amd64"}),
    "windows-x86_64": ("x86_64-pc-windows-msvc", 36, "Windows", {"x86_64", "amd64"}),
    "macos-x86_64": ("x86_64-apple-darwin", 37, "Darwin", {"x86_64", "amd64"}),
    "macos-arm64": ("aarch64-apple-darwin", 37, "Darwin", {"arm64", "aarch64"}),
}


def main(root, evidence, target_platform, rust_host, expected_go):
    root, evidence = Path(root).resolve(), Path(evidence).resolve()
    reports = evidence / "summaries"
    reports.mkdir(parents=True)
    build = root / "target"
    native = root / "native-target"
    env = dict(os.environ, CARGO_TARGET_DIR=str(build), NAGI_NATIVE_TARGET_DIR=str(native))
    for key in ("NAGI_ROOT", "ICU4X_DATA_DIR", "GH_TOKEN", "GITHUB_TOKEN"):
        env.pop(key, None)
    py = sys.executable
    expected_host, known_go, expected_os, cpu_names = NATIVE[target_platform]
    rust = subprocess.check_output(["rustc", "-vV"], cwd=root, env=env, text=True)
    go = subprocess.check_output(["go", "version"], cwd=root, env=env, text=True).strip()
    assert (rust_host, expected_go) == (expected_host, known_go)
    assert f"host: {expected_host}" in rust.splitlines(), rust
    assert host_platform.system() == expected_os and host_platform.machine().lower() in cpu_names
    assert "go1.27.3 " in go, go
    head = subprocess.check_output(["git", "rev-parse", "HEAD", "HEAD^{tree}"], cwd=root, text=True)
    assert head.split() == [CANDIDATE, TREE], head
    rows = []
    result = {
        "commit": CANDIDATE, "tree": TREE, "base": BASE, "platform": target_platform,
        "host": host_platform.machine(), "rust_host": rust_host, "go": go, "commands": rows,
    }

    def run(name, *argv, cwd=root, environment=env):
        print(f"::group::{name}", flush=True)
        log = evidence / f"{name}.log"
        with log.open("wb") as output:
            process = subprocess.run(argv, cwd=cwd, env=environment, stdout=output, stderr=subprocess.STDOUT)
        lines = log.read_text(errors="replace").splitlines()
        summary = [line for line in lines if line.startswith(("test result:", "Verified ", "Ran ", "HTTP route ", "Standard HTTP "))]
        rows.append({"name": name, "exit": process.returncode, "summary": summary})
        print(f"{name}: exit {process.returncode}", flush=True)
        for line in summary:
            if re.fullmatch(r"test result: (?:ok|FAILED)\. \d+ passed; \d+ failed; \d+ ignored; \d+ measured; \d+ filtered out; finished in [\d.]+s", line):
                print(line, flush=True)
            if re.fullmatch(r"Ran \d+ tests? in [\d.]+s", line):
                print(line, flush=True)
        if process.returncode:
            python_codes = {"AssertionError", "CalledProcessError", "FileNotFoundError", "NoticeError",
                            "OSError", "RuntimeError", "TimeoutExpired", "UnicodeDecodeError", "ValueError"}
            kinds = {line.split(":", 1)[0].rsplit(".", 1)[-1] for line in lines}
            codes = {match.group() for line in lines for match in re.finditer(r"error\[E\d{4}\]", line)}
            diagnostics = sorted((kinds & python_codes) | codes)
            if diagnostics:
                print("Diagnostic categories: " + ", ".join(diagnostics), flush=True)
            if name in {"workspace", "go", "scoped-tasks"}:
                failures = {match.group(1) for line in lines
                            if (match := re.fullmatch(r"test ([A-Za-z_][A-Za-z_0-9:]*) \.\.\. FAILED", line))}
                for failure in sorted(failures):
                    print(f"Failed test: {failure}", flush=True)
        print("::endgroup::", flush=True)
        return process.returncode

    def suite(name, folder, pattern):
        return run(name, py, "-B", "-m", "unittest", "discover", "-s", folder, "-p", pattern)

    def archive_check(archive, stem):
        checksum = archive.with_name(archive.name + ".sha256")
        with archive.open("rb") as stream:
            actual = hashlib.file_digest(stream, "sha256").hexdigest()
        assert checksum.read_text().split() == [actual, archive.name]
        member = f"{stem}/release.json"
        if archive.suffix == ".zip":
            with zipfile.ZipFile(archive) as wrapped:
                provenance = json.loads(wrapped.read(member))
        else:
            with tarfile.open(archive) as wrapped:
                with wrapped.extractfile(member) as stream:
                    provenance = json.load(stream)
        assert (provenance["commit"], provenance["platform"], provenance["version"]) == (CANDIDATE, target_platform, "0.3.0")
        shutil.copyfile(checksum, reports / checksum.name)
        (reports / "release.json").write_text(json.dumps(provenance, indent=2) + "\n")
        result["archive_sha256"] = actual

    try:
        if target_platform == "linux-x86_64":
            suite("ci-policy", "scripts/ci", "test_*.py")
            suite("release-policy", "scripts/releases", "test_*.py")
        run("release-plan", py, "-B", "scripts/releases/plan.py", "--base", BASE,
            "--head", CANDIDATE, "--output", str(evidence / "release-plan.outputs"))
        plan = dict(line.split("=", 1) for line in (evidence / "release-plan.outputs").read_text().splitlines())
        assert plan["package_nagi"] == "true"
        assert all(plan[f"release_{name}"] == "false" for name in ("nagi", "vscode", "jetbrains"))
        fetch_status = run("fetch-locked-target", "cargo", "fetch", "--locked", "--target", expected_host)
        build_status = run("build-release", "cargo", "build", "--release", "--locked", "-p", "nagic")
        if build_status == 0:
            executable = str(build / "release" / ("nagic.exe" if os.name == "nt" else "nagic"))
            distribution = evidence / "distribution"
            stem = f"nagi-0.3.0-{target_platform}"
            archive = distribution / (stem + (".zip" if os.name == "nt" else ".tar.gz"))
            if fetch_status == 0 and run("package", py, "-B", "scripts/releases/package.py", "--binary", executable,
                                         "--version", "0.3.0", "--platform", target_platform, "--out", str(distribution)) == 0:
                archive_check(archive, stem)
                verify = ("-B", "scripts/releases/verify.py", "--archive", str(archive), "--version", "0.3.0",
                          "--platform", target_platform, "--target", str(native), "--go-smoke")
                run("archive-rust-go", py, *verify)
                if os.name == "nt":
                    for shell in ("pwsh", "powershell"):
                        run(f"{shell}-install", shell, "-NoProfile", "-File", "scripts/releases/test_install.ps1",
                            "-Archive", str(archive), "-Version", "0.3.0", "-TestUserPath")
                        run(f"{shell}-uninstall", shell, "-NoProfile", "-File", "scripts/releases/test_uninstall.ps1")
                else:
                    run("shell-installed-cli", py, str(Path(__file__).with_name("public_candidate_install.py")), str(root), str(archive))
            for test in ("tests/route_integration.py", "tests/http_stdlib_integration.py"):
                run(Path(test).stem, py, "-B", test, "--compiler", executable, "--target", str(native))
            for test in ("scripts/verify_onboarding_examples.py", "scripts/verify_application_examples.py"):
                run(Path(test).stem, py, "-B", test, "--compiler", executable)
            editor_env = dict(env, PATH=str(Path(executable).parent) + os.pathsep + env["PATH"])
            editor = sorted(str(path.relative_to(root)) for path in (root / "editors/vscode-nagi/test").glob("*.test.js"))
            assert editor
            run("editor", "node", "--test", *editor, environment=editor_env)
        if target_platform == "linux-x86_64":
            run("fmt", "cargo", "fmt", "--all", "--", "--check")
            run("clippy", "cargo", "clippy", "--locked", "--all-targets", "--", "-D", "warnings")
            run("workspace", "cargo", "test", "--locked", "--workspace", "--no-fail-fast")
            lines = (evidence / "workspace.log").read_text(errors="replace").splitlines()
            start = next((index for index,line in enumerate(lines) if line.startswith("     Running tests/go_backend.rs ")), None)
            if start is not None:
                end = next((index for index in range(start + 1, len(lines)) if lines[index].startswith("     Running ")), len(lines))
                (evidence / "go.log").write_text("\n".join(lines[start:end]) + "\n", encoding="utf-8")
        else:
            run("go", "cargo", "test", "--locked", "-p", "nagic", "--test", "go_backend")
            run("scoped-tasks", "cargo", "test", "--locked", "-p", "nagic", "--test", "scoped_tasks")
        run("go-count", py, "-B", "scripts/ci/assert_go_backend_summary.py", str(evidence / "go.log"), "--expected", str(expected_go))
        assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=root, text=True).strip()
        return int(any(row["exit"] for row in rows))
    finally:
        (reports / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        if summary_path := os.environ.get("GITHUB_STEP_SUMMARY"):
            with Path(summary_path).open("a", encoding="utf-8") as report:
                report.write(f"### {target_platform} at `{CANDIDATE}`\n\n")
                report.write(f"- Rust host: `{rust_host}`\n- Go: `{go}`\n")
                if digest := result.get("archive_sha256"):
                    report.write(f"- Local archive SHA-256: `{digest}`\n")
                report.write("\n| Check | Exit |\n|---|---:|\n")
                for row in rows:
                    report.write(f"| {row['name']} | {row['exit']} |\n")


if __name__ == "__main__":
    raise SystemExit(main(*sys.argv[1:5], int(sys.argv[5])))
