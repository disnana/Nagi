"""Closed acceptance check for the pinned Playwright 1.64.1 JSON report.

This module accepts an already-parsed JSON-compatible value. File ownership,
byte limits, duplicate JSON-key detection, and process/report freshness belong
to the coordinator that reads the report. The return value deliberately
contains only a fixed summary; no report values are copied into it.
"""

from __future__ import annotations

import math
from typing import Any


PROJECTS = ("chromium", "firefox", "webkit")
CASES = (
    "TLS bootstrap sets the strict prelogin cookie and managed response headers",
    "browser CSRF rejection preserves a claim and isolated replay refuses the consumed cookie",
    "browser enforces allowed credentialed CORS preflight and response access",
    "unapproved owned loopback origin cannot read a CORS response",
)
EXPECTED_EXECUTIONS = len(PROJECTS) * len(CASES)


class ReportValidationError(ValueError):
    """A stable, non-data-bearing rejection code for a malformed/nonpassing report."""


def _reject(code: str) -> None:
    raise ReportValidationError(code)


def _object(value: Any, code: str) -> dict[str, Any]:
    if type(value) is not dict:
        _reject(code)
    return value


def _array(value: Any, code: str) -> list[Any]:
    if type(value) is not list:
        _reject(code)
    return value


def _field(obj: dict[str, Any], key: str, code: str) -> Any:
    if key not in obj:
        _reject(code)
    return obj[key]


def _string(value: Any, code: str) -> str:
    if type(value) is not str:
        _reject(code)
    return value


def _finite_number(value: Any, code: str) -> int | float:
    if type(value) not in (int, float):
        _reject(code)
    try:
        finite = math.isfinite(value)
    except OverflowError:
        finite = False
    if not finite:
        _reject(code)
    return value


def _counter(value: Any, expected: int, code: str) -> None:
    # bool is a subclass of int in Python, but it is not a JSON numeric count.
    if type(value) is not int or value != expected:
        _reject(code)


def _string_array(value: Any, code: str) -> list[str]:
    items = _array(value, code)
    if any(type(item) is not str for item in items):
        _reject(code)
    return items


def _validate_project_rows(config: dict[str, Any]) -> dict[str, str]:
    rows = _array(_field(config, "projects", "config_projects_missing"), "config_projects_shape")
    if len(rows) != len(PROJECTS):
        _reject("config_project_count")

    name_to_id: dict[str, str] = {}
    ids: set[str] = set()
    for raw in rows:
        row = _object(raw, "config_project_shape")
        # These are fields required by JSONReport's narrowed project object.
        name = _string(_field(row, "name", "config_project_name_missing"), "config_project_name_shape")
        project_id = _string(_field(row, "id", "config_project_id_missing"), "config_project_id_shape")
        _string(_field(row, "outputDir", "config_project_output_missing"), "config_project_output_shape")
        _string(_field(row, "testDir", "config_project_testdir_missing"), "config_project_testdir_shape")
        _counter(_field(row, "repeatEach", "config_project_repeat_missing"), 1, "config_project_repeat")
        _counter(_field(row, "retries", "config_project_retries_missing"), 0, "config_project_retries")
        _finite_number(_field(row, "timeout", "config_project_timeout_missing"), "config_project_timeout_shape")
        if type(_field(row, "metadata", "config_project_metadata_missing")) is not dict:
            _reject("config_project_metadata_shape")
        _string_array(_field(row, "testIgnore", "config_project_ignore_missing"), "config_project_ignore_shape")
        _string_array(_field(row, "testMatch", "config_project_match_missing"), "config_project_match_shape")

        if name not in PROJECTS:
            _reject("config_project_name_unknown")
        if not project_id or name in name_to_id or project_id in ids:
            _reject("config_project_duplicate_identity")
        name_to_id[name] = project_id
        ids.add(project_id)

    if set(name_to_id) != set(PROJECTS):
        _reject("config_project_set")
    return name_to_id


def _validate_result(raw: Any) -> None:
    result = _object(raw, "result_shape")
    _finite_number(_field(result, "workerIndex", "result_worker_missing"), "result_worker_shape")
    _finite_number(_field(result, "parallelIndex", "result_parallel_missing"), "result_parallel_shape")
    _finite_number(_field(result, "duration", "result_duration_missing"), "result_duration_shape")
    if _string(_field(result, "status", "result_status_missing"), "result_status_shape") != "passed":
        _reject("result_not_passed")
    _string(_field(result, "startTime", "result_start_missing"), "result_start_shape")
    _counter(_field(result, "retry", "result_retry_missing"), 0, "result_retry")

    errors = _array(_field(result, "errors", "result_errors_missing"), "result_errors_shape")
    if errors:
        _reject("result_has_errors")
    _array(_field(result, "stdout", "result_stdout_missing"), "result_stdout_shape")
    _array(_field(result, "stderr", "result_stderr_missing"), "result_stderr_shape")
    _array(_field(result, "attachments", "result_attachments_missing"), "result_attachments_shape")
    _array(_field(result, "annotations", "result_annotations_missing"), "result_annotations_shape")
    # TestResult.error is typed TestError | undefined. JSON.stringify omits an
    # undefined value; an error object or JSON null contradicts status=passed.
    if "error" in result:
        _reject("result_error_present")


def validate_report(report: Any) -> dict[str, int | bool]:
    """Accept exactly the 4-case x 3-project successful run, returning a closed summary."""
    root = _object(report, "report_shape")

    global_errors = _array(_field(root, "errors", "global_errors_missing"), "global_errors_shape")
    if global_errors:
        _reject("global_errors_present")

    stats = _object(_field(root, "stats", "stats_missing"), "stats_shape")
    _string(_field(stats, "startTime", "stats_start_missing"), "stats_start_shape")
    _finite_number(_field(stats, "duration", "stats_duration_missing"), "stats_duration_shape")
    for key, expected in (
        ("expected", EXPECTED_EXECUTIONS),
        ("unexpected", 0),
        ("flaky", 0),
        ("skipped", 0),
    ):
        _counter(_field(stats, key, f"stats_{key}_missing"), expected, f"stats_{key}")

    config = _object(_field(root, "config", "config_missing"), "config_shape")
    project_ids = _validate_project_rows(config)
    cases_seen: set[tuple[str, str]] = set()

    def visit_suite(raw_suite: Any) -> None:
        suite = _object(raw_suite, "suite_shape")
        _string(_field(suite, "title", "suite_title_missing"), "suite_title_shape")
        _string(_field(suite, "file", "suite_file_missing"), "suite_file_shape")
        _finite_number(_field(suite, "line", "suite_line_missing"), "suite_line_shape")
        _finite_number(_field(suite, "column", "suite_column_missing"), "suite_column_shape")

        specs = _array(_field(suite, "specs", "suite_specs_missing"), "suite_specs_shape")
        for raw_spec in specs:
            spec = _object(raw_spec, "spec_shape")
            title = _string(_field(spec, "title", "spec_title_missing"), "spec_title_shape")
            if title not in CASES:
                _reject("spec_title_unknown")
            _string(_field(spec, "id", "spec_id_missing"), "spec_id_shape")
            _string(_field(spec, "file", "spec_file_missing"), "spec_file_shape")
            _finite_number(_field(spec, "line", "spec_line_missing"), "spec_line_shape")
            _finite_number(_field(spec, "column", "spec_column_missing"), "spec_column_shape")
            _string_array(_field(spec, "tags", "spec_tags_missing"), "spec_tags_shape")
            if type(_field(spec, "ok", "spec_ok_missing")) is not bool or spec["ok"] is not True:
                _reject("spec_not_ok")

            tests = _array(_field(spec, "tests", "spec_tests_missing"), "spec_tests_shape")
            for raw_test in tests:
                test = _object(raw_test, "test_shape")
                _finite_number(_field(test, "timeout", "test_timeout_missing"), "test_timeout_shape")
                _array(_field(test, "annotations", "test_annotations_missing"), "test_annotations_shape")
                if _string(_field(test, "expectedStatus", "test_expected_missing"), "test_expected_shape") != "passed":
                    _reject("test_expected_status")
                if _string(_field(test, "status", "test_status_missing"), "test_status_shape") != "expected":
                    _reject("test_not_expected")

                project_name = _string(_field(test, "projectName", "test_project_missing"), "test_project_shape")
                project_id = _string(_field(test, "projectId", "test_project_id_missing"), "test_project_id_shape")
                if project_name not in project_ids or project_ids[project_name] != project_id:
                    _reject("test_project_identity")
                pair = (title, project_name)
                if pair in cases_seen:
                    _reject("duplicate_case_project")

                results = _array(_field(test, "results", "test_results_missing"), "test_results_shape")
                if len(results) != 1:
                    _reject("test_result_count")
                _validate_result(results[0])
                cases_seen.add(pair)

        # JSONReportSuite.suites is optional, while specs is required.
        children = suite.get("suites", [])
        for child in _array(children, "suite_children_shape"):
            visit_suite(child)

    suites = _array(_field(root, "suites", "suites_missing"), "suites_shape")
    for suite in suites:
        visit_suite(suite)

    expected_pairs = {(title, project) for title in CASES for project in PROJECTS}
    if cases_seen != expected_pairs:
        _reject("execution_set_mismatch")

    return {
        "accepted": True,
        "cases": len(CASES),
        "projects": len(PROJECTS),
        "executions": EXPECTED_EXECUTIONS,
    }


FAILURE_RESULT_STATUSES = frozenset(("passed", "failed", "timedOut", "interrupted", "skipped"))
FAILURE_TEST_STATUSES = {
    "passed": "expected",
    "failed": "unexpected",
    "timedOut": "unexpected",
    "interrupted": "unexpected",
    "skipped": "skipped",
}
FAILURE_PROJECTION_KEYS = {"schema_version", "status", "rows"}
FAILURE_ROW_KEYS = {"project_index", "case_index", "status", "count"}


def unavailable_failure_projection() -> dict[str, Any]:
    return {"schema_version": 1, "status": "unavailable", "rows": []}


def validate_failure_projection(value: Any) -> dict[str, Any]:
    """Validate and rebuild the closed, non-success diagnostic sidecar schema."""
    root = _object(value, "failure_projection_shape")
    if set(root) != FAILURE_PROJECTION_KEYS:
        _reject("failure_projection_keys")
    _counter(_field(root, "schema_version", "failure_projection_version_missing"), 1,
             "failure_projection_version")
    status = _string(_field(root, "status", "failure_projection_status_missing"),
                     "failure_projection_status_shape")
    rows = _array(_field(root, "rows", "failure_projection_rows_missing"),
                  "failure_projection_rows_shape")
    if status == "unavailable":
        if rows:
            _reject("failure_projection_unavailable_rows")
        return unavailable_failure_projection()
    if status != "available" or len(rows) != EXPECTED_EXECUTIONS:
        _reject("failure_projection_status_or_count")

    checked = []
    failures = 0
    for index, raw in enumerate(rows):
        row = _object(raw, "failure_projection_row_shape")
        if set(row) != FAILURE_ROW_KEYS:
            _reject("failure_projection_row_keys")
        project_index = _field(row, "project_index", "failure_projection_project_missing")
        case_index = _field(row, "case_index", "failure_projection_case_missing")
        if type(project_index) is not int or type(case_index) is not int:
            _reject("failure_projection_index_shape")
        expected_project, expected_case = divmod(index, len(CASES))
        if (project_index, case_index) != (expected_project, expected_case):
            _reject("failure_projection_index_order")
        result_status = _string(_field(row, "status", "failure_projection_result_missing"),
                                "failure_projection_result_shape")
        if result_status not in FAILURE_RESULT_STATUSES:
            _reject("failure_projection_result_unknown")
        _counter(_field(row, "count", "failure_projection_count_missing"), 1,
                 "failure_projection_count")
        if result_status in ("failed", "timedOut", "interrupted"):
            failures += 1
        checked.append({"project_index": project_index, "case_index": case_index,
                        "status": result_status, "count": 1})
    if failures == 0:
        _reject("failure_projection_no_failure")
    return {"schema_version": 1, "status": "available", "rows": checked}


def project_failure(report: Any) -> dict[str, Any]:
    """Project only closed case/project indices and fixed result enums.

    No TestError content, title, file name, path, or report string is copied to
    the returned value. A pass-only report cannot become an available failure
    diagnostic.
    """
    try:
        root = _object(report, "failure_report_shape")
        if _array(_field(root, "errors", "failure_global_errors_missing"),
                  "failure_global_errors_shape"):
            _reject("failure_global_errors_present")

        stats = _object(_field(root, "stats", "failure_stats_missing"), "failure_stats_shape")
        _string(_field(stats, "startTime", "failure_stats_start_missing"), "failure_stats_start_shape")
        _finite_number(_field(stats, "duration", "failure_stats_duration_missing"),
                       "failure_stats_duration_shape")

        config = _object(_field(root, "config", "failure_config_missing"), "failure_config_shape")
        project_ids = _validate_project_rows(config)
        observed: dict[tuple[str, str], str] = {}

        def visit_suite(raw_suite: Any) -> None:
            suite = _object(raw_suite, "failure_suite_shape")
            specs = _array(_field(suite, "specs", "failure_suite_specs_missing"),
                           "failure_suite_specs_shape")
            for raw_spec in specs:
                spec = _object(raw_spec, "failure_spec_shape")
                title = _string(_field(spec, "title", "failure_spec_title_missing"),
                                "failure_spec_title_shape")
                if title not in CASES:
                    _reject("failure_spec_title_unknown")
                tests = _array(_field(spec, "tests", "failure_spec_tests_missing"),
                               "failure_spec_tests_shape")
                if not tests:
                    _reject("failure_spec_tests_empty")
                spec_has_unexpected = False
                for raw_test in tests:
                    test = _object(raw_test, "failure_test_shape")
                    if _string(_field(test, "expectedStatus", "failure_expected_missing"),
                               "failure_expected_shape") != "passed":
                        _reject("failure_expected_status")
                    project_name = _string(_field(test, "projectName", "failure_project_missing"),
                                           "failure_project_shape")
                    project_id = _string(_field(test, "projectId", "failure_project_id_missing"),
                                         "failure_project_id_shape")
                    if project_name not in project_ids or project_ids[project_name] != project_id:
                        _reject("failure_project_identity")
                    pair = (project_name, title)
                    if pair in observed:
                        _reject("failure_duplicate_case_project")

                    results = _array(_field(test, "results", "failure_results_missing"),
                                     "failure_results_shape")
                    if len(results) != 1:
                        _reject("failure_result_count")
                    result = _object(results[0], "failure_result_shape")
                    result_status = _string(_field(result, "status", "failure_result_status_missing"),
                                            "failure_result_status_shape")
                    if result_status not in FAILURE_RESULT_STATUSES:
                        _reject("failure_result_status_unknown")
                    if _string(_field(test, "status", "failure_test_status_missing"),
                               "failure_test_status_shape") != FAILURE_TEST_STATUSES[result_status]:
                        _reject("failure_test_result_status_mismatch")
                    _counter(_field(result, "retry", "failure_retry_missing"), 0,
                             "failure_retry")
                    errors = _array(_field(result, "errors", "failure_errors_missing"),
                                    "failure_errors_shape")
                    if result_status == "passed" and errors:
                        _reject("failure_pass_has_errors")
                    if result_status == "passed" and "error" in result:
                        _reject("failure_pass_has_error")
                    for key in ("stdout", "stderr", "attachments", "annotations"):
                        _array(_field(result, key, "failure_result_" + key + "_missing"),
                               "failure_result_" + key + "_shape")
                    observed[pair] = result_status
                    if result_status in ("failed", "timedOut", "interrupted"):
                        spec_has_unexpected = True
                spec_ok = _field(spec, "ok", "failure_spec_ok_missing")
                if type(spec_ok) is not bool:
                    _reject("failure_spec_ok_shape")
                if spec_has_unexpected and spec_ok is not False:
                    _reject("failure_spec_ok_mismatch")

            for child in _array(suite.get("suites", []), "failure_suite_children_shape"):
                visit_suite(child)

        for suite in _array(_field(root, "suites", "failure_suites_missing"),
                            "failure_suites_shape"):
            visit_suite(suite)

        expected_pairs = {(project, case) for project in PROJECTS for case in CASES}
        if set(observed) != expected_pairs:
            _reject("failure_execution_set")
        passing = sum(status == "passed" for status in observed.values())
        failing = sum(status in ("failed", "timedOut", "interrupted") for status in observed.values())
        skipped = sum(status == "skipped" for status in observed.values())
        for key, expected in (("expected", passing), ("unexpected", failing),
                              ("flaky", 0), ("skipped", skipped)):
            _counter(_field(stats, key, "failure_stats_" + key + "_missing"), expected,
                     "failure_stats_" + key)

        rows = []
        for project_index, project in enumerate(PROJECTS):
            for case_index, case in enumerate(CASES):
                rows.append({"project_index": project_index, "case_index": case_index,
                             "status": observed[(project, case)], "count": 1})
        return validate_failure_projection({"schema_version": 1, "status": "available", "rows": rows})
    except (ReportValidationError, RecursionError):
        return unavailable_failure_projection()
