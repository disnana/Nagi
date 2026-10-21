"""Fail-closed coverage for the two whole four-platform regression steps."""
import copy
import hashlib
from pathlib import Path
import re
import unittest

import gate
import json
import levels_projection
import parallel_regression_projection as parallel
import sf07_request_projection as historical
from sf07_projection_contract import BASE_WORKFLOW_SHA256
import test_checks


ROOT = Path(__file__).resolve().parents[2]


def historical_digest(workflow):
    # The assembled historical entrypoint must compose the parallel and source
    # guard inverses before its unchanged historical checks. No pin is replaced.
    restored = historical.inverse_workflow(workflow)
    if hashlib.sha256(restored.encode()).hexdigest() != BASE_WORKFLOW_SHA256:
        raise AssertionError("historical whole-workflow identity changed")


class ParallelWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.actual_workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        cls.workflow = levels_projection.inverse_workflow(cls.actual_workflow)

    def test_exact_job_and_ordered_reassembly_keep_historical_identity(self):
        historical_digest(self.workflow)
        restored = parallel.inverse_workflow(self.workflow)
        self.assertNotIn("  nagi-regression:\n", restored)
        for step in (parallel.CLI_STEP, parallel.GENERATED_STEP):
            self.assertEqual(self.workflow.count(step), 1)
            self.assertEqual(restored.count(step), 1)
        package = re.search(
            r"(?ms)^  nagi-package:\n.*?(?=^  [a-zA-Z0-9_-]+:|\Z)", restored
        ).group()
        self.assertIn(parallel.CLI_STEP + parallel.EDITOR_ANCHOR, package)
        self.assertIn(parallel.GENERATED_STEP + parallel.SF01_ANCHOR, package)

    def test_added_job_is_exact_and_cannot_omit_coverage_or_evidence(self):
        job = parallel.REGRESSION_JOB
        mutations = [
            self.workflow.replace(job, "", 1),
            self.workflow.replace(job, job + job, 1),
        ]
        for old, new in (
            (parallel.CLI_STEP, ""),
            (parallel.GENERATED_STEP, ""),
            (parallel.CLI_STEP, parallel.CLI_STEP + parallel.CLI_STEP),
            ("os: windows-latest", "os: ubuntu-latest"),
            ("rust_host: aarch64-apple-darwin", "rust_host: x86_64-apple-darwin"),
            ("fail-fast: false", "fail-fast: true"),
            ("needs: [changes, release-plan]", "needs: [release-plan]"),
            ("needs: [changes, release-plan]", "needs: [changes, release-plan, nagi-package]"),
            ("needs.changes.result == 'success'", "needs.changes.result != 'failure'"),
            ("needs.release-plan.outputs.package_nagi == 'true'", "true"),
            ("ref: ${{ github.event.pull_request.head.sha || github.sha }}", "ref: main"),
            ("python-version: '3.12'", "python-version: '3.11'"),
            ("node-version: '22'", "node-version: '20'"),
            ("RELEASE_RUST_HOST: ${{ matrix.rust_host }}", "RELEASE_RUST_HOST: wrong"),
            ('run: cargo fetch --locked --target "$RELEASE_RUST_HOST"', "run: cargo fetch"),
            ("      - run: cargo build --release --locked -p nagic\n", ""),
            ("NAGI_FAILURE_DIR:", "NAGI_OTHER_FAILURE_DIR:"),
            ("NAGI_GOLDEN_CAPTURE_DIR:", "NAGI_OTHER_GOLDEN_DIR:"),
            ("            native-target\n", ""),
            ("name: compiler-failures-regression-", "name: compiler-failures-"),
            ("path: build/compiler-failures/", "path: build/other/"),
            ("if: failure()", "if: success()"),
            ("if-no-files-found: ignore", "if-no-files-found: warn"),
            ("--test conformance", "--test conformance -- --ignored"),
        ):
            with self.subTest(anchor=old):
                self.assertIn(old, job)
                mutations.append(self.workflow.replace(job, job.replace(old, new, 1), 1))
        for changed in mutations:
            with self.subTest(mutation=changed[-120:]):
                with self.assertRaises((ValueError, AssertionError)):
                    historical_digest(changed)

    def test_duplicate_old_steps_and_gate_drift_fail_closed(self):
        mutations = [
            self.workflow.replace(parallel.EDITOR_ANCHOR,
                                  parallel.CLI_STEP + parallel.EDITOR_ANCHOR, 1),
            self.workflow.replace(parallel.SF01_ANCHOR,
                                  parallel.GENERATED_STEP + parallel.SF01_ANCHOR, 1),
            self.workflow.replace(parallel.READY_NEW, parallel.READY_OLD, 1),
            self.workflow.replace(parallel.PUBLISH_NEW, parallel.PUBLISH_OLD, 1),
            self.workflow.replace(parallel.PUBLISH_CONDITION_NEW,
                                  parallel.PUBLISH_CONDITION_OLD, 1),
            self.workflow.replace("name: Nagi checks\n", "name: Other checks\n", 1),
        ]
        for changed in mutations:
            with self.subTest(mutation=changed[:80]):
                with self.assertRaises((ValueError, AssertionError)):
                    historical_digest(changed)


class RegressionGateTests(unittest.TestCase):
    def test_shard_result_tracks_handwritten_l1_l2_l3_profiles(self):
        # Handwritten matrix rows and selected outcomes, not values produced by levels.expected().
        rows = [
            {'os':'ubuntu-latest','platform':'linux-x86_64','executable':'nagic','rust_host':'x86_64-unknown-linux-gnu'},
            {'os':'windows-latest','platform':'windows-x86_64','executable':'nagic.exe','rust_host':'x86_64-pc-windows-msvc'},
            {'os':'macos-15','platform':'macos-arm64','executable':'nagic','rust_host':'aarch64-apple-darwin'},
            {'os':'macos-15-intel','platform':'macos-x86_64','executable':'nagic','rust_host':'x86_64-apple-darwin'},
        ]
        for level, critical, comprehensive, count, expected in [
            ('1','true','false',3,'skipped'), ('2','false','true',3,'success'), ('3','false','true',4,'success')]:
            needs = test_checks.GateTests().needs(full='true', package_nagi='true')
            needs['ci-plan']['outputs'] = dict(level=level, critical_checks=critical,
                comprehensive_checks=comprehensive, package_checks=comprehensive,
                matrix=json.dumps({'include':rows[:count]}, separators=(',', ':')))
            for job in ['linux','rust-1-88','nagi-package','nagi-regression']:
                needs[job]['result'] = expected
            needs['critical']['result'] = 'success' if critical == 'true' else 'skipped'
            self.assertEqual(gate.errors(needs), [], level)
            for result in ['success','skipped','failure','cancelled','',None]:
                changed=copy.deepcopy(needs);changed['nagi-regression']['result']=result
                self.assertEqual(gate.errors(changed) == [], result == expected, (level,result))
            for bad in [[], rows[:2], rows[:3] if level == '3' else rows]:
                changed=copy.deepcopy(needs)
                changed['ci-plan']['outputs']['matrix']=json.dumps({'include':bad}, separators=(',', ':'))
                self.assertTrue(gate.errors(changed), (level,bad))
            del needs['nagi-regression']
            self.assertTrue(gate.errors(needs))

    def test_source_guard_failure_and_invalid_plan_never_allow_shard_success(self):
        valid = test_checks.GateTests().needs(package_nagi="true")
        for job in ("changes", "release-plan"):
            for result in ("failure", "cancelled", "skipped", None):
                needs = copy.deepcopy(valid)
                needs[job]["result"] = result
                self.assertTrue(gate.errors(needs))
        for planned in (None, "", "TRUE", True, False):
            needs = copy.deepcopy(valid)
            needs["release-plan"]["outputs"]["package_nagi"] = planned
            self.assertTrue(gate.errors(needs))


if __name__ == "__main__":
    unittest.main()
