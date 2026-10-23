"""Stream-framing regressions; strict stage and native-case parsers stay unchanged."""
import contextlib
import hashlib
import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts/ci"))
import auth_stage_diagnostic as stages
import auth_stage_join as joiner
import auth_stage_stream_projection as streams
import browser_login_stage_coverage as coverage
import sf07_request_projection as historical
from sf07_projection_contract import BASE_WORKFLOW_SHA256


def stdout_bytes(ending="\n"):
    return ("".join(f"test {name} ... ok{ending}" for name in coverage.REQUIRED)
            + "test result: ok. 434 passed; 0 failed; 0 ignored; 0 measured; "
              f"0 filtered out; finished in 1.00s{ending}").encode()


def stderr_bytes(ending="\n"):
    return "".join(
        f"{ending}auth-stage slot={slot} (test-only; scheduling perturbed){ending}"
        "op=Bootstrap stage=Return event=Success elapsed_us=Some(1) "
        f"utc_remaining_ms=Some(2) mono=None kind=None outcome=None{ending}lost=false{ending}"
        for slot in (1, 2)).encode()


class AuthStageJoinTests(unittest.TestCase):
    def setUp(self):
        owner = tempfile.TemporaryDirectory()
        self.addCleanup(owner.cleanup)
        self.root = Path(owner.name)

    def write(self, stdout, stderr, name="broad"):
        (self.root / f"auth-stage-{name}.raw").write_bytes(stdout)
        (self.root / f"auth-stage-{name}.stderr.raw").write_bytes(stderr)

    def join(self, name="broad"):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            status = joiner.join(self.root, name)
        return status, output.getvalue()

    def captured(self, raw, code=0):
        (self.root / "auth-stage-broad.raw").write_bytes(raw)
        output = io.TextIOWrapper(io.BytesIO(), encoding="utf-8")
        with contextlib.redirect_stdout(output):
            valid = stages.capture("broad", self.root, code)
        output.flush()
        return valid, output.buffer.getvalue()

    def test_interleaved_realistic_prefix_is_red_and_split_join_is_green(self):
        for ending in ("\n", "\r\n"):
            with self.subTest(ending=ending):
                stdout, stderr = stdout_bytes(ending), stderr_bytes(ending)
                prefix = ("test " + coverage.REQUIRED[-1] + " ... ").encode()
                split = stdout.index(prefix) + len(prefix)
                interleaved = stdout[:split] + stderr + stdout[split:]
                before, code = coverage.coverage(interleaved, 0)
                self.assertEqual(code, 1)
                self.assertEqual(before["cases"][coverage.REQUIRED[-1]], "missing")
                # Reproduces the observed shape: auth slots can pass while a
                # required native test row is missing. It does not prove cause.
                self.assertTrue(self.captured(interleaved)[0])
                self.write(stdout, stderr)
                self.assertEqual(self.join()[0], 0)
                combined = (self.root / "auth-stage-broad.raw").read_bytes()
                self.assertEqual(combined, stdout + b"\n" + stderr)
                self.assertEqual(coverage.coverage(combined, 0)[1], 0)
                self.assertTrue(self.captured(combined)[0])

    def test_separator_frames_streams_without_boundary_newlines(self):
        self.write(stdout_bytes().rstrip(b"\n"), stderr_bytes().lstrip(b"\n"))
        self.assertEqual(self.join()[0], 0)
        combined = (self.root / "auth-stage-broad.raw").read_bytes()
        self.assertEqual(coverage.coverage(combined, 0)[1], 0)
        self.assertTrue(self.captured(combined)[0])

    def test_nonzero_exit_missing_duplicate_and_lost_stay_red(self):
        missing = stdout_bytes().replace(("test " + coverage.REQUIRED[0] + " ... ok\n").encode(), b"")
        duplicate = stdout_bytes() + ("test " + coverage.REQUIRED[0] + " ... ok\n").encode()
        for stdout, stderr, code, expect_cases, expect_stages in (
            (stdout_bytes(), stderr_bytes(), 101, False, False),
            (missing, stderr_bytes(), 0, False, True),
            (duplicate, stderr_bytes(), 0, False, True),
            (stdout_bytes(), stderr_bytes().replace(b"lost=false", b"lost=true"), 0, True, False),
            (stdout_bytes(), stderr_bytes().split(b"\nauth-stage slot=2")[0], 0, True, False),
            (stdout_bytes(), stderr_bytes() + stderr_bytes(), 0, True, False),
            (stdout_bytes(), stderr_bytes().replace(b"stage=Return", b"stage=Unknown"), 0, True, False),
            (stdout_bytes(), b"", 0, True, False),
        ):
            self.write(stdout, stderr)
            self.assertEqual(self.join()[0], 0)
            combined = (self.root / "auth-stage-broad.raw").read_bytes()
            self.assertEqual(coverage.coverage(combined, code)[1] == 0, expect_cases)
            self.assertEqual(self.captured(combined, code)[0], expect_stages)

    def test_total_limit_includes_separator_and_preserves_oversize_sentinel(self):
        limit = stages.RAW_LIMIT
        for stdout_size, stderr_size, expected in (
            (limit - 1, 0, limit), (limit, 0, limit + 1),
            (limit // 2, limit // 2, limit + 1),
            (limit + 1000, limit + 1000, limit + 1),
            (0, limit + 1000, limit + 1),
        ):
            self.write(b"a" * stdout_size, b"b" * stderr_size)
            self.assertEqual(self.join()[0], 0)
            combined = (self.root / "auth-stage-broad.raw").read_bytes()
            self.assertEqual(len(combined), expected)
            if expected > limit:
                self.assertEqual(coverage.coverage(combined, 0)[1], 1)
                valid, normalized = self.captured(combined)
                self.assertFalse(valid)
                self.assertIn(b"raw_limit=exceeded", normalized)

    def test_late_stderr_cannot_hide_combined_oversize_after_valid_rows(self):
        self.write(stdout_bytes(), stderr_bytes() + b"x" * stages.RAW_LIMIT)
        self.assertEqual(self.join()[0], 0)
        combined = (self.root / "auth-stage-broad.raw").read_bytes()
        self.assertEqual(len(combined), stages.RAW_LIMIT + 1)
        self.assertEqual(coverage.coverage(combined, 0)[1], 1)
        self.assertFalse(self.captured(combined)[0])

    def test_missing_or_nonregular_stream_refuses_without_private_echo(self):
        stdout = self.root / "auth-stage-broad.raw"
        stdout.write_bytes(b"original")
        status, output = self.join()
        self.assertEqual(status, 1)
        self.assertEqual(stdout.read_bytes(), b"original")
        (self.root / "auth-stage-broad.stderr.raw").mkdir()
        self.assertEqual(self.join()[0], 1)
        with patch.object(Path, "lstat", side_effect=OSError("secret-path")):
            self.assertEqual(self.join(), (1, "auth_stage_join state=unavailable reason=io\n"))

    def test_aliased_streams_and_write_failures_stay_red(self):
        self.write(b"original", b"stderr")
        with patch.object(Path, "samefile", return_value=True):
            self.assertEqual(self.join()[0], 1)
        self.assertEqual((self.root / "auth-stage-broad.raw").read_bytes(), b"original")
        original_open = Path.open

        def open_failure(path, mode="r", *args, **kwargs):
            if mode == "wb":
                raise OSError("private-write-path")
            return original_open(path, mode, *args, **kwargs)

        with patch.object(Path, "open", open_failure):
            self.assertEqual(self.join(), (1, "auth_stage_join state=unavailable reason=io\n"))

    def test_invalid_scope_and_arity_never_echo_arguments(self):
        self.assertEqual(self.join("secret"), (1, "auth_stage_join state=unavailable reason=arguments\n"))
        output = io.StringIO()
        with patch.object(sys, "argv", ["join", "secret"]), contextlib.redirect_stdout(output):
            self.assertEqual(joiner.main(), 1)
        self.assertNotIn("secret", output.getvalue())

    def test_raw_secrets_are_not_published_by_helper_or_unchanged_consumers(self):
        private = b"Cookie=secret-cookie SQL /private/path"
        self.write(stdout_bytes(), stderr_bytes() + private)
        status, output = self.join()
        self.assertEqual(status, 0)
        self.assertNotIn("secret", output)
        combined = (self.root / "auth-stage-broad.raw").read_bytes()
        valid, normalized = self.captured(combined)
        self.assertTrue(valid)
        self.assertNotIn(private, normalized)
        self.assertNotIn("secret", str(coverage.coverage(combined, 0)[0]))

    def test_workflow_exit_order_cleanup_and_historical_identity(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        for old, new in streams.edits():
            self.assertEqual(workflow.count(new), 2)
            for mutant in (workflow.replace(new, "", 1),
                           workflow.replace(new, new + new, 1)):
                with self.assertRaises(ValueError):
                    streams.inverse(mutant)
        for name, _, _ in streams.PROFILES:
            adjacent = (f'          {name}_rc=$?\n'
                        f'          python -B scripts/ci/auth_stage_join.py "$RUNNER_TEMP" {name} || join_failed=1\n')
            self.assertEqual(workflow.count(adjacent), 2)
        self.assertEqual(hashlib.sha256(historical.inverse_workflow(workflow).encode()).hexdigest(),
                         BASE_WORKFLOW_SHA256)
        stage1 = streams.inverse(workflow)
        self.assertNotIn("auth_stage_join.py", stage1)
        self.assertNotIn("join_failed", stage1)


if __name__ == "__main__":
    unittest.main()
