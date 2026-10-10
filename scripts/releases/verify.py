"""Verify a distribution by using PATH from an unrelated project directory."""
import argparse
import hashlib
import json
import os
import re
import subprocess
import tarfile
import tempfile
import tomllib
from collections import Counter
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
            "rows.nagi": ('import std.db.sqlite as sqlite\nasync def rows(tx: view[sqlite.Tx]) -> Result[List[i64], sqlite.Failure]:\n    return await sqlite.all[i64](tx, sqlite.literal("SELECT 1"), sqlite.parameters())\n', 3),
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
                if filename == "rows.nagi":
                    assert "SQLite行型" in result.stderr, result.stderr
                assert "Rust backend" not in result.stderr and "Cargo" not in result.stderr, result.stderr
                assert not (folder / "build" / path.stem / "src" / "main.rs").exists()
        verify_sql(exe, folder, environment)
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
        verify_actor(exe, folder, environment)
        verify_task_handles(exe, folder, environment)
        verify_sqlite_pool(exe, folder, environment)


def verify_sqlite_pool(exe: Path, folder: Path, environment: dict) -> None:
    project = folder / "SQLite pool distribution 凪"
    project.mkdir()
    high = project / "sqlite_pool.nagi"
    tutorial = Path(__file__).resolve().parents[2] / "examples/sqlite_pool.nagi"
    high.write_bytes(tutorial.read_bytes())
    isolated = {**environment, "PATH": "", "NAGI_ROOT": str(folder / "missing runtime")}
    native_environment = dict(environment)
    native_environment.pop("NAGI_ROOT", None)
    lowered = project / "lowered"
    subprocess.run([str(exe), "lower", str(high), "--no-project", "--out", str(lowered)],
                   cwd=folder, env=isolated, check=True, capture_output=True, text=True,
                   encoding="utf-8", timeout=15)
    saved = project / "saved.low"
    saved.write_bytes((lowered / "generated.low").read_bytes())
    for form, source in (("high", high), ("saved-low", saved)):
        if form == "saved-low":
            high.unlink()
        output = project / form
        subprocess.run([str(exe), "check", str(source), "--no-project", "--out", str(output)],
                       cwd=folder, env=isolated, check=True, capture_output=True, text=True,
                       encoding="utf-8", timeout=15)
        completed = subprocess.run([str(exe), "run", str(source), "--no-project", "--out", str(output)],
                                   cwd=folder, env=native_environment, check=True, capture_output=True,
                                   text=True, encoding="utf-8", timeout=600)
        manifest = tomllib.loads((output / "Cargo.toml").read_text(encoding="utf-8"))
        runtime = (output / manifest["dependencies"]["nagi-runtime"]["path"]).resolve()
        assert runtime == (exe.parent / "runtime").resolve(), f"SQLite runtime outside distribution: {runtime}"
        assert completed.stdout.splitlines() == ["7", "closed"], f"SQLite native output ({form}): {completed.stdout}"
    print("Verified SQLite Pool/Tx: extracted runtime, High and independent saved Low, typed parameters, commit/rollback/close")


def verify_task_handles(exe: Path, folder: Path, environment: dict) -> None:
    project = folder / "Task handle project 凪"
    project.mkdir()
    isolated = {**environment, "PATH": "", "NAGI_ROOT": str(folder / "missing runtime")}
    native_environment = dict(environment)
    native_environment.pop("NAGI_ROOT", None)
    negative_sources = {
        "unreceived.nagi": ('''async def work() -> i64:
    return 7
async def main() -> Result[unit, Error]:
    async with scope:
        task = spawn work()  # primary
    return ok(print(0))
''', 5, "未受取Task"),
        "unreceived.low": ('''async fn work() -> i64 { return 7; }
async fn main() -> Result[unit, Error] {
    scope {
        let task = spawn work(); # primary
    }
    return ok(print(0));
}
''', 4, "未受取Task"),
        "double-await.nagi": ('''async def work() -> i64:
    return 7
async def main() -> Result[unit, Error]:
    async with scope:
        task = spawn work()
        first = await task
        second = await task  # primary
    return ok(print(0))
''', 7, "move後"),
        "double-await.low": ('''async fn work() -> i64 { return 7; }
async fn main() -> Result[unit, Error] {
    scope {
        let task = spawn work();
        let first = await task;
        let second = await task; # primary
    }
    return ok(print(0));
}
''', 6, "move後"),
        "discard-await.nagi": ('''import std.task as tasks
async def work() -> i64:
    return 7
async def main() -> Result[unit, Error]:
    async with scope:
        task = spawn work()
        tasks.discard(task)
        received = await task  # primary
    return ok(print(0))
''', 8, "move後"),
        "discard-await.low": ('''import std.task as tasks;
async fn work() -> i64 { return 7; }
async fn main() -> Result[unit, Error] {
    scope {
        let task = spawn work();
        tasks.discard(task);
        let received = await task; # primary
    }
    return ok(print(0));
}
''', 7, "move後"),
    }
    # Both CLI paths must reject at the checker, before Cargo or a runtime can
    # be found. Parser rejection, a backend failure, or a different primary
    # source line cannot stand in for ownership evidence.
    for filename, (source, line, detail) in negative_sources.items():
        path = project / filename
        path.write_text(source, encoding="utf-8")
        for action in ("check", "build"):
            output = project / f"negative-{path.stem}-{path.suffix[1:]}-{action}"
            result = subprocess.run([str(exe), action, str(path), "--no-project", "--out", str(output)],
                                    cwd=folder, env=isolated, capture_output=True, text=True,
                                    encoding="utf-8", timeout=15)
            assert result.returncode != 0, f"Task {filename} {action} was accepted"
            assert detail in result.stderr, result.stderr
            location = re.search(re.escape(filename) + r":(\d+)(?::|\b)", result.stderr)
            assert location and int(location.group(1)) == line, f"Task primary line: {result.stderr}"
            assert not any(phase in result.stderr.lower() for phase in
                           ("cargo", "rust backend", "parse error", "compiler defect", "internal compiler")), result.stderr
            assert not (output / "src/main.rs").exists(), "Task rejection generated native Rust"
            assert not (output / "Cargo.toml").exists(), "Task rejection reached native generation"

    high = project / "main.nagi"
    high.write_text('''import std.task as tasks
async def answer() -> i64:
    return 42
async def business() -> Result[i64, Error]:
    return error("business sentinel")
async def discarded() -> i64:
    print("discarded child")
    return 9
async def empty() -> unit:
    return print("unit child")
async def main() -> Result[unit, Error]:
    async with scope:
        task = spawn answer()
        first = await task
        match first:
            case Ok(value):
                print(value)
            case Err(problem):
                print("unexpected answer failure")
        task_result = spawn business()
        nested = await task_result
        match nested:
            case Ok(inner):
                match inner:
                    case Ok(value):
                        print("unexpected business success")
                    case Err(problem):
                        print(error_message(problem))
            case Err(problem):
                print("unexpected outer failure")
        ignored = spawn discarded()
        tasks.discard(ignored)
        unit_task = spawn empty()
        received = await unit_task
        match received:
            case Ok(done):
                print("received unit")
            case Err(problem):
                print("unexpected unit failure")
    print("after scope")
    return ok(print("completed"))
''', encoding="utf-8")
    handwritten = project / "handwritten.low"
    handwritten.write_text('''import std.task as tasks;
async fn answer() -> i64 { return 42; }
async fn business() -> Result[i64, Error] { return error("business sentinel"); }
async fn discarded() -> i64 { print("discarded child"); return 9; }
async fn empty() -> unit { return print("unit child"); }
async fn main() -> Result[unit, Error] {
    scope {
        let task = spawn answer();
        let first = await task;
        match first {
            case Ok(value) { print(value); }
            case Err(problem) { print("unexpected answer failure"); }
        }
        let task_result = spawn business();
        let nested = await task_result;
        match nested {
            case Ok(inner) { match inner {
                case Ok(value) { print("unexpected business success"); }
                case Err(problem) { print(error_message(problem)); }
            } }
            case Err(problem) { print("unexpected outer failure"); }
        }
        let ignored = spawn discarded();
        tasks.discard(ignored);
        let unit_task = spawn empty();
        let received = await unit_task;
        match received {
            case Ok(done) { print("received unit"); }
            case Err(problem) { print("unexpected unit failure"); }
        }
    }
    print("after scope");
    return ok(print("completed"));
}
''', encoding="utf-8")
    lowered = project / "lowered"
    subprocess.run([str(exe), "lower", str(high), "--no-project", "--out", str(lowered)],
                   cwd=folder, env=isolated, check=True, capture_output=True, text=True,
                   encoding="utf-8", timeout=15)
    saved = project / "saved.low"
    saved.write_bytes((lowered / "generated.low").read_bytes())
    expected = Counter(("42", "business sentinel", "discarded child", "unit child",
                        "received unit", "after scope", "completed"))
    for mode, source in (("high", high), ("saved-low", saved), ("handwritten-low", handwritten)):
        if mode == "saved-low":
            # Keep only the saved Low text, without its High file/source map.
            high.unlink()
        output = project / mode
        command = [str(exe), "check", str(source), "--no-project", "--out", str(output)]
        subprocess.run(command, cwd=folder, env=isolated, check=True, capture_output=True,
                       text=True, encoding="utf-8", timeout=15)
        result = subprocess.run([str(exe), "run", str(source), "--no-project", "--out", str(output)],
                                cwd=folder, env=native_environment, check=True, capture_output=True,
                                text=True, encoding="utf-8", timeout=600)
        manifest = tomllib.loads((output / "Cargo.toml").read_text(encoding="utf-8"))
        runtime = (output / manifest["dependencies"]["nagi-runtime"]["path"]).resolve()
        # Development compilers can fall back to their original checkout.
        # Successful execution alone therefore does not prove the shipped
        # runtime was used; the generated dependency must point beside this exe.
        assert runtime == (exe.parent / "runtime").resolve(), f"Task runtime came from outside the distribution: {runtime}"
        lines = result.stdout.splitlines()
        assert Counter(lines) == expected, f"Task native output ({mode}): {result.stdout}"
        assert lines[-2:] == ["after scope", "completed"], f"Task scope completion ({mode}): {result.stdout}"
    print("Verified Task handles: extracted compiler/runtime, High/saved Low/handwritten Low, receive/discard, nested business Err, early ownership diagnostics")


def verify_sql(exe: Path, folder: Path, environment: dict) -> None:
    project = folder / "SQL check project 凪"
    project.mkdir()
    schema = project / "schema.sql"
    schema.write_text("CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n", encoding="utf-8")
    isolated = {**environment, "PATH": "", "NAGI_ROOT": str(folder / "missing runtime")}
    cases = {
        "good": ("SELECT name, id FROM users WHERE id = ?", None),
        "column": ("SELECT id, naem AS name FROM users WHERE id = ?", "naem"),
        "bind": ("SELECT id, name FROM users WHERE id = ? OR id = ?", "bind"),
    }
    for name, (sql, expected) in cases.items():
        source = project / f"{name}.nagi"
        source.write_text(
            "import std.db.sqlite as sqlite\nclass User:\n    id: i64\n    name: str\n"
            "async def find(tx: view[sqlite.Tx], id: i64) -> Result[User?, sqlite.Failure]:\n"
            f"    return await sqlite.query[User](tx, sqlite.literal({json.dumps(sql)}), sqlite.bind_i64(sqlite.parameters(), id))\n", encoding="utf-8")
        command = [str(exe), "check", str(source), "--no-project", "--out", str(project / name)]
        ordinary = subprocess.run(command, cwd=folder, env=isolated, capture_output=True,
                                  text=True, encoding="utf-8", timeout=15)
        assert ordinary.returncode == 0, ordinary.stderr
        result = subprocess.run(command + ["--sql-schema", str(schema.relative_to(folder)),
                                           "--sql-dialect", "sqlite"],
                                cwd=folder, env=isolated, capture_output=True,
                                text=True, encoding="utf-8", timeout=15)
        if expected is None:
            assert result.returncode == 0, result.stderr
            assert "SQL checked 1 literal queries" in result.stderr, result.stderr
        else:
            assert result.returncode != 0, f"SQL {name} error was accepted: {result.stderr}"
            assert f"{source.name}:6" in result.stderr and expected in result.stderr, result.stderr
        assert "Cargo" not in result.stderr and "Rust backend" not in result.stderr, result.stderr
        assert not (project / name / "src/main.rs").exists(), "SQL check generated native Rust"
    print("Verified SQL engine: extracted compiler, offline schema, bad columns/binds, Nagi locations, no Cargo/runtime")


def verify_actor(exe: Path, folder: Path, environment: dict) -> None:
    source = folder / "supervised distribution.nagi"
    source.write_text('''import std.actor as actor
class Context:
    value: i64
async def child(context: shared[Context]) -> Result[unit, Error]:
    return ok(print(context.value))
async def main() -> Result[unit, Error]:
    group = actor.supervisor[Context](Context(value=42), actor.default_options())
    try actor.task(view(group), "print", child, actor.RestartPolicy.TEMPORARY)
    return await actor.run(group)
''', encoding="utf-8")
    result = subprocess.run([str(exe), "run", str(source)], cwd=folder,
                            env=environment, check=True, capture_output=True,
                            text=True, encoding="utf-8")
    assert result.stdout.splitlines()[-1:] == ["42"], result.stdout
    print("Verified standard actor library: extracted runtime, typed context, task, tracked cleanup")


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
