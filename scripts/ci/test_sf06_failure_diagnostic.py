"""Execute the real unchanged SF06 guard and observation wrapper with owned logs."""
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile
import textwrap
import unittest
from unittest.mock import patch

import sf06_failure_projection as projection
import sf06_http_client_failure_diagnostic as diag
import sf07_request_projection as history
from sf07_projection_contract import BASE_WORKFLOW_SHA256

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/ci.yml"
# Public synthetic input for exclusion checks, never a credential.
PUBLIC_FIXTURE_MARKER = "PUBLIC_DIAGNOSTIC_FIXTURE_MARKER"


def guard_source(source):
    match = re.search(r'(?ms)^          cat > "\$RUNNER_TEMP/sf06-http-client-guard.py" <<\x27PY\x27\n(.*?)^          PY\n', source)
    return textwrap.dedent(match[1])


def cargo_text(failed=None):
    names = diag.INVENTORY["tests"]["runtime-http-client"]
    rows = [f"test {name} ... {'FAILED' if name == failed else 'ok'}" for name in names]
    count = int(failed is not None)
    return ("running 221 tests\n" + "\n".join(rows)
            + f"\ntest result: {'FAILED' if count else 'ok'}. {221-count} passed; {count} failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s\n")


class DiagnosticTests(unittest.TestCase):
    def test_updated_source_boundaries_accept_exact_final_line_and_refuse_next_line(self):
        paths = ("runtime/src/auth/session_deadline_observer.rs", "runtime/src/auth/session_native_finish.rs",
                 "runtime/src/http_server/browser_login.rs", "runtime/src/sqlite/mod.rs")
        name = diag.INVENTORY["tests"]["runtime-http-client"][0]
        for path in paths:
            end = len((ROOT / path).read_text().splitlines())
            self.assertEqual(diag.INVENTORY["panic_sources"][path], end)
            for line, accepted in ((end, True), (end + 1, False)):
                text = cargo_text(name) + f"thread 'owned' (123) panicked at /private/{path}:{line}:1:\n"
                record = diag.analyze("runtime-http-client", 101, 1, text, "ok")
                self.assertEqual(bool(record["panic_sources"]), accepted, (path, line))
                self.assertEqual(record["unknown_panics"], int(not accepted))
                self.assertNotIn("/private/", json.dumps(record))

    def test_inventory_is_closed_and_preserves_the_existing_guard(self):
        source = WORKFLOW.read_text()
        tree = ast.parse(guard_source(source))
        namespace = {}
        assignments = [node for node in tree.body if isinstance(node, ast.Assign)
                       and any(isinstance(n, ast.Name) and n.id in ("RUNTIME_CASES", "SUITES") for n in node.targets)]
        exec(compile(ast.Module(body=assignments, type_ignores=[]), "inventory", "exec"), namespace)
        self.assertEqual(len(diag.INVENTORY["tests"]["runtime-http-client"]), 221)
        for name, (count, exact, required) in namespace["SUITES"].items():
            inventory = set(diag.INVENTORY["tests"][name])
            self.assertEqual(len(inventory), count)
            if exact is not None:
                self.assertEqual(inventory, exact)
            if required is not None:
                self.assertTrue(required.issubset(inventory))
        for name, lines in diag.INVENTORY["panic_sources"].items():
            self.assertEqual(len((ROOT / name).read_text().splitlines()), lines)

    def test_failure_reports_only_known_ids_locations_and_codes(self):
        name = diag.INVENTORY["tests"]["runtime-http-client"][0]
        text = cargo_text(name) + (
            f"test {PUBLIC_FIXTURE_MARKER} ... FAILED\nthread '{PUBLIC_FIXTURE_MARKER}' panicked at C:\\private\\runtime\\src\\http_client.rs:12:4:\n"
            f"assertion left={PUBLIC_FIXTURE_MARKER}; Authorization: Bearer {PUBLIC_FIXTURE_MARKER}\nerror[E0308]: {PUBLIC_FIXTURE_MARKER}\n"
            f"  process didn't exit successfully: `{PUBLIC_FIXTURE_MARKER}` (exit code: 0xc0000005, STATUS_ACCESS_VIOLATION)\n")
        record = diag.analyze("runtime-http-client", 101, 1, text, "ok")
        self.assertEqual(record["failed_test_ids"], [name])
        self.assertEqual(record["panic_sources"], [{"file": "runtime/src/http_client.rs", "line": 12, "column": 4}])
        self.assertEqual(record["rust_error_codes"], ["E0308"])
        self.assertEqual(record["process_terminations"], ["windows_access_violation"])
        self.assertEqual(record["unknown_test_rows"], 1)
        self.assertNotIn(PUBLIC_FIXTURE_MARKER, json.dumps(record))
        self.assertNotIn("private", json.dumps(record))

    def test_unrecognized_or_out_of_range_panic_does_not_publish_text(self):
        text = f"thread '{PUBLIC_FIXTURE_MARKER}' panicked at /host/{PUBLIC_FIXTURE_MARKER}.rs:1:1:\n{PUBLIC_FIXTURE_MARKER}\n"
        text += "thread 'worker' panicked at runtime/src/http_client.rs:999999:1:\n"
        record = diag.analyze("runtime-http-client", 101, 1, text, "ok")
        self.assertEqual(record["panic_sources"], [])
        self.assertEqual(record["unknown_panics"], 2)
        self.assertNotIn(PUBLIC_FIXTURE_MARKER, json.dumps(record))

    def test_panic_location_accepts_optional_numeric_thread_id_and_crlf(self):
        expected = [{"file": "runtime/src/http_client.rs", "line": 12, "column": 4}]
        for suffix in ("", " (251248)", " (12345678901234567890)"):
            with self.subTest(suffix=suffix):
                text = (f"thread '{PUBLIC_FIXTURE_MARKER}'{suffix} panicked at C:\\private\\runtime\\src\\http_client.rs:12:4:\r\n"
                        f"assertion left={PUBLIC_FIXTURE_MARKER}; Cookie: {PUBLIC_FIXTURE_MARKER}\r\n")
                record = diag.analyze("runtime-http-client", 101, 1, text, "ok")
                self.assertEqual(record["panic_sources"], expected)
                self.assertEqual(record["unknown_panics"], 0)
                encoded = json.dumps(record)
                for private in (PUBLIC_FIXTURE_MARKER, "251248", "12345678901234567890", "private"):
                    self.assertNotIn(private, encoded)

    def test_malformed_thread_id_does_not_publish_a_location_or_text(self):
        for suffix in (" ()", " (123456789012345678901)", " (12abc)",
                       f" ({PUBLIC_FIXTURE_MARKER})", f" (12) {PUBLIC_FIXTURE_MARKER}", " (12\r\n34)"):
            with self.subTest(suffix=suffix):
                text = f"thread '{PUBLIC_FIXTURE_MARKER}'{suffix} panicked at runtime/src/http_client.rs:12:4:\r\n"
                record = diag.analyze("runtime-http-client", 101, 1, text, "ok")
                self.assertEqual(record["panic_sources"], [])
                self.assertNotIn(PUBLIC_FIXTURE_MARKER, json.dumps(record))

    def test_process_termination_is_a_closed_enum(self):
        text = (f"  process didn't exit successfully: `{PUBLIC_FIXTURE_MARKER}` (signal: 9, SIGKILL: {PUBLIC_FIXTURE_MARKER})\n"
                f"  process didn't exit successfully: `{PUBLIC_FIXTURE_MARKER}` (exit code: {PUBLIC_FIXTURE_MARKER})\n")
        record = diag.analyze("runtime-http-client", 101, 1, text, "ok")
        self.assertEqual(record["process_terminations"], ["unix_sigkill"])
        self.assertNotIn(PUBLIC_FIXTURE_MARKER, json.dumps(record))

    def test_publication_binds_only_a_new_completed_closed_file(self):
        record = diag.analyze("runtime-http-client", 101, 1, "", "ok")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            stale = root / "sf06-check-closed-stale"
            stale.mkdir()
            (stale / "diagnostic.json").write_text(PUBLIC_FIXTURE_MARKER)
            output = root / "output"
            with patch.dict(os.environ, GITHUB_OUTPUT=str(output)):
                with patch.object(diag.tempfile, "mkdtemp", side_effect=OSError):
                    self.assertFalse(diag.publish(record, root))
                self.assertFalse(output.exists())
                self.assertTrue(diag.publish(record, root))
            header, path, end = output.read_text().splitlines()
            self.assertEqual(header, "closed_failure_diagnostic<<SF06_CLOSED_JSON")
            self.assertEqual(end, "SF06_CLOSED_JSON")
            self.assertNotEqual(Path(path).parent, stale)
            self.assertEqual(json.loads(Path(path).read_text()), record)
            self.assertEqual((stale / "diagnostic.json").read_text(), PUBLIC_FIXTURE_MARKER)
            with patch.dict(os.environ, GITHUB_OUTPUT=""):
                self.assertFalse(diag.publish(record, root))

    def test_read_bounds_and_nonregular_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertEqual(diag.bounded_read(root / "missing")[1], "unreadable")
            self.assertEqual(diag.bounded_read(root)[1], "not_regular")
            path = root / "log"
            path.write_bytes(b"x" * (diag.LIMIT + 1))
            self.assertEqual(diag.bounded_read(path)[1], "oversize")
            path.write_bytes(b"\xff")
            self.assertEqual(diag.bounded_read(path)[1], "invalid_utf8")

    def test_complete_and_malformed_records_are_distinct(self):
        self.assertEqual(diag.analyze("runtime-http-client", 0, 1, cargo_text(), "ok")["parse_state"], "complete")
        for text in ("", cargo_text().replace("221 passed", "220 passed"), cargo_text() + cargo_text()):
            self.assertEqual(diag.analyze("runtime-http-client", 0, 1, text, "ok")["parse_state"], "incomplete")
        self.assertEqual(diag.analyze(PUBLIC_FIXTURE_MARKER, 101, 1, PUBLIC_FIXTURE_MARKER, "ok")["suite"], "unknown")


class WorkflowTests(unittest.TestCase):
    def invoke(self, text, cargo_exit, *, remove=False, diagnostic_missing=False, guard_status=1):
        source = WORKFLOW.read_text()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            guard = guard_source(source)
            if guard_status != 1:
                guard = guard.replace("raise SystemExit(1)", f"raise SystemExit({guard_status})")
            (root / "sf06-http-client-guard.py").write_text(guard)
            (root / "payload").write_bytes(text.encode())
            output = root / "github-output"
            output.write_text("")
            fake = root / "fake.py"
            fake.write_text("import os,sys,pathlib\n"
                            "pathlib.Path(os.environ['OBSERVED']).write_text(str('GITHUB_OUTPUT' in os.environ))\n"
                            "if 'GITHUB_OUTPUT' in os.environ: pathlib.Path(os.environ['GITHUB_OUTPUT']).write_text('closed_failure_diagnostic=UNVALIDATED\\n')\n"
                            "sys.stdout.buffer.write(pathlib.Path(os.environ['PAYLOAD']).read_bytes())\n"
                            "if os.environ['REMOVE']=='1': pathlib.Path(os.environ['RUNNER_TEMP'],'sf06-runtime-http-client.log').unlink()\n"
                            "raise SystemExit(int(os.environ['CARGO_STATUS']))\n")
            wrapper = re.search(r"(?ms)^          run_check\(\) \{.*?^          \}\n", source)[0]
            if diagnostic_missing:
                wrapper = wrapper.replace("scripts/ci/sf06_http_client_failure_diagnostic.py", "missing-closed-diagnostic.py")
            script = "set -euo pipefail\n" + textwrap.dedent(wrapper)
            script += "run_check runtime-http-client " + shlex.quote(sys.executable) + " " + shlex.quote(str(fake)) + "\n"
            env = dict(os.environ, RUNNER_TEMP=str(root), GITHUB_OUTPUT=str(output),
                       PAYLOAD=str(root / "payload"), OBSERVED=str(root / "observed"),
                       REMOVE=str(int(remove)), CARGO_STATUS=str(cargo_exit))
            result = subprocess.run(["bash", "-c", script], cwd=ROOT, env=env, capture_output=True, text=True, timeout=30)
            bound = output.read_text()
            record = None
            if bound:
                key, path, end = bound.splitlines()
                self.assertEqual(key, "closed_failure_diagnostic<<SF06_CLOSED_JSON")
                self.assertEqual(end, "SF06_CLOSED_JSON")
                self.assertTrue(Path(path).is_relative_to(root))
                self.assertNotEqual(Path(path).parent, root)
                record = json.loads(Path(path).read_text())
            self.assertEqual((root / "observed").read_text(), "False")
            self.assertNotIn(PUBLIC_FIXTURE_MARKER, result.stdout + result.stderr + json.dumps(record))
            return result, record

    def test_success_preserves_guard_pass_without_diagnostic(self):
        result, record = self.invoke(cargo_text() + PUBLIC_FIXTURE_MARKER, 0)
        self.assertEqual(result.returncode, 0)
        self.assertIsNone(record)

    def test_cargo_failure_is_not_repaired_by_a_complete_log(self):
        result, record = self.invoke(cargo_text() + PUBLIC_FIXTURE_MARKER, 101)
        self.assertEqual(result.returncode, 1)
        self.assertEqual((record["cargo_exit"], record["guard_exit"]), (101, 1))

    def test_actual_guard_status_is_returned_even_if_diagnostic_invocation_fails(self):
        for missing in (False, True):
            result, record = self.invoke(PUBLIC_FIXTURE_MARKER, 101, diagnostic_missing=missing, guard_status=7)
            self.assertEqual(result.returncode, 7)
            if not missing:
                self.assertEqual(record["guard_exit"], 7)

    def test_nonzero_guard_and_closed_failure_survive_all_diagnostic_cases(self):
        failed = diag.INVENTORY["tests"]["runtime-http-client"][0]
        for text, remove, state in [(cargo_text(failed), False, "ok"),
                                    (PUBLIC_FIXTURE_MARKER * (diag.LIMIT // len(PUBLIC_FIXTURE_MARKER) + 1), False, "oversize"),
                                    (PUBLIC_FIXTURE_MARKER, True, "unreadable")]:
            with self.subTest(state=state):
                result, record = self.invoke(text, 101, remove=remove)
                self.assertEqual(result.returncode, 1)
                self.assertEqual(record["read_state"], state)
        result, record = self.invoke(PUBLIC_FIXTURE_MARKER, 101, diagnostic_missing=True)
        self.assertEqual(result.returncode, 1)
        self.assertIsNone(record)
        self.assertIn("reason=invocation_error", result.stdout)

    def test_strict_guard_inventory_failure_still_fails(self):
        result, record = self.invoke(cargo_text().replace("221 passed", "220 passed"), 0)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(record["cargo_exit"], 0)
        self.assertEqual(record["parse_state"], "incomplete")

    def test_full_frozen_inverse_and_rejection_of_weakened_hooks(self):
        source = WORKFLOW.read_text()
        self.assertEqual(hashlib.sha256(history.inverse_workflow(source).encode()).hexdigest(), BASE_WORKFLOW_SHA256)
        for old, new in [(projection.OBSERVED, projection.ORIGINAL),
                         (projection.OBSERVED, projection.OBSERVED + projection.OBSERVED),
                         (projection.WITH_ID, projection.HEADER),
                         ('return "$guard_status"', 'return 0'),
                         ('unset GITHUB_OUTPUT; "$@"', '"$@"'),
                         ('steps.sf06_public_client.outputs.closed_failure_diagnostic', 'runner.temp'),
                         (projection.OBSERVED_ARTIFACT, projection.OBSERVED_ARTIFACT + '            ${{ runner.temp }}/sf06-runtime-http-client.log\n'),
                         ('"runtime-http-client": (221,', '"runtime-http-client": (220,')]:
            with self.subTest(old=old[:40]):
                mutant = source.replace(old, new, 1)
                try:
                    restored = history.inverse_workflow(mutant)
                except ValueError:
                    continue
                self.assertNotEqual(hashlib.sha256(restored.encode()).hexdigest(), BASE_WORKFLOW_SHA256)


if __name__ == "__main__":
    unittest.main()
