"""Exercise complete Git ranges and the required-check result contract."""
import copy
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import changes
import gate


class ChangeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.run_git("init", "-q", "-b", "main")
        self.write("compiler/src/lib.rs", "// original compiler\n")
        self.write("docs/start.md", "# Start\n")
        self.first = self.commit()
        self.root_patch = patch.object(changes, "ROOT", self.root)
        self.root_patch.start()

    def tearDown(self):
        self.root_patch.stop()
        self.temporary.cleanup()

    def run_git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root, stderr=subprocess.PIPE).decode().strip()

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def commit(self):
        self.run_git("add", "-A")
        self.run_git("-c", "user.name=CI Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "test")
        return self.run_git("rev-parse", "HEAD")

    def full(self, head=None, base=None, event="pull_request"):
        return changes.classify(event, base or self.first, head or self.run_git("rev-parse", "HEAD"))["full_checks"]

    def test_docs_and_site_files_skip_full_checks(self):
        for path in ("README.md", "README.en.md", "CONTRIBUTING.md", "CONTRIBUTING.en.md",
                     "SECURITY.md", "SECURITY.en.md", "PERFORMANCE.md", "CHANGELOG.md",
                     "docs/http.md", "docs/en/http.md", "docs/guide/setup.md",
                     "website/README.md", "website/build.py", "website/requirements.txt",
                     "website/assets/site.js", "website/assets/site.css", "website/assets/plot.svg",
                     "website/assets/img/photo.png", "website/templates/page.html",
                     "editors/vscode-nagi/README.md", "test-nagi-code/web-demo/README.md"):
            with self.subTest(path=path):
                base = self.run_git("rev-parse", "HEAD")
                self.write(path, "presentation update\n")
                self.assertEqual(self.full(self.commit(), base), "false")

    def test_code_config_dependencies_and_unknown_paths_run_full_checks(self):
        for path in ("compiler/src/lib.rs", "runtime/src/lib.rs", "Cargo.toml", "Cargo.lock",
                     "compiler/README.md", "runtime/README.md", "LICENSE", ".gitignore",
                     ".github/workflows/ci.yml", ".github/workflows/pages.yml",
                     "scripts/install.sh", "scripts/install.ps1", "scripts/releases/plan.py",
                     "scripts/releases/README.md", "scripts/ci/changes.py", "tests/http_integration.py",
                     "tests/requirements.txt", "editors/vscode-nagi/src/features.js",
                     "editors/vscode-nagi/README.en.md", "test-nagi-code/cpu.nagi",
                     "website/examples/double.nagi", "website/assets/helper.rs",
                     "website/templates/helper.py", "website/new-config.toml", "new-feature/README.md"):
            with self.subTest(path=path):
                base = self.run_git("rev-parse", "HEAD")
                self.write(path, "changed\n")
                self.assertEqual(self.full(self.commit(), base), "true")

    def test_mixed_docs_and_code_run_full_checks(self):
        self.write("docs/start.md", "# New docs\n")
        self.write("runtime/src/lib.rs", "// changed runtime\n")
        self.assertEqual(self.full(self.commit()), "true")

    def test_earlier_code_commit_is_not_hidden_by_last_docs_commit(self):
        self.write("compiler/src/lib.rs", "// changed compiler\n")
        self.commit()
        self.write("docs/start.md", "# Later docs\n")
        self.assertEqual(self.full(self.commit()), "true")

    def test_complete_docs_push_can_contain_multiple_commits(self):
        self.write("docs/start.md", "# Earlier docs\n")
        self.commit()
        self.write("website/assets/site.js", "// Later site change\n")
        self.assertEqual(self.full(self.commit(), event="push"), "false")

    def test_docs_deletion_skips_full_checks(self):
        (self.root / "docs/start.md").unlink()
        self.assertEqual(self.full(self.commit()), "false")

    def test_code_deletion_runs_full_checks(self):
        (self.root / "compiler/src/lib.rs").unlink()
        self.assertEqual(self.full(self.commit()), "true")

    def test_docs_directory_rename_checks_old_and_new_paths(self):
        (self.root / "docs").rename(self.root / "moved")
        self.assertEqual(self.full(self.commit()), "true")

    def test_docs_rename_within_docs_skips_full_checks(self):
        self.run_git("mv", "docs/start.md", "docs/intro.md")
        self.assertEqual(self.full(self.commit()), "false")

    def test_identical_source_renamed_into_docs_still_runs_full_checks(self):
        self.run_git("mv", "compiler/src/lib.rs", "docs/compiler.md")
        self.assertEqual(self.full(self.commit()), "true")

    def test_identical_docs_renamed_into_source_still_runs_full_checks(self):
        self.run_git("mv", "docs/start.md", "compiler/src/doc.rs")
        self.assertEqual(self.full(self.commit()), "true")

    def test_pr_merge_commit_compares_against_current_base(self):
        self.run_git("checkout", "-qb", "docs-branch")
        self.write("docs/start.md", "# Feature docs\n")
        feature = self.commit()
        self.run_git("checkout", "-q", "main")
        self.write("runtime/src/lib.rs", "// base branch advanced\n")
        base = self.commit()
        self.run_git("-c", "user.name=CI Test", "-c", "user.email=test@example.invalid",
                     "merge", "--no-ff", "-qm", "PR merge", feature)
        self.assertEqual(self.full(base=base), "false")

    def test_empty_comparison_runs_full_checks(self):
        self.assertEqual(self.full(), "true")

    def test_new_branches_and_invalid_commit_ranges_run_full_checks(self):
        self.write("docs/start.md", "# Docs\n")
        head = self.commit()
        for base in ("", "0" * 40, "invalid", "-HEAD", "a" * 40, "../docs", "a" * 64):
            with self.subTest(base=base):
                self.assertEqual(changes.classify("push", base, head)["full_checks"], "true")
        for value in ("", "0" * 40, "invalid", "--help", "a" * 40):
            with self.subTest(head=value):
                self.assertEqual(changes.classify("push", self.first, value)["full_checks"], "true")

    def test_manual_scheduled_and_unknown_events_run_full_checks(self):
        self.write("docs/start.md", "# Docs\n")
        head = self.commit()
        for event in ("workflow_dispatch", "schedule", "pull_request_target", "unexpected"):
            with self.subTest(event=event):
                self.assertEqual(self.full(head, event=event), "true")

    def test_force_push_with_unrelated_history_runs_full_checks(self):
        self.run_git("checkout", "-q", "--orphan", "replacement")
        self.write("docs/start.md", "# New branch history\n")
        self.assertEqual(self.full(self.commit(), event="push"), "true")

    def test_nul_paths_preserve_newlines_and_unicode(self):
        self.write("docs/with\nnewline.md", "# Docs\n")
        self.write("docs/日本語.md", "# Docs\n")
        self.assertEqual(self.full(self.commit()), "false")
        self.write("compiler/src/with\nnewline.rs", "// Code\n")
        self.assertEqual(self.full(self.commit()), "true")

    @unittest.skipIf(os.name == "nt", "Windows file names require valid Unicode")
    def test_undecodable_git_path_runs_full_checks(self):
        with open(os.fsencode(self.root / "docs") + b"/invalid-\xff.md", "wb") as file:
            file.write(b"Docs\n")
        self.assertEqual(self.full(self.commit()), "true")

    def test_malformed_path_inputs_are_not_normalized_into_docs(self):
        for path in ("", "/docs/start.md", "docs//start.md", "docs/../compiler.rs",
                     "docs/./start.md", "docs\\start.md", "docs/start.md\0compiler.rs"):
            with self.subTest(path=path):
                self.assertFalse(changes.is_docs_path(path))

    def test_incomplete_git_output_runs_full_checks(self):
        head = "a" * 40
        with patch.object(changes, "git", side_effect=[self.first.encode(), head.encode(), b"", b"docs/start.md"]):
            self.assertEqual(self.full(head), "true")

    def test_git_failure_runs_full_checks(self):
        with patch.object(changes, "git", side_effect=OSError("git unavailable")):
            self.assertEqual(self.full("a" * 40), "true")


class GateTests(unittest.TestCase):
    def needs(self, full="true", package_nagi="false", package_vscode="false"):
        return {
            "changes": {"result": "success", "outputs": {"full_checks": full}},
            "linux": {"result": "success" if full == "true" else "skipped"},
            "release-plan": {"result": "success", "outputs": {"package_nagi": package_nagi, "package_vscode": package_vscode}},
            "nagi-package": {"result": "success" if package_nagi == "true" else "skipped"},
            "vscode-package": {"result": "success" if package_vscode == "true" else "skipped"},
        }

    def test_docs_only_and_full_checks_require_the_planned_results(self):
        for full in ("true", "false"):
            for nagi in ("true", "false"):
                for vscode in ("true", "false"):
                    with self.subTest(full=full, nagi=nagi, vscode=vscode):
                        self.assertEqual(gate.errors(self.needs(full, nagi, vscode)), [])

    def test_failed_or_canceled_detection_cannot_make_checks_optional(self):
        for result in ("failure", "cancelled", "skipped", ""):
            with self.subTest(result=result):
                needs = self.needs("false")
                needs["changes"]["result"] = result
                self.assertTrue(gate.errors(needs))

    def test_missing_or_malformed_check_plan_is_rejected(self):
        for value in (None, "", "False", "TRUE", True, False):
            with self.subTest(value=value):
                needs = self.needs("false")
                needs["changes"]["outputs"]["full_checks"] = value
                self.assertTrue(gate.errors(needs))

    def test_docs_skip_cannot_hide_failed_or_canceled_rust_job(self):
        for result in ("failure", "cancelled", "success", ""):
            with self.subTest(result=result):
                needs = self.needs("false")
                needs["linux"]["result"] = result
                self.assertTrue(gate.errors(needs))

    def test_required_rust_job_cannot_be_skipped(self):
        needs = self.needs()
        needs["linux"]["result"] = "skipped"
        self.assertTrue(gate.errors(needs))

    def test_release_plan_must_succeed(self):
        needs = self.needs("false")
        needs["release-plan"]["result"] = "failure"
        self.assertTrue(gate.errors(needs))

    def test_required_packages_cannot_fail_be_canceled_or_skipped(self):
        for job in ("nagi-package", "vscode-package"):
            for result in ("failure", "cancelled", "skipped"):
                with self.subTest(job=job, result=result):
                    needs = self.needs(package_nagi="true", package_vscode="true")
                    needs[job]["result"] = result
                    self.assertTrue(gate.errors(needs))

    def test_unplanned_packages_must_be_skipped(self):
        for job in ("nagi-package", "vscode-package"):
            needs = self.needs("false")
            needs[job]["result"] = "success"
            self.assertTrue(gate.errors(needs))

    def test_invalid_package_plans_are_rejected(self):
        for value in (None, "", "FALSE", True, False):
            for component in ("nagi", "vscode"):
                with self.subTest(value=value, component=component):
                    needs = self.needs("false")
                    needs["release-plan"]["outputs"][f"package_{component}"] = value
                    self.assertTrue(gate.errors(needs))

    def test_missing_jobs_or_outputs_are_rejected(self):
        valid = self.needs("false")
        for job in valid:
            with self.subTest(job=job):
                needs = copy.deepcopy(valid)
                del needs[job]
                self.assertTrue(gate.errors(needs))
        for job in ("changes", "release-plan"):
            with self.subTest(outputs=job):
                needs = copy.deepcopy(valid)
                del needs[job]["outputs"]
                self.assertTrue(gate.errors(needs))


if __name__ == "__main__":
    unittest.main()
