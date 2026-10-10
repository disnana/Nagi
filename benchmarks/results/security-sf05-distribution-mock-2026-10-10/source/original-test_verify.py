"""Keep distribution gates from accepting missing or ineffective compiler features."""
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import verify


class SqlDistributionVerificationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="nagi SQL distribution 凪 ")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.compiler = self.root / "nagic"
        self.environment = {"PATH": "existing tools", "NAGI_ROOT": "existing runtime"}

    def answer(self, command, **kwargs):
        # The gate must use the extracted compiler without borrowing build
        # tools or a runtime from the machine that produced the package.
        self.assertEqual(command[0], str(self.compiler))
        self.assertEqual(kwargs["cwd"], self.root)
        self.assertEqual(kwargs["env"]["PATH"], "")
        self.assertFalse(Path(kwargs["env"]["NAGI_ROOT"]).exists())
        self.assertEqual(kwargs["timeout"], 15)
        if "--sql-schema" not in command:
            return subprocess.CompletedProcess(command, 0, "", "checked source\n")
        source = Path(command[2])
        schema = self.root / command[command.index("--sql-schema") + 1]
        self.assertTrue(schema.is_file())
        query = source.read_text(encoding="utf-8").splitlines()[4]
        if "naem" in query:
            return subprocess.CompletedProcess(command, 1, "", f"{source.name}:5: no such column: naem\n")
        if "?2" in query:
            return subprocess.CompletedProcess(command, 1, "", f"{source.name}:5: bind count mismatch\n")
        return subprocess.CompletedProcess(command, 0, "", "SQL checked 1 literal queries; 0 runtime/unsupported sites\n")

    def check(self, answer):
        previous_path = os.environ.get("PATH")
        with patch.object(verify.subprocess, "run", side_effect=answer):
            verify.verify_sql(self.compiler, self.root, self.environment)
        self.assertEqual(os.environ.get("PATH"), previous_path)
        self.assertEqual(self.environment, {"PATH": "existing tools", "NAGI_ROOT": "existing runtime"})

    def test_valid_engine_checks_the_schema_and_preserves_environment(self):
        self.check(self.answer)

    def test_a_compiler_without_the_engine_cannot_pass_distribution_verification(self):
        def no_engine(command, **kwargs):
            if "--sql-schema" in command:
                return subprocess.CompletedProcess(command, 1, "", "SQL checks require the sql-check feature\n")
            return self.answer(command, **kwargs)
        with self.assertRaisesRegex(AssertionError, "sql-check"):
            self.check(no_engine)

    def test_success_without_an_inspected_query_is_not_verification(self):
        def no_query(command, **kwargs):
            result = self.answer(command, **kwargs)
            if "--sql-schema" in command and result.returncode == 0:
                result.stderr = "SQL checked 0 literal queries; 1 runtime/unsupported sites\n"
            return result
        with self.assertRaisesRegex(AssertionError, "SQL checked 0"):
            self.check(no_query)

    def test_a_checker_that_accepts_the_bad_query_cannot_pass(self):
        def accepts_bad_query(command, **kwargs):
            result = self.answer(command, **kwargs)
            if result.returncode != 0:
                result.returncode = 0
            return result
        with self.assertRaisesRegex(AssertionError, "column error was accepted"):
            self.check(accepts_bad_query)

    def test_a_checker_that_misses_bind_count_mismatches_cannot_pass(self):
        def accepts_bad_bind(command, **kwargs):
            result = self.answer(command, **kwargs)
            if "--sql-schema" in command and Path(command[2]).stem == "bind":
                result.returncode = 0
            return result
        with self.assertRaisesRegex(AssertionError, "bind error was accepted"):
            self.check(accepts_bad_bind)

    def test_a_backend_error_cannot_replace_a_nagi_query_diagnostic(self):
        def backend_error(command, **kwargs):
            result = self.answer(command, **kwargs)
            if result.returncode != 0:
                result.stderr = "Cargo missing; SQL validation did not run\n"
            return result
        with self.assertRaisesRegex(AssertionError, "Cargo missing"):
            self.check(backend_error)


class TaskDistributionVerificationTests(unittest.TestCase):
    OUTPUT = "42\nbusiness sentinel\ndiscarded child\nunit child\nreceived unit\nafter scope\ncompleted\n"

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="nagi Task distribution 凪 ")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.compiler = self.root / "installed compiler" / "nagic"
        self.environment = {"PATH": "existing tools", "NAGI_ROOT": "existing runtime"}
        self.calls = []

    def answer(self, command, **kwargs):
        self.calls.append((command, kwargs))
        self.assertEqual(command[0], str(self.compiler))
        self.assertEqual(kwargs["cwd"], self.root)
        self.assertTrue(kwargs["capture_output"])
        self.assertEqual(kwargs["encoding"], "utf-8")
        source = Path(command[2])
        self.assertTrue(source.is_file())
        action = command[1]
        if action == "run":
            self.assertEqual(kwargs["env"]["PATH"], "existing tools")
            self.assertNotIn("NAGI_ROOT", kwargs["env"])
            self.assertTrue(kwargs["check"])
            if source.name == "saved.low":
                # A saved Low run must not quietly reload the original High.
                self.assertFalse(source.with_name("main.nagi").exists())
            if source.name == "handwritten.low":
                self.assertIn("async fn main()", source.read_text(encoding="utf-8"))
            output = Path(command[command.index("--out") + 1])
            output.mkdir()
            (output / "Cargo.toml").write_text(
                "[dependencies]\nnagi-runtime = { path = "
                + json.dumps(str(self.compiler.parent / "runtime")) + " }\n", encoding="utf-8")
            return subprocess.CompletedProcess(command, 0, self.OUTPUT, "native build progress\n")
        self.assertEqual(kwargs["env"]["PATH"], "")
        self.assertFalse(Path(kwargs["env"]["NAGI_ROOT"]).exists())
        self.assertEqual(kwargs["timeout"], 15)
        if action == "lower":
            output = Path(command[command.index("--out") + 1])
            output.mkdir()
            (output / "generated.low").write_text("# independently emitted Low\n", encoding="utf-8")
            return subprocess.CompletedProcess(command, 0, "lowered source\n", "")
        if source.stem in ("unreceived", "double-await", "discard-await"):
            marker = "# primary"
            lines = source.read_text(encoding="utf-8").splitlines()
            line = next(index for index, text in enumerate(lines, 1) if marker in text)
            detail = "未受取Task" if source.stem == "unreceived" else "move後"
            return subprocess.CompletedProcess(command, 1, "", f"{source.name}:{line}: {detail}\n")
        return subprocess.CompletedProcess(command, 0, "checked source\n", "")

    def check(self, answer):
        previous_path = os.environ.get("PATH")
        with patch.object(verify.subprocess, "run", side_effect=answer):
            verify.verify_task_handles(self.compiler, self.root, self.environment)
        self.assertEqual(os.environ.get("PATH"), previous_path)
        self.assertEqual(self.environment, {"PATH": "existing tools", "NAGI_ROOT": "existing runtime"})

    def test_archive_task_gate_checks_three_source_forms_and_early_negatives(self):
        self.check(self.answer)
        native = [Path(command[2]).name for command, _ in self.calls if command[1] == "run"]
        self.assertEqual(native, ["main.nagi", "saved.low", "handwritten.low"])
        negatives = [(Path(command[2]).suffix, command[1]) for command, kwargs in self.calls
                     if Path(command[2]).stem in ("unreceived", "double-await", "discard-await")]
        self.assertEqual(len(negatives), 12)
        self.assertEqual(set(negatives), {(suffix, action) for suffix in (".nagi", ".low")
                                         for action in ("check", "build")})

    def test_missing_business_result_is_not_a_successful_native_task_run(self):
        def missing_business(command, **kwargs):
            result = self.answer(command, **kwargs)
            if command[1] == "run":
                result.stdout = result.stdout.replace("business sentinel\n", "")
            return result
        with self.assertRaisesRegex(AssertionError, "Task native output"):
            self.check(missing_business)

    def test_checkout_runtime_cannot_replace_the_bundled_task_runtime(self):
        def checkout_runtime(command, **kwargs):
            result = self.answer(command, **kwargs)
            if command[1] == "run":
                output = Path(command[command.index("--out") + 1])
                (output / "Cargo.toml").write_text(
                    "[dependencies]\nnagi-runtime = { path = "
                    + json.dumps(str(self.root / "checkout/runtime")) + " }\n", encoding="utf-8")
            return result
        with self.assertRaisesRegex(AssertionError, "Task runtime came from outside the distribution"):
            self.check(checkout_runtime)

    def test_discarded_child_must_finish_before_scope_exit(self):
        def late_child(command, **kwargs):
            result = self.answer(command, **kwargs)
            if command[1] == "run":
                result.stdout = result.stdout.replace("discarded child\n", "") + "discarded child\n"
            return result
        with self.assertRaisesRegex(AssertionError, "Task scope completion"):
            self.check(late_child)

    def test_a_checker_accepting_pending_tasks_cannot_pass(self):
        def accepts_task(command, **kwargs):
            result = self.answer(command, **kwargs)
            if Path(command[2]).stem == "unreceived":
                result.returncode = 0
            return result
        with self.assertRaisesRegex(AssertionError, "Task unreceived.*was accepted"):
            self.check(accepts_task)

    def test_parser_rejection_does_not_prove_task_ownership(self):
        def parser_rejection(command, **kwargs):
            result = self.answer(command, **kwargs)
            if result.returncode != 0:
                result.stderr = result.stderr.replace("未受取Task", "parse error: unexpected spawn")
            return result
        with self.assertRaisesRegex(AssertionError, "parse error"):
            self.check(parser_rejection)

    def test_wrong_primary_line_does_not_prove_task_ownership(self):
        def wrong_line(command, **kwargs):
            result = self.answer(command, **kwargs)
            if result.returncode != 0:
                source = Path(command[2])
                line = next(index for index, text in enumerate(source.read_text(encoding="utf-8").splitlines(), 1)
                            if "# primary" in text)
                result.stderr = result.stderr.replace(f":{line}:", f":{line + 1}:")
            return result
        with self.assertRaisesRegex(AssertionError, "Task primary line"):
            self.check(wrong_line)

    def test_backend_rejection_cannot_replace_task_checker_evidence(self):
        def backend_rejection(command, **kwargs):
            result = self.answer(command, **kwargs)
            if result.returncode != 0:
                result.stderr += "Cargo missing; Rust backend failed\n"
            return result
        with self.assertRaisesRegex(AssertionError, "Cargo missing"):
            self.check(backend_rejection)


class SqlitePoolDistributionVerificationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="nagi SQLite distribution 凪 ")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.compiler = self.root / "installed compiler" / "nagic"
        self.environment = {"PATH": "existing tools", "NAGI_ROOT": "existing runtime"}
        self.native_sources = []

    def answer(self, command, **kwargs):
        self.assertEqual(command[0], str(self.compiler))
        self.assertEqual(kwargs["cwd"], self.root)
        source = Path(command[2])
        self.assertTrue(source.is_file())
        output = Path(command[command.index("--out") + 1])
        if command[1] == "run":
            self.native_sources.append(source.name)
            self.assertNotIn("NAGI_ROOT", kwargs["env"])
            self.assertEqual(kwargs["env"]["PATH"], "existing tools")
            if source.name == "saved.low":
                self.assertFalse(source.with_name("sqlite_pool.nagi").exists())
            output.mkdir()
            (output / "Cargo.toml").write_text(
                "[dependencies]\nnagi-runtime = { path = "
                + json.dumps(str(self.compiler.parent / "runtime")) + " }\n", encoding="utf-8")
            return subprocess.CompletedProcess(command, 0, "7\nclosed\n", "build progress\n")
        self.assertEqual(kwargs["env"]["PATH"], "")
        self.assertFalse(Path(kwargs["env"]["NAGI_ROOT"]).exists())
        if command[1] == "lower":
            output.mkdir()
            (output / "generated.low").write_text("# saved Low fixture\n", encoding="utf-8")
        return subprocess.CompletedProcess(command, 0, "", "")

    def check(self, answer):
        with patch.object(verify.subprocess, "run", side_effect=answer):
            verify.verify_sqlite_pool(self.compiler, self.root, self.environment)
        self.assertEqual(self.environment, {"PATH": "existing tools", "NAGI_ROOT": "existing runtime"})

    def test_both_forms_use_the_bundled_runtime_after_high_removal(self):
        self.check(self.answer)
        self.assertEqual(self.native_sources, ["sqlite_pool.nagi", "saved.low"])

    def test_checkout_runtime_is_not_distribution_evidence(self):
        def wrong_runtime(command, **kwargs):
            result = self.answer(command, **kwargs)
            if command[1] == "run":
                output = Path(command[command.index("--out") + 1])
                (output / "Cargo.toml").write_text('[dependencies]\nnagi-runtime={path="/checkout/runtime"}\n', encoding="utf-8")
            return result
        with self.assertRaisesRegex(AssertionError, "runtime outside distribution"):
            self.check(wrong_runtime)

    def test_missing_native_effect_is_not_success(self):
        def no_native_effect(command, **kwargs):
            result = self.answer(command, **kwargs)
            if command[1] == "run":
                result.stdout = "checked source\n"
            return result
        with self.assertRaisesRegex(AssertionError, "SQLite native output"):
            self.check(no_native_effect)


if __name__ == "__main__":
    unittest.main()
