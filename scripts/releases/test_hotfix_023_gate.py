from __future__ import annotations

import hashlib
import tempfile
import unittest
from pathlib import Path

import hotfix_023_gate as gate


ROOT = Path(__file__).resolve().parents[2]


def source_fixtures():
    run = {
        "id": gate.SOURCE_RUN_ID,
        "head_sha": gate.SOURCE_SHA,
        "status": "completed",
        "conclusion": "success",
        "event": "workflow_dispatch",
        "path": ".github/workflows/ci.yml",
        "repository": {"full_name": gate.REPOSITORY},
    }
    jobs = [{"name": name, "status": "completed", "conclusion": "success"}
            for name in gate.EXPECTED_JOBS]
    artifacts = [{
        "name": name,
        "id": artifact_id,
        "expired": False,
        "workflow_run": {"id": gate.SOURCE_RUN_ID, "head_sha": gate.SOURCE_SHA},
    } for name, artifact_id in gate.EXPECTED_ARTIFACTS.items()]
    return run, {"total_count": len(jobs), "jobs": jobs}, {
        "total_count": len(artifacts), "artifacts": artifacts}


class SourceGateTests(unittest.TestCase):
    def test_accepts_the_reviewed_source(self):
        gate.validate_source(*source_fixtures())

    def test_rejects_another_commit(self):
        run, jobs, artifacts = source_fixtures()
        run["head_sha"] = "0" * 40
        with self.assertRaisesRegex(ValueError, "another commit"):
            gate.validate_source(run, jobs, artifacts)

    def test_rejects_a_failed_required_job(self):
        run, jobs, artifacts = source_fixtures()
        jobs["jobs"][0]["conclusion"] = "failure"
        with self.assertRaisesRegex(ValueError, "did not succeed"):
            gate.validate_source(run, jobs, artifacts)

    def test_rejects_an_expired_artifact(self):
        run, jobs, artifacts = source_fixtures()
        artifacts["artifacts"][0]["expired"] = True
        with self.assertRaisesRegex(ValueError, "expired"):
            gate.validate_source(run, jobs, artifacts)

    def test_ignores_another_components_release_artifact(self):
        run, jobs, artifacts = source_fixtures()
        artifacts["artifacts"].append({"name": "release-jetbrains", "id": 1})
        artifacts["total_count"] += 1
        gate.validate_source(run, jobs, artifacts)


class AssetGateTests(unittest.TestCase):
    def test_accepts_exact_archives_and_checksums(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            original = gate.EXPECTED_ARCHIVES
            try:
                gate.EXPECTED_ARCHIVES = {}
                for index, name in enumerate(original):
                    data = f"archive-{index}".encode()
                    digest = hashlib.sha256(data).hexdigest()
                    gate.EXPECTED_ARCHIVES[name] = digest
                    (root / name).write_bytes(data)
                    (root / (name + ".sha256")).write_text(
                        f"{digest}  {name}\n", encoding="utf-8")
                gate.verify_assets(root)
            finally:
                gate.EXPECTED_ARCHIVES = original

    def test_rejects_an_extra_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "unexpected").write_text("x", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "file set changed"):
                gate.verify_assets(root)

    def test_rejects_an_extra_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "unexpected").mkdir()
            with self.assertRaisesRegex(ValueError, "non-regular entry"):
                gate.verify_assets(root)


class WorkflowBindingTests(unittest.TestCase):
    def test_workflow_is_bound_to_the_reviewed_release(self):
        workflow = (ROOT / ".github/workflows/publish-nagi-0.2.3-hotfix.yml").read_text(
            encoding="utf-8")
        self.assertIn("  workflow_dispatch:\n", workflow)
        self.assertNotIn("  pull_request:\n", workflow)
        self.assertNotIn("  push:\n", workflow)
        self.assertIn("      actions: read\n", workflow)
        self.assertIn("      contents: write\n", workflow)
        self.assertIn(f"          run-id: {gate.SOURCE_RUN_ID}\n", workflow)
        self.assertIn(f"          ref: {gate.SOURCE_SHA}\n", workflow)
        self.assertIn(f"          --version {gate.VERSION}\n", workflow)
        self.assertIn(f"          --sha {gate.SOURCE_SHA}\n", workflow)
        for name in gate.EXPECTED_ARTIFACTS:
            self.assertEqual(workflow.count(f"          name: {name}\n"), 1)
        self.assertEqual(workflow.count(
            "actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093"), 4)


class ExistingReleaseTests(unittest.TestCase):
    def setUp(self):
        self.expected = {"archive": "a", "archive.sha256": "b"}

    def test_accepts_an_empty_preflight(self):
        gate.validate_release_state(None, None, {}, self.expected, require_public=False)

    def test_accepts_a_matching_partial_draft(self):
        release = {"tag_name": gate.TAG, "draft": True, "prerelease": False}
        gate.validate_release_state(gate.SOURCE_SHA, release, {"archive": "a"},
                                    self.expected, require_public=False)

    def test_rejects_an_unreviewed_draft_asset(self):
        release = {"tag_name": gate.TAG, "draft": True, "prerelease": False}
        with self.assertRaisesRegex(ValueError, "unexpected asset"):
            gate.validate_release_state(gate.SOURCE_SHA, release, {"extra": "x"},
                                        self.expected, require_public=False)

    def test_rejects_a_matching_name_with_other_bytes(self):
        release = {"tag_name": gate.TAG, "draft": True, "prerelease": False}
        with self.assertRaisesRegex(ValueError, "differs"):
            gate.validate_release_state(gate.SOURCE_SHA, release, {"archive": "x"},
                                        self.expected, require_public=False)

    def test_postflight_requires_an_exact_public_release(self):
        release = {"tag_name": gate.TAG, "draft": False, "prerelease": False}
        gate.validate_release_state(gate.SOURCE_SHA, release, dict(self.expected),
                                    self.expected, require_public=True)
        with self.assertRaisesRegex(ValueError, "exactly eight"):
            gate.validate_release_state(gate.SOURCE_SHA, release, {"archive": "a"},
                                        self.expected, require_public=True)

    def test_public_release_requires_the_exact_tag_ref(self):
        release = {"tag_name": gate.TAG, "draft": False, "prerelease": False}
        with self.assertRaisesRegex(ValueError, "tag is missing"):
            gate.validate_release_state(None, release, dict(self.expected), self.expected,
                                        require_public=True)

    def test_rejects_a_prerelease(self):
        release = {"tag_name": gate.TAG, "draft": True, "prerelease": True}
        with self.assertRaisesRegex(ValueError, "must not be a prerelease"):
            gate.validate_release_state(gate.SOURCE_SHA, release, {}, self.expected,
                                        require_public=False)

    def test_rejects_a_conflicting_tag(self):
        with self.assertRaisesRegex(ValueError, "another commit"):
            gate.validate_release_state("0" * 40, None, {}, self.expected,
                                        require_public=False)


if __name__ == "__main__":
    unittest.main()
