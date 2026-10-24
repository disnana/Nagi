"""Pure harness checks: no Cargo, Nagi compiler, or native build is invoked."""
from __future__ import annotations

import contextlib
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import benchmark_smart_build_matrix as matrix


class MatrixTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def out(self):
        out = self.root / "out"
        out.mkdir()
        (out / "Cargo.lock").write_bytes(b"original lock bytes\n")
        app = out / ".nagi/apps/application"
        app.mkdir(parents=True)
        (app / "latest.json").write_bytes(b"original latest bytes\n")
        return out

    def test_eight_cumulative_cases_have_independent_expected_results(self):
        self.assertEqual(len(matrix.CASES), 8)
        fixture = matrix.Fixture(100)
        expected = [4999, 4999, 6000, 6000, 7005, 7013, 7013]
        snapshots = []
        for case, value in zip(matrix.CASES, expected):
            fixture.edit(case)
            self.assertEqual(fixture.expected(), f"{value}\n", case)
            snapshots.append(fixture.sources())
        self.assertEqual(snapshots[0], snapshots[1])
        self.assertEqual(snapshots[1]["main.nagi"], snapshots[2]["main.nagi"])
        self.assertIn("api_value() -> i32", snapshots[3]["helper.nagi"])
        self.assertIn("i64(helper.api_value())", snapshots[3]["main.nagi"])
        self.assertNotEqual(snapshots[3]["helper.nagi"], snapshots[4]["helper.nagi"])
        self.assertNotEqual(snapshots[3]["helper2.nagi"], snapshots[4]["helper2.nagi"])
        self.assertIn("async def ready() -> i64:\n    return 11", snapshots[5]["main.nagi"])
        self.assertEqual(snapshots[5], snapshots[6])

    def test_runtime_workspace_rewrite_changes_only_members(self):
        text = ('[workspace]\nmembers = [\n "compiler",\n "runtime",\n]\nresolver="2"\n'
                '[workspace.package]\nversion="0.2.0"\n[profile.dev]\nincremental=false\n')
        expected = tomllib.loads(text)
        expected["workspace"]["members"] = ["runtime"]
        self.assertEqual(tomllib.loads(matrix.runtime_workspace_manifest(text)), expected)
        for bad in ('[workspace]\nmembers=["compiler"]\n',
                    '[workspace]\nmembers=["runtime"]\ndefault-members=["compiler"]\n'):
            with self.assertRaises(ValueError):
                matrix.runtime_workspace_manifest(bad)

    def test_runtime_copy_and_edit_never_modify_original_sources(self):
        source = self.root / "source"
        (source / "runtime/src").mkdir(parents=True)
        (source / "Cargo.toml").write_text('[workspace]\nmembers=["compiler", "runtime"]\n')
        (source / "Cargo.lock").write_text("version=4\n")
        (source / "runtime/Cargo.toml").write_text('[package]\nname="nagi-runtime"\n')
        original = source / "runtime/src/lib.rs"
        original.write_bytes(b"pub fn original() {}\n")
        before = matrix.digest(original)
        destination = self.root / "runtime-copy"
        record = matrix.copy_runtime(source, destination)
        (destination / "runtime/src/lib.rs").write_text("pub const OWNED: u64 = 1;\n")
        self.assertEqual(matrix.digest(original), before)
        self.assertEqual(record["original_members"], ["compiler", "runtime"])
        self.assertEqual(record["copied_members"], ["runtime"])

    def test_env_and_unknown_flags_are_rejected_without_normalization(self):
        matrix.reject_overrides({"PATH": "toolchain", "CARGO_HOME": "registry", "CARGO_NET_OFFLINE": "true"})
        for variable in ("RUSTFLAGS", "RUSTC_WRAPPER", "CARGO_PROFILE_DEV_OPT_LEVEL",
                         "CARGO_TARGET_DIR", "CARGO_BUILD_JOBS", "NAGI_RUN_RETENTION", "NAGI_FUTURE_FLAG"):
            with self.subTest(variable=variable), self.assertRaises(ValueError):
                matrix.reject_overrides({variable: ""})
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            matrix.argument_parser().parse_args(["--compiler", "c", "--work-dir", "w",
                                                "--output", "o", "--skip-checks"])
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            matrix.main(["--compiler", "missing", "--work-dir", "w", "--output", "o", "--repeat", "2"])

    def test_core_runtime_copy_preserves_the_shared_library_and_only_selected_members(self):
        source = self.root / "source"
        (source / "runtime/src").mkdir(parents=True)
        (source / "runtime-core").mkdir()
        (source / "Cargo.toml").write_text('[workspace]\nmembers=["compiler", "runtime", "runtime-core"]\n')
        (source / "Cargo.lock").write_text("version=4\n")
        (source / "runtime/Cargo.toml").write_text('[package]\nname="nagi-runtime"\n')
        (source / "runtime/src/lib.rs").write_bytes(b"pub const VALUE: i64 = 7;\n")
        core_manifest = '[package]\nname="nagi-runtime-core"\n[lib]\npath="../runtime/src/lib.rs"\n'
        (source / "runtime-core/Cargo.toml").write_text(core_manifest)
        (source / "runtime-core/build.rs").write_text('fn main() { println!("cargo:rustc-cfg=nagi_runtime_core"); }\n')
        destination = self.root / "copy"
        record = matrix.copy_runtime(source, destination)
        self.assertEqual(record["copied_members"], ["runtime", "runtime-core"])
        self.assertEqual((destination / "runtime-core/Cargo.toml").read_text(), core_manifest)
        self.assertEqual((destination / "runtime-core/../runtime/src/lib.rs").read_bytes(), b"pub const VALUE: i64 = 7;\n")
        (destination / "runtime/src/lib.rs").write_bytes(b"owned edit\n")
        self.assertEqual((source / "runtime/src/lib.rs").read_bytes(), b"pub const VALUE: i64 = 7;\n")
        for members in (("runtime", "compiler"), ("runtime-core",), ("runtime", "runtime-core", "extra")):
            with self.subTest(members=members), self.assertRaises(ValueError):
                matrix.runtime_workspace_manifest((source / "Cargo.toml").read_text(), members)

    def test_core_runtime_alias_is_rejected_before_copy(self):
        if not hasattr(os, "symlink"):
            return
        source = self.root / "source"
        (source / "runtime/src").mkdir(parents=True)
        outside = self.root / "outside"
        outside.mkdir()
        try:
            (source / "runtime-core").symlink_to(outside, target_is_directory=True)
        except OSError as error:
            self.skipTest(f"symlink privilege unavailable: {error}")
        with self.assertRaisesRegex(ValueError, "symlinks"):
            matrix.copy_runtime(source, self.root / "copy")
        self.assertFalse((self.root / "copy").exists())

    def test_runtime_copies_cannot_recurse_into_either_original_tree(self):
        source = self.root / "source"
        (source / "runtime").mkdir(parents=True)
        (source / "runtime-core").mkdir()
        for name in ("runtime", "runtime-core"):
            destination = source / name / "owned-looking-copy"
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "outside"):
                matrix.copy_runtime(source, destination)
            self.assertFalse(destination.exists())
            with self.assertRaises(ValueError):
                matrix.validate_measurement_paths(source, self.root / "safe", source / name / "report.json")
        matrix.validate_measurement_paths(source, self.root / "safe-work", self.root / "safe-report.json")

    def test_corrupt_lock_rejection_restores_exact_bytes_before_recovery(self):
        out = self.out()
        observed = []

        def fail():
            self.assertEqual((out / "Cargo.lock").read_bytes(), matrix.BROKEN_LOCK)
            return {"exit_code": 1, "native": None, "cargo_lock_parse_rejection_observed": True}

        def recover():
            self.assertEqual((out / "Cargo.lock").read_bytes(), b"original lock bytes\n")
            return {"exit_code": 0, "native": "new success"}

        result = matrix.corrupt_lock_case(out, fail, recover, lambda: observed.append("old verified"))
        self.assertEqual(observed, ["old verified", "old verified"])
        self.assertTrue(result["prior_latest_preserved_on_failure"])
        self.assertEqual((out / "Cargo.lock").read_bytes(), b"original lock bytes\n")

    def test_existing_report_is_never_overwritten(self):
        report = self.root / "existing.json"
        report.write_bytes(b"retained evidence")
        with (patch.dict(os.environ, {}, clear=True), contextlib.redirect_stderr(io.StringIO()),
              self.assertRaises(SystemExit)):
            matrix.main(["--compiler", "missing", "--work-dir", str(self.root),
                         "--output", str(report)])
        self.assertEqual(report.read_bytes(), b"retained evidence")

    def test_false_cache_success_and_latest_mutation_are_fail_closed(self):
        out = self.out()
        path, before = matrix.latest(out)
        for result in ({"exit_code": 0, "native": None}, {"exit_code": 1, "native": "cached exe"}):
            with self.subTest(result=result), self.assertRaises(RuntimeError):
                matrix.corrupt_lock_case(out, lambda: result, lambda: self.fail("recovery after false success"),
                                         lambda: self.fail("preservation after false success"))
            self.assertEqual((out / "Cargo.lock").read_bytes(), b"original lock bytes\n")
        path.write_bytes(b"changed latest")
        with self.assertRaises(RuntimeError):
            matrix.reject_false_success({"exit_code": 1, "native": None,
                                         "cargo_lock_parse_rejection_observed": True}, path, before)
        with self.assertRaisesRegex(RuntimeError, "infrastructure"):
            matrix.reject_false_success({"exit_code": 1, "native": None}, path, before)

    def test_measure_records_cpu_rss_raw_logs_and_expected_failure_status(self):
        log = self.root / "measured.log"
        source = 'print(\'nagi-build-stats: {"checks":"full"}\'); print("native: /fake/published")'
        result = matrix.measure([sys.executable, "-c", source], self.root, dict(os.environ), log)
        self.assertEqual(result["exit_code"], 0)
        self.assertEqual(result["stats"], {"checks": "full"})
        self.assertEqual(result["native"], "/fake/published")
        self.assertGreater(result["wall_s"], 0)
        self.assertEqual(result["log_sha256"], matrix.digest(log))
        if hasattr(os, "wait4"):
            self.assertGreaterEqual(result["cpu_s"], 0)
            self.assertGreater(result["peak_child_rss_kib"], 0)
        failed = matrix.measure([sys.executable, "-c", "raise SystemExit(7)"], self.root,
                                dict(os.environ), self.root / "failure.log")
        self.assertEqual(failed["exit_code"], 7)
        self.assertIsNone(failed["native"])

    def test_native_output_and_stderr_failures_are_not_success_samples(self):
        for result in (subprocess.CompletedProcess([], 0, b"bad\n", b""),
                       subprocess.CompletedProcess([], 1, b"42\n", b""),
                       subprocess.CompletedProcess([], 0, b"42\n", b"warning")):
            with patch.object(matrix.subprocess, "run", return_value=result), self.assertRaises(RuntimeError):
                matrix.execute(Path("unused"), "42\n", self.root, {}, self.root / "native.log")

    def test_measure_recognizes_full_and_core_runtime_rebuilds(self):
        for name, expected in (("nagi-runtime", True), ("nagi-runtime-core", True), ("nagi-runtime-other", False)):
            result = matrix.measure([sys.executable, "-c", f"print('   Compiling {name} v0.2.1')"],
                                    self.root, dict(os.environ), self.root / (name + ".log"))
            self.assertEqual(result["runtime_compilation_observed"], expected)

    def test_prune_removes_only_completed_owned_target_and_preserves_outputs(self):
        folder = self.root / "trial-1-units"
        target, out = folder / "target", folder / "out"
        target.mkdir(parents=True)
        out.mkdir()
        (target / "cache").write_text("rebuildable")
        (out / "published").write_text("immutable executable")
        configuration = {"target": str(target), "complete": False}
        with self.assertRaises(RuntimeError):
            matrix.prune_target(self.root, configuration)
        outside = self.root / "unowned"
        outside.mkdir()
        with self.assertRaises(RuntimeError):
            matrix.prune_target(self.root, {"target": str(outside), "complete": True})
        configuration["complete"] = True
        matrix.prune_target(self.root, configuration)
        self.assertFalse(target.exists())
        self.assertEqual((out / "published").read_text(), "immutable executable")
        self.assertTrue(outside.exists())

    def test_summary_ranges_keep_unknown_metrics_unknown(self):
        self.assertEqual(matrix.distribution([3, 1, 2]), {"median": 2, "min": 1, "max": 3})
        self.assertIsNone(matrix.distribution([1, None, 3]))
        result = matrix.metrics([{"wall_s": value, "native_execution": {"wall_s": value / 10}}
                                 for value in (3, 1, 2)])
        self.assertEqual(result["count"], 3)
        self.assertEqual(result["wall_s"]["median"], 2)
        self.assertIsNone(result["cpu_s"])
        self.assertIsNone(result["source_cache_hits"])
        self.assertEqual(result["native_startup_wall_s"]["median"], 0.2)


if __name__ == "__main__":
    unittest.main()
