"""Closed-field failure supplement controls; native Rust execution is separate."""
import contextlib
import hashlib
import io
import os
from pathlib import Path
import subprocess
import stat
import sys
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts/ci"))
import sf07_prelogin_owner_diagnostic as diagnostic
import sf07_request_projection as projection
from sf07_projection_contract import BASE_WORKFLOW_SHA256

HEADER = "auth-stage slot=2 (test-only; scheduling perturbed)\n"
RECORD = ("op=Bootstrap stage=Return event=Success elapsed_us=Some(1) "
          "utc_remaining_ms=Some(2) mono=None kind=None outcome=None\n")
FRAME = HEADER + RECORD + "lost=false\n"
ENABLE = '''    // Slot 2 also belongs to the ordinary prelogin diagnostic. Enable this
    // witness only in its own exact-test process, never in the broad suite.
    #[cfg(nagi_auth_stage_diag)]
    let observer = (std::env::var_os("NAGI_PRELOGIN_WIRE_STAGE_DIAG").as_deref()
        == Some(std::ffi::OsStr::new("1")))
    .then(|| crate::auth::stage_observer::Guard::new(2));
'''
ATTACH = '''    #[cfg(nagi_auth_stage_diag)]
    if let Some(observer) = &observer {
        observer.attach(&p.stage_slot);
    }
'''


def raw(frame=FRAME, code=0):
    status, state, passed, failed = (("ok", "ok", 1, 0) if code == 0
                                    else ("FAILED", "FAILED", 0, 1))
    return (f"running 1 test\ntest {diagnostic.CASE} ... {state}\n" + frame
            + f"test result: {status}. {passed} passed; {failed} failed; 0 ignored; "
              "0 measured; 999 filtered out; finished in 0.01s\n").encode()


class PreloginOwnerDiagnosticTests(unittest.TestCase):
    def setUp(self):
        owner = tempfile.TemporaryDirectory()
        self.addCleanup(owner.cleanup)
        self.root = Path(owner.name) / "diagnostic"
        self.github_output = Path(owner.name) / "github-output"
        self.github_output.touch()

    def run_capture(self, content=None, code=0, oversized=False, error=None, env=None):
        stream = io.TextIOWrapper(io.BytesIO(), encoding="utf-8")
        environment = {"GITHUB_OUTPUT": str(self.github_output), **(env or {})}
        with contextlib.redirect_stdout(stream), patch.dict(os.environ, environment, clear=True):
            with patch.object(diagnostic, "capture_process", return_value=(
                    raw() if content is None else content, code, oversized), side_effect=error) as capture:
                result = diagnostic.run(self.root)
        stream.flush()
        return result, stream.buffer.getvalue(), capture

    def assert_not_publishable(self):
        self.assertEqual(self.github_output.read_bytes(), b"")

    def test_exact_command_keeps_parallelism_and_existing_deadlines(self):
        self.assertEqual(diagnostic.command(), [
            "cargo", "test", "--locked", "-p", "nagi-runtime", "--lib", diagnostic.CASE,
            "--", "--exact", "--nocapture"])

    def test_success_is_only_a_supplement_and_raw_is_removed(self):
        result, output, capture = self.run_capture(env={"RUSTFLAGS": "--cfg existing"})
        self.assertEqual(result, 0)
        self.assertIn(b"attempt=supplemental scheduling=perturbed", output)
        self.assertIn(b"test=prelogin cargo_exit=0 inventory=pass", output)
        self.assertEqual((self.root / "auth-stage-prelogin.trace").read_bytes(),
                         output[output.index(b"test=prelogin"):])
        self.assertFalse((self.root / "auth-stage-prelogin.raw").exists())
        command, environment = capture.call_args.args
        self.assertEqual(command, diagnostic.command())
        self.assertEqual(environment[diagnostic.OPT_IN], "1")
        self.assertEqual(environment["RUSTFLAGS"], "--cfg existing --cfg nagi_auth_stage_diag")
        self.assertNotIn("RUST_TEST_THREADS", environment)
        self.assertNotIn("GITHUB_OUTPUT", environment)
        self.assertEqual(self.github_output.read_text(), "sanitized=true\n")

    def test_failed_test_with_complete_frame_stays_failed_without_private_echo(self):
        frame = FRAME.replace("event=Success", "event=Failure").replace(
            "kind=None outcome=None", "kind=Some(Unavailable) outcome=Some(Committed)")
        content = raw(frame, 101) + b"secret-cookie /private/path SQL SELECT\n"
        result, output, _ = self.run_capture(content, 101)
        self.assertEqual(result, 1)
        self.assertIn(b"cargo_exit=101 inventory=fail", output)
        self.assertIn(b"kind=Some(Unavailable) outcome=Some(Committed)", output)
        for private in (b"secret-cookie", b"/private/path", b"SQL SELECT"):
            self.assertNotIn(private, output)
            self.assertNotIn(private, (self.root / "auth-stage-prelogin.trace").read_bytes())
        self.assertFalse((self.root / "auth-stage-prelogin.raw").exists())
        # The collector wrote a closed sanitized failure, so the trace remains
        # available for diagnosis even though the original test stays red.
        self.assertEqual(self.github_output.read_text(), "sanitized=true\n")

    def test_missing_duplicate_wrong_slot_malformed_lost_and_unclosed_refuse(self):
        frames = ["", FRAME * 2, FRAME.replace("slot=2", "slot=1"),
                  FRAME.replace("stage=Return", "stage=secret-cookie"),
                  FRAME.replace("lost=false", "lost=true"), FRAME.removesuffix("lost=false\n")]
        for index, frame in enumerate(frames):
            with self.subTest(index=index):
                self.root = self.root.parent / f"invalid-{index}"
                result, output, _ = self.run_capture(raw(frame))
                self.assertEqual(result, 1)
                self.assertNotIn(b"secret-cookie", output)
                self.assertFalse((self.root / "auth-stage-prelogin.raw").exists())

    def test_oversized_capture_is_not_a_passing_prefix(self):
        result, output, _ = self.run_capture(raw(), oversized=True)
        self.assertEqual(result, 1)
        self.assertIn(b"raw_limit=exceeded", output)
        self.assertFalse((self.root / "auth-stage-prelogin.raw").exists())

    def test_spawn_and_timeout_details_stay_private(self):
        for index, error in enumerate((OSError("secret-path"),
                                        subprocess.TimeoutExpired("secret-command", 600))):
            self.root = self.root.parent / f"error-{index}"
            result, output, _ = self.run_capture(error=error)
            self.assertEqual(result, 1)
            self.assertIn(b"state=unavailable reason=capture", output)
            self.assertNotIn(b"secret", output)
            self.assertFalse((self.root / "auth-stage-prelogin.raw").exists())
            self.assert_not_publishable()

    def test_existing_directory_is_never_overwritten_or_cleaned(self):
        self.root.mkdir()
        foreign = self.root / "auth-stage-prelogin.raw"
        foreign.write_bytes(b"other owner")
        foreign_trace = self.root / "auth-stage-prelogin.trace"
        foreign_trace.write_bytes(b"unverified secret trace")
        result, _, capture = self.run_capture()
        self.assertEqual(result, 1)
        capture.assert_not_called()
        self.assertEqual(foreign.read_bytes(), b"other owner")
        self.assertEqual(foreign_trace.read_bytes(), b"unverified secret trace")
        self.assert_not_publishable()

    def test_encoded_flags_fail_closed_without_running_a_different_cfg(self):
        result, output, capture = self.run_capture(env={"CARGO_ENCODED_RUSTFLAGS": "secret"})
        self.assertEqual(result, 1)
        capture.assert_not_called()
        self.assertIn(b"reason=encoded_flags", output)
        self.assertNotIn(b"secret", output)
        self.assert_not_publishable()

    def test_collector_failure_still_removes_raw(self):
        with patch.object(diagnostic.stages, "capture", side_effect=ValueError("secret")):
            result, output, _ = self.run_capture()
        self.assertEqual(result, 1)
        self.assertNotIn(b"secret", output)
        self.assertFalse((self.root / "auth-stage-prelogin.raw").exists())
        self.assert_not_publishable()

    def test_cleanup_failure_is_red_without_private_error_details(self):
        with patch.object(Path, "unlink", side_effect=OSError("secret")):
            result, output, _ = self.run_capture()
        self.assertEqual(result, 1)
        self.assertIn(b"state=unavailable reason=cleanup", output)
        self.assertNotIn(b"secret", output)
        self.assert_not_publishable()

    def test_collector_internal_exception_or_missing_file_cannot_authorize_upload(self):
        for index, raises in enumerate((True, False)):
            self.root = self.root.parent / f"collector-{index}"

            def broken(*args, **kwargs):
                if raises:
                    (self.root / "auth-stage-prelogin.trace").write_bytes(b"partial secret")
                    raise RuntimeError("private collector bug")
                return False

            with patch.object(diagnostic.stages, "capture", side_effect=broken):
                result, output, _ = self.run_capture()
            self.assertEqual(result, 1)
            self.assert_not_publishable()
            self.assertNotIn(b"private", output)
            self.assertFalse((self.root / "auth-stage-prelogin.raw").exists())

    def test_child_created_trace_or_raw_is_not_overwritten_or_deleted(self):
        for index, filename in enumerate(("auth-stage-prelogin.trace", "auth-stage-prelogin.raw")):
            self.root = self.root.parent / f"foreign-{index}"

            def capture(*args):
                (self.root / filename).write_bytes(b"foreign secret")
                return raw(), 0, False

            with patch.object(diagnostic, "capture_process", side_effect=capture):
                # Run directly so run_capture's process mock does not hide this.
                with patch.dict(os.environ, {"GITHUB_OUTPUT": str(self.github_output)}, clear=True), \
                        contextlib.redirect_stdout(io.TextIOWrapper(io.BytesIO(), encoding="utf-8")):
                    self.assertEqual(diagnostic.run(self.root), 1)
            self.assert_not_publishable()
            self.assertEqual((self.root / filename).read_bytes(), b"foreign secret")

    def test_replaced_root_is_not_cleaned_or_published(self):
        def capture(*args):
            self.root.rename(self.root.parent / "original-owner")
            self.root.mkdir()
            (self.root / "auth-stage-prelogin.raw").write_bytes(b"foreign raw")
            return raw(), 0, False

        with patch.object(diagnostic, "capture_process", side_effect=capture), \
                patch.dict(os.environ, {"GITHUB_OUTPUT": str(self.github_output)}, clear=True), \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(diagnostic.run(self.root), 1)
        self.assert_not_publishable()
        self.assertEqual((self.root / "auth-stage-prelogin.raw").read_bytes(), b"foreign raw")

    def test_trace_replaced_during_cleanup_is_not_published(self):
        original_unlink = Path.unlink

        def unlink(path, *args, **kwargs):
            original_unlink(path, *args, **kwargs)
            foreign = self.root / "replacement"
            foreign.write_bytes(b"foreign trace")
            foreign.replace(self.root / "auth-stage-prelogin.trace")

        with patch.object(Path, "unlink", unlink):
            result, _, _ = self.run_capture()
        self.assertEqual(result, 1)
        self.assert_not_publishable()
        self.assertEqual((self.root / "auth-stage-prelogin.trace").read_bytes(), b"foreign trace")

    def test_replaced_raw_is_not_deleted_or_published(self):
        original_capture = diagnostic.stages.capture

        def collect(*args, **kwargs):
            valid = original_capture(*args, **kwargs)
            foreign = self.root / "replacement"
            foreign.write_bytes(b"foreign raw")
            foreign.replace(self.root / "auth-stage-prelogin.raw")
            return valid

        with patch.object(diagnostic.stages, "capture", side_effect=collect):
            result, _, _ = self.run_capture()
        self.assertEqual(result, 1)
        self.assert_not_publishable()
        self.assertEqual((self.root / "auth-stage-prelogin.raw").read_bytes(), b"foreign raw")

    def test_nonregular_or_linked_trace_cannot_authorize_upload(self):
        original_lstat = Path.lstat
        for index, linked in enumerate((False, True)):
            self.root = self.root.parent / f"linked-{index}"

            def lstat(path, *args, **kwargs):
                info = original_lstat(path, *args, **kwargs)
                if path == self.root / "auth-stage-prelogin.trace":
                    return SimpleNamespace(st_mode=info.st_mode if linked else stat.S_IFLNK,
                                           st_nlink=2 if linked else 1)
                return info

            with patch.object(Path, "lstat", lstat):
                result, _, _ = self.run_capture()
            self.assertEqual(result, 1)
            self.assert_not_publishable()

    def test_output_open_failure_is_red_after_safe_cleanup(self):
        original_open = Path.open

        def open_file(path, *args, **kwargs):
            if path == self.github_output:
                raise OSError("private output path")
            return original_open(path, *args, **kwargs)

        with patch.object(Path, "open", open_file):
            result, output, _ = self.run_capture()
        self.assertEqual(result, 1)
        self.assert_not_publishable()
        self.assertNotIn(b"private output path", output)
        self.assertFalse((self.root / "auth-stage-prelogin.raw").exists())

    def test_wrong_arguments_fail_without_echo(self):
        stream = io.StringIO()
        with patch.object(sys, "argv", ["diagnostic", "secret", "extra"]), contextlib.redirect_stdout(stream):
            self.assertEqual(diagnostic.main(), 1)
        self.assertNotIn("secret", stream.getvalue())

    def test_only_cfg_opt_in_wiring_was_added_to_the_exact_native_oracle(self):
        source = (ROOT / "runtime/src/auth/prelogin/fixed_wire_owner_tests.rs").read_text()
        self.assertEqual(source.count(ENABLE), 1)
        self.assertEqual(source.count(ATTACH), 1)
        self.assertLess(source.index(ATTACH), source.index("p.bootstrap(&o, None).await.unwrap();"))
        original = source.replace(ENABLE, "").replace(ATTACH, "")
        self.assertEqual(hashlib.sha256(original.encode()).hexdigest(),
                         "4a30e5e886ad6a6c4ef10895e18b7b1b2ddfa1f14d2771afb2409da6dcc96de6")

    def test_primary_failure_and_sanitized_only_artifact_are_preserved(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        self.assertEqual(workflow.count(projection.PRELOGIN_OWNER_DIAGNOSTIC_STEP), 2)
        step = projection.PRELOGIN_OWNER_DIAGNOSTIC_STEP
        self.assertIn("if: failure() && steps.sf07_checkpoint.outcome == 'failure' && !cancelled()", step)
        self.assertNotIn("continue-on-error", step)
        self.assertNotIn(".raw", step)
        self.assertIn("/auth-stage-prelogin.trace", step)
        self.assertIn("id: sf07_prelogin_owner_diag", step)
        self.assertIn("steps.sf07_prelogin_owner_diag.outputs.sanitized == 'true'", step)
        primary = ("          cargo test --locked -p nagi-runtime --lib "
                   "auth::prelogin::tests::fixed_wire_owner_tests:: 2>&1 | python -B "
                   "scripts/ci/sf07_checkpoint_summary.py prelogin-issuer\n")
        self.assertEqual(workflow.count(primary), 2)
        self.assertEqual(hashlib.sha256(projection.inverse_workflow(workflow).encode()).hexdigest(),
                         BASE_WORKFLOW_SHA256)

    def test_missing_duplicated_or_weakened_diagnostic_registration_refuses(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        for addition in (projection.PRELOGIN_OWNER_DIAGNOSTIC_STEP,
                         projection.PRELOGIN_OWNER_PRIMARY_ID,
                         projection.PRELOGIN_OWNER_UNIT_LINE):
            for mutant in (workflow.replace(addition, "", 1),
                           workflow.replace(addition, addition + addition, 1)):
                with self.assertRaises(ValueError):
                    projection.inverse_workflow(mutant)
        mutant = workflow.replace("if: failure() && steps.sf07_checkpoint.outcome", "if: success() && steps.sf07_checkpoint.outcome", 1)
        with self.assertRaises(ValueError):
            projection.inverse_workflow(mutant)
        mutant = workflow.replace(" && steps.sf07_prelogin_owner_diag.outputs.sanitized == 'true'", "", 1)
        with self.assertRaises(ValueError):
            projection.inverse_workflow(mutant)
        # Counts alone cannot prove the failure hook refers to the right step.
        for addition in (projection.PRELOGIN_OWNER_DIAGNOSTIC_STEP,
                         projection.PRELOGIN_OWNER_PRIMARY_ID,
                         projection.PRELOGIN_OWNER_UNIT_LINE):
            moved = workflow.replace(addition, "", 1) + addition
            with self.assertRaises(ValueError):
                projection.inverse_workflow(moved)

    def test_historical_infrastructure_composes_without_accepting_mixed_profiles(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        inherited = projection._LEVELS.inverse_workflow(workflow)
        self.assertEqual(hashlib.sha256(projection.inverse_workflow(inherited).encode()).hexdigest(),
                         BASE_WORKFLOW_SHA256)
        for count in (1, 2):
            mutant = workflow.replace(projection.PRELOGIN_OWNER_DIAGNOSTIC_STEP,
                                      projection.PRELOGIN_OWNER_INHERITED_STEP, count)
            with self.assertRaises(ValueError):
                projection.inverse_workflow(mutant)
        mixed = inherited.replace(projection.PRELOGIN_OWNER_INHERITED_STEP,
                                  projection.PRELOGIN_OWNER_DIAGNOSTIC_STEP, 1)
        with self.assertRaises(ValueError):
            projection.inverse_workflow(mixed)


if __name__ == "__main__":
    unittest.main()
