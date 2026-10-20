from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

MODULE_PATH = (
    Path(__file__).resolve().parents[1]
    / "scripts"
    / "ci"
    / "sf06_http_client_signature_diagnostic.py"
)
SPEC = importlib.util.spec_from_file_location("sf06_signature_diagnostic", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
DIAG = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DIAG)

CASE = DIAG.TEST


def cargo_log(message: str, *, newline: str = "\n") -> bytes:
    lines = [
        "running 2 tests",
        "test generated_operations_execute_real_tls_and_grant_in_three_test_network_paths ... ok",
        f"test {CASE} ... FAILED",
        "",
        "failures:",
        f"---- {CASE} stdout ----",
        "",
        f"thread '{CASE}' panicked at compiler/tests/support/native_triple.rs:211:13:",
        message,
        "source echo: public_signatures_and_failure_projections_build_and_run_in_three_forms",
        "failures:",
        f"    {CASE}",
        "test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s",
    ]
    return (newline.join(lines) + newline).encode("utf-8")


class SignatureDiagnosticTests(unittest.TestCase):
    def test_rust_188_leading_empty_capture_separator_is_accepted(self) -> None:
        record = DIAG.analyze(cargo_log("high backend Cargo build:"))
        self.assertEqual(
            (record["status"], record["failure_stage"], record["form"]),
            ("classified", "cargo_build", "high"),
        )

    def test_echo_before_first_panic_is_not_used_as_a_cause(self) -> None:
        text = cargo_log("high backend Cargo build:").decode("utf-8")
        text = text.replace(
            f"---- {CASE} stdout ----\n\nthread '{CASE}' panicked",
            f"---- {CASE} stdout ----\n\ncommand echo: high backend Cargo build:\n"
            f"thread '{CASE}' panicked",
        )
        record = DIAG.analyze(text.encode("utf-8"))
        self.assertEqual((record["status"], record["unavailable_reason"]),
                         ("unavailable", "panic_missing"))
        self.assertEqual(record["failure_stage"], "unavailable")

    def test_more_than_one_initial_empty_line_is_not_normalized(self) -> None:
        raw = cargo_log("high backend Cargo build:").replace(
            f"---- {CASE} stdout ----\n\nthread".encode(),
            f"---- {CASE} stdout ----\n\n\nthread".encode(),
        )
        record = DIAG.analyze(raw)
        self.assertEqual((record["status"], record["unavailable_reason"]),
                         ("unavailable", "panic_missing"))

    def test_classifies_exact_outer_cargo_build_panic_and_one_rust_code(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / f"explicit-move-{DIAG.CASE}-high-build.stdout").write_text(
                json.dumps({"reason": "compiler-message", "message": {
                    "level": "error", "code": {"code": "E0599"},
                    "rendered": "no method named `example` found at private/source.rs",
                }}) + "\n", encoding="utf-8"
            , newline="\n")
            record = DIAG.analyze(cargo_log("high backend Cargo build:"), root)
        self.assertEqual(record["status"], "classified")
        self.assertEqual(record["failure_stage"], "cargo_build")
        self.assertEqual(record["form"], "high")
        self.assertEqual(record["rust_error_code"], "E0599")
        self.assertEqual(record["rust_error_code_status"], "single")
        self.assertNotIn("raw", json.dumps(record).lower())
        self.assertNotIn("no method named", json.dumps(record))

    def test_crlf_and_lone_cr_are_explicitly_unavailable(self) -> None:
        crlf = DIAG.analyze(cargo_log("handwritten Low check: rejected", newline="\r\n"))
        lone_cr = DIAG.analyze(cargo_log("handwritten Low check: rejected").replace(b"\n", b"\r"))
        self.assertEqual((crlf["status"], crlf["unavailable_reason"]), ("unavailable", "crlf_log"))
        self.assertEqual((lone_cr["status"], lone_cr["unavailable_reason"]), ("unavailable", "lone_cr_log"))

    def test_command_echo_markers_without_a_target_failure_are_not_a_cause(self) -> None:
        log = (
            "command echo: high backend Cargo build:\n"
            "command echo: error[E0599]: not a rustc diagnostic\n"
            "running 2 tests\n"
            "test generated_operations_execute_real_tls_and_grant_in_three_test_network_paths ... ok\n"
            "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n"
        ).encode()
        record = DIAG.analyze(log)
        self.assertEqual(record["status"], "unavailable")
        self.assertEqual(record["unavailable_reason"], "target_row_missing")
        self.assertEqual(record["failure_stage"], "unavailable")

    def test_unknown_panic_and_multiple_target_panics_are_unavailable(self) -> None:
        unknown = DIAG.analyze(cargo_log("unexpected assertion details"))
        repeated = cargo_log("high backend Cargo build:").replace(
            b"source echo:",
            f"thread '{CASE}' panicked at compiler/tests/support/native_triple.rs:214:13:\nhigh backend Cargo build:\nsource echo:".encode(),
        )
        multiple = DIAG.analyze(repeated)
        self.assertEqual((unknown["status"], unknown["unavailable_reason"]), ("unavailable", "panic_unclassified"))
        self.assertEqual((multiple["status"], multiple["unavailable_reason"]), ("unavailable", "panic_ambiguous"))

    def test_other_case_failure_does_not_bind_to_target_name(self) -> None:
        log = (
            "running 2 tests\n"
            "test another_case ... FAILED\n"
            "failures:\n---- another_case stdout ----\n"
            "thread 'another_case' panicked at src/lib.rs:1:1:\n"
            "high backend Cargo build:\n"
        ).encode()
        record = DIAG.analyze(log)
        self.assertEqual(record["status"], "unavailable")
        self.assertEqual(record["unavailable_reason"], "target_row_missing")

    def test_oversize_and_invalid_utf8_top_logs_are_unavailable(self) -> None:
        oversize = DIAG.analyze(b"x" * (DIAG.LOG_LIMIT + 1))
        invalid = DIAG.analyze(b"\xff")
        self.assertEqual((oversize["status"], oversize["unavailable_reason"]), ("unavailable", "oversize_log"))
        self.assertEqual((invalid["status"], invalid["unavailable_reason"]), ("unavailable", "invalid_utf8_log"))

    def test_multiple_rust_error_codes_are_not_reported_as_one_cause(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / f"explicit-move-{DIAG.CASE}-saved-low-build.stdout").write_text(
                "\n".join(json.dumps({"reason": "compiler-message", "message": {
                    "level": "error", "code": {"code": code}, "rendered": "private source"
                }}) for code in ("E0599", "E0277")) + "\n", encoding="utf-8"
            , newline="\n")
            record = DIAG.analyze(cargo_log("saved-low backend Cargo build:"), root)
        self.assertEqual(record["failure_stage"], "cargo_build")
        self.assertEqual(record["form"], "saved-low")
        self.assertEqual(record["rust_error_code"], None)
        self.assertEqual(record["rust_error_code_status"], "multiple")

    def test_crlf_build_artifact_remains_unavailable_without_loosening_parser(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / f"explicit-move-{DIAG.CASE}-high-build.stdout"
            payload = json.dumps({"reason": "compiler-message", "message": {
                "level": "error", "code": {"code": "E0599"}
            }}).encode() + b"\r\n"
            path.write_bytes(payload)
            result = DIAG.analyze(cargo_log("high backend Cargo build:"), root)
        self.assertEqual(result["failure_stage"], "cargo_build")
        self.assertEqual((result["rust_error_code"], result["rust_error_code_status"]),
                         (None, "unavailable_crlf"))

    def test_unknown_compiler_code_shape_is_unavailable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / f"explicit-move-{DIAG.CASE}-high-build.stdout"
            path.write_text(json.dumps({"reason": "compiler-message", "message": {
                "level": "error", "code": {"code": "rust-error"}
            }}) + "\n", encoding="utf-8", newline="\n")
            result = DIAG.analyze(cargo_log("high backend Cargo build:"), root)
        self.assertEqual(result["failure_stage"], "cargo_build")
        self.assertEqual(result["rust_error_code"], None)
        self.assertEqual(result["rust_error_code_status"], "unavailable_unknown_error_code")

    def test_build_script_echo_is_not_accepted_as_a_rustc_error_code(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / f"explicit-move-{DIAG.CASE}-high-build.stdout"
            path.write_text(json.dumps({"reason": "build-script-executed", "stdout": "error[E0599]: echo"}) + "\n", encoding="utf-8", newline="\n")
            result = DIAG.analyze(cargo_log("high backend Cargo build:"), root)
        self.assertEqual(result["failure_stage"], "cargo_build")
        self.assertEqual((result["rust_error_code"], result["rust_error_code_status"]), (None, "none"))

    def test_unavailable_build_artifact_keeps_stage_but_marks_code_unavailable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / f"explicit-move-{DIAG.CASE}-handwritten-low-build.stdout"
            path.write_bytes(b"\xff")
            record = DIAG.analyze(cargo_log("handwritten-low backend Cargo build:"), root)
        self.assertEqual((record["status"], record["failure_stage"], record["form"]), ("classified", "cargo_build", "handwritten-low"))
        self.assertEqual(record["rust_error_code_status"], "unavailable_invalid_utf8")

    def test_locked_dependency_form_uses_only_ordered_completed_capture(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / f"explicit-move-{DIAG.CASE}-high.stdout").write_text("ok\n", encoding="utf-8", newline="\n")
            for suffix in ("generated.rs", "build.stdout", "build.stderr"):
                (root / f"explicit-move-{DIAG.CASE}-saved-low-{suffix}").write_text("bounded\n", encoding="utf-8", newline="\n")
            record = DIAG.analyze(cargo_log('fixture changed locked dependency: ("serde", "1.0", None)'), root)
        self.assertEqual((record["failure_stage"], record["form"]), ("locked_dependencies", "saved-low"))

    def test_oversized_build_stdout_keeps_cause_stage_and_marks_code_unavailable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / f"explicit-move-{DIAG.CASE}-high-build.stdout"
            path.write_bytes(b"x" * (DIAG.STDERR_LIMIT + 1))
            record = DIAG.analyze(cargo_log("high backend Cargo build:"), root)
        self.assertEqual((record["failure_stage"], record["form"]), ("cargo_build", "high"))
        self.assertEqual(record["rust_error_code_status"], "unavailable_oversize")

    def test_cli_writes_only_closed_json_and_marks_saved_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            log_path = root / "cargo.log"
            log_path.write_bytes(cargo_log("handwritten-low backend run requires a nonempty successful libtest summary:"))
            output = StringIO()
            with redirect_stdout(output):
                exit_code = DIAG.main([str(log_path), str(root)])
            record = json.loads((root / "sf06-signature-diagnostic.json").read_text(encoding="utf-8"))
        self.assertEqual(exit_code, 0)
        self.assertTrue(record["artifact_saved"])
        self.assertEqual((record["status"], record["failure_stage"], record["form"]), ("classified", "cargo_run", "handwritten-low"))
        self.assertNotIn(str(root), output.getvalue())
        self.assertNotIn("requires a nonempty", json.dumps(record))
        self.assertIn("artifact_saved=true", output.getvalue())

    def test_pass_and_ignored_rows_are_not_failures(self) -> None:
        passed = cargo_log("high backend Cargo build:").replace(b" ... FAILED", b" ... ok")
        ignored = cargo_log("high backend Cargo build:").replace(b" ... FAILED", b" ... ignored")
        self.assertEqual(DIAG.analyze(passed)["status"], "passed")
        self.assertEqual(DIAG.analyze(ignored)["status"], "not_run")

    def test_classifies_timeout_form_without_emitting_path(self) -> None:
        record = DIAG.analyze(cargo_log("saved-low-run timed out: stdout=/runner/private/output stderr=/runner/private/error"))
        self.assertEqual(record["status"], "classified")
        self.assertEqual(record["failure_stage"], "run_timeout")
        self.assertEqual(record["form"], "saved-low")
        rendered = json.dumps(record)
        self.assertNotIn("/runner/private", rendered)
        self.assertNotIn("stdout=", rendered)


if __name__ == "__main__":
    unittest.main()
