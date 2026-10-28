"""Undo only the reviewed High elif harness addition before frozen CI proofs.

The complete inherited command remains the contract: no test is removed or
rewritten, and the new harness must occur exactly once in its regression job.
"""
import importlib.util
from pathlib import Path
import re


_spec = importlib.util.spec_from_file_location(
    "parallel_regression_projection", Path(__file__).with_name("parallel_regression_projection.py"))
_parallel = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_parallel)

ORIGINAL_STEP = _parallel.CLI_STEP
_ANCHOR = "--test expression_contracts --test frontend_contracts"
if ORIGINAL_STEP.count(_ANCHOR) != 1:
    raise ValueError("High elif inherited command anchor mismatch")
ADDED_STEP = ORIGINAL_STEP.replace(
    _ANCHOR, "--test expression_contracts --test high_elif --test frontend_contracts", 1)
_ORIGINAL_COMMAND = ORIGINAL_STEP.split("        run: ", 1)[1]


def inverse_workflow(source):
    jobs = list(re.finditer(
        r"(?ms)^  nagi-regression:\n.*?(?=^  [a-zA-Z0-9_-]+:|\Z)", source))
    if len(jobs) != 1:
        raise ValueError("High elif regression job count mismatch")
    job = jobs[0]
    if (source.count("--test high_elif") != 1 or source.count(ADDED_STEP) != 1
            or job.group().count(ADDED_STEP) != 1
            or _ORIGINAL_COMMAND in source):
        raise ValueError("High elif regression command registration mismatch")
    restored = job.group().replace(ADDED_STEP, ORIGINAL_STEP, 1)
    return source[:job.start()] + restored + source[job.end():]
