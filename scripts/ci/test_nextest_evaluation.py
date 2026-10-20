"""Pure-Python tests for the opt-in nextest comparison helper."""
import json
import tempfile
import unittest
from pathlib import Path

from nextest_evaluation import (
    EvaluationError,
    cargo_build_command,
    cargo_test_run_command,
    combine_libtest_inventories,
    compare_inventories,
    ensure_no_repo_nextest_config,
    libtest_list_command,
    nextest_list_command,
    nextest_run_command,
    parse_libtest_listing,
    parse_nextest_inventory,
    require_nextest_version,
    warm_order_pairs,
)


class InventoryTests(unittest.TestCase):
    def test_measured_version_must_equal_the_reviewed_installed_tool(self):
        for version in ("cargo-nextest 0.9.148", "cargo-nextest 0.9.148 (abc123 2026-10-08)"):
            self.assertEqual(require_nextest_version(version + "\n"), version)
        for version in ("cargo-nextest 0.9.147", "cargo-nextest 0.9.1480", "unknown", "cargo-nextest 0.9.148\nextra"):
            with self.assertRaises(EvaluationError):
                require_nextest_version(version)

    def test_libtest_full_and_ignored_lists_preserve_membership(self):
        all_tests = parse_libtest_listing(
            "compiler::parser::accepts: test\ncompiler::parser::rejects: test\n"
            "compiler::parser::legacy: test\n\n3 tests, 0 benchmarks\n",
        )
        ignored_subset = parse_libtest_listing(
            "compiler::parser::legacy: test\n\n1 test, 0 benchmarks\n",
        )
        self.assertEqual(
            combine_libtest_inventories(all_tests, ignored_subset),
            {"compiler::parser::accepts": False, "compiler::parser::rejects": False,
             "compiler::parser::legacy": True},
        )

    def test_libtest_rejects_incomplete_duplicate_and_benchmark_lists(self):
        with self.assertRaises(EvaluationError):
            parse_libtest_listing("one: test\n")
        with self.assertRaises(EvaluationError):
            parse_libtest_listing("one: test\none: test\n2 tests, 0 benchmarks\n")
        with self.assertRaises(EvaluationError):
            parse_libtest_listing("bench: benchmark\n0 tests, 1 benchmark\n")
        with self.assertRaises(EvaluationError):
            combine_libtest_inventories({"same"}, {"outside-full-list"})

    def test_nextest_json_keeps_ignored_cases_and_skips_unselected_suites(self):
        payload = {
            "test-count": 3,
            "rust-suites": {
                "nagic::enum_parser": {
                    "package-name": "nagic", "binary-name": "enum_parser", "status": "listed",
                    "testcases": {
                        "parser::accepts": {"ignored": False, "filter-match": {"status": "matches"}},
                        "parser::legacy": {"ignored": True, "filter-match": {"status": "mismatch", "reason": "ignored"}},
                    },
                },
                "nagic::builtin_type_contracts": {
                    "package-name": "nagic", "binary-name": "builtin_type_contracts", "status": "listed",
                    "testcases": {
                        "types::canonical": {"ignored": False, "filter-match": {"status": "matches"}},
                    },
                },
                "nagic::another-target": {
                    "package-name": "nagic", "binary-name": "another-target", "status": "skipped",
                    "testcases": {},
                },
            },
        }
        observed = parse_nextest_inventory(json.dumps(payload))
        self.assertEqual(observed, {
            "enum_parser": {"parser::accepts": False, "parser::legacy": True},
            "builtin_type_contracts": {"types::canonical": False},
        })

    def test_nextest_rejects_missing_targets_extra_listed_tests_or_changed_ignore_state(self):
        base = {
            "test-count": 1,
            "rust-suites": {
                "nagic::enum_parser": {
                    "package-name": "nagic", "binary-name": "enum_parser", "status": "listed",
                    "testcases": {"parser::accepts": {"ignored": False, "filter-match": {"status": "matches"}}},
                },
            },
        }
        with self.assertRaises(EvaluationError):
            parse_nextest_inventory(json.dumps(base))
        extra = json.loads(json.dumps(base))
        extra["rust-suites"]["nagic::builtin_type_contracts"] = {
            "package-name": "nagic", "binary-name": "builtin_type_contracts", "status": "listed",
            "testcases": {"types::bad-filter": {"ignored": False, "filter-match": {"status": "mismatch", "reason": "string"}}},
        }
        extra["test-count"] = 2
        with self.assertRaises(EvaluationError):
            parse_nextest_inventory(json.dumps(extra))
        wrong_count = json.loads(json.dumps(base))
        wrong_count["rust-suites"]["nagic::builtin_type_contracts"] = {
            "package-name": "nagic", "binary-name": "builtin_type_contracts", "status": "listed",
            "testcases": {"types::ok": {"ignored": True, "filter-match": {"status": "mismatch", "reason": "ignored"}}},
        }
        wrong_count["test-count"] = 2
        parsed = parse_nextest_inventory(json.dumps(wrong_count))
        with self.assertRaises(EvaluationError):
            compare_inventories({"enum_parser": {"parser::accepts": False},
                                 "builtin_type_contracts": {"types::ok": False}}, parsed)

    def test_nextest_rejects_empty_selection_and_duplicate_json_keys(self):
        empty = {"test-count": 0, "rust-suites": {
            "nagic::enum_parser": {"package-name": "nagic", "binary-name": "enum_parser", "status": "listed", "testcases": {}},
            "nagic::builtin_type_contracts": {"package-name": "nagic", "binary-name": "builtin_type_contracts", "status": "listed", "testcases": {}},
        }}
        with self.assertRaises(EvaluationError):
            parse_nextest_inventory(json.dumps(empty))
        with self.assertRaises(EvaluationError):
            parse_nextest_inventory('{"test-count":0,"test-count":1,"rust-suites":{}}')

    def test_inventory_comparison_detects_missing_extra_and_ignored_state_drift(self):
        base = {"enum_parser": {"a": False, "b": True}, "builtin_type_contracts": {"c": False}}
        compare_inventories(base, dict(base))
        for candidate in (
            {"enum_parser": {"a": False}, "builtin_type_contracts": {"c": False}},
            {"enum_parser": {"a": False, "b": True, "extra": False}, "builtin_type_contracts": {"c": False}},
            {"enum_parser": {"a": False, "b": False}, "builtin_type_contracts": {"c": False}},
        ):
            with self.subTest(candidate=candidate), self.assertRaises(EvaluationError):
                compare_inventories(base, candidate)


class InvocationTests(unittest.TestCase):
    def test_build_and_list_commands_are_locked_and_target_bounded(self):
        self.assertEqual(cargo_build_command(), [
            "cargo", "test", "--locked", "-p", "nagic", "--test", "enum_parser",
            "--test", "builtin_type_contracts", "--no-run",
        ])
        self.assertEqual(libtest_list_command("enum_parser", ignored=True), [
            "cargo", "test", "--locked", "-p", "nagic", "--test", "enum_parser",
            "--", "--list", "--ignored",
        ])
        self.assertEqual(nextest_list_command(), [
            "cargo", "nextest", "list", "--locked", "--message-format=json", "-p", "nagic",
            "--test", "enum_parser", "--test", "builtin_type_contracts",
        ])

    def test_run_commands_keep_no_fail_fast_zero_retries_and_empty_run_failure(self):
        baseline = cargo_test_run_command()
        candidate = nextest_run_command()
        self.assertIn("--locked", baseline)
        self.assertIn("--no-fail-fast", baseline)
        self.assertIn("--locked", candidate)
        self.assertIn("--no-fail-fast", candidate)
        self.assertEqual(candidate[candidate.index("--retries") + 1], "0")
        self.assertIn("--no-tests=fail", candidate)
        self.assertNotIn("--test-threads=1", candidate)
        self.assertNotIn("--ignored", candidate)

    def test_warm_run_orders_alternate_and_balance_first_place(self):
        forward = warm_order_pairs("libtest")
        reverse = warm_order_pairs("nextest")
        self.assertEqual(forward, [
            ("libtest", "nextest"), ("nextest", "libtest"),
            ("libtest", "nextest"), ("nextest", "libtest"),
        ])
        self.assertEqual(reverse[0], ("nextest", "libtest"))
        self.assertEqual(reverse[1], ("libtest", "nextest"))
        for schedule in (forward, reverse):
            self.assertEqual(sum(pair[0] == "libtest" for pair in schedule), 2)
            self.assertEqual(sum(pair[0] == "nextest" for pair in schedule), 2)
        with self.assertRaises(EvaluationError):
            warm_order_pairs("libtest", pairs=3)

    def test_hidden_repository_config_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            ensure_no_repo_nextest_config(root)
            (root / ".config").mkdir()
            (root / ".config" / "nextest.toml").write_text("[profile.default]\n")
            with self.assertRaises(EvaluationError):
                ensure_no_repo_nextest_config(root)


if __name__ == "__main__":
    unittest.main()
