"""Native-host-only bounded inventory for the private owned-state lock gates."""
import os
import sys

if __package__:
    from .csrf_deadline_mutations import inventory
else:
    from csrf_deadline_mutations import inventory

MAX_BYTES = 64 * 1024
COMMON = {
    "lock": (
        "security_sf07_private::owned_state_lock::tests::cold_first_lock_unlock_and_drop_have_no_owned_heap",
        "security_sf07_private::owned_state_lock::tests::arc_receipt_covers_cold_lock_and_last_owner_releases_payload_once",
        "security_sf07_private::owned_state_lock::tests::fresh_concurrent_writers_keep_all_updates_and_one_final_payload",
        "security_sf07_private::owned_state_lock::tests::writer_poison_recovers_for_disposal_and_stays_sticky_until_drop",
    ),
    "owners": (
        "security_sf07_private::owned_state_owner_tests::quota_cold_receipt_matches_allocated_payload_and_claim_keeps_final_owner",
        "security_sf07_private::owned_state_owner_tests::rate_cold_receipt_contains_state_and_short_budget_refuses_before_allocation",
    ),
    "send": (
        "http_server::response_send::owned_state_lock_tests::poisoned_ledger_disposal_keeps_native_cache_and_permit_release_order",
    ),
}
MAC_CONTROL = "security_sf07_private::owned_state_lock::tests::cold_std_mutex_control_retains_native_heap_until_owner_drop"


def native_host():
    host = {"linux": "Linux", "win32": "Windows", "darwin": "macOS"}.get(sys.platform)
    if (host is None or os.environ.get("RUNNER_OS") != host
            or "CARGO_BUILD_TARGET" in os.environ):
        return None
    return host


def expected(mode):
    host = native_host()
    if host is None or mode not in (*COMMON, "private"):
        return None
    names = COMMON["lock"] + COMMON["owners"] if mode == "private" else COMMON[mode]
    if host == "macOS" and mode in ("lock", "private"):
        names += (MAC_CONTROL,)
    return frozenset(names)


def matches(raw, names, exit_code=0):
    if names is None or not names or len(raw) > MAX_BYTES or b"\x1b" in raw:
        return False
    try:
        text = raw.decode("utf-8", errors="strict").replace("\r\n", "\n")
    except UnicodeError:
        return False
    test_lines = [line for line in text.splitlines()
                  if line.startswith("test ") and not line.startswith("test result:")]
    return ("\r" not in text and len(test_lines) == len(names) and type(exit_code) is int
            and inventory(text, exit_code, frozenset(names)))


def accepts(raw, mode, exit_code=0):
    return matches(raw, expected(mode), exit_code)


def capture(stream):
    # Drain overflow without retaining it or sending raw diagnostics to stdout.
    raw = bytearray()
    while True:
        chunk = stream.read(8192)
        if not chunk:
            return bytes(raw)
        remaining = MAX_BYTES + 1 - len(raw)
        if remaining:
            raw.extend(chunk[:remaining])


def main():
    mode = sys.argv[1] if len(sys.argv) == 2 and sys.argv[1] in COMMON else "invalid"
    try:
        passed = accepts(capture(sys.stdin.buffer), mode)
    except (OSError, ValueError):
        passed = False
    print("sf07_owned_state scope=" + mode + " state=" + ("passed" if passed else "invalid"))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
