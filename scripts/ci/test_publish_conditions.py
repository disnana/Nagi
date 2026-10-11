"""Evaluate the real publish-release condition with a small, safe GitHub-expression subset."""
from __future__ import annotations

import copy
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MAIN_REF = "refs/heads/main"
COMPONENTS = ("nagi", "vscode", "jetbrains")
JOBS = ("linux", "release-plan", "jetbrains", "vscode-package", "nagi-package")
ALLOWED_NAMES = {
    "github.event_name", "github.ref",
    "needs.linux.result", "needs.release-plan.result",
    "needs.release-plan.outputs.release_nagi",
    "needs.release-plan.outputs.release_vscode",
    "needs.release-plan.outputs.release_jetbrains",
    "needs.jetbrains.result", "needs.vscode-package.result", "needs.nagi-package.result",
}
TOKEN = re.compile(
    r"\s+|&&|\|\||==|!=|[()]|'[^']*'|"
    r"[A-Za-z_][A-Za-z0-9_-]*(?:\.[A-Za-z_][A-Za-z0-9_-]*)*"
)


def publish_expression() -> str:
    """Read only the publish-release YAML folded scalar; do not evaluate YAML or code."""
    workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    job = re.search(
        r"(?ms)^  publish-release:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:\s*$|\Z)", workflow
    )
    if not job:
        raise AssertionError("publish-release job is missing")
    condition = re.search(
        r"(?m)^    if:\s*>-\s*\n(?P<lines>(?:^      .*\n?)+)", job.group("body")
    )
    if not condition:
        raise AssertionError("publish-release must use a folded if expression")
    expression = " ".join(line.strip() for line in condition.group("lines").splitlines() if line.strip())
    if not expression:
        raise AssertionError("publish-release condition is empty")
    return expression


class RestrictedExpression:
    """Parser for literals, whitelisted dot names, comparisons, booleans, and always()."""

    def __init__(self, expression: str, context: dict):
        self.context = context
        self.tokens = self._tokenize(expression)
        self.position = 0

    @staticmethod
    def _tokenize(expression: str) -> list[str]:
        tokens = []
        position = 0
        while position < len(expression):
            match = TOKEN.match(expression, position)
            if not match:
                raise ValueError(f"Unsupported expression syntax at offset {position}")
            token = match.group()
            position = match.end()
            if not token.isspace():
                tokens.append(token)
        return tokens

    def _peek(self) -> str | None:
        return self.tokens[self.position] if self.position < len(self.tokens) else None

    def _take(self, expected: str | None = None) -> str:
        token = self._peek()
        if token is None or (expected is not None and token != expected):
            raise ValueError(f"Expected {expected or 'expression token'}, found {token!r}")
        self.position += 1
        return token

    def evaluate(self) -> bool:
        value = self._parse_or()
        if self._peek() is not None:
            raise ValueError(f"Unexpected expression token: {self._peek()!r}")
        if not isinstance(value, bool):
            raise ValueError("A publish condition must evaluate to a boolean")
        return value

    def _parse_or(self):
        value = self._parse_and()
        while self._peek() == "||":
            self._take("||")
            right = self._parse_and()
            value = self._boolean(value) or self._boolean(right)
        return value

    def _parse_and(self):
        value = self._parse_comparison()
        while self._peek() == "&&":
            self._take("&&")
            right = self._parse_comparison()
            value = self._boolean(value) and self._boolean(right)
        return value

    def _parse_comparison(self):
        left = self._parse_primary()
        operator = self._peek()
        if operator in ("==", "!="):
            self._take()
            right = self._parse_primary()
            return left == right if operator == "==" else left != right
        return left

    def _parse_primary(self):
        token = self._peek()
        if token == "(":
            self._take("(")
            value = self._parse_or()
            self._take(")")
            return value
        if token is None:
            raise ValueError("Unexpected end of expression")
        if token.startswith("'") and token.endswith("'"):
            return self._take()[1:-1]
        if token == "always":
            self._take("always")
            self._take("(")
            self._take(")")
            return True
        if token in ALLOWED_NAMES:
            self._take()
            value = self.context
            for part in token.split("."):
                if not isinstance(value, dict) or part not in value:
                    raise ValueError(f"Unknown fixture name: {token}")
                value = value[part]
            return value
        raise ValueError(f"Unsupported name or function: {token!r}")

    @staticmethod
    def _boolean(value) -> bool:
        if not isinstance(value, bool):
            raise ValueError("Logical operators accept boolean operands only")
        return value


def evaluate(expression: str, context: dict) -> bool:
    return RestrictedExpression(expression, context).evaluate()


def fixture(*, event="push", ref=MAIN_REF, releases=(), results=None):
    results = results or {}
    release_set = set(releases)
    needs = {
        "linux": {"result": results.get("linux", "success")},
        "release-plan": {
            "result": results.get("release-plan", "success"),
            "outputs": {
                f"release_{component}": str(component in release_set).lower()
                for component in COMPONENTS
            },
        },
        "jetbrains": {"result": results.get("jetbrains", "success")},
        "vscode-package": {"result": results.get("vscode-package", "success")},
        "nagi-package": {"result": results.get("nagi-package", "success")},
    }
    return {"github": {"event_name": event, "ref": ref}, "needs": needs}


class PublishConditionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.expression = publish_expression()

    def test_evaluator_rejects_unlisted_functions_and_expression_features(self):
        for expression in (
            "system('anything')",
            "__import__('os')",
            "github.ref; system('anything')",
            "contains(github.ref, 'main')",
        ):
            with self.subTest(expression=expression), self.assertRaises(ValueError):
                evaluate(expression, fixture())

    def test_main_release_truth_table_covers_component_sets_gates_and_contexts(self):
        rows = [
            (
                "unchanged versions do not publish",
                fixture(releases=(), results={
                    "linux": "success", "jetbrains": "skipped",
                    "vscode-package": "skipped", "nagi-package": "skipped",
                }),
                False,
            ),
            (
                "JetBrains only permits skipped Linux",
                fixture(releases=("jetbrains",), results={
                    "linux": "skipped", "jetbrains": "success",
                    "vscode-package": "skipped", "nagi-package": "skipped",
                }),
                True,
            ),
            (
                "JetBrains only also permits successful Linux",
                fixture(releases=("jetbrains",), results={
                    "linux": "success", "jetbrains": "success",
                    "vscode-package": "skipped", "nagi-package": "skipped",
                }),
                True,
            ),
            (
                "JetBrains only rejects failed Linux",
                fixture(releases=("jetbrains",), results={
                    "linux": "failure", "jetbrains": "success",
                    "vscode-package": "skipped", "nagi-package": "skipped",
                }),
                False,
            ),
            (
                "JetBrains only rejects cancelled Linux",
                fixture(releases=("jetbrains",), results={
                    "linux": "cancelled", "jetbrains": "success",
                    "vscode-package": "skipped", "nagi-package": "skipped",
                }),
                False,
            ),
            (
                "Nagi only requires Linux and its package",
                fixture(releases=("nagi",), results={
                    "linux": "success", "jetbrains": "skipped",
                    "vscode-package": "skipped", "nagi-package": "success",
                }),
                True,
            ),
            (
                "VS Code only requires Linux and its package",
                fixture(releases=("vscode",), results={
                    "linux": "success", "jetbrains": "skipped",
                    "vscode-package": "success", "nagi-package": "skipped",
                }),
                True,
            ),
            (
                "Nagi cannot publish with skipped Linux",
                fixture(releases=("nagi",), results={
                    "linux": "skipped", "jetbrains": "skipped",
                    "vscode-package": "skipped", "nagi-package": "success",
                }),
                False,
            ),
            (
                "VS Code cannot publish with failed Linux",
                fixture(releases=("vscode",), results={
                    "linux": "failure", "jetbrains": "skipped",
                    "vscode-package": "success", "nagi-package": "skipped",
                }),
                False,
            ),
            (
                "JetBrains plus Nagi cannot use the JetBrains-only Linux exception",
                fixture(releases=("jetbrains", "nagi"), results={
                    "linux": "skipped", "jetbrains": "success",
                    "vscode-package": "skipped", "nagi-package": "success",
                }),
                False,
            ),
            (
                "JetBrains plus VS Code cannot use the JetBrains-only Linux exception",
                fixture(releases=("jetbrains", "vscode"), results={
                    "linux": "skipped", "jetbrains": "success",
                    "vscode-package": "success", "nagi-package": "skipped",
                }),
                False,
            ),
            (
                "JetBrains and Nagi publish together after all required checks",
                fixture(releases=("jetbrains", "nagi"), results={
                    "linux": "success", "jetbrains": "success",
                    "vscode-package": "skipped", "nagi-package": "success",
                }),
                True,
            ),
            (
                "all components publish after all required checks",
                fixture(releases=COMPONENTS, results={
                    "linux": "success", "jetbrains": "success",
                    "vscode-package": "success", "nagi-package": "success",
                }),
                True,
            ),
            (
                "Nagi and VS Code retain their Linux requirement when released together",
                fixture(releases=("nagi", "vscode"), results={
                    "linux": "success", "jetbrains": "skipped",
                    "vscode-package": "success", "nagi-package": "success",
                }),
                True,
            ),
            (
                "JetBrains release requires its product matrix job",
                fixture(releases=("jetbrains",), results={
                    "linux": "skipped", "jetbrains": "skipped",
                    "vscode-package": "skipped", "nagi-package": "skipped",
                }),
                False,
            ),
            (
                "Nagi release requires its package job",
                fixture(releases=("nagi",), results={
                    "linux": "success", "jetbrains": "skipped",
                    "vscode-package": "skipped", "nagi-package": "skipped",
                }),
                False,
            ),
            (
                "VS Code release requires its package job",
                fixture(releases=("vscode",), results={
                    "linux": "success", "jetbrains": "skipped",
                    "vscode-package": "skipped", "nagi-package": "skipped",
                }),
                False,
            ),
            (
                "unneeded packages may be skipped for a JetBrains-only release",
                fixture(releases=("jetbrains",), results={
                    "linux": "skipped", "jetbrains": "success",
                    "vscode-package": "skipped", "nagi-package": "skipped",
                }),
                True,
            ),
            (
                "PR never publishes",
                fixture(event="pull_request", releases=COMPONENTS),
                False,
            ),
            (
                "manual runs never publish",
                fixture(event="workflow_dispatch", releases=COMPONENTS),
                False,
            ),
            (
                "tag pushes never publish",
                fixture(ref="refs/tags/nagi-v0.1.0", releases=COMPONENTS),
                False,
            ),
            (
                "non-main pushes never publish",
                fixture(ref="refs/heads/feature", releases=COMPONENTS),
                False,
            ),
        ]
        for name, context, expected in rows:
            with self.subTest(name=name):
                self.assertEqual(evaluate(self.expression, context), expected)

    def test_failure_and_cancellation_of_any_required_or_unneeded_job_blocks_publish(self):
        valid = fixture(releases=COMPONENTS)
        for job in JOBS:
            for status in ("failure", "cancelled"):
                with self.subTest(job=job, status=status):
                    context = copy.deepcopy(valid)
                    context["needs"][job]["result"] = status
                    self.assertFalse(evaluate(self.expression, context))

        unrelated_failure = fixture(releases=("jetbrains",), results={
            "linux": "skipped", "jetbrains": "success",
            "vscode-package": "failure", "nagi-package": "success",
        })
        self.assertFalse(evaluate(self.expression, unrelated_failure))

    def test_release_plan_must_succeed_even_if_required_packages_succeeded(self):
        for status in ("skipped", "failure", "cancelled"):
            with self.subTest(status=status):
                context = fixture(releases=("jetbrains",), results={
                    "linux": "skipped", "release-plan": status, "jetbrains": "success",
                    "vscode-package": "skipped", "nagi-package": "skipped",
                })
                self.assertFalse(evaluate(self.expression, context))


if __name__ == "__main__":
    unittest.main()
