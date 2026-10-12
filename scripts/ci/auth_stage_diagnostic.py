"""Publish only bounded, closed-field auth-stage observations from three CI runs."""
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


def capture(name: str, root: pathlib.Path, exit_code: int) -> bool:
    raw_file = root / f"auth-stage-{name}.raw"
    trace_file = root / f"auth-stage-{name}.trace"
    oversize = raw_file.stat().st_size > RAW_LIMIT
    raw = raw_file.read_bytes() if not oversize else b""
    text = raw.decode("utf-8", errors="replace")
    summary = BROAD_SUMMARY if name == "broad" else FOCUSED_SUMMARY
    valid = exit_code == 0 and not oversize and bool(summary.search(text))
    lines = [f"test={name} cargo_exit={exit_code} inventory={'pass' if valid else 'fail'}"]
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
    return valid and not invalid_record and not truncated and slots_complete and not saw_loss


def main() -> int:
    if len(sys.argv) != 5:
        return 2
    root = pathlib.Path(sys.argv[1])
    results = [capture(name, root, int(code)) for name, code in zip(("csrf", "prelogin", "broad"), sys.argv[2:])]
    return 0 if all(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
