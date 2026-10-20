"""Publish bounded, closed-field auth-stage and HTTP test I/O observations."""
import pathlib
import re
import sys

RAW_LIMIT = 256 * 1024
OP = r"(?:CsrfIssue|CsrfCheckpoint|Bootstrap|Claim)"
STAGE = r"(?:Entry|BeginAck|Binding|Row|Expiry|Precommit|FinishAck|RestoreBefore|RestoreAck|PostcommitBefore|PostcommitAck|FinalClock|Gate|Return)"
EVENT = r"(?:Seen|Match|Miss|Nominal|AbsoluteClamped|Passed|Refused|WallExpired|GateExpiredOrEnded|MonoExpired|Success|Failure)"
NUMBER = r"(?:None|Some\(-?[0-9]+\))"
MONO = r"(?:None|Some\((?:Before\([0-9]+\)|At|After\([0-9]+\))\))"
KIND = r"(?:None|Some\((?:InvalidCredential|Denied|Expired|InvalidRequest|Unavailable|Internal)\))"
OUTCOME = r"(?:None|Some\((?:NotApplicable|Active|Committed|RolledBack|Unknown)\))"
RECORD = re.compile(rf"op={OP} stage={STAGE} event={EVENT} elapsed_us={NUMBER} utc_remaining_ms={NUMBER} mono={MONO} kind={KIND} outcome={OUTCOME}\Z")
HEADER = re.compile(r"auth-stage slot=[12] \(test-only; scheduling perturbed\)\Z")
LOST = re.compile(r"lost=(?:true|false)\Z")
OUTPUT_LIMIT = {"csrf": 8192, "prelogin": 8192, "broad": 16384}
FOCUSED_SUMMARY = re.compile(r"test result: ok\. 1 passed; 0 failed; 0 ignored;")
BROAD_SUMMARY = re.compile(r"test result: ok\. [1-9][0-9]* passed; 0 failed; [0-9]+ ignored;")
EXPECTED_SLOTS = {"csrf": (1,), "prelogin": (2,), "broad": (1, 2)}


# Exact cfg-identical libtest listing; raw failure names are never copied.
IDENT = r"[A-Za-z_][A-Za-z0-9_]*"
QUALIFIED = rf"{IDENT}(?:::{IDENT})*"
CASE_ENTRY = re.compile(rf"({QUALIFIED}): test\Z")
CASE_FOOTER = re.compile(r"([0-9]{1,4}) tests, 0 benchmarks\Z")
FAILED_ROW = re.compile(rf"test ({QUALIFIED}) \.\.\. FAILED\Z")
FAILED_ITEM = re.compile(rf"    ({QUALIFIED})\Z")
CASE_LIMIT = 1024
CASE_NAME_LIMIT = 256
FAILURE_LIMIT = 16
CASE_NAMES = (
    "auth::prelogin::tests::consumed_bearer_identity_can_issue_session_without_replacing_original_ceiling",
    "auth::prelogin::tests::observer_tests::observed_consumed_missing_row_actual_restoration_wait_gate_expiry",
    "auth::prelogin::tests::observer_tests::observed_consumed_missing_row_live_denial",
    "auth::prelogin::tests::one_use_claim_actual_delete_and_full_commit_precede_verifier",
    "auth::prelogin::tests::two_actual_claims_have_exactly_one_committed_witness",
)
CASE_SOURCE = {
    CASE_NAMES[0]: "tests.rs",
    CASE_NAMES[1]: "observer_tests.rs",
    CASE_NAMES[2]: "observer_tests.rs",
    CASE_NAMES[3]: "tests.rs",
    CASE_NAMES[4]: "tests.rs",
}
SOURCE_BINDINGS = {
    "tests.rs": ("runtime/src/auth/prelogin/tests.rs",
                 "7f9fea291cf9bf9e7f9fce6db7797038ef898293f6f6fab3f2eb85e51ce5da4d"),
    "observer_tests.rs": ("runtime/src/auth/prelogin/observer_tests.rs",
                          "30238968a7745259abe2dc08ffde7b4f8e4ecdcad37d3bd122d730b56d000ce9"),
}
SOURCE_LINE_COUNTS = {"tests.rs": 509, "observer_tests.rs": 150}
PANIC = re.compile(
    r"^thread '([^'\r\n]{1,256})'(?: \([0-9]{1,20}\))? panicked at "
    r"(.{1,512}):([0-9]{1,6}):([0-9]{1,5}):\s*$"
)
IO_PHASE = r"(?:RequestWrite|ResponseStatus|ResponseHeaders|ResponseBody|ConnectionEof)"
IO_KIND = (
    r"(?:NotFound|PermissionDenied|ConnectionRefused|ConnectionReset|HostUnreachable|"
    r"NetworkUnreachable|ConnectionAborted|NotConnected|AddrInUse|AddrNotAvailable|"
    r"NetworkDown|BrokenPipe|AlreadyExists|WouldBlock|NotADirectory|IsADirectory|"
    r"DirectoryNotEmpty|ReadOnlyFilesystem|FilesystemLoop|StaleNetworkFileHandle|"
    r"InvalidInput|InvalidData|TimedOut|WriteZero|StorageFull|NotSeekable|QuotaExceeded|"
    r"FileTooLarge|ResourceBusy|ExecutableFileBusy|Deadlock|CrossesDevices|TooManyLinks|"
    r"InvalidFilename|ArgumentListTooLong|Interrupted|Unsupported|UnexpectedEof|"
    r"OutOfMemory|InProgress|Uncategorized|Other)"
)
TEST_IO_MARKER_PREFIX = "HTTP test I/O failed:"
TEST_IO_MARKER = re.compile(
    rf"{re.escape(TEST_IO_MARKER_PREFIX)} phase=({IO_PHASE}) kind=({IO_KIND})\Z"
)
TEST_IO_FAILURE_LIMIT = 16
HTTP_STALLED_PREFIX = "http_stalled_fixture"
HTTP_STALLED_CASES = {
    "Capacity": "http_server::tests::stalled_response_send_expires_and_releases_connection_permit",
    "Graceful": "http_server::tests::graceful_shutdown_preserves_stalled_response_send_deadline",
}
HTTP_STALLED_PHASE = (
    r"(?:Setup|AwaitPending|ArmedStatus|ArmedBody|AwaitJoin|Expired|EndUnchanged|"
    r"FreshConnect|FreshRequestWrite|FreshResponse|FreshStatus|FreshBody|StopJoin|"
    r"ShutdownSignal|AwaitShutdown|GraceBeforeEnd|JoinCount|TerminalRead|TerminalPartial)"
)
HTTP_STALLED_MARKER = re.compile(
    rf"http_stalled_fixture case=(Capacity|Graceful) phase=({HTTP_STALLED_PHASE}) "
    r"armed=(true|false) pending=(true|false) expired=(true|false) "
    r"grace_before_end=(true|false) joins=(None|Some\([0-9]{1,10}\)) "
    r"status=(None|Some\([0-9]{1,5}\)) body_bytes=(None|Some\([0-9]{1,8}\)) "
    r"end_changed=(None|Some\((?:true|false)\))\Z"
)
HTTP_STALLED_FAILURE_LIMIT = 2
SESSION_TOKEN_FAILURE_CASES = {
    "finite_origin_and_header_refusals_preserve_existing_token_and_capacity":
        "finite_origin_and_header_refusals_preserve_existing_token_and_capacity",
    "failed_guarded_handler_never_finalizes_committed_rotation_cookie":
        "failed_guarded_handler_never_finalizes_committed_rotation_cookie",
    "body_checkpoint_rejects_generation_changed_after_actual_authorization":
        "body_checkpoint_rejects_generation_changed_after_actual_authorization",
}
WIRE_BODY_FAILURE_CASE = "wire_body_cap_is_independent_of_db_quota_and_checked_before_claim"
PUBLIC_TEST_STDOUT = re.compile(rf"---- ({QUALIFIED}) stdout ----\Z")
PUBLIC_STATUS_CLASSES = (
    r"(?:success|invalid_request|invalid_or_expired|denied|internal|unavailable|deadline|other)"
)
PUBLIC_DECIMAL = r"(?:0|[1-9][0-9]{0,4})"
SAFE_REPLY_PREFIX = "safe_reply_diag"
SAFE_REPLY_MARKER = re.compile(
    rf"safe_reply_diag check=(cookie_count|token_response|token_cookie) "
    rf"status=({PUBLIC_DECIMAL}) class=({PUBLIC_STATUS_CLASSES}) "
    rf"set_cookie_count=({PUBLIC_DECIMAL}) body_bytes=({PUBLIC_DECIMAL})\Z"
)
WIRE_BODY_PREFIX = "browser_login_public_api_diag"
WIRE_BODY_MARKER = re.compile(
    rf"browser_login_public_api_diag case=wire_body_cap phase=exact_body_response "
    rf"iteration=([01]) status=({PUBLIC_DECIMAL}) body_bytes=({PUBLIC_DECIMAL}) "
    rf"expected_body_bytes=(192) body_matches=(true|false) "
    rf"set_cookie_count=({PUBLIC_DECIMAL}) auth_cookie=(true|false) "
    rf"deletion=(true|false)\Z"
)
TEST_RESPONSE_LIMIT = 8192
TOKEN_RESPONSE_BYTES = 60
WIRE_BODY_EXPECTED_BYTES = 192
SAFE_REPLY_CASES = set(SESSION_TOKEN_FAILURE_CASES)
PUBLIC_MARKER_LIMIT = len(SAFE_REPLY_CASES) + 1


def read_bounded(path: pathlib.Path):
    try:
        with path.open("rb") as stream:
            raw = stream.read(RAW_LIMIT + 1)
    except OSError:
        return None, False
    return (None, True) if len(raw) > RAW_LIMIT else (raw, False)


def load_cases(root: pathlib.Path, exit_code: int):
    if exit_code != 0:
        return None
    raw, oversize = read_bounded(root / "auth-stage-cases.raw")
    if raw is None or oversize:
        return None
    names = {}
    footer = []
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError:
        return None
    for line in text.splitlines():
        entry = CASE_ENTRY.fullmatch(line)
        if entry:
            name = entry.group(1)
            if len(name) > CASE_NAME_LIMIT or name in names or len(names) >= CASE_LIMIT:
                return None
            names[name] = name
        summary = CASE_FOOTER.fullmatch(line)
        if summary:
            footer.append(int(summary.group(1)))
    if len(footer) != 1 or footer[0] == 0 or footer[0] != len(names):
        return None
    return names


def failed_cases(text: str, cases):
    if cases is None:
        return ["failure_cases=unavailable"]
    found = set()
    unknown = False
    section = False
    for line in text.splitlines():
        row = FAILED_ROW.fullmatch(line)
        candidate = row.group(1) if row else None
        if candidate is None and line.startswith("test ") and " ... FAILED" in line:
            unknown = True
        if line == "failures:":
            section = True
            continue
        if section and line:
            item = FAILED_ITEM.fullmatch(line)
            if item:
                candidate = item.group(1)
            else:
                unknown = True
                section = False
        if candidate is not None:
            canonical = cases.get(candidate)
            if canonical is None or len(candidate) > CASE_NAME_LIMIT:
                unknown = True
            else:
                found.add(canonical)
    lines = ["failed_case=" + name for name in sorted(found)[:FAILURE_LIMIT]]
    if len(found) > FAILURE_LIMIT:
        lines.append("failure_case_loss=observed")
    if unknown or not found:
        lines.append("failure_cases=unavailable")
    return lines


def _panic_locations(panic_lines, expected):
    locations = {name: None for name in expected}
    malformed = False
    for line in panic_lines:
        match = PANIC.fullmatch(line)
        if not match:
            malformed = True
            continue
        thread, path, line_raw, col_raw = match.groups()
        if thread not in locations or locations[thread] is not None:
            malformed = True
            continue
        normalized = path.replace("\\", "/")
        source = None
        for candidate, (relative, _expected_sha) in SOURCE_BINDINGS.items():
            if normalized == relative or normalized.endswith("/" + relative):
                source = candidate
        binding = CASE_SOURCE.get(thread)
        try:
            number, column = int(line_raw), int(col_raw)
        except ValueError:
            malformed = True
            continue
        if (source is None or binding is None or source != binding
                or not 1 <= number <= SOURCE_LINE_COUNTS[source]
                or not 1 <= column <= 9999):
            malformed = True
            continue
        locations[thread] = (source.removesuffix(".rs"), number, column)
    return locations, malformed


def _failed_source_positions(text: str, failed_lines, cases):
    if cases is None:
        return []
    failed = set()
    for line in failed_lines:
        if not line.startswith("failed_case="):
            continue
        name = line[len("failed_case="):]
        if name in CASE_SOURCE and cases.get(name) == name:
            failed.add(name)

    projected = []
    raw_lines = text.splitlines()
    for name in sorted(failed):
        prefix = f"thread '{name}'"
        panic_lines = [line for line in raw_lines if line.startswith(prefix)]
        locations, malformed = _panic_locations(panic_lines, (name,))
        location = locations[name]
        if malformed or location is None:
            projected.append(f"failed_source_case={name} source=unavailable")
        else:
            source, number, column = location
            projected.append(
                f"failed_source_case={name} source={source} line={number} column={column}"
            )
    return projected


def http_io_failures(text: str):
    found = set()
    invalid = False
    for line in text.splitlines():
        if TEST_IO_MARKER_PREFIX not in line:
            continue
        marker = TEST_IO_MARKER.fullmatch(line)
        if marker is None:
            invalid = True
        else:
            found.add(marker.groups())
    lines = [
        f"http_test_io=phase={phase} kind={kind}"
        for phase, kind in sorted(found)[:TEST_IO_FAILURE_LIMIT]
    ]
    if len(found) > TEST_IO_FAILURE_LIMIT:
        lines.append("http_test_io_loss=observed")
    if invalid:
        lines.append("http_test_io=unavailable")
    return lines


def http_stalled_diagnostics(text: str, exit_code: int, cases):
    """Return only complete, canonical records for the two fixed failed tests."""
    marker_lines = [line for line in text.splitlines() if HTTP_STALLED_PREFIX in line]
    failed = set()
    in_failures = False
    for line in text.splitlines():
        if line == "failures:":
            in_failures = True
            continue
        row = FAILED_ROW.fullmatch(line)
        item = FAILED_ITEM.fullmatch(line) if in_failures and line else None
        name = row.group(1) if row else item.group(1) if item else None
        if name in HTTP_STALLED_CASES.values():
            failed.add(name)
        if in_failures and line and item is None:
            in_failures = False

    if not marker_lines:
        if exit_code != 0 and failed:
            return ["http_stalled=unavailable"], True
        return [], False
    if exit_code == 0 or len(marker_lines) > HTTP_STALLED_FAILURE_LIMIT:
        return ["http_stalled=unavailable"], True

    if not failed or cases is None or any(name not in cases for name in failed):
        return ["http_stalled=unavailable"], True

    parsed = {}
    invalid = False
    for line in marker_lines:
        marker = HTTP_STALLED_MARKER.fullmatch(line)
        if marker is None:
            invalid = True
            continue
        (case, phase, armed, pending, expired, grace_before_end, joins,
         status, body_bytes, end_changed) = marker.groups()
        canonical = HTTP_STALLED_CASES[case]
        if canonical not in failed or canonical in parsed:
            invalid = True
            continue

        def bounded_option(value, maximum):
            if value == "None":
                return "None"
            number = int(value[5:-1])
            if number > maximum:
                raise ValueError
            return f"Some({number})"

        try:
            joins = bounded_option(joins, 2**32 - 1)
            status = bounded_option(status, 65535)
            body_bytes = bounded_option(body_bytes, 16 * 1024 * 1024)
            if end_changed != "None":
                value = end_changed[5:-1]
                if value not in ("true", "false"):
                    raise ValueError
                end_changed = f"Some({value})"
        except ValueError:
            invalid = True
            continue

        parsed[canonical] = (
            f"http_stalled=case={canonical} phase={phase} armed={armed} "
            f"pending={pending} expired={expired} grace_before_end={grace_before_end} "
            f"joins={joins} status={status} body_bytes={body_bytes} "
            f"end_changed={end_changed}"
        )

    if invalid or set(parsed) != failed or len(parsed) != len(marker_lines):
        return ["http_stalled=unavailable"], True
    return [parsed[name] for name in sorted(parsed)], False


def _status_class(status: int) -> str:
    if 200 <= status <= 299:
        return "success"
    return {
        400: "invalid_request",
        401: "invalid_or_expired",
        403: "denied",
        500: "internal",
        503: "unavailable",
        504: "deadline",
    }.get(status, "other")


def _failed_public_targets(text: str, cases):
    targets = SAFE_REPLY_CASES | {WIRE_BODY_FAILURE_CASE}
    found = set()
    seen_rows = set()
    seen_items = set()
    invalid = cases is None
    in_failures = False
    for line in text.splitlines():
        if line == "failures:":
            in_failures = True
            continue
        row = FAILED_ROW.fullmatch(line)
        item = FAILED_ITEM.fullmatch(line) if in_failures and line else None
        candidate = row.group(1) if row else item.group(1) if item else None
        if candidate in targets:
            seen = seen_rows if row else seen_items
            if candidate in seen or cases is None or cases.get(candidate) != candidate:
                invalid = True
            seen.add(candidate)
            found.add(candidate)
        if in_failures and line and item is None:
            in_failures = False
    return found, invalid


def public_test_diagnostics(text: str, exit_code: int, cases):
    """Publish only complete fixed records tied to their exact failed test."""
    failed, invalid_inventory = _failed_public_targets(text, cases)
    reply_expected = failed & SAFE_REPLY_CASES
    wire_expected = failed & {WIRE_BODY_FAILURE_CASE}
    reply_records = {}
    wire_records = {}
    reply_invalid = invalid_inventory
    wire_invalid = invalid_inventory
    saw_reply = False
    saw_wire = False
    current_test = None
    reply_markers = 0
    wire_markers = 0

    for line in text.splitlines():
        header = PUBLIC_TEST_STDOUT.fullmatch(line)
        if header:
            current_test = header.group(1)
        elif line == "failures:" or line.startswith("test result:"):
            current_test = None

        if SAFE_REPLY_PREFIX in line:
            saw_reply = True
            reply_markers += 1
            marker = SAFE_REPLY_MARKER.fullmatch(line)
            if (marker is None or current_test not in SAFE_REPLY_CASES or
                    current_test not in reply_expected or cases is None or
                    cases.get(current_test) != current_test or
                    current_test in reply_records):
                reply_invalid = True
                continue
            check, status_text, status_class, count_text, body_text = marker.groups()
            status, cookie_count, body_bytes = map(
                int, (status_text, count_text, body_text)
            )
            if (status > 65535 or cookie_count > TEST_RESPONSE_LIMIT or
                    body_bytes > TEST_RESPONSE_LIMIT or
                    status_class != _status_class(status)):
                reply_invalid = True
                continue
            if check == "token_response":
                if status == 200 and body_bytes == TOKEN_RESPONSE_BYTES:
                    reply_invalid = True
                    continue
            elif check == "cookie_count":
                if cookie_count == 1:
                    reply_invalid = True
                    continue
            elif check == "token_cookie":
                if (status != 200 or body_bytes != TOKEN_RESPONSE_BYTES or
                        cookie_count == 0):
                    reply_invalid = True
                    continue
            reply_records[current_test] = (
                f"session_reply=test={current_test} check={check} status={status} "
                f"class={status_class} set_cookie_count={cookie_count} "
                f"body_bytes={body_bytes}"
            )

        if WIRE_BODY_PREFIX in line:
            saw_wire = True
            wire_markers += 1
            marker = WIRE_BODY_MARKER.fullmatch(line)
            if (marker is None or current_test != WIRE_BODY_FAILURE_CASE or
                    current_test not in wire_expected or cases is None or
                    cases.get(current_test) != current_test or
                    current_test in wire_records):
                wire_invalid = True
                continue
            (iteration_text, status_text, body_text, expected_text, matches,
             count_text, auth_cookie, deletion) = marker.groups()
            iteration, status, body_bytes, expected_bytes, cookie_count = map(
                int, (iteration_text, status_text, body_text, expected_text,
                      count_text)
            )
            if (iteration not in (0, 1) or status > 65535 or
                    body_bytes > TEST_RESPONSE_LIMIT or
                    expected_bytes != WIRE_BODY_EXPECTED_BYTES or
                    cookie_count > TEST_RESPONSE_LIMIT or
                    (matches == "true" and body_bytes != expected_bytes) or
                    (status == 200 and matches == "true" and
                     auth_cookie == "true" and deletion == "true")):
                wire_invalid = True
                continue
            wire_records[current_test] = (
                f"browser_wire=test={current_test} case=wire_body_cap "
                f"phase=exact_body_response iteration={iteration} status={status} "
                f"body_bytes={body_bytes} expected_body_bytes={expected_bytes} "
                f"body_matches={matches} set_cookie_count={cookie_count} "
                f"auth_cookie={auth_cookie} deletion={deletion}"
            )

    if reply_markers > len(SAFE_REPLY_CASES) or wire_markers > 1:
        reply_invalid |= reply_markers > len(SAFE_REPLY_CASES)
        wire_invalid |= wire_markers > 1

    reply_lines = []
    if saw_reply or reply_expected:
        if (exit_code == 0 or reply_invalid or
                set(reply_records) != reply_expected):
            reply_lines = ([f"session_reply=test={name} unavailable"
                            for name in sorted(reply_expected)]
                           or ["session_reply=unavailable"])
            reply_invalid = True
        else:
            reply_lines = [reply_records[name] for name in sorted(reply_records)]

    wire_lines = []
    if saw_wire or wire_expected:
        if (exit_code == 0 or wire_invalid or
                set(wire_records) != wire_expected):
            wire_lines = ([f"browser_wire=test={name} unavailable"
                           for name in sorted(wire_expected)]
                          or ["browser_wire=unavailable"])
            wire_invalid = True
        else:
            wire_lines = [wire_records[name] for name in sorted(wire_records)]

    return (reply_lines + wire_lines,
            (reply_invalid and (saw_reply or bool(reply_expected))) or
            (wire_invalid and (saw_wire or bool(wire_expected))))


def capture(name: str, root: pathlib.Path, exit_code: int, cases=None) -> bool:
    raw_file = root / f"auth-stage-{name}.raw"
    trace_file = root / f"auth-stage-{name}.trace"
    raw, oversize = read_bounded(raw_file)
    missing = raw is None and not oversize
    text = (raw or b"").decode("utf-8", errors="replace")
    summary = BROAD_SUMMARY if name == "broad" else FOCUSED_SUMMARY
    valid = exit_code == 0 and not oversize and not missing and bool(summary.search(text))
    lines = [f"test={name} cargo_exit={exit_code} inventory={'pass' if valid else 'fail'}"]
    if exit_code != 0:
        failed = failed_cases(text, cases)
        lines.extend(failed)
        lines.extend(_failed_source_positions(text, failed, cases))
    if missing:
        lines.append("raw_file=unavailable")
    io_diagnostics = http_io_failures(text) if exit_code != 0 else []
    lines.extend(io_diagnostics)
    invalid_io_record = any(
        line in ("http_test_io=unavailable", "http_test_io_loss=observed")
        for line in io_diagnostics
    )
    stalled_diagnostics, invalid_stalled_record = http_stalled_diagnostics(
        text, exit_code, cases
    )
    lines.extend(stalled_diagnostics)
    public_diagnostics, invalid_public_record = public_test_diagnostics(
        text, exit_code, cases
    )
    lines.extend(public_diagnostics)
    invalid_record = False
    expected_slots = set(EXPECTED_SLOTS[name])
    opened_slots = set()
    closed_slots = set()
    active_slot = None
    active_records = 0
    saw_loss = False
    for line in text.splitlines():
        if line.startswith("auth-stage slot="):
            good = bool(HEADER.fullmatch(line))
            if good:
                slot = int(line[len("auth-stage slot=")])
                good = active_slot is None and slot in expected_slots and slot not in opened_slots
                if good:
                    active_slot = slot
                    opened_slots.add(slot)
                    active_records = 0
        elif line.startswith("op="):
            good = active_slot is not None and bool(RECORD.fullmatch(line))
            if good:
                active_records += 1
        elif line.startswith("lost="):
            good = active_slot is not None and active_records > 0 and bool(LOST.fullmatch(line))
            if good:
                saw_loss |= line == "lost=true"
                closed_slots.add(active_slot)
                active_slot = None
        else:
            continue
        if good:
            lines.append(line)
        else:
            invalid_record = True
    slots_complete = active_slot is None and opened_slots == expected_slots and closed_slots == expected_slots
    if not slots_complete:
        lines.append("trace=unavailable")
    if saw_loss:
        lines.append("trace_loss=observed")
    if oversize:
        lines.append("raw_limit=exceeded")
    if invalid_record:
        lines.append("trace_validation=failed")
    out = bytearray()
    truncated = False
    for line in lines:
        encoded = (line + "\n").encode("ascii")
        if len(out) + len(encoded) > OUTPUT_LIMIT[name] - 16:
            truncated = True
            break
        out.extend(encoded)
    if truncated:
        out.extend(b"lost=true\n")
    trace_file.write_bytes(out)
    sys.stdout.buffer.write(out)
    return (valid and not invalid_record and not invalid_io_record and not invalid_stalled_record
            and not invalid_public_record and not truncated
            and slots_complete and not saw_loss)


def main() -> int:
    if len(sys.argv) != 6:
        return 2
    root = pathlib.Path(sys.argv[1])
    cases = load_cases(root, int(sys.argv[5]))
    results = [capture(name, root, int(code), cases) for name, code in zip(("csrf", "prelogin", "broad"), sys.argv[2:5])]
    return 0 if all(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
