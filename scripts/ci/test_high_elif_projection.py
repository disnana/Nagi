"""The new harness is required; all inherited workflow bytes remain pinned."""
import hashlib
from pathlib import Path
import unittest

import high_elif_projection as projection
import go_backend_projection
import levels_projection
import sf07_request_projection as history
from sf07_projection_contract import BASE_WORKFLOW_SHA256


ROOT = Path(__file__).resolve().parents[2]
# Reviewed inherited workflow after the response-budget source pins and before
# the Go/High elif harness additions are projected away below.
BEFORE_ELIF_WORKFLOW_SHA256 = "40b466fddb09bcc72d58c73e96b5dcecdb4d1e2a353efe41d6f0195a4dabe0ae"


class HighElifProjectionTests(unittest.TestCase):
    def setUp(self):
        self.source = (ROOT / ".github/workflows/ci.yml").read_text()

    def test_exact_command_inverse_retains_raw_and_frozen_history(self):
        restored = projection.inverse_workflow(self.source)
        inherited = go_backend_projection.inverse_workflow(restored)
        self.assertEqual(hashlib.sha256(inherited.encode()).hexdigest(), BEFORE_ELIF_WORKFLOW_SHA256)
        self.assertEqual(restored.count(projection.ORIGINAL_STEP), 1)
        self.assertNotIn("--test high_elif", restored)
        self.assertEqual(restored.replace(projection.ORIGINAL_STEP, projection.ADDED_STEP, 1), self.source)
        self.assertIn(projection.ORIGINAL_STEP, levels_projection.inverse_workflow(self.source))
        self.assertEqual(hashlib.sha256(history.inverse_workflow(self.source).encode()).hexdigest(),
                         BASE_WORKFLOW_SHA256)

    def test_missing_duplicate_moved_or_weakened_command_fails_closed(self):
        mutants = [
            self.source.replace(projection.ADDED_STEP, projection.ORIGINAL_STEP, 1),
            self.source.replace(projection.ADDED_STEP, "", 1),
            self.source.replace(projection.ADDED_STEP, projection.ADDED_STEP * 2, 1),
            self.source.replace(projection.ADDED_STEP, "", 1) + projection.ADDED_STEP,
            self.source + projection.ORIGINAL_STEP,
            self.source + projection.ORIGINAL_STEP.replace("Check CLI,", "Unreviewed CLI,", 1),
            self.source.replace("  nagi-regression:\n", "  nagi-regression:\n  nagi-regression:\n", 1),
        ]
        for before, after in [
            ("--test high_elif ", ""),
            ("--test high_elif", "--test high_elif --test high_elif"),
            ("--test high_elif", "--test high_else"),
            ("--test frontend_contracts ", ""),
            ("--test enum_parser ", ""),
            ("--test frontend_contracts", "--test frontend_contracts -- --ignored"),
            ("Check CLI,", "Changed CLI,"),
        ]:
            self.assertIn(before, projection.ADDED_STEP)
            mutants.append(self.source.replace(
                projection.ADDED_STEP, projection.ADDED_STEP.replace(before, after, 1), 1))
        for mutant in mutants:
            for inverse in [projection.inverse_workflow, levels_projection.inverse_workflow,
                            history.inverse_workflow]:
                with self.subTest(inverse=inverse.__module__, mutant=hashlib.sha256(mutant.encode()).hexdigest()):
                    with self.assertRaises(ValueError):
                        inverse(mutant)

    def test_unrelated_command_drift_remains_visible_to_history(self):
        before = "cargo test --locked -p nagi-runtime --lib auth::ed25519::"
        after = "cargo test --locked -p nagi-runtime --lib typo::"
        self.assertIn(before, self.source)
        mutant = self.source.replace(before, after, 1)
        restored = projection.inverse_workflow(mutant)
        self.assertIn(after, restored)
        inherited = go_backend_projection.inverse_workflow(restored)
        self.assertNotEqual(hashlib.sha256(inherited.encode()).hexdigest(), BEFORE_ELIF_WORKFLOW_SHA256)
        try:
            inherited = history.inverse_workflow(mutant)
        except ValueError:
            return
        self.assertNotEqual(hashlib.sha256(inherited.encode()).hexdigest(), BASE_WORKFLOW_SHA256)
