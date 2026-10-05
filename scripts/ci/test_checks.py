"""Exercise complete Git ranges and the required-check result contract."""
import copy
import fnmatch
import os
import re
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

    def jetbrains(self, head=None, base=None, event="pull_request"):
        return changes.classify(event, base or self.first, head or self.run_git("rev-parse", "HEAD"))["jetbrains_checks"]

    def test_docs_and_site_files_skip_full_checks(self):
        for path in ("README.md", "README.en.md", "CONTRIBUTING.md", "CONTRIBUTING.en.md",
                     "SECURITY.md", "SECURITY.en.md", "PERFORMANCE.md", "CHANGELOG.md",
                     "DESIGN.md", "DESIGN.en.md", "AGENTS.md", "ai/README.md", "ai/language.md",
                     "ai/skills/nagi-development/SKILL.md",
                     "docs/http.md", "docs/en/http.md", "docs/guide/setup.md",
                     "website/README.md", "website/build.py", "website/requirements.txt",
                     "website/CNAME", ".github/workflows/pages.yml",
                     "website/assets/site.js", "website/assets/site.css", "website/assets/plot.svg",
                     "website/assets/img/photo.png", "website/templates/page.html",
                     "editors/vscode-nagi/README.md", "test-nagi-code/web-demo/README.md",
                     "test-nagi-code/application-examples/stock-report/README.en.md"):
            with self.subTest(path=path):
                base = self.run_git("rev-parse", "HEAD")
                self.write(path, "presentation update\n")
                self.assertEqual(self.full(self.commit(), base), "false")

    def test_code_config_dependencies_and_unknown_paths_run_full_checks(self):
        for path in ("compiler/src/lib.rs", "runtime/src/lib.rs", "Cargo.toml", "Cargo.lock",
                     "compiler/README.md", "runtime/README.md", "LICENSE", ".gitignore",
                     ".github/workflows/ci.yml", ".github/workflows/unknown.yml",
                     "scripts/install.sh", "scripts/install.ps1", "scripts/uninstall.sh",
                     "scripts/uninstall.ps1", "scripts/releases/plan.py",
                     "scripts/releases/README.md", "scripts/ci/changes.py", "tests/http_integration.py",
                     "tests/requirements.txt", "editors/vscode-nagi/src/features.js",
                     "editors/vscode-nagi/README.en.md", "test-nagi-code/cpu.nagi",
                     "ai/assistant.py", "ai/README.txt",
                     "test-nagi-code/application-examples/stock-report/main.nagi",
                     "test-nagi-code/application-examples/stock-report/smoke.py",
                     "test-nagi-code/application-examples/stock-report/nagi.toml",
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

    def test_jetbrains_plugin_only_ranges_use_the_independent_checks(self):
        for path in ("editors/jetbrains-nagi/src/main/java/com/disnana/nagi/NagiLexer.java",
                     "editors/jetbrains-nagi/src/test/java/com/disnana/nagi/NagiEditorTest.java",
                     "editors/jetbrains-nagi/src/main/resources/META-INF/plugin.xml",
                     "editors/jetbrains-nagi/build.gradle.kts", "editors/jetbrains-nagi/gradle.properties",
                     "editors/jetbrains-nagi/gradle/wrapper/gradle-wrapper.jar",
                     "editors/jetbrains-nagi/gradle/wrapper/gradle-wrapper.properties",
                     "editors/jetbrains-nagi/gradlew", "editors/jetbrains-nagi/gradlew.bat",
                     "editors/jetbrains-nagi/README.md", "editors/jetbrains-nagi/README.en.md",
                     ".github/workflows/jetbrains.yml"):
            with self.subTest(path=path):
                base = self.run_git("rev-parse", "HEAD")
                self.write(path, "plugin update\n")
                head = self.commit()
                for event in ("push", "pull_request"):
                    self.assertEqual(self.full(head, base, event), "false")
                    self.assertEqual(self.jetbrains(head, base, event), "true")

    def test_jetbrains_and_docs_can_share_a_complete_range(self):
        self.write("editors/jetbrains-nagi/build.gradle.kts", "// plugin build change\n")
        self.commit()
        self.write("README.md", "# Plugin setup\n")
        self.write("docs/editors.md", "# Editor support\n")
        head = self.commit()
        for event in ("push", "pull_request"):
            self.assertEqual(self.full(head, event=event), "false")
            self.assertEqual(self.jetbrains(head, event=event), "true")

    def test_jetbrains_workflow_and_compiler_check_inputs_require_both_checks(self):
        for path in (".github/workflows/ci.yml", "scripts/ci/changes.py", "scripts/ci/gate.py",
                     "scripts/ci/test_checks.py", "Cargo.toml", "Cargo.lock",
                     "compiler/src/lib.rs", "runtime/src/lib.rs"):
            with self.subTest(path=path):
                base = self.run_git("rev-parse", "HEAD")
                self.write(path, "CI control-plane change\n")
                head = self.commit()
                for event in ("push", "pull_request"):
                    self.assertEqual(self.full(head, base, event), "true")
                    self.assertEqual(self.jetbrains(head, base, event), "true")

    def test_docs_and_other_editors_skip_jetbrains(self):
        for path in ("README.md", "docs/start.md", "website/assets/site.js",
                     "editors/vscode-nagi/src/features.js", "editors/jetbrains-nagi-old/build.gradle.kts"):
            with self.subTest(path=path):
                base = self.run_git("rev-parse", "HEAD")
                self.write(path, "unrelated update\n")
                head = self.commit()
                for event in ("push", "pull_request"):
                    self.assertEqual(self.jetbrains(head, base, event), "false")

    def test_plugin_and_core_changes_require_both_checks(self):
        self.write("editors/jetbrains-nagi/build.gradle.kts", "// plugin update\n")
        self.commit()
        self.write("runtime/src/lib.rs", "// runtime update\n")
        head = self.commit()
        self.assertEqual(self.full(head), "true")
        self.assertEqual(self.jetbrains(head), "true")

    def test_jetbrains_cannot_hide_code_shared_detection_or_other_editors(self):
        for path in ("compiler/src/lib.rs", "runtime/src/lib.rs", "Cargo.toml", "Cargo.lock",
                     ".github/workflows/ci.yml", "scripts/ci/changes.py", "scripts/ci/test_checks.py",
                     "editors/vscode-nagi/src/features.js", "editors/jetbrains-other/build.gradle.kts",
                     "editors/jetbrains-nagi-old/build.gradle.kts", ".github/workflows/jetbrains.yaml"):
            with self.subTest(path=path):
                base = self.run_git("rev-parse", "HEAD")
                self.write("editors/jetbrains-nagi/build.gradle.kts", f"// plugin update alongside {path}\n")
                self.write(path, "changed check input\n")
                self.assertEqual(self.full(self.commit(), base), "true")

    def test_jetbrains_deletion_is_checked_by_its_workflow(self):
        self.write("editors/jetbrains-nagi/src/main/java/com/disnana/nagi/NagiLexer.java", "// plugin code\n")
        base = self.commit()
        (self.root / "editors/jetbrains-nagi/src/main/java/com/disnana/nagi/NagiLexer.java").unlink()
        head = self.commit()
        self.assertEqual(self.full(head, base), "false")
        self.assertEqual(self.jetbrains(head, base), "true")

    def test_renames_across_the_jetbrains_boundary_run_full_checks(self):
        self.write("editors/jetbrains-nagi/src/main/java/com/disnana/nagi/NagiLexer.java", "// source code\n")
        base = self.commit()
        self.run_git("mv", "editors/jetbrains-nagi/src/main/java/com/disnana/nagi/NagiLexer.java", "compiler/src/editor.rs")
        self.assertEqual(self.full(self.commit(), base), "true")
        base = self.run_git("rev-parse", "HEAD")
        self.run_git("mv", "compiler/src/editor.rs", "editors/jetbrains-nagi/NagiLexer.java")
        self.assertEqual(self.full(self.commit(), base), "true")

    def test_an_earlier_rust_change_is_not_hidden_by_the_last_plugin_commit(self):
        self.write("runtime/src/lib.rs", "// runtime changed\n")
        self.commit()
        self.write("editors/jetbrains-nagi/build.gradle.kts", "// later plugin change\n")
        self.assertEqual(self.full(self.commit()), "true")

    def test_pages_and_domain_only_complete_ranges_skip_full_checks(self):
        self.write(".github/workflows/pages.yml", "# Pages presentation update\n")
        self.commit()
        self.write("website/CNAME", "docs.example.invalid\n")
        head = self.commit()
        for event in ("push", "pull_request"):
            with self.subTest(event=event):
                self.assertEqual(self.full(head, event=event), "false")

    def test_pages_and_domain_cannot_hide_check_workflows_or_detection_changes(self):
        for path in (".github/workflows/ci.yml", ".github/workflows/unknown.yml", "scripts/ci/changes.py"):
            with self.subTest(path=path):
                base = self.run_git("rev-parse", "HEAD")
                self.write(".github/workflows/pages.yml", f"# Pages update alongside {path}\n")
                self.write("website/CNAME", f"# Domain update alongside {path}\n")
                self.write(path, "# Changed code-check input\n")
                self.assertEqual(self.full(self.commit(), base), "true")

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
        self.assertEqual(self.jetbrains(), "true")

    def test_new_branches_and_invalid_commit_ranges_run_full_checks(self):
        self.write("docs/start.md", "# Docs\n")
        head = self.commit()
        for base in ("", "0" * 40, "invalid", "-HEAD", "a" * 40, "../docs", "a" * 64):
            with self.subTest(base=base):
                plan = changes.classify("push", base, head)
                self.assertEqual(plan["full_checks"], "true")
                self.assertEqual(plan["jetbrains_checks"], "true")
        for value in ("", "0" * 40, "invalid", "--help", "a" * 40):
            with self.subTest(head=value):
                plan = changes.classify("push", self.first, value)
                self.assertEqual(plan["full_checks"], "true")
                self.assertEqual(plan["jetbrains_checks"], "true")

    def test_manual_scheduled_and_unknown_events_run_full_checks(self):
        self.write("docs/start.md", "# Docs\n")
        head = self.commit()
        for event in ("workflow_dispatch", "schedule", "pull_request_target", "unexpected"):
            with self.subTest(event=event):
                self.assertEqual(self.full(head, event=event), "true")
                self.assertEqual(self.jetbrains(head, event=event), "true")

    def test_force_push_with_unrelated_history_runs_full_checks(self):
        self.run_git("checkout", "-q", "--orphan", "replacement")
        self.write("docs/start.md", "# New branch history\n")
        head = self.commit()
        self.assertEqual(self.full(head, event="push"), "true")
        self.assertEqual(self.jetbrains(head, event="push"), "true")

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
        head = self.commit()
        self.assertEqual(self.full(head), "true")
        self.assertEqual(self.jetbrains(head), "true")

    def test_malformed_path_inputs_are_not_normalized_into_docs(self):
        for path in ("", "/docs/start.md", "docs//start.md", "docs/../compiler.rs",
                     "docs/./start.md", "docs\\start.md", "docs/start.md\0compiler.rs"):
            with self.subTest(path=path):
                self.assertFalse(changes.is_docs_path(path))

    def test_malformed_paths_and_siblings_are_not_jetbrains_paths(self):
        for path in ("", "/editors/jetbrains-nagi/build.gradle.kts",
                     "editors//jetbrains-nagi/build.gradle.kts", "editors/jetbrains-nagi/../Cargo.toml",
                     "editors/jetbrains-nagi/./build.gradle.kts", "editors\\jetbrains-nagi\\build.gradle.kts",
                     "editors/jetbrains-nagi/build.gradle.kts\0compiler/src/lib.rs",
                     "editors/jetbrains-nagi", "editors/jetbrains-nagi-old/build.gradle.kts",
                     ".github/workflows/jetbrains.yaml", "scripts/ci/changes.py"):
            with self.subTest(path=path):
                self.assertFalse(changes.is_jetbrains_path(path))

    def test_malformed_compiler_inputs_do_not_require_jetbrains_checks(self):
        for path in ("compiler//src/lib.rs", "compiler/../Cargo.toml", "compiler\\src\\lib.rs",
                     "runtime/./src/lib.rs", "runtime/src/lib.rs\0Cargo.toml"):
            with self.subTest(path=path):
                self.assertFalse(changes.requires_jetbrains(path))

    def test_incomplete_git_output_runs_full_checks(self):
        head = "a" * 40
        with patch.object(changes, "git", side_effect=[self.first.encode(), head.encode(), b"", b"docs/start.md"]):
            plan = changes.classify("pull_request", self.first, head)
            self.assertEqual(plan["full_checks"], "true")
            self.assertEqual(plan["jetbrains_checks"], "true")

    def test_git_failure_runs_full_checks(self):
        with patch.object(changes, "git", side_effect=OSError("git unavailable")):
            plan = changes.classify("pull_request", self.first, "a" * 40)
            self.assertEqual(plan["full_checks"], "true")
            self.assertEqual(plan["jetbrains_checks"], "true")

    def test_malformed_git_paths_cannot_disable_either_check_plan(self):
        head = "a" * 40
        for path in ("", "docs//start.md", "editors/jetbrains-nagi/../Cargo.toml"):
            with self.subTest(path=path):
                raw = path.encode() + b"\0"
                with patch.object(changes, "git", side_effect=[self.first.encode(), head.encode(), b"", raw]):
                    plan = changes.classify("pull_request", self.first, head)
                self.assertEqual(plan["full_checks"], "true")
                self.assertEqual(plan["jetbrains_checks"], "true")

    def test_cli_exports_both_check_plans_without_overwriting_other_outputs(self):
        self.write("editors/jetbrains-nagi/build.gradle.kts", "// plugin update\n")
        head = self.commit()
        output = self.root / "github-output"
        output.write_text("existing=value\n", encoding="utf-8")
        arguments = ["changes.py", "--event", "pull_request", "--base", self.first,
                     "--head", head, "--output", str(output)]
        with patch("sys.argv", arguments), patch("builtins.print"):
            changes.main()
        self.assertEqual(output.read_text(encoding="utf-8"),
                         "existing=value\nfull_checks=false\njetbrains_checks=true\n")


class GateTests(unittest.TestCase):
    def needs(self, full="true", package_nagi="false", package_vscode="false", jetbrains="false"):
        return {
            "changes": {"result": "success", "outputs": {"full_checks": full, "jetbrains_checks": jetbrains}},
            "linux": {"result": "success" if full == "true" else "skipped"},
            "jetbrains": {"result": "success" if jetbrains == "true" else "skipped"},
            "release-plan": {"result": "success", "outputs": {"package_nagi": package_nagi, "package_vscode": package_vscode}},
            "nagi-package": {"result": "success" if package_nagi == "true" else "skipped"},
            "vscode-package": {"result": "success" if package_vscode == "true" else "skipped"},
        }

    def test_docs_only_and_full_checks_require_the_planned_results(self):
        for full in ("true", "false"):
            for nagi in ("true", "false"):
                for vscode in ("true", "false"):
                    for jetbrains in ("true", "false"):
                        with self.subTest(full=full, nagi=nagi, vscode=vscode, jetbrains=jetbrains):
                            self.assertEqual(gate.errors(self.needs(full, nagi, vscode, jetbrains)), [])

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

    def test_plugin_only_failures_cannot_pass_the_existing_merge_gate(self):
        for result in ("failure", "cancelled", "skipped", ""):
            with self.subTest(result=result):
                needs = self.needs(full="false", jetbrains="true")
                needs["jetbrains"]["result"] = result
                self.assertEqual(gate.errors(needs), [f"jetbrains: {result}, expected success"])

    def test_missing_or_malformed_jetbrains_plan_is_rejected(self):
        for value in (None, "", "False", "TRUE", True, False):
            with self.subTest(value=value):
                needs = self.needs("false")
                needs["changes"]["outputs"]["jetbrains_checks"] = value
                self.assertEqual(gate.errors(needs), ["jetbrains: missing or invalid check plan"])

    def test_unplanned_plugin_checks_must_be_skipped(self):
        for result in ("failure", "cancelled", "success", ""):
            with self.subTest(result=result):
                needs = self.needs("false")
                needs["jetbrains"]["result"] = result
                self.assertEqual(gate.errors(needs), [f"jetbrains: {result}, expected skipped"])

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


class JetBrainsWorkflowTests(unittest.TestCase):
    def test_paths_skipped_by_rust_have_main_and_reusable_plugin_checks(self):
        workflow = Path(__file__).resolve().parents[2] / ".github/workflows/jetbrains.yml"
        text = workflow.read_text(encoding="utf-8")
        plugin_paths = (
            "editors/jetbrains-nagi/src/main/java/com/disnana/nagi/NagiLexer.java",
            "editors/jetbrains-nagi/src/test/java/com/disnana/nagi/NagiEditorTest.java",
            "editors/jetbrains-nagi/build.gradle.kts",
            "editors/jetbrains-nagi/gradle/wrapper/gradle-wrapper.jar",
            ".github/workflows/jetbrains.yml",
        )
        watched_paths = (*plugin_paths, *sorted(changes.JETBRAINS_CHECK_INPUTS),
                         "compiler/src/lib.rs", "runtime/src/lib.rs")
        for path in plugin_paths:
            self.assertTrue(changes.is_jetbrains_path(path), path)
        block = re.search(r"^  push:\n((?: {4}[^\n]*\n|\n)+)", text, re.MULTILINE)
        self.assertIsNotNone(block, "Missing main push trigger")
        patterns = re.findall(r"^\s+- ['\"]([^'\"]+)['\"]\s*$", block.group(1), re.MULTILINE)
        for path in watched_paths:
            self.assertTrue(changes.requires_jetbrains(path), path)
            self.assertTrue(any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns),
                            f"main push does not check {path}")
        self.assertIn("    branches: [main]\n", text)
        self.assertIn("  workflow_dispatch:\n", text)
        self.assertIn("  workflow_call:\n", text)
        self.assertNotIn("  pull_request:\n", text)

    def test_existing_pr_merge_gate_waits_for_the_reusable_plugin_workflow(self):
        workflow = Path(__file__).resolve().parents[2] / ".github/workflows/ci.yml"
        text = workflow.read_text(encoding="utf-8")
        self.assertIn("on: [push, pull_request, workflow_dispatch]\n", text)
        self.assertIn("      jetbrains_checks: ${{ steps.changes.outputs.jetbrains_checks }}\n", text)
        job = re.search(r"^  jetbrains:\n((?: {4}[^\n]*\n|\n)+)", text, re.MULTILINE)
        self.assertIsNotNone(job)
        self.assertIn("    needs: changes\n", job.group(1))
        self.assertIn("github.event_name != 'push'", job.group(1))
        self.assertIn("needs.changes.outputs.jetbrains_checks == 'true'", job.group(1))
        self.assertIn("    uses: ./.github/workflows/jetbrains.yml\n", job.group(1))
        ready = re.search(r"^  ready:\n((?: {4,}[^\n]*\n|\n)+)", text, re.MULTILINE)
        self.assertIsNotNone(ready)
        dependencies = re.search(r"^    needs: \[([^\]]+)\]$", ready.group(1), re.MULTILINE)
        self.assertIsNotNone(dependencies)
        self.assertIn("jetbrains", [name.strip() for name in dependencies.group(1).split(",")])
        self.assertIn("    if: github.event_name == 'pull_request' && always()\n", ready.group(1))
        self.assertIn("        run: python scripts/ci/gate.py\n", ready.group(1))


if __name__ == "__main__":
    unittest.main()
