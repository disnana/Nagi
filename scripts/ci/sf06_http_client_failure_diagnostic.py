"""Observe a failed SF06 guard using closed identifiers; never authorize a pass."""
from __future__ import annotations

import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile

INVENTORY = json.loads(Path(__file__).with_name("sf06_http_client_failure_inventory.json").read_text())
LIMIT = 256 * 1024
COUNT_LIMIT = 10000
ITEM_LIMIT = 16
ROW = re.compile(r"^test ([A-Za-z0-9_:]{1,512}) \.\.\. (ok|FAILED|ignored)$", re.M)
RUNNING = re.compile(r"^running ([0-9]{1,6}) tests?$", re.M)
RESULT = re.compile(r"^test result: (ok|FAILED)\. ([0-9]{1,6}) passed; ([0-9]{1,6}) failed; ([0-9]{1,6}) ignored; [0-9]{1,6} measured; [0-9]{1,6} filtered out; finished in [^\r\n]{1,80}$", re.M)
# Current Rust may include a numeric thread ID. Neither name nor ID is captured.
PANIC = re.compile(r"^thread '[^'\r\n]{1,512}'(?: \([0-9]{1,20}\))? panicked at ([^\r\n]{1,1024}\.rs):([1-9][0-9]{0,5}):([1-9][0-9]{0,3}):$", re.M)
CODE = re.compile(r"^error\[(E[0-9]{4})\]:", re.M)
TERMINATIONS = {
    "windows_access_violation": r"exit code: 0xc0000005(?:, STATUS_ACCESS_VIOLATION)?",
    "windows_stack_buffer_overrun": r"exit code: 0xc0000409(?:, STATUS_STACK_BUFFER_OVERRUN)?",
    "windows_stack_overflow": r"exit code: 0xc00000fd(?:, STATUS_STACK_OVERFLOW)?",
    "windows_dll_not_found": r"exit code: 0xc0000135(?:, STATUS_DLL_NOT_FOUND)?",
    "windows_control_c": r"exit code: 0xc000013a(?:, STATUS_CONTROL_C_EXIT)?",
    "unix_sigkill": r"signal: 9, SIGKILL(?:: [^\r\n)]{1,80})?",
    "unix_sigsegv": r"signal: 11, SIGSEGV(?:: [^\r\n)]{1,80})?",
    "unix_sigabrt": r"signal: 6, SIGABRT(?:: [^\r\n)]{1,80})?",
    "unix_sigterm": r"signal: 15, SIGTERM(?:: [^\r\n)]{1,80})?",
}


def bounded_read(path: Path) -> tuple[str | None, str]:
    try:
        if not stat.S_ISREG(path.lstat().st_mode):
            return None, "not_regular"
        with path.open("rb") as stream:
            data = stream.read(LIMIT + 1)
    except OSError:
        return None, "unreadable"
    if len(data) > LIMIT:
        return None, "oversize"
    try:
        return data.decode("utf-8", "strict"), "ok"
    except UnicodeError:
        return None, "invalid_utf8"


def exit_code(value: str) -> int | None:
    return int(value) if re.fullmatch(r"[0-9]{1,3}", value) and int(value) <= 255 else None


def analyze(suite: str, cargo_exit: int | None, guard_exit: int | None,
            text: str | None, read_state: str) -> dict:
    known = set(INVENTORY["tests"].get(suite, ()))
    result = {
        "schema": "sf06-http-client-check-failure-v1",
        "suite": suite if known else "unknown",
        "cargo_exit": cargo_exit,
        "guard_exit": guard_exit,
        "read_state": read_state,
        "parse_state": "unavailable",
        "expected_count": len(known) if known else None,
        "running_count": None, "result_kind": "missing",
        "passed_count": None, "failed_count": None, "ignored_count": None,
        "failed_test_ids": [], "panic_sources": [], "rust_error_codes": [], "process_terminations": [],
        "unknown_test_rows": 0, "unknown_panics": 0, "item_loss": False,
    }
    if not known or text is None:
        return result
    # Rust's Windows output uses CRLF. Normalize only line endings; messages are
    # never echoed, persisted, evaluated, or used to relax the original guard.
    text = text.replace("\r\n", "\n")
    rows = ROW.findall(text)
    running = RUNNING.findall(text)
    summaries = RESULT.findall(text)
    result["unknown_test_rows"] = min(COUNT_LIMIT, sum(name not in known for name, _ in rows))
    failed = sorted({name for name, outcome in rows if outcome == "FAILED" and name in known})
    result["failed_test_ids"] = failed[:ITEM_LIMIT]
    if len(running) == 1 and int(running[0]) <= COUNT_LIMIT:
        result["running_count"] = int(running[0])
    if len(summaries) == 1 and all(int(n) <= COUNT_LIMIT for n in summaries[0][1:]):
        kind, passed, failures, ignored = summaries[0]
        result.update(result_kind=kind, passed_count=int(passed), failed_count=int(failures), ignored_count=int(ignored))
    elif len(summaries) > 1:
        result["result_kind"] = "multiple"
    names = [name for name, _ in rows]
    complete = (result["running_count"] == len(known) and len(summaries) == 1
                and len(rows) == len(known) and len(set(names)) == len(known)
                and set(names) == known
                and result["passed_count"] is not None
                and all(sum(outcome == state for _, outcome in rows) == result[key]
                        for state, key in (("ok", "passed_count"), ("FAILED", "failed_count"), ("ignored", "ignored_count"))))
    result["parse_state"] = "complete" if complete else "incomplete"
    sources = []
    unknown = 0
    for path, line, column in PANIC.findall(text):
        path = path.replace("\\", "/")
        matches = [name for name, maximum in INVENTORY["panic_sources"].items()
                   if (path == name or path.endswith("/" + name)) and int(line) <= maximum]
        if len(matches) != 1 or int(column) > 4096:
            unknown += 1
        else:
            location = {"file": matches[0], "line": int(line), "column": int(column)}
            if location not in sources:
                sources.append(location)
    codes = sorted(set(CODE.findall(text)))
    terminations = sorted(name for name, pattern in TERMINATIONS.items() if re.search(
        r"^\s*process didn't exit successfully: [^\r\n]{1,2048} \(" + pattern + r"\)$", text, re.M | re.I))
    result.update(panic_sources=sources[:ITEM_LIMIT], rust_error_codes=codes[:ITEM_LIMIT],
                  process_terminations=terminations,
                  unknown_panics=min(COUNT_LIMIT, unknown),
                  item_loss=any(len(items) > ITEM_LIMIT for items in (failed, sources, codes)))
    return result


def publish(record: dict, temporary_root: Path) -> bool:
    """Only this invocation's fully written closed record can be uploaded."""
    output = os.environ.get("GITHUB_OUTPUT")
    if not output or any(char in str(temporary_root) for char in "\r\n"):
        return False
    try:
        directory = Path(tempfile.mkdtemp(prefix="sf06-check-closed-", dir=temporary_root))
        destination = directory / "diagnostic.json"
        payload = (json.dumps(record, sort_keys=True) + "\n").encode("utf-8")
        if len(payload) > 16384:
            return False
        with destination.open("xb") as stream:
            stream.write(payload)
        # Children cannot set this field: the workflow strips GITHUB_OUTPUT from
        # Cargo and the existing guard, and publishes only after this closed write.
        with open(output, "a", encoding="utf-8", newline="\n") as stream:
            # A truncated output write must not become an ancestor-directory
            # upload path. GitHub requires the complete multiline terminator.
            stream.write(f"closed_failure_diagnostic<<SF06_CLOSED_JSON\n{destination}\nSF06_CLOSED_JSON\n")
        return True
    except OSError:
        return False


def main(args: list[str]) -> int:
    if len(args) != 5:
        print("sf06_http_client_failure_diag=v1 status=unavailable reason=arguments artifact_saved=false")
        return 1
    suite, cargo, guard, path, temporary_root = args
    cargo, guard = exit_code(cargo), exit_code(guard)
    text, read_state = bounded_read(Path(path))
    record = analyze(suite, cargo, guard, text, read_state)
    saved = False
    if suite in INVENTORY["tests"] and cargo is not None and guard not in (None, 0):
        saved = publish(record, Path(temporary_root))
    print(f"sf06_http_client_failure_diag=v1 suite={record['suite']} cargo_exit={cargo} "
          f"guard_exit={guard} read={read_state} parse={record['parse_state']} "
          f"failed_known={len(record['failed_test_ids'])} artifact_saved={'true' if saved else 'false'}")
    return 0 if saved else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
