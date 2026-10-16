"""Source and synthetic host/inventory controls; Rust/native execution is separate."""
import ast
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "scripts/ci"))
import sf07_owned_state_summary as gate


def green(names):
    names = sorted(names)
    count = len(names)
    return (f"running {count} tests\n" + "".join("test " + n + " ... ok\n" for n in names)
            + f"test result: ok. {count} passed; 0 failed; 0 ignored; 0 measured; 999 filtered out; finished in 0.01s\n").encode()


def projection():
    spec = importlib.util.spec_from_file_location("owned_projection", ROOT / "scripts/ci/sf07_request_projection.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class OwnedStateSummaryTests(unittest.TestCase):
    def test_host_is_fixed_to_python_runner_and_no_target_override(self):
        for platform, runner in (("linux", "Linux"), ("win32", "Windows"), ("darwin", "macOS")):
            with patch.object(sys, "platform", platform), patch.dict(os.environ, {"RUNNER_OS": runner}, clear=True):
                self.assertEqual(gate.native_host(), runner)
                for target in ("", "x86_64-unknown-linux-gnu", "aarch64-apple-darwin", "unknown"):
                    with patch.dict(os.environ, {"CARGO_BUILD_TARGET": target}):
                        self.assertIsNone(gate.native_host())
            for environment in ({}, {"RUNNER_OS": "unknown"}, {"RUNNER_OS": runner.lower()},
                                {"RUNNER_OS": "Windows" if runner != "Windows" else "Linux"}):
                with patch.object(sys, "platform", platform), patch.dict(os.environ, environment, clear=True):
                    self.assertIsNone(gate.native_host())
                    self.assertIsNone(gate.expected("lock"))
        with patch.object(sys, "platform", "unknown"), patch.dict(os.environ, {"RUNNER_OS": "Linux"}, clear=True):
            self.assertIsNone(gate.native_host())

    def test_handwritten_names_match_eight_actual_source_cases_and_mac_cfg(self):
        expected = {
            "lock": {
                "cold_first_lock_unlock_and_drop_have_no_owned_heap",
                "arc_receipt_covers_cold_lock_and_last_owner_releases_payload_once",
                "fresh_concurrent_writers_keep_all_updates_and_one_final_payload",
                "writer_poison_recovers_for_disposal_and_stays_sticky_until_drop",
            },
            "owners": {
                "quota_cold_receipt_matches_allocated_payload_and_claim_keeps_final_owner",
                "rate_cold_receipt_contains_state_and_short_budget_refuses_before_allocation",
            },
            "send": {"poisoned_ledger_disposal_keeps_native_cache_and_permit_release_order"},
        }
        paths = {
            "lock": ("runtime/src/security_sf07_private/owned_state_lock_tests.rs", "security_sf07_private::owned_state_lock::tests::"),
            "owners": ("runtime/src/security_sf07_private/owned_state_owner_tests.rs", "security_sf07_private::owned_state_owner_tests::"),
            "send": ("runtime/src/http_server/response_send_owned_state_tests.rs", "http_server::response_send::owned_state_lock_tests::"),
        }
        control = "cold_std_mutex_control_retains_native_heap_until_owner_drop"
        for mode, (path, prefix) in paths.items():
            source = (ROOT / path).read_text()
            names = re.findall(r"#\[test\]\s*fn (\w+)\(", source)
            all_names = expected[mode] | ({control} if mode == "lock" else set())
            self.assertEqual(set(names), all_names)
            self.assertEqual(len(names), len(all_names))
            self.assertEqual(set(gate.COMMON[mode]), {prefix + n for n in expected[mode]})
            self.assertNotIn("#[ignore", source)
        source = (ROOT / paths["lock"][0]).read_text()
        self.assertIn('#[cfg(target_os = "macos")]\n#[test]\nfn ' + control, source)
        self.assertEqual(gate.MAC_CONTROL, paths["lock"][1] + control)
        self.assertIn('#[path = "owned_state_lock_tests.rs"]\nmod tests;',
                      (ROOT / "runtime/src/security_sf07_private/owned_state_lock.rs").read_text())
        self.assertIn('#[cfg(test)]\nmod owned_state_owner_tests;',
                      (ROOT / "runtime/src/security_sf07_private/mod.rs").read_text())
        self.assertIn('#[path = "response_send_owned_state_tests.rs"]\nmod owned_state_lock_tests;',
                      (ROOT / "runtime/src/http_server/response_send.rs").read_text())

    def test_each_host_lf_crlf_and_mac_control_cannot_be_omitted_or_required_elsewhere(self):
        for platform, runner in (("linux", "Linux"), ("win32", "Windows"), ("darwin", "macOS")):
            with patch.object(sys, "platform", platform), patch.dict(os.environ, {"RUNNER_OS": runner}, clear=True):
                self.assertEqual(tuple(len(gate.expected(mode)) for mode in ("lock", "owners", "send", "private")),
                                 (5, 2, 1, 7) if platform == "darwin" else (4, 2, 1, 6))
                for mode in gate.COMMON:
                    raw = green(gate.expected(mode))
                    self.assertTrue(gate.accepts(raw, mode))
                    self.assertTrue(gate.accepts(raw.replace(b"\n", b"\r\n"), mode))
                self.assertEqual(gate.MAC_CONTROL in gate.expected("private"), platform == "darwin")
                wrong = set(gate.expected("lock")) ^ {gate.MAC_CONTROL}
                self.assertFalse(gate.accepts(green(wrong), "lock"))

    def test_zero_missing_duplicate_ignored_wrong_filter_and_nonzero_exit_fail(self):
        with patch.object(sys, "platform", "darwin"), patch.dict(os.environ, {"RUNNER_OS": "macOS"}, clear=True):
            for mode in gate.COMMON:
                names = gate.expected(mode)
                raw = green(names)
                first = sorted(names)[0].encode()
                row = b"test " + first + b" ... ok\n"
                for value in (b"", green([]), raw.replace(row, b""), raw + row,
                              raw.replace(first, b"foreign::case"), raw.replace(b"... ok", b"... ignored", 1),
                              raw.replace(b"0 ignored", b"1 ignored"), raw + b"test malformed\n", raw + raw):
                    self.assertFalse(gate.accepts(value, mode))
                for code in (-1, 1, 101, True):
                    self.assertFalse(gate.accepts(raw, mode, code))
                other = "owners" if mode != "owners" else "lock"
                self.assertFalse(gate.accepts(raw, other))
            self.assertFalse(gate.accepts(green(gate.expected("lock")), "unknown"))

    def test_bounds_encoding_and_cli_closed_failure(self):
        host = {"linux": "Linux", "win32": "Windows", "darwin": "macOS"}[sys.platform]
        environment = dict(os.environ, RUNNER_OS=host)
        environment.pop("CARGO_BUILD_TARGET", None)
        with patch.dict(os.environ, environment, clear=True):
            raw = green(gate.expected("lock"))
            padded = raw + b" " * (gate.MAX_BYTES - len(raw))
            self.assertTrue(gate.accepts(padded, "lock"))
            for value in (padded + b"x", raw + b"\xff", raw + b"\r", raw + b"\x1b[0m"):
                self.assertFalse(gate.accepts(value, "lock"))
            stream = io.BytesIO(padded + b"overflow" * 10000)
            self.assertEqual(len(gate.capture(stream)), gate.MAX_BYTES + 1)
            self.assertEqual(stream.read(), b"")
        script = str(ROOT / "scripts/ci/sf07_owned_state_summary.py")
        for args, value, code, label in ((["lock"], raw, 0, "lock"), (["lock"], b"SECRET/path", 1, "lock"),
                                        (["unknown/SECRET"], raw, 1, "invalid"), (["lock", "--non-mac"], raw, 1, "invalid")):
            result = subprocess.run([sys.executable, "-B", script, *args], input=value, capture_output=True, env=environment, timeout=15)
            self.assertEqual(result.returncode, code)
            self.assertEqual(result.stdout.replace(b"\r\n", b"\n"),
                             f"sf07_owned_state scope={label} state={'passed' if code == 0 else 'invalid'}\n".encode())
            self.assertEqual(result.stderr, b"")
        for overrides in ({"RUNNER_OS": "unknown"}, {"CARGO_BUILD_TARGET": ""}, {"CARGO_BUILD_TARGET": "aarch64-apple-darwin"}):
            result = subprocess.run([sys.executable, "-B", script, "lock"], input=raw, capture_output=True,
                                    env=dict(environment, **overrides), timeout=15)
            self.assertEqual(result.returncode, 1)
            self.assertEqual(result.stdout.replace(b"\r\n", b"\n"), b"sf07_owned_state scope=lock state=invalid\n")
            self.assertEqual(result.stderr, b"")

    def test_unchanged_83_modes_remain_required_beside_new_exact_filters(self):
        spec = importlib.util.spec_from_file_location("old83", ROOT / "scripts/ci/sf07_checkpoint_summary.py")
        summary = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(summary)
        self.assertEqual(sum(map(len, summary.EXPECTED.values())), 83)
        self.assertEqual(hashlib.sha256(json.dumps(summary.EXPECTED, sort_keys=True, separators=(",", ":")).encode()).hexdigest(),
                         "643c31aa2c66ac769a3cd478c5bdd57f324f058ba9a314352927b0e63dc4409d")
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        for mode in summary.EXPECTED:
            self.assertEqual(workflow.count("| python -B scripts/ci/sf07_checkpoint_summary.py " + mode + "\n"), 2)
        self.assertEqual(workflow.count("python -B scripts/tests/test_sf07_owned_state_summary.py\n"), 2)
        filters = {"lock": "security_sf07_private::owned_state_lock::tests::", "owners": "security_sf07_private::owned_state_owner_tests::",
                   "send": "http_server::response_send::owned_state_lock_tests::"}
        for mode, filter_ in filters.items():
            command = f"cargo test --locked -p nagi-runtime --lib {filter_} 2>&1 | python -B scripts/ci/sf07_owned_state_summary.py {mode}\n"
            self.assertEqual(workflow.count(command), 2)
        self.assertEqual(workflow.count("cargo test --locked -p nagi-runtime --lib native_finish 2>&1 | python -B scripts/ci/session_native_finish_summary.py\n"), 2)

    def test_workflow_inverse_removes_only_exact_additions_and_preserves_frozen_hash(self):
        p = projection()
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        self.assertEqual(hashlib.sha256(p.inverse_workflow(workflow).encode()).hexdigest(),
                         "5d7923afbefb4108ccfaedfb2441cf8832aea38d25a683fd6cc1d62400783eb7")
        for addition in p.OWNED_STATE_CI_ADDITIONS:
            for changed in (workflow.replace(addition, "", 1), workflow.replace(addition, addition + addition, 1),
                            workflow.replace(addition, addition.replace("python", "python -c pass", 1), 1)):
                with self.assertRaises(ValueError):
                    p.inverse_workflow(changed)
        changed = workflow.replace("set -euo pipefail", "set -eu pipefail", 1)
        self.assertNotEqual(hashlib.sha256(p.inverse_workflow(changed).encode()).hexdigest(),
                            "5d7923afbefb4108ccfaedfb2441cf8832aea38d25a683fd6cc1d62400783eb7")

    def test_quota_import_inverse_is_exact_and_retains_all_old_semantic_bytes(self):
        p = projection()
        source = (ROOT / "runtime/src/security_sf07_private/quota.rs").read_text()
        frozen = "a7031c5a4ad071d719f6efc41dd8889d9582169602197a337c94a54cbc4f6861"
        self.assertEqual(hashlib.sha256(p.inverse_quota(source).encode()).hexdigest(), frozen)
        alias = "use super::OwnedStateLock as Mutex;\n"
        arc = "use std::sync::Arc;\n"
        for changed in (source.replace(alias, "", 1), source + alias, source + arc,
                        source.replace("super::OwnedStateLock", "super::DifferentLock", 1),
                        source + "use std::sync::{Arc, Mutex};\n",
                        source.replace("if minimum > retained_limit {", "if false {", 1),
                        source.replace("    used: usize,", "    used: u16,", 1),
                        source.replace("state.faulted = true;", "state.faulted = false;", 1)):
            self.assertNotEqual(changed, source)
            try:
                restored = p.inverse_quota(changed)
            except ValueError:
                continue
            self.assertNotEqual(hashlib.sha256(restored.encode()).hexdigest(), frozen)

    def test_broad_catalog_preserves_frozen_55_plus_12_plus_3_and_adds_only_private_6_or_7(self):
        path = ROOT / "scripts/check_sf07_private_tests.py"
        source = path.read_text()
        tree = ast.parse(source)
        first = next(n for n in tree.body if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == "EXPECTED" for t in n.targets))
        additions = [n for n in tree.body if isinstance(n, ast.AugAssign) and isinstance(n.target, ast.Name) and n.target.id == "EXPECTED"]
        self.assertEqual(len(additions), 2)
        old = ast.literal_eval(first.value)
        rate, retained = [ast.literal_eval(n.value) for n in additions]
        self.assertEqual(tuple(map(len, (old, rate, retained))), (55, 12, 3))
        self.assertEqual(hashlib.sha256(json.dumps(old, separators=(",", ":")).encode()).hexdigest(),
                         "a9494c0c4080e9bf75d4c631f23e873fd37380ada1dffdf908f87c571a9df985")
        base = old + rate + retained
        code = compile(tree, str(path), "exec")
        for platform, runner, count in (("linux", "Linux", 76), ("win32", "Windows", 76), ("darwin", "macOS", 77)):
            with patch.object(sys, "platform", platform), patch.dict(os.environ, {"RUNNER_OS": runner}, clear=True):
                names = base + sorted(gate.expected("private"))
                self.assertEqual(len(set(names)), count)
                for raw, good in ((green(names), True), (green(base), False), (green(names[:-1]), False),
                                  (green(names + [names[0]]), False), (green(names + list(gate.COMMON["send"])), False),
                                  (green(names).replace(b"0 ignored", b"1 ignored"), False)):
                    with patch.object(Path, "open", return_value=io.BytesIO(raw)), patch.object(sys, "argv", ["checker", "fixture"]):
                        if good:
                            exec(code, {"__name__": "__main__"})
                        else:
                            with self.assertRaises(AssertionError):
                                exec(code, {"__name__": "__main__"})

    def test_actual_broad_cli_imports_without_ci_directory_in_python_path(self):
        path = ROOT / "scripts/check_sf07_private_tests.py"
        tree = ast.parse(path.read_text())
        first = next(n for n in tree.body if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == "EXPECTED" for t in n.targets))
        base = ast.literal_eval(first.value)
        for node in tree.body:
            if isinstance(node, ast.AugAssign) and isinstance(node.target, ast.Name) and node.target.id == "EXPECTED":
                base += ast.literal_eval(node.value)
        host = {"linux": "Linux", "win32": "Windows", "darwin": "macOS"}[sys.platform]
        environment = dict(os.environ, RUNNER_OS=host)
        environment.pop("CARGO_BUILD_TARGET", None)
        with patch.dict(os.environ, environment, clear=True):
            raw = green(base + sorted(gate.expected("private")))
        wrapper = '''import io,pathlib,runpy,sys
from unittest.mock import patch
script=sys.argv[1]
sys.path.insert(0,str(pathlib.Path(script).parent))
sys.argv=[script,"fixture"]
data=sys.stdin.buffer.read()
with patch.object(pathlib.Path,"open",return_value=io.BytesIO(data)):
 runpy.run_path(script,run_name="__main__")
'''
        for value, success in ((raw, True), (green(base), False), (b"SECRET/path", False)):
            result = subprocess.run([sys.executable, "-B", "-c", wrapper, str(path)], input=value,
                                    capture_output=True, env=environment, timeout=15)
            self.assertEqual(result.returncode == 0, success)
            self.assertEqual(result.stdout, b"")
            self.assertNotIn(b"SECRET", result.stderr)
            if success:
                self.assertEqual(result.stderr, b"")


if __name__ == "__main__":
    unittest.main()
