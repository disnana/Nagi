import copy
import json
import unittest
from pathlib import Path

import changes
import gate
import go_backend_projection
import levels

ROOT = Path(__file__).resolve().parents[2]


class GoBackendCIContractTests(unittest.TestCase):
    def source(self):
        return (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")

    def test_job_is_three_platform_by_default_and_four_for_release(self):
        for level, count in (("2", 3), ("3", 4)):
            selected = levels.expected(level, "true", "false", "true")
            matrix = json.loads(selected["matrix"])["include"]
            self.assertEqual(len(matrix), count)
            self.assertEqual({row["os"] for row in matrix[:3]}, {
                "ubuntu-latest", "windows-latest", "macos-15",
            })
            self.assertEqual([row["go_tests"] for row in matrix[:3]], ["29", "28", "29"])

    def test_exact_job_uses_pinned_local_go_and_real_execution_guard(self):
        source = self.source()
        self.assertIn(go_backend_projection.GO_JOB, source)
        job = go_backend_projection.GO_JOB
        self.assertIn("actions/setup-go@b7ad1dad31e06c5925ef5d2fc7ad053ef454303e", job)
        self.assertIn("go-version: '1.27.3'", job)
        self.assertIn(
            '      - name: Run the real Go backend build and execution contract\n'
            '        env:\n'
            '          GOCACHE: ${{ runner.temp }}/nagi-go-build-cache\n',
            job,
        )
        self.assertIn("GOTOOLCHAIN: local", job)
        self.assertIn("cargo test --locked -p nagic --test go_backend", job)
        self.assertIn("assert_go_backend_summary.py", job)
        self.assertIn('matrix: ${{ fromJSON(needs.ci-plan.outputs.matrix) }}', job)

    def test_projection_rejects_gate_toolchain_or_execution_weakening(self):
        source = self.source()
        for before, after in (
            ("go-version: '1.27.3'", "go-version: stable"),
            ("GOTOOLCHAIN: local", "GOTOOLCHAIN: auto"),
            ("cargo test --locked -p nagic --test go_backend", "cargo test --locked -p nagic --lib"),
            ("--expected \"${{ matrix.go_tests }}\"", "--expected 0"),
            ("critical, go-backend, linux", "critical, linux"),
        ):
            with self.subTest(change=after), self.assertRaises(ValueError):
                go_backend_projection.inverse_workflow(source.replace(before, after, 1))

    def test_gate_requires_selected_success_and_unselected_skip(self):
        release = {prefix + name: "false" for prefix in ("package_", "release_") for name in ("nagi", "vscode", "jetbrains")}
        for go_selected, result in (("true", "success"), ("false", "skipped")):
            selected = levels.expected("1", "false", "false", go_selected)
            needs = {
                "ci-plan": {"result": "success", "outputs": selected},
                "changes": {"result": "success", "outputs": {"full_checks": "false", "jetbrains_checks": "false", "go_checks": go_selected}},
                "release-plan": {"result": "success", "outputs": release},
                "critical": {"result": "skipped"}, "go-backend": {"result": result},
                "linux": {"result": "skipped"}, "linux-workspace": {"result": "skipped"},
                "rust-1-88": {"result": "skipped"}, "jetbrains": {"result": "skipped"},
                "sf08-browser": {"result": "skipped"}, "vscode-package": {"result": "skipped"},
                "nagi-package": {"result": "skipped"}, "nagi-regression": {"result": "skipped"},
                "windows-contracts": {"result": "skipped"},
            }
            self.assertEqual(gate.errors(needs), [])
            broken = copy.deepcopy(needs)
            broken["go-backend"]["result"] = "skipped" if go_selected == "true" else "success"
            self.assertTrue(gate.errors(broken))

    def test_unknown_and_shared_paths_select_go_while_target_only_paths_skip(self):
        for path in ("compiler/src/lib.rs", "compiler/go-runtime/internal/lifecycle/lifecycle.go", "compiler/go-runtime/go.mod", "Cargo.lock", ".github/workflows/ci.yml", "new/unknown"):
            self.assertTrue(changes.requires_go(path), path)
        for path in ("runtime/src/lib.rs", "runtime-core/src/lib.rs", "docs/http.md", "editors/vscode-nagi/src/features.js"):
            self.assertFalse(changes.requires_go(path), path)


if __name__ == "__main__":
    unittest.main()
