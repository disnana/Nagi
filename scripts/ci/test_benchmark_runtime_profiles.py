"""Validate runtime-profile measurements without invoking Cargo."""
from __future__ import annotations

import contextlib
import copy
import io
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import benchmark_runtime_profiles as bench


class RuntimeProfileBenchmarkTests(unittest.TestCase):
    def sample(self, mode="auto"):
        return {"exit_code": 0, "stats": {
            "profile": "dev", "checks": "full", "cargo_always_invoked": True,
            "native_reuse": "cargo_only", "generated_body": {"mode": "monolithic"},
            "cargo_fresh_artifacts": 12, "cargo_compiled_artifacts": 1,
            "runtime": {"requested": mode, "selected": "core" if mode == "auto" else "full",
                        "reason": "eligible_core" if mode == "auto" else "full_requested"}}}

    def test_fallback_or_weakened_build_cannot_be_a_core_success_sample(self):
        bench.validate_sample(self.sample(), "auto")
        bench.validate_sample(self.sample("full"), "full")
        variants = []
        failed = self.sample()
        failed["exit_code"] = 1
        variants.append(failed)
        for key, value in (("checks", "partial"), ("cargo_always_invoked", False),
                           ("profile", "release"), ("native_reuse", "nagi_cache"),
                           ("cargo_compiled_artifacts", None), ("cargo_fresh_artifacts", -1),
                           ("cargo_fresh_artifacts", True)):
            sample = self.sample()
            sample["stats"][key] = value
            variants.append(sample)
        fallback = self.sample()
        fallback["stats"]["runtime"] = {"requested": "auto", "selected": "full", "reason": "unsupported_ir"}
        variants.append(fallback)
        wrong_mode = self.sample()
        wrong_mode["stats"]["generated_body"]["mode"] = "cargo_units"
        variants.append(wrong_mode)
        for sample in variants:
            with self.subTest(sample=sample), self.assertRaises(RuntimeError):
                bench.validate_sample(sample, "auto")

    def test_api_and_body_changes_have_distinct_source_contracts(self):
        hello = bench.Fixture("hello")
        self.assertEqual(hello.expected(), "Hello, Nagi!\n42\n")
        hello.edit("bodyedit")
        self.assertEqual(hello.expected(), "Hello, Nagi!\n43\n")
        hello.edit("public_api_edit")
        self.assertIn("def value() -> f64:", hello.source())
        self.assertIn("return 42.0", hello.source())
        self.assertEqual(hello.expected(), "Hello, Nagi!\n43\n")
        point = bench.Fixture("point")
        self.assertEqual(point.expected(), "4\n")
        original = point.source()
        point.edit("bodyedit")
        self.assertEqual(point.expected(), "5\n")
        self.assertNotEqual(point.source(), original)
        point.edit("public_api_edit")
        self.assertIn("z: i64", point.source())
        self.assertIn("z=7", point.source())
        self.assertEqual(point.expected(), "5\n")
        with self.assertRaises(ValueError):
            bench.Fixture("unknown").source()

    def test_existing_reports_and_unknown_build_switches_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            report = root / "report.json"
            report.write_bytes(b"retained evidence")
            with (patch.dict(os.environ, {}, clear=True), contextlib.redirect_stderr(io.StringIO()),
                  self.assertRaises(SystemExit)):
                bench.main(["--compiler", "missing", "--work-dir", str(root), "--output", str(report)])
            self.assertEqual(report.read_bytes(), b"retained evidence")
        for extra in (["--repeat", "2"], ["--jobs", "0"], ["--skip-checks"]):
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                bench.main(["--compiler", "missing", "--work-dir", "unused", "--output", "unused", *extra])

    def test_summary_keeps_corrupt_failure_recovery_and_modes_separate(self):
        configurations = []
        for fixture in ("hello", "point"):
            for mode in ("full", "auto"):
                for wall in (3, 1, 2):
                    samples = {case: {"wall_s": wall} for case in bench.CASES}
                    samples["corrupt_cache"] = {"rejected": {"wall_s": wall / 10},
                                                "recovery": {"wall_s": wall}}
                    configurations.append({"fixture": fixture, "mode": mode, "samples": samples})
        original = copy.deepcopy(configurations)
        summary = bench.summaries(configurations)
        self.assertEqual(summary["hello"]["full"]["clean"]["wall_s"]["median"], 2)
        self.assertEqual(summary["point"]["auto"]["corrupt_cache"]["rejected"]["wall_s"]["median"], .2)
        self.assertEqual(summary["point"]["auto"]["clean"]["count"], 3)
        self.assertIsNone(summary["point"]["auto"]["clean"]["cpu_s"])
        self.assertEqual(configurations, original)

    def test_different_closures_retain_seed_versions_and_cannot_drift(self):
        a = ("base", "1.0", "registry", "hash-a")
        b = ("http", "2.0", "registry", "hash-b")
        report = {"seed_external_packages": [a, b]}
        bench.record_dependencies(report, "full", [a, b])
        bench.record_dependencies(report, "auto", [a])
        bench.record_dependencies(report, "auto", [a])
        self.assertEqual(report["external_packages"]["auto"], [a])
        with self.assertRaisesRegex(RuntimeError, "seed lock"):
            bench.record_dependencies(report, "auto", [("base", "1.1", "registry", "new-hash")])
        with self.assertRaisesRegex(RuntimeError, "one runtime mode"):
            bench.record_dependencies(report, "auto", [a, b])
        with self.assertRaisesRegex(RuntimeError, "subset"):
            bench.record_dependencies({"seed_external_packages": [a, b], "external_packages": {"full": [a]}},
                                      "auto", [a, b])


if __name__ == "__main__":
    unittest.main()
