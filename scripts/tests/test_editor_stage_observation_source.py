"""Source/inverse mutation tests only; never a compiler/native result."""
import hashlib
import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("editor_source_guard", ROOT / "scripts/ci/editor_stage_source_guard.py")
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


def main():
    count = 0
    read = lambda name: (ROOT / name).read_bytes()
    GUARD.verify(read)
    count += 1
    for name in GUARD.ORIGINAL:
        restored = GUARD.inverse_source(name, read(name))
        assert hashlib.sha256(restored).hexdigest() == GUARD.ORIGINAL[name]
        count += 1
    for name in GUARD.BOUND:
        def mutant(path):
            return read(path) + (b"\n" if path == name else b"")
        try:
            GUARD.verify(mutant)
        except ValueError:
            count += 1
        else:
            raise AssertionError("bound mutant accepted")
    for name, fragments in GUARD.INVERSE.items():
        data = read(name)
        for added, old in fragments:
            for mutated in [data.replace(added.encode(), b"", 1), data + added.encode(),
                            data.replace(added.encode(), (added + "\n").encode(), 1)]:
                try:
                    GUARD.inverse_source(name, mutated)
                except ValueError:
                    count += 1
                else:
                    raise AssertionError("inverse mutant accepted")
    for name in ["editors/vscode-nagi/test/actor-navigation.test.js",
                 "editors/vscode-nagi/test/stdlib-navigation.test.js"]:
        now = read(name)
        old = GUARD.inverse_source(name, now)
        assert [line for line in now.splitlines() if b"assert." in line] == [line for line in old.splitlines() if b"assert." in line]
        assert [line for line in now.splitlines() if b"test(" in line] == [line for line in old.splitlines() if b"test(" in line]
        count += 1
    lib = read("compiler/src/lib.rs")
    assert b"#[cfg(nagi_editor_stage_diag)]\nmod editor_stage_diag;" in lib
    assert b"pub mod editor_stage_diag" not in lib
    assert read("compiler/Cargo.toml").count(b"cfg(nagi_editor_stage_diag)") == 1
    count += 1
    # The original extension wrapper and existing mandatory CI stay exact.
    for name in ["editors/vscode-nagi/src/compiler.js", ".github/workflows/ci.yml"]:
        assert GUARD.inverse_source(name, read(name)) == read(name)
        count += 1
    print('{"source_fake_pass":%d,"native_cases_executed":0}' % count)


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print('{"source_fake_failed":true,"native_cases_executed":0}')
        raise SystemExit(1)
