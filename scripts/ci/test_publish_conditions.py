"""Evaluate the real publish-release condition with a small, safe GitHub-expression subset."""
from __future__ import annotations

import copy
import json
import gate
import publication_gate
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MAIN_REF = "refs/heads/main"
COMPONENTS = ("nagi", "vscode", "jetbrains")
SHA = "a" * 40
JOBS = ("ready", "ci-plan", "critical", "go-backend", "changes", "linux", "linux-workspace", "rust-1-88", "sf08-browser", "release-plan",
        "jetbrains", "vscode-package", "nagi-package", "nagi-regression", "windows-contracts")
ALLOWED_NAMES = {
    "github.event_name", "github.ref", "needs.changes.result",
    "needs.ready.result", "needs.ci-plan.result", "needs.ci-plan.outputs.level",
    "needs.linux.result", "needs.rust-1-88.result", "needs.release-plan.result",
    "needs.sf08-browser.result",
    "needs.release-plan.outputs.release_nagi",
    "needs.release-plan.outputs.release_vscode",
    "needs.release-plan.outputs.release_jetbrains",
    "needs.jetbrains.result", "needs.vscode-package.result", "needs.nagi-package.result",
    "needs.nagi-regression.result", "needs.release-plan.outputs.package_nagi",
    "needs.changes.outputs.full_checks", "needs.changes.outputs.jetbrains_checks",
    "needs.release-plan.outputs.package_jetbrains",
    "needs.ci-plan.outputs.comprehensive_checks", "needs.ci-plan.outputs.package_checks",
}
TOKEN = re.compile(
    r"\s+|&&|\|\||==|!=|[()!]|'[^']*'|"
    r"[A-Za-z_][A-Za-z0-9_-]*(?:\.[A-Za-z_][A-Za-z0-9_-]*)*"
)


def job_expression(name: str) -> str:
    """Read only a job's if scalar; do not evaluate YAML or code."""
    workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    job = re.search(
        r"(?ms)^  " + re.escape(name) + r":\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:\s*$|\Z)", workflow
    )
    if not job:
        raise AssertionError(name + " job is missing")
    condition = re.search(
        r"(?m)^    if:\s*>-\s*\n(?P<lines>(?:^      .*\n?)+)", job.group("body")
    )
    if not condition:
        condition = re.search(r"(?m)^    if: (?P<inline>[^\n]+)$", job.group("body"))
        if not condition:
            raise AssertionError(name + " condition is missing")
        expression = condition.group("inline")
    else:
        expression = " ".join(line.strip() for line in condition.group("lines").splitlines() if line.strip())
    if not expression:
        raise AssertionError("publish-release condition is empty")
    return expression


def publish_expression() -> str:
    return job_expression('publish-release')


class RestrictedExpression:
    """Parser for literals, whitelisted names, booleans, and status functions."""

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
        if token == "!":
            self._take("!")
            return not self._boolean(self._parse_primary())
        if token == "(":
            self._take("(")
            value = self._parse_or()
            self._take(")")
            return value
        if token is None:
            raise ValueError("Unexpected end of expression")
        if token.startswith("'") and token.endswith("'"):
            return self._take()[1:-1]
        if token in ("always", "cancelled"):
            self._take()
            self._take("(")
            self._take(")")
            if token == "always":
                return True
            return self._boolean(self.context.get('cancelled'))
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


# Handwritten platform contract, independent of the selector implementation.
RELEASE_MATRIX = {'include': [
    {'os':'ubuntu-latest', 'platform':'linux-x86_64', 'executable':'nagic', 'rust_host':'x86_64-unknown-linux-gnu', 'go_tests':'25'},
    {'os':'windows-latest', 'platform':'windows-x86_64', 'executable':'nagic.exe', 'rust_host':'x86_64-pc-windows-msvc', 'go_tests':'24'},
    {'os':'macos-15', 'platform':'macos-arm64', 'executable':'nagic', 'rust_host':'aarch64-apple-darwin', 'go_tests':'25'},
    {'os':'macos-15-intel', 'platform':'macos-x86_64', 'executable':'nagic', 'rust_host':'x86_64-apple-darwin', 'go_tests':'25'},
]}


def fixture(*, event="push", ref=MAIN_REF, releases=(), results=None, cancelled=False):
    results = results or {}
    releases = set(releases)
    # Each legacy row explicitly chooses whether Nagi analysis was selected.
    full = results.get('linux', 'success') != 'skipped'
    package = results.get('nagi-package', 'success') != 'skipped'
    comprehensive = full or package
    selection = dict(level='3', critical_checks='false',
        comprehensive_checks=str(comprehensive).lower(), package_checks=str(comprehensive).lower(),
        go_checks=str(full or package).lower(),
        matrix=json.dumps(RELEASE_MATRIX, separators=(',', ':')))
    plan = {'sha': SHA}
    for component, job in [('nagi','nagi-package'), ('vscode','vscode-package'), ('jetbrains','jetbrains')]:
        plan['release_'+component] = str(component in releases).lower()
        plan['package_'+component] = str(results.get(job, 'success') != 'skipped').lower()
    needs = {
        'ci-plan': {'result': results.get('ci-plan', 'success'), 'outputs':selection},
        'ready': {'result': results.get('ready', 'success')},
        'critical': {'result': results.get('critical', 'skipped')},
        'changes': {'result': results.get('changes','success'),
                    'outputs':{'full_checks':str(full).lower(), 'jetbrains_checks':'false',
                               'go_checks':str(full or package).lower()}},
        'release-plan': {'result':results.get('release-plan','success'), 'outputs':plan},
    }
    for job in ['linux','sf08-browser','jetbrains','vscode-package','nagi-package']:
        needs[job] = {'result':results.get(job,'success')}
    needs['rust-1-88'] = {'result':results.get('rust-1-88', results.get('linux','success'))}
    needs['go-backend'] = {'result':results.get('go-backend', 'success' if full or package else 'skipped')}
    needs['linux-workspace'] = {'result':results.get('linux-workspace', results.get('linux','success'))}
    needs['nagi-regression'] = {'result':results.get('nagi-regression',
        'skipped' if results.get('nagi-package') == 'skipped' else 'success')}
    needs['windows-contracts'] = {'result':results.get('windows-contracts',
        'skipped' if results.get('nagi-package') == 'skipped' else 'success')}
    return {'github': {'event_name':event, 'ref':ref}, 'needs':needs, 'cancelled':cancelled}


def can_publish(expression, context):
    # Simulate the actual always-running aggregate, then independently run the
    # publication recheck against the original (possibly fictional Ready) needs.
    actual = copy.deepcopy(context)
    if gate.errors(actual['needs']) and actual['needs'].get('ready', {}).get('result') == 'success':
        actual['needs']['ready']['result'] = 'failure'
    try:
        selected = evaluate(expression, actual)
    except ValueError:
        selected = False
    errors = publication_gate.errors(context['needs'], context['github']['event_name'],
                                     context['github']['ref'], SHA)
    return selected and not errors


class PublishConditionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.expression = publish_expression()

    def test_cancelled_function_requires_explicit_boolean_context(self):
        for cancelled in (False, True):
            self.assertEqual(evaluate('cancelled()', {'cancelled': cancelled}), cancelled)
            self.assertEqual(evaluate('!cancelled()', {'cancelled': cancelled}), not cancelled)
        for context in ({}, {'cancelled': None}, {'cancelled': 'false'}, {'cancelled': 0}):
            with self.subTest(context=context), self.assertRaises(ValueError):
                evaluate('!cancelled()', context)

    def test_failure_independent_jobs_stop_on_cancellation(self):
        jobs = ('ci-plan', 'jetbrains', 'sf08-browser', 'nagi-package', 'ready', 'publish-release')
        for event in ('push', 'pull_request'):
            for cancelled in (False, True):
                context = fixture(event=event, releases=COMPONENTS, cancelled=cancelled)
                for job in jobs:
                    with self.subTest(event=event, cancelled=cancelled, job=job):
                        expected = not cancelled and (job != 'publish-release' or event == 'push')
                        self.assertEqual(evaluate(job_expression(job), context), expected)

    def test_ready_still_observes_failed_or_skipped_dependencies_unless_cancelled(self):
        for result in ('failure', 'skipped'):
            context = fixture(releases=COMPONENTS, results={'linux': result})
            self.assertTrue(evaluate(job_expression('ready'), context))
            self.assertTrue(gate.errors(context['needs']))
            context['cancelled'] = True
            self.assertFalse(evaluate(job_expression('ready'), context))

    def test_manual_cancellation_blocks_main_publication_after_successful_ready(self):
        for releases in (('nagi',), ('vscode',), ('jetbrains',), COMPONENTS):
            context = fixture(releases=releases)
            self.assertTrue(can_publish(self.expression, context))
            context['cancelled'] = True
            self.assertEqual(context['needs']['ready']['result'], 'success')
            self.assertFalse(evaluate(self.expression, context))
            self.assertFalse(can_publish(self.expression, context))

    def test_evaluator_rejects_unlisted_functions_and_expression_features(self):
        for expression in (
            "system('anything')",
            "__import__('os')",
            "github.ref; system('anything')",
            "contains(github.ref, 'main')",
            "needs.unlisted.result == 'success'",
            "needs.rust-1-89.result == 'success'",
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
                "JetBrains only permits skipped Linux and browser gates",
                fixture(releases=("jetbrains",), results={
                    "linux": "skipped", "jetbrains": "success",
                    "sf08-browser": "skipped",
                    "vscode-package": "skipped", "nagi-package": "skipped",
                }),
                True,
            ),
            (
                "JetBrains only also permits successful Linux",
                fixture(releases=("jetbrains",), results={
                    "linux": "success", "jetbrains": "success",
                    "vscode-package": "skipped", "nagi-package": "success",
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
                "JetBrains only with skipped analysis jobs still requires browser success unless all three are skipped",
                fixture(releases=("jetbrains",), results={
                    "linux": "success", "rust-1-88": "success", "sf08-browser": "skipped",
                    "jetbrains": "success", "vscode-package": "skipped", "nagi-package": "skipped",
                }),
                False,
            ),
            (
                "JetBrains only cannot skip browser while Linux and Rust succeed",
                fixture(releases=("jetbrains",), results={
                    "linux": "skipped", "rust-1-88": "skipped", "sf08-browser": "success",
                    "jetbrains": "success", "vscode-package": "skipped", "nagi-package": "skipped",
                }),
                False,
            ),
            (
                "pure JetBrains release permits browser skip only with all three analysis jobs skipped",
                fixture(releases=("jetbrains",), results={
                    "linux": "skipped", "rust-1-88": "skipped", "sf08-browser": "skipped",
                    "jetbrains": "success", "vscode-package": "skipped", "nagi-package": "skipped",
                }),
                True,
            ),
            (
                "failed browser job blocks the pure JetBrains exception",
                fixture(releases=("jetbrains",), results={
                    "linux": "skipped", "rust-1-88": "skipped", "sf08-browser": "failure",
                    "jetbrains": "success", "vscode-package": "skipped", "nagi-package": "skipped",
                }),
                False,
            ),
            (
                "cancelled browser job blocks the pure JetBrains exception",
                fixture(releases=("jetbrains",), results={
                    "linux": "skipped", "rust-1-88": "skipped", "sf08-browser": "cancelled",
                    "jetbrains": "success", "vscode-package": "skipped", "nagi-package": "skipped",
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
                    "vscode-package": "success", "nagi-package": "success",
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
                    "linux": "skipped", "sf08-browser": "skipped", "jetbrains": "success",
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
                self.assertEqual(can_publish(self.expression, context), expected)

    def test_failure_and_cancellation_of_any_required_or_unneeded_job_blocks_publish(self):
        valid = fixture(releases=COMPONENTS)
        for job in JOBS:
            for status in ("failure", "cancelled"):
                with self.subTest(job=job, status=status):
                    context = copy.deepcopy(valid)
                    context["needs"][job]["result"] = status
                    self.assertFalse(can_publish(self.expression, context))

        unrelated_failure = fixture(releases=("jetbrains",), results={
            "linux": "skipped", "jetbrains": "success",
            "vscode-package": "failure", "nagi-package": "success",
        })
        self.assertFalse(can_publish(self.expression, unrelated_failure))

    def test_rust_profile_result_is_required_independently_of_linux(self):
        for component in COMPONENTS:
            for status in ("failure", "cancelled", "skipped", "", None):
                with self.subTest(component=component, status=status):
                    context = fixture(releases=(component,), results={"rust-1-88": status})
                    self.assertFalse(can_publish(self.expression, context))
        context = fixture(releases=COMPONENTS)
        del context["needs"]["rust-1-88"]
        self.assertTrue(publication_gate.errors(context["needs"], "push", MAIN_REF, SHA))
        self.assertFalse(can_publish(self.expression, context))

    def test_jetbrains_skip_exception_requires_all_three_analysis_jobs_skipped(self):
        for linux_status in ("success", "skipped"):
            for rust_status in ("success", "skipped", "failure", "cancelled", "", None):
                for browser_status in ("success", "skipped", "failure", "cancelled", "", None):
                    with self.subTest(linux=linux_status, rust=rust_status, browser=browser_status):
                        context = fixture(releases=("jetbrains",), results={
                            "linux": linux_status, "rust-1-88": rust_status,
                            "sf08-browser": browser_status,
                            "vscode-package": "skipped",
                            "nagi-package": "success" if linux_status == "success" else "skipped",
                        })
                        expected = (
                            (linux_status, rust_status, browser_status) == ("success", "success", "success")
                            or (linux_status, rust_status, browser_status) == ("skipped", "skipped", "skipped")
                        )
                        self.assertEqual(can_publish(self.expression, context), expected)

    def test_nagi_regression_plan_and_result_must_match_exactly(self):
        for planned in ("true", "false", None, "", True, False):
            for result in ("success", "skipped", "failure", "cancelled", "", None):
                with self.subTest(planned=planned, result=result):
                    context = fixture(releases=("nagi",))
                    context["needs"]["release-plan"]["outputs"]["package_nagi"] = planned
                    context["needs"]["nagi-regression"]["result"] = result
                    expected = (planned, result) == ("true", "success")
                    self.assertEqual(can_publish(self.expression, context), expected)
        # A release of another component may skip the unplanned Nagi shard.
        context = fixture(releases=("vscode",), results={"linux":"skipped", "sf08-browser":"skipped",
            "nagi-package":"skipped", "jetbrains":"skipped"})
        self.assertTrue(can_publish(self.expression, context))
        context = fixture(releases=("nagi",))
        del context["needs"]["nagi-regression"]
        self.assertTrue(publication_gate.errors(context["needs"], "push", MAIN_REF, SHA))
        self.assertFalse(can_publish(self.expression, context))
        context = fixture(releases=("nagi",))
        del context["needs"]["release-plan"]["outputs"]["package_nagi"]
        self.assertTrue(publication_gate.errors(context["needs"], "push", MAIN_REF, SHA))
        self.assertFalse(can_publish(self.expression, context))

    def test_fictionally_successful_ready_cannot_hide_failed_cancelled_or_required_skipped_children(self):
        valid = fixture(releases=COMPONENTS)
        for job in JOBS:
            if job == 'ready':continue
            for status in ['failure', 'cancelled', 'skipped']:
                if valid['needs'][job]['result'] == status:continue
                context=copy.deepcopy(valid);context['needs'][job]['result']=status
                self.assertEqual(context['needs']['ready']['result'], 'success')
                self.assertTrue(publication_gate.errors(context['needs'], 'push', MAIN_REF, SHA), (job,status))
                self.assertFalse(can_publish(self.expression, context), (job,status))

    def test_release_plan_must_succeed_even_if_required_packages_succeeded(self):
        for status in ("skipped", "failure", "cancelled"):
            with self.subTest(status=status):
                context = fixture(releases=("jetbrains",), results={
                    "linux": "skipped", "release-plan": status, "jetbrains": "success",
                    "vscode-package": "skipped", "nagi-package": "skipped",
                })
                self.assertFalse(can_publish(self.expression, context))


if __name__ == "__main__":
    unittest.main()
