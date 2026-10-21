"""Tests for the standalone user-project GitHub Actions generator."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
GENERATOR = ROOT / "scripts/generate_project_ci.py"


class ProjectCiGeneratorTests(unittest.TestCase):
    def run_generator(self, *args):
        return subprocess.run(
            [sys.executable, str(GENERATOR), *args],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_default_workflow_has_native_free_check_and_release_build(self):
        result = self.run_generator()

        self.assertEqual(result.returncode, 0, result.stderr)
        workflow = result.stdout
        self.assertIn("name: Nagi project CI", workflow)
        self.assertIn("  pull_request:\n", workflow)
        self.assertIn("branches: ['main']", workflow)
        self.assertIn("cancel-in-progress: true", workflow)
        self.assertNotIn("pull_request_target", workflow)
        self.assertIn("permissions:\n  contents: read", workflow)
        self.assertIn("needs: check", workflow)
        check_job, build_job = workflow.split("\n  build:\n", 1)
        self.assertIn("nagic check", check_job)
        self.assertNotIn("dtolnay/rust-toolchain", check_job)
        self.assertNotIn("actions/cache", check_job)
        self.assertIn("nagic build\n", build_job)
        self.assertNotIn("--profile", build_job)
        self.assertNotIn("--build-stats", build_job)
        self.assertIn("NAGI_NATIVE_TARGET_DIR: ${{ github.workspace }}/native-target", build_job)
        self.assertIn("persist-credentials: false", workflow)
        self.assertIn("nagi-v0.2.0/scripts/install.sh", workflow)
        self.assertIn("--version 0.2.0 --no-path", workflow)
        self.assertIn("actions/checkout@11d5960a326750d5838078e36cf38b85af677262", workflow)
        self.assertIn("actions/cache/restore@0057852bfaa89a56745cba8c7296529d2fc39830", build_job)
        self.assertIn("actions/cache/save@0057852bfaa89a56745cba8c7296529d2fc39830", build_job)
        self.assertIn("steps.rust.outputs.cachekey", build_job)
        self.assertIn("hashFiles('**/Cargo.lock'", build_job)
        self.assertIn("github.event.repository.default_branch", build_job)
        self.assertIn("steps.cargo-cache.outputs.cache-primary-key", build_job)
        self.assertNotIn("restore-keys:", build_job)
        self.assertEqual(build_job.count("            build/*/Cargo.lock\n"), 2)
        self.assertIn("key: nagi-project-v1-0.2.0-", build_job)
        self.assertNotIn("            build/\n", build_job)
        self.assertIn("caches can be read by fork PRs", workflow)
        self.assertNotIn("secrets.", workflow)
        self.assertNotIn("uv", workflow)
        self.assertNotIn("sccache", workflow)
        self.assertIn("GITHUB_STEP_SUMMARY", workflow)

    def test_toolchain_and_opt_in_os_matrix(self):
        result = self.run_generator(
            "--toolchain", "1.85.1-x86_64-unknown-linux-gnu", "--os-matrix"
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("matrix:\n        os: [ubuntu-latest, macos-latest, windows-latest]", result.stdout)
        self.assertIn("dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8", result.stdout)
        self.assertIn("toolchain: '1.85.1-x86_64-unknown-linux-gnu'", result.stdout)
        self.assertIn("nagic build\n", result.stdout)
        self.assertIn("scripts/install.ps1", result.stdout)
        self.assertIn("if: runner.os == 'Windows'", result.stdout)

    def test_version_must_be_a_complete_safe_semver(self):
        for value in ("", "1.2", "1.2.3-rc1", "v1.2.3", "1.2.3\n", "1.2.3;echo bad", "${{ secrets.X }}"):
            with self.subTest(value=value):
                result = self.run_generator("--nagi-version", value)
                self.assertEqual(result.returncode, 2)
                self.assertIn("full X.Y.Z", result.stderr)
                self.assertEqual(result.stdout, "")

    def test_invalid_toolchain_is_rejected(self):
        result = self.run_generator("--toolchain", "stable\nrun: injected")

        self.assertEqual(result.returncode, 2)
        self.assertIn("Rust toolchain", result.stderr)
        self.assertEqual(result.stdout, "")

    def test_output_path_creates_workflow_and_refuses_overwrite(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / ".github/workflows/nagi.yml"
            first = self.run_generator("--output", str(output))
            self.assertEqual(first.returncode, 0, first.stderr)
            original = output.read_text(encoding="utf-8")
            self.assertEqual(first.stdout, "")

            second = self.run_generator("--output", str(output), "--toolchain", "nightly")
            self.assertEqual(second.returncode, 2)
            self.assertIn("refusing to overwrite", second.stderr)
            self.assertEqual(output.read_text(encoding="utf-8"), original)

            forced = self.run_generator(
                "--output", str(output), "--toolchain", "nightly", "--force"
            )
            self.assertEqual(forced.returncode, 0, forced.stderr)
            self.assertIn("toolchain: 'nightly'", output.read_text(encoding="utf-8"))
            self.assertEqual(list(output.parent.glob("*.tmp")), [])

    def test_force_requires_output_file(self):
        result = self.run_generator("--force")

        self.assertEqual(result.returncode, 2)
        self.assertIn("--force requires --output", result.stderr)

    def test_default_branch_avoids_duplicate_push_and_pr_builds(self):
        result = self.run_generator("--default-branch", "master")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("branches: ['master']", result.stdout)
        for branch in ("main'", "main\nrun: injected", "${{ secrets.X }}", "main*"):
            self.assertEqual(self.run_generator("--default-branch", branch).returncode, 2)


if __name__ == "__main__":
    unittest.main()
