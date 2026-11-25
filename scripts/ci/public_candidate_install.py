"""Run actual shell install/reinstall/uninstall with only the download mocked."""
import json
from pathlib import Path
import subprocess
import sys

root, archive = (Path(value).resolve() for value in sys.argv[1:3])
sys.path.insert(0, str(root / "scripts/releases"))
from test_install import ShellInstallTests, VERSION
from test_uninstall import ShellUninstallTests

fixture = ShellInstallTests()
fixture.setUp()
try:
    fixture.environment["HOME"] = str(fixture.root)
    fixture.environment["NAGI_TEST_ASSETS"] = str(archive.parent)
    fixture.environment.pop("NAGI_ROOT", None)
    profile = fixture.root / "profile"
    profile.write_text("# candidate installer test\n")
    version = fixture.root / "versions" / fixture.stem
    link = fixture.root / "bin" / "nagic"
    for _ in range(2):
        installed = fixture.install("--version", VERSION)
        assert installed.returncode == 0, (installed.stdout, installed.stderr)
        actual = subprocess.check_output([link, "--version"], cwd=fixture.root, env=fixture.environment, text=True)
        assert actual.strip() == f"nagic {VERSION}", actual
    project = fixture.root / "external project"
    project.mkdir()
    source = project / "main.nagi"
    source.write_text("def main():\n    print(42)\n", encoding="utf-8")
    for name, flags in (("rust-default", []), ("go-opt-in", ["--backend", "go"])):
        result = subprocess.run([str(link), "run", str(source), "--no-project", "--out",
                                 str(project / name), *flags], cwd=project, env=fixture.environment,
                                capture_output=True, text=True, timeout=300)
        assert (result.returncode, result.stdout.splitlines()) == (0, ["42"]), result
        print(f"Verified installed CLI {name}: exit 0 and exact output")
    provenance = json.loads((version / "release.json").read_text())
    from public_candidate_validation import CANDIDATE
    assert provenance["commit"] == CANDIDATE, provenance
    sentinel = fixture.root / "bin" / "unrelated"
    sentinel.write_text("preserve me\n")
    removed = ShellUninstallTests.uninstall(fixture)
    assert removed.returncode == 0, (removed.stdout, removed.stderr)
    assert not link.is_symlink() and not version.exists()
    assert sentinel.read_text() == "preserve me\n" and "# candidate installer test" in profile.read_text()
    print(f"Verified native install, reinstall, version, and uninstall for {provenance['platform']}")
finally:
    fixture.tearDown()
