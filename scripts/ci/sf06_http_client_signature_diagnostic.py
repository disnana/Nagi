#!/usr/bin/env python3
"""Classify one fixed SF06 native-test panic without rerunning Cargo."""
from __future__ import annotations

import json
import os
import re
import stat
import sys
import tempfile
from pathlib import Path
from typing import Any

SCHEMA = "sf06-http-client-signature-diagnostic-v1"
TEST = "public_signatures_and_failure_projections_build_and_run_in_three_forms"
CASE = "http-client-native-contract"
FORMS = ("high", "saved-low", "handwritten-low")
LOG_LIMIT = 256 * 1024
STDERR_LIMIT = 64 * 1024
ROW = re.compile(rf"^test {re.escape(TEST)} \.\.\. (ok|FAILED|ignored)$", re.M)
GROUP = f"---- {TEST} stdout ----"
ANY_GROUP = re.compile(r"^---- [A-Za-z0-9_:]+ stdout ----$")
PANIC = re.compile(r"^thread '([^']+)' panicked at .+:[0-9]+:[0-9]+:$")
RUST_CODE = re.compile(r"E[0-9]{4}")


def record(**values: Any) -> dict[str, Any]:
    result = {
        "schema": SCHEMA,
        "test_name": TEST,
        "status": "unavailable",
        "unavailable_reason": "internal_error",
        "failure_stage": "unavailable",
        "form": "unavailable",
        "rust_error_code": None,
        "rust_error_code_status": "not_applicable",
        "newline": "unavailable",
        "input_bytes_read": 0,
        "artifact_saved": False,
    }
    result.update(values)
    return result


def unavailable(reason: str, size: int = 0) -> dict[str, Any]:
    return record(unavailable_reason=reason, input_bytes_read=size)


def bounded_read(path: Path, limit: int) -> tuple[bytes | None, str, int]:
    try:
        with path.open("rb") as stream:
            data = stream.read(limit + 1)
    except (OSError, ValueError):
        return None, "unreadable", 0
    return (None, "oversize", len(data)) if len(data) > limit else (data, "ok", len(data))


def decode_log(raw: bytes) -> tuple[str | None, str]:
    if b"\r" in raw:
        return None, "lone_cr" if b"\r" in raw.replace(b"\r\n", b"") else "crlf"
    try:
        return raw.decode("utf-8", "strict"), "ok"
    except UnicodeError:
        return None, "invalid_utf8"


def regular(path: Path) -> bool:
    try:
        return stat.S_ISREG(path.lstat().st_mode)
    except OSError:
        return False


def current_form(root: Path | None, *, require_build: bool) -> str:
    if root is None or not root.is_dir():
        return "unavailable"
    completed = [regular(root / f"explicit-move-{CASE}-{form}.stdout") for form in FORMS]
    first_pending = next((i for i, done in enumerate(completed) if not done), len(FORMS))
    if any(completed[first_pending:]) or first_pending == len(FORMS):
        return "unavailable"
    form = FORMS[first_pending]
    if require_build and not all(
        regular(root / f"explicit-move-{CASE}-{form}-{suffix}")
        for suffix in ("generated.rs", "build.stdout", "build.stderr")
    ):
        return "unavailable"
    return form


def classify(message: str, root: Path | None) -> tuple[str, str]:
    checks = {
        "High check before backend:": ("high_check", "high"),
        "independent saved Low check:": ("saved_low_check", "saved-low"),
        "handwritten Low check:": ("handwritten_low_check", "handwritten-low"),
        "saved Low after High removal:": ("saved_low_recheck", "saved-low"),
    }
    for prefix, outcome in checks.items():
        if message.startswith(prefix):
            return outcome
    if message.startswith("sealed Rust emission:"):
        return "sealed_emission", current_form(root, require_build=False)

    for form in FORMS:
        if message == f"{form} backend Cargo build:":
            return "cargo_build", form
        if message == f"{form} backend run requires a nonempty successful libtest summary:":
            return "cargo_run", form
        for operation in ("build", "run"):
            if message.startswith(f"{form}-{operation} timed out:"):
                return f"{operation}_timeout", form
            if message.startswith(f"{form}-{operation} spawn:"):
                return f"{operation}_spawn", form
    if message.startswith("fixture changed locked dependency:"):
        return "locked_dependencies", current_form(root, require_build=True)
    if message == "Cargo must report this fixture's actual test executable":
        return "cargo_executable", current_form(root, require_build=True)
    if message.startswith("exclusive move fixture:"):
        return "fixture_setup", "unavailable"
    return "unavailable", "unavailable"


def rust_error_code(root: Path | None, form: str) -> tuple[str | None, str]:
    if root is None or form not in FORMS:
        return None, "unavailable_form"
    raw, state, _size = bounded_read(
        root / f"explicit-move-{CASE}-{form}-build.stdout", STDERR_LIMIT
    )
    if state == "oversize":
        return None, "unavailable_oversize"
    if raw is None:
        return None, "unavailable_missing"
    text, decode_state = decode_log(raw)
    if decode_state != "ok" or text is None:
        return None, f"unavailable_{decode_state}"

    codes = []
    for line in text.splitlines():
        try:
            event = json.loads(line)
        except (json.JSONDecodeError, TypeError):
            return None, "unavailable_malformed_json"
        if not isinstance(event, dict) or event.get("reason") != "compiler-message":
            continue
        message = event.get("message")
        if not isinstance(message, dict):
            return None, "unavailable_message_schema"
        if message.get("level") != "error":
            continue
        detail = message.get("code")
        if detail is None:
            continue
        code = detail.get("code") if isinstance(detail, dict) else None
        if not isinstance(code, str) or RUST_CODE.fullmatch(code) is None:
            return None, "unavailable_unknown_error_code"
        codes.append(code)
    if not codes:
        return None, "none"
    return (codes[0], "single") if len(codes) == 1 else (None, "multiple")


def analyze(raw: bytes, root: Path | None = None) -> dict[str, Any]:
    if len(raw) > LOG_LIMIT:
        return unavailable("oversize_log", LOG_LIMIT + 1)
    text, state = decode_log(raw)
    if state != "ok" or text is None:
        return unavailable(f"{state}_log", len(raw))

    rows = ROW.findall(text)
    if not rows:
        return unavailable("target_row_missing", len(raw))
    if len(rows) != 1:
        return unavailable("target_row_ambiguous", len(raw))
    if rows[0] == "ok":
        return record(status="passed", unavailable_reason="none", failure_stage="none",
                      form="none", newline="lf", input_bytes_read=len(raw))
    if rows[0] == "ignored":
        return record(status="not_run", unavailable_reason="target_ignored",
                      failure_stage="none", form="none", newline="lf",
                      input_bytes_read=len(raw))

    lines = text.split("\n")
    headers = [i for i, line in enumerate(lines) if line == GROUP]
    if len(headers) != 1:
        return unavailable("failure_group_missing" if not headers else "failure_group_ambiguous", len(raw))
    start = headers[0]
    if "failures:" not in lines[:start]:
        return unavailable("failure_group_unbound", len(raw))
    end = next((i for i in range(start + 1, len(lines)) if ANY_GROUP.fullmatch(lines[i])), len(lines))
    group = lines[start + 1:end]
    # Rust 1.88's default panic hook begins with one newline; libtest captures
    # that byte-for-byte after its failure-group header. Accept only this exact
    # single empty separator, then require the very next line to be the panic.
    panic_line = 1 if group and group[0] == "" else 0
    panic = PANIC.fullmatch(group[panic_line]) if len(group) > panic_line else None
    if panic is None or panic.group(1).split("::")[-1] != TEST:
        return unavailable("panic_missing", len(raw))
    matching_panics = [
        match for line in group
        if (match := PANIC.fullmatch(line)) is not None
        and match.group(1).split("::")[-1] == TEST
    ]
    if len(matching_panics) != 1:
        return unavailable("panic_ambiguous", len(raw))
    message_line = panic_line + 1
    if len(group) <= message_line or not group[message_line] or group[message_line].startswith("thread '"):
        return unavailable("panic_message_missing", len(raw))

    stage, form = classify(group[message_line], root)
    if stage == "unavailable":
        return unavailable("panic_unclassified", len(raw))
    result = record(status="classified", unavailable_reason="none", failure_stage=stage,
                    form=form, newline="lf", input_bytes_read=len(raw))
    if stage == "cargo_build":
        result["rust_error_code"], result["rust_error_code_status"] = rust_error_code(root, form)
    return result


def analyze_file(log_path: Path, root: Path) -> dict[str, Any]:
    raw, state, size = bounded_read(log_path, LOG_LIMIT)
    if state == "oversize":
        return unavailable("oversize_log", size)
    return analyze(raw, root) if raw is not None else unavailable("unreadable_log", size)


def save(root: Path | None, result: dict[str, Any]) -> bool:
    if root is None:
        return False
    try:
        root.mkdir(parents=True, exist_ok=True)
        fd, temporary = tempfile.mkstemp(prefix=".sf06-signature-", suffix=".tmp", dir=root)
        with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as stream:
            json.dump(result, stream, sort_keys=True, separators=(",", ":"))
            stream.write("\n")
        os.replace(temporary, root / "sf06-signature-diagnostic.json")
        return True
    except (OSError, ValueError, TypeError):
        try:
            if "temporary" in locals():
                Path(temporary).unlink(missing_ok=True)
        except OSError:
            pass
        return False


def main(argv: list[str] | None = None) -> int:
    args = sys.argv[1:] if argv is None else argv
    root: Path | None = None
    try:
        if len(args) != 2:
            result = unavailable("arguments_invalid")
        else:
            root = Path(args[1])
            result = analyze_file(Path(args[0]), root)
    except Exception:
        result = unavailable("internal_error")
    result["artifact_saved"] = True
    if not save(root, result):
        result["artifact_saved"] = False
    code = result["rust_error_code"] or "none"
    print(
        "sf06_http_client_signature_diag=v1 "
        f"status={result['status']} reason={result['unavailable_reason']} "
        f"stage={result['failure_stage']} form={result['form']} "
        f"rust_error_code={code} code_status={result['rust_error_code_status']} "
        f"newline={result['newline']} "
        f"artifact_saved={'true' if result['artifact_saved'] else 'false'}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
