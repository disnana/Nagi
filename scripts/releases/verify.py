"""Verify a distribution by using PATH from an unrelated project directory."""
import argparse
import hashlib
import json
import os
import subprocess
import tarfile
import tempfile
import tomllib
from pathlib import Path
from zipfile import ZipFile


def verify(archive: Path, version: str, platform: str, target: Path | None = None) -> None:
    archive = archive.resolve()
    target = target.resolve() if target is not None else None
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    assert archive.with_name(archive.name + ".sha256").read_text().strip() == f"{digest}  {archive.name}"
    stem = f"nagi-{version}-{platform}"
    with tempfile.TemporaryDirectory(prefix="nagi distribution ") as folder:
        folder = Path(folder)
        if archive.suffix == ".zip":
            with ZipFile(archive) as source:
                source.extractall(folder)
        else:
            with tarfile.open(archive) as source:
                source.extractall(folder, filter="data")
        root = folder / stem
        metadata = json.loads((root / "release.json").read_text())
        assert metadata["version"] == version and metadata["platform"] == platform, metadata
        exe = root / ("nagic.exe" if os.name == "nt" else "nagic")
        environment = dict(os.environ)
        environment.pop("NAGI_ROOT", None)
        environment.pop("NAGI_NATIVE_TARGET_DIR", None)
        if target is not None:
            environment["NAGI_NATIVE_TARGET_DIR"] = str(target)
        for flag in ("--version", "-V", "version", "--help"):
            result = subprocess.run([str(exe), flag], cwd=folder, env={**environment, "PATH": ""},
                                    check=True, capture_output=True, text=True, encoding="utf-8")
            assert not result.stderr, result.stderr
            if flag == "--help":
                assert "--version" in result.stdout and "--project" in result.stdout, result.stdout
            else:
                assert result.stdout.strip() == f"nagic {version}", result.stdout
        project = folder / "outside project 凪"
        project.mkdir()
        (project / "nagi.toml").write_text("entry='main.nagi'\n", encoding="utf-8")
        marker = folder.name
        (project / "main.nagi").write_text(
            'def main():\n    print("Hello, Nagi!")\n    print(2 + 2)\n'
            f'    print({json.dumps(marker)})\n', encoding="utf-8")
        records = folder / "record project 凪"
        records.mkdir()
        (records / "records.nagi").write_text('''class Future:
    value: i64
class Payload:
    value: shared[i64]
def first() -> Future:
    return Future(value=1)
def second() -> Future:
    return Future(value=42)
def show(payload: Payload):
    encoded = json_encode(payload)
    match encoded:
        case Ok(text):
            print(text)
        case Err(problem):
            print("encode failed")
def main():
    second()
    selected = first
    selected = second
    print(selected().value)
    show(Payload(value=share(42)))
    input = "{\\"value\\":42}"
    decoded = json_decode[Payload](view(input))
    match decoded:
        case Ok(payload):
            show(payload)
        case Err(problem):
            print("decode failed")
    text = "\\\"Nagi\\\""
    borrowed = json_decode[view[str]](text)
    match borrowed:
        case Ok(value):
            print(value)
            print(text)
        case Err(problem):
            print("borrowed decode failed")
    print(view("temporary"))
''', encoding="utf-8")
        invalid = folder / "invalid project 凪"
        invalid.mkdir()
        invalid_sources = {
            "temporary.nagi": ('def main():\n    saved = view("text")\n    print(saved)\n', 2),
            "json.nagi": ('def main():\n    result = json_decode[Error]("{}")\n', 2),
            "rows.nagi": ('async def rows(db: Db):\n    result = await db_all[i64](db, "select 1")\n', 2),
            "borrow.nagi": ('def main() -> Result[unit, Error]:\n    text = "\\\"Nagi\\\""\n    saved = try json_decode[view[str]](text)\n    text = "other"\n    return ok(print(saved))\n', 4),
        }
        # Invalid Nagi programs must fail before looking for Cargo or runtime.
        for filename, (source, line) in invalid_sources.items():
            path = invalid / filename
            path.write_text(source, encoding="utf-8")
            for command in ("check", "build"):
                result = subprocess.run([str(exe), command, str(path)], cwd=folder,
                                        env={**environment, "PATH": "", "NAGI_ROOT": str(folder / "missing runtime")},
                                        capture_output=True, text=True, encoding="utf-8")
                assert result.returncode != 0, (filename, command, result.stdout)
                assert f"{filename}:{line}" in result.stderr, result.stderr
                assert "Rust backend" not in result.stderr and "Cargo" not in result.stderr, result.stderr
                assert not (folder / "build" / path.stem / "src" / "main.rs").exists()
        environment["PATH"] = str(root) + os.pathsep + environment["PATH"]
        # Windows resolves an executable using the parent's PATH. Update this
        # verification process too, so the bare command tests PATH on every OS.
        previous_path = os.environ["PATH"]
        os.environ["PATH"] = environment["PATH"]
        try:
            result = subprocess.run(["nagic", "run", "--project", str(project / "nagi.toml")],
                                    cwd=folder, env=environment, check=True, capture_output=True, text=True, encoding="utf-8")
            assert result.stdout.splitlines()[-3:] == ["Hello, Nagi!", "4", marker], result.stdout
            assert not (root / "native-target").exists(), "Build wrote into the installed distribution"
            result = subprocess.run(["nagic", "run", str(records / "records.nagi")],
                                    cwd=folder, env=environment, check=True, capture_output=True, text=True, encoding="utf-8")
            assert result.stdout.splitlines()[-6:] == ["42", '{"value":42}', '{"value":42}', "Nagi", '"Nagi"', "temporary"], result.stdout
            assert not (root / "native-target").exists(), "Record build wrote into the installed distribution"
            print(f"Verified {version} {platform}: PATH, external projects, version, help, function aliases, shared/borrowed JSON, early Nagi diagnostics")
        finally:
            os.environ["PATH"] = previous_path
        verify_local_dependency(exe, folder, environment)


def verify_local_dependency(exe: Path, folder: Path, environment: dict) -> None:
    library = folder / "shared rules 凪"
    (library / "src").mkdir(parents=True)
    (library / "Cargo.toml").write_text('''[package]
name = "release-rules"
version = "0.1.0"
edition = "2021"
[workspace]
[features]
default = ["fee"]
fee = []
bonus = []
''', encoding="utf-8")
    (library / "src/lib.rs").write_text('''pub fn answer() -> i64 {
    let mut answer = 40;
    #[cfg(feature = "bonus")]
    { answer += 2; }
    #[cfg(feature = "fee")]
    { answer -= 5; }
    answer
}
''', encoding="utf-8")
    project = folder / "local dependency project 凪"
    project.mkdir()
    manifest = project / "nagi.toml"
    manifest.write_text('''entry = "main.nagi"
[rust]
file = "native.rs"
[rust.dependencies]
rules = { version = "0.1", path = "../shared rules 凪", package = "release-rules", features = ["bonus"], default-features = false }
''', encoding="utf-8")
    (project / "native.rs").write_text("pub fn answer() -> i64 { rules::answer() }\n", encoding="utf-8")
    (project / "main.nagi").write_text('''@rust("native::answer")
extern def answer() -> i64
def main():
    print(answer())
''', encoding="utf-8")
    # Project inspection must not need Cargo, including a new path dependency.
    subprocess.run([str(exe), "check", "--project", str(manifest)], cwd=folder,
                   env={**environment, "PATH": ""}, check=True, capture_output=True,
                   text=True, encoding="utf-8")
    output = folder / "relocated output 凪"
    result = subprocess.run([str(exe), "run", "--project", str(manifest), "--out", str(output)],
                            cwd=folder, env=environment, check=True, capture_output=True,
                            text=True, encoding="utf-8")
    assert result.stdout.splitlines()[-1:] == ["42"], result.stdout
    dependency = tomllib.loads((output / "Cargo.toml").read_text(encoding="utf-8"))["dependencies"]["rules"]
    assert Path(dependency["path"]).samefile(library), dependency
    assert dependency["package"] == "release-rules" and dependency["version"] == "0.1", dependency
    assert dependency["features"] == ["bonus"] and dependency["default-features"] is False, dependency
    print("Verified local Rust dependency: manifest-relative path, package alias, features, default-features, separate cwd/output")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--target", type=Path, help="Reuse native dependencies in this build directory")
    args = parser.parse_args()
    verify(args.archive, args.version, args.platform, args.target)
