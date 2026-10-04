"""Keep SQL distribution verification from accepting a missing or ineffective engine."""
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


if __name__ == "__main__":
    unittest.main()
