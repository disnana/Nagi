from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import sys
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).resolve().parents[1] / "ci" / "sf07_profile_summary.py"
SPEC = importlib.util.spec_from_file_location("sf07_profile_summary", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
SUMMARY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SUMMARY)


class ProfileSummaryTests(unittest.TestCase):
    def record(self):
        samples = list(range(1, 33))
        return {
            "schema": "nagi.sf07.small_profile.v1",
            "platform": "linux",
            "architecture": "x86_64",
            "fixture": {
                "transport": "loopback_http1",
                "route_count": 4,
                "connection_slots": 8,
                "request_slots": 8,
                "client_concurrency": 1,
                "authority_entry_limit": 1,
                "authority_byte_limit": 128,
                "other_server_options": "default_options; not introspected",
                "response_limits": {
                    "body_bytes": 4096,
                    "user_header_bytes": 4096,
                    "user_header_count": 16,
                    "managed_header_bytes": 4096,
                    "managed_header_count": 32,
                    "serialization_writes": 4096,
                    "live_responses": 8,
                    "retained_bytes": 2 * 1024 * 1024,
                    "fallback_responses": 8,
                    "fallback_retained_bytes": 2 * 1024 * 1024,
                    "fallback_managed_header_bytes": 4096,
                    "fallback_managed_header_count": 32,
                },
                "rate_limits": {
                    "rules": 8,
                    "domains": 8,
                    "subject_slots": 1,
                    "retained_bytes": 1024 * 1024,
                    "request_work": 65536,
                },
                "route_policies": [
                    {"path": "/ordinary", "burst": 1000, "refill_ms": 1000},
                    {"path": "/headers", "burst": 1000, "refill_ms": 1000},
                    {"path": "/limited", "burst": 1, "refill_ms": 86_400_000},
                    {"path": "/healthy", "burst": 1000, "refill_ms": 1000},
                ],
                "header_heavy_value_bytes": 96,
            },
            "type_sizes_bytes": {
                "response_context": 8,
                "response": 8,
                "response_limits": 8,
                "rate_limits": 8,
                "rate_policy": 8,
            },
            "observed": {
                "warmup_requests": 4,
                "ordinary_requests": 32,
                "ordinary_latency_ns": samples,
                "ordinary_median_ns": 16,
                "ordinary_p95_ns": 31,
                "header_heavy_status": 200,
                "header_heavy_custom_header_count": 12,
                "header_heavy_custom_header_value_bytes": 12 * 96,
                "header_heavy_body_bytes": 7,
                "header_heavy_latency_ns": 101,
                "limited_accepted_latency_ns": 102,
                "rate_limited_latency_ns": 103,
                "rate_limited_status": 429,
                "rate_limited_retry_after_seconds": 86_400,
                "healthy_after_rate_fallback_status": 200,
                "healthy_after_rate_fallback_latency_ns": 104,
                "fallback_retained_high_water": None,
                "retained_high_water_note": "not exposed by the public API",
                "shutdown_joined": True,
                "allocation_scope": SUMMARY.ALLOCATION_SCOPE,
                "calling_thread_allocations": {
                    "allocations": 100,
                    "reallocations": 1,
                    "allocated_bytes": 4096,
                    "deallocations": 90,
                },
            },
        }

    def capture(self, record=None, *, test_line=None, summary=None):
        value = self.record() if record is None else record
        encoded = json.dumps(value, separators=(",", ":"))
        return (
            "    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.01s\n"
            "     Running tests/sf07_small_profile.rs (target/debug/deps/sf07_small_profile-0123456789abcdef)\n"
            "\n"
            "running 1 test\n"
            + (test_line or f"test {SUMMARY.TEST_NAME} ... ok")
            + "\n\nsuccesses:\n\n"
            + f"---- {SUMMARY.TEST_NAME} stdout ----\n"
            + "SF07_PROFILE_JSON=" + encoded + "\n\n\n"
            + "successes:\n"
            + f"    {SUMMARY.TEST_NAME}\n\n"
            + (summary or "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s")
            + "\n\n"
        ).encode()

    def test_accepts_one_valid_measurement(self):
        parsed = SUMMARY.parse_capture(self.capture())
        self.assertEqual(parsed["observed"]["ordinary_requests"], 32)

    def test_rejects_zero_tests_and_missing_marker(self):
        with self.assertRaises(ValueError):
            SUMMARY.parse_capture(b"running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n")

    def test_rejects_duplicate_record_and_duplicate_json_key(self):
        valid = self.capture().decode()
        marker = next(line for line in valid.splitlines() if line.startswith("SF07_PROFILE_JSON="))
        with self.assertRaises(ValueError):
            SUMMARY.parse_capture((valid + marker + "\n").encode())
        duplicate_key = marker[:-1] + ',"schema":"forged"}'
        with self.assertRaises(ValueError):
            SUMMARY.parse_capture(("running 1 test\n" + duplicate_key + "\ntest " + SUMMARY.TEST_NAME + " ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n").encode())

    def test_rejects_nonzero_or_incomplete_cargo_result(self):
        with self.assertRaises(ValueError):
            SUMMARY.parse_capture(self.capture(summary="test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s"))
        with self.assertRaises(ValueError):
            SUMMARY.parse_capture(self.capture(test_line=f"test {SUMMARY.TEST_NAME} ... FAILED"))

    def test_rejects_foreign_failed_ignored_unknown_and_duplicate_rows_or_summaries(self):
        valid = self.capture().decode()
        summary = "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s"
        invalid_captures = (
            valid.replace(
                f"test {SUMMARY.TEST_NAME} ... ok\n",
                f"test {SUMMARY.TEST_NAME} ... ok\ntest foreign_case ... FAILED\n",
            ),
            valid.replace(
                f"test {SUMMARY.TEST_NAME} ... ok\n",
                f"test {SUMMARY.TEST_NAME} ... ok\ntest foreign_case ... ignored\n",
            ),
            valid.replace(
                f"test {SUMMARY.TEST_NAME} ... ok\n",
                f"test {SUMMARY.TEST_NAME} ... ok\ntest {SUMMARY.TEST_NAME} ... ok\n",
            ),
            valid.replace(summary, summary + "\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s"),
            valid.replace(summary, summary + "\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s"),
            valid.replace("running 1 test\n", "running 2 tests\n"),
            valid.replace("successes:\n\n", "successes:\ntest foreign_case ... FAILED\n\n", 1),
            valid.replace("\n\n", "\nunknown output\n\n", 1),
        )
        for invalid in invalid_captures:
            with self.subTest(capture=invalid[:100]):
                with self.assertRaises(ValueError):
                    SUMMARY.parse_capture(invalid.encode())

    def test_rejects_bool_and_float_in_fixed_and_computed_integer_fields(self):
        mutations = (
            lambda record: record["fixture"]["rate_limits"].__setitem__("subject_slots", True),
            lambda record: record["fixture"]["response_limits"].__setitem__("body_bytes", 4096.0),
            lambda record: record["fixture"]["route_policies"][0].__setitem__("burst", 1000.0),
            lambda record: record["observed"].__setitem__("ordinary_requests", 32.0),
            lambda record: record["observed"].__setitem__("ordinary_median_ns", 16.0),
            lambda record: record["observed"]["calling_thread_allocations"].__setitem__("allocations", 100.0),
        )
        for mutate in mutations:
            broken = self.record()
            mutate(broken)
            with self.subTest(record=broken):
                with self.assertRaises(ValueError):
                    SUMMARY.parse_capture(self.capture(broken))
        with self.assertRaises(ValueError):
            SUMMARY.parse_capture(self.capture(summary="test result: ok. 1.0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s"))

    def test_rejects_nonfinite_or_mismatched_profile(self):
        raw = self.capture().decode().replace('"ordinary_median_ns":16', '"ordinary_median_ns":NaN')
        with self.assertRaises(ValueError):
            SUMMARY.parse_capture(raw.encode())
        broken = self.record()
        broken["observed"]["ordinary_requests"] = 0
        with self.assertRaises(ValueError):
            SUMMARY.parse_capture(self.capture(broken))

    def test_rejects_wrong_json_types_without_traceback_path(self):
        broken = self.record()
        broken["platform"] = []
        with self.assertRaises(ValueError):
            SUMMARY.parse_capture(self.capture(broken))

    def test_rejects_oversize_capture(self):
        with self.assertRaises(ValueError):
            SUMMARY.parse_capture(b"x" * (SUMMARY.MAX_CAPTURE_BYTES + 1))

    def test_accepts_crlf_and_rejects_bare_cr_separator(self):
        valid = self.capture()
        self.assertEqual(SUMMARY.parse_capture(valid.replace(b"\n", b"\r\n"))["schema"], "nagi.sf07.small_profile.v1")
        with self.assertRaises(ValueError):
            SUMMARY.parse_capture(valid.replace(b"\n", b"\r", 1))

    def test_cli_emits_only_validated_record(self):
        env = dict(os.environ)
        env["PYTHONDONTWRITEBYTECODE"] = "1"
        process = subprocess.run(
            [sys.executable, str(MODULE_PATH), "--cargo-exit-code", "0"],
            input=self.capture(),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
            timeout=5,
            check=False,
        )
        self.assertEqual(process.returncode, 0)
        self.assertEqual(process.stderr, b"")
        self.assertTrue(process.stdout.startswith(b"SF07_PROFILE_ACCEPTED={"))

    def test_cli_rejects_nonzero_cargo_exit_even_with_valid_test_output(self):
        env = dict(os.environ)
        env["PYTHONDONTWRITEBYTECODE"] = "1"
        process = subprocess.run(
            [sys.executable, str(MODULE_PATH), "--cargo-exit-code", "101"],
            input=self.capture(),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
            timeout=5,
            check=False,
        )
        self.assertEqual(process.returncode, 2)
        self.assertEqual(process.stdout, b"")
        self.assertEqual(process.stderr, b"sf07_profile=unavailable\n")

    def test_cli_fails_closed_without_echoing_invalid_input(self):
        env = dict(os.environ)
        env["PYTHONDONTWRITEBYTECODE"] = "1"
        process = subprocess.run(
            [sys.executable, str(MODULE_PATH), "--cargo-exit-code", "0"],
            input=b"sentinel-must-not-echo\n",
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
            timeout=5,
            check=False,
        )
        self.assertEqual(process.returncode, 2)
        self.assertEqual(process.stdout, b"")
        self.assertEqual(process.stderr, b"sf07_profile=unavailable\n")


if __name__ == "__main__":
    unittest.main()
