import copy
import unittest

import levels
import publication_gate


SHA = "a" * 40


def valid(component="nagi"):
    nagi = component == "nagi"
    plan = {"sha": SHA}
    for name in ("nagi", "vscode", "jetbrains"):
        plan["release_" + name] = plan["package_" + name] = str(name == component).lower()
    needs = {name: {"result": "success"} for name in ("changes", "release-plan", "ci-plan", "ready")}
    needs["changes"]["outputs"] = {"full_checks": str(nagi).lower(), "jetbrains_checks": str(nagi or component == "jetbrains").lower()}
    needs["release-plan"]["outputs"] = plan
    needs["ci-plan"]["outputs"] = levels.expected("3", str(nagi).lower(), plan["package_nagi"])
    for name in ("linux", "rust-1-88", "sf08-browser", "nagi-package", "nagi-regression"):
        needs[name] = {"result": "success" if nagi else "skipped"}
    needs["critical"] = {"result": "skipped"}
    needs["jetbrains"] = {"result": "success" if nagi or component == "jetbrains" else "skipped"}
    needs["vscode-package"] = {"result": "success" if component == "vscode" else "skipped"}
    return needs


class PublicationGateTests(unittest.TestCase):
    def verdict(self, needs, event="push", ref="refs/heads/main", sha=SHA):
        return publication_gate.errors(needs, event, ref, sha)

    def test_component_specific_complete_release(self):
        for component in ("nagi", "vscode", "jetbrains"):
            with self.subTest(component=component):
                self.assertEqual(self.verdict(valid(component)), [])

    def test_every_selected_job_must_finish_successfully(self):
        original = valid()
        for name, job in original.items():
            if job["result"] != "success":
                continue
            for result in ("failure", "cancelled", "skipped", "queued", None):
                with self.subTest(job=name, result=result):
                    needs = copy.deepcopy(original)
                    needs[name]["result"] = result
                    self.assertTrue(self.verdict(needs))
            needs = copy.deepcopy(original)
            del needs[name]
            self.assertTrue(self.verdict(needs))

    def test_pr_tag_manual_and_wrong_source_never_publish(self):
        for event, ref, sha in (("pull_request", "refs/pull/1/merge", SHA),
                                ("workflow_dispatch", "refs/heads/main", SHA),
                                ("push", "refs/tags/v0.2.1", SHA),
                                ("push", "refs/heads/main", "b" * 40),
                                ("push", "refs/heads/main", "")):
            self.assertTrue(self.verdict(valid(), event, ref, sha))

    def test_lower_level_matrix_missing_or_unknown_is_rejected(self):
        for level in ("1", "2", "4", "", None):
            needs = valid()
            needs["ci-plan"]["outputs"]["level"] = level
            self.assertTrue(self.verdict(needs))
        for matrix in ('{"include":[]}', '{"include":[] ,"extra":true}', "not-json"):
            needs = valid()
            needs["ci-plan"]["outputs"]["matrix"] = matrix
            self.assertTrue(self.verdict(needs))

    def test_invalid_or_absent_release_flags_and_shapes_fail_closed(self):
        for component in ("nagi", "vscode", "jetbrains"):
            for flag in (None, True, "TRUE", "false\n", ""):
                needs = valid()
                needs["release-plan"]["outputs"]["release_" + component] = flag
                self.assertTrue(self.verdict(needs))
        needs = valid()
        for component in ("nagi", "vscode", "jetbrains"):
            needs["release-plan"]["outputs"]["release_" + component] = "false"
        self.assertTrue(self.verdict(needs))
        for needs in (None, [], {}, {"ci-plan": None}, {"release-plan": {"outputs": []}}):
            self.assertTrue(self.verdict(needs))

    def test_releasing_component_cannot_omit_its_package_and_use_successful_skips(self):
        for component in ("vscode", "jetbrains"):
            needs = valid(component)
            needs["release-plan"]["outputs"]["package_" + component] = "false"
            needs["vscode-package" if component == "vscode" else "jetbrains"]["result"] = "skipped"
            needs["changes"]["outputs"]["jetbrains_checks"] = "false"
            self.assertTrue(self.verdict(needs))


if __name__ == "__main__":
    unittest.main()
