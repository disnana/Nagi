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
        lines.extend(failed_cases(text, cases))
    if missing:
        lines.append("raw_file=unavailable")
    io_diagnostics = http_io_failures(text) if exit_code != 0 else []
    lines.extend(io_diagnostics)
    invalid_io_record = any(
        line in ("http_test_io=unavailable", "http_test_io_loss=observed")
        for line in io_diagnostics
    )
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
    return (valid and not invalid_record and not invalid_io_record and not truncated
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
