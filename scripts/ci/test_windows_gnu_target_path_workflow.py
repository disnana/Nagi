import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class WindowsGnuTargetPathWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.workflow = (ROOT / ".github/workflows/compiler-contracts.yml").read_text(encoding="utf-8")
        cls.probe = (ROOT / "scripts/ci/windows_gnu_target_path.ps1").read_text(encoding="utf-8")

    def test_probe_is_an_explicit_optional_profile(self):
        self.assertIn("options: [compiler-contracts, windows-gnu-target-path]", self.workflow)
        self.assertIn("default: compiler-contracts", self.workflow)
        self.assertIn(
            "if: github.event_name == 'workflow_dispatch' && inputs.profile == 'windows-gnu-target-path'",
            self.workflow,
        )
        self.assertIn(
            "if: github.event_name != 'workflow_dispatch' || inputs.profile == 'compiler-contracts'",
            self.workflow,
        )
        self.assertIn("mingw-w64-ucrt-x86_64-gcc", self.workflow)
        self.assertIn('C:\\msys64\\usr\\bin;$env:PATH', self.workflow)
        self.assertIn("Reporter used GCC 15.2.0; hosted UCRT64 provides", self.workflow)
        self.assertIn("GCC 15.2.0 remains unverified", self.workflow)
        self.assertIn("Tee-Object -FilePath $setupLog -Append", self.workflow)
        self.assertNotIn("throw \"Expected the reporter's GCC 15.2.0", self.workflow)
        self.assertIn("build/ci-evidence/windows-gnu-target-path", self.workflow)
        self.assertIn("GITHUB_STEP_SUMMARY", self.workflow)

    def test_probe_compares_a_pinned_release_without_overriding_the_target(self):
        self.assertIn(
            "eed9424998dcbca1bc148329626adb906e7f6b7ab5e4b85a227f33f6f5d76bf9",
            self.probe,
        )
        self.assertIn("Remove-Item Env:NAGI_NATIVE_TARGET_DIR", self.probe)
        self.assertIn("Remove-Item Env:CARGO_TARGET_DIR", self.probe)
        self.assertIn("baseline-explicit", self.probe)
        self.assertIn("baseline-project", self.probe)
        self.assertIn("current-project", self.probe)
        self.assertIn("native_targets_fresh_per_case = $true", self.probe)
        self.assertIn("Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction Stop", self.probe)
        self.assertIn("Could not remove the previous comparison path", self.probe)

    def test_non_reproduction_is_reported_but_current_failure_is_rejected(self):
        self.assertIn("Issue #201 did not reproduce on this hosted runner", self.probe)
        self.assertIn("The corrected project-mode compiler failed under Windows GNU/UCRT64", self.probe)
        self.assertIn("The explicit-source baseline control failed", self.probe)


if __name__ == "__main__":
    unittest.main()
