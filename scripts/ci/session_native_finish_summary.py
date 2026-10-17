"""Bounded, closed inventory gate; Cargo failure also propagates via pipefail."""
import sys

from csrf_deadline_mutations import inventory

MAX_BYTES = 64 * 1024
REQUIRED = frozenset((
    "auth::session_deadline_observer::native_finish::tests::unseen_sentinel_is_distinct_from_zero_and_clamped_elapsed",
    "auth::session_deadline_observer::native_finish::tests::result_reason_and_checkout_are_closed_independent_boundaries",
    "auth::session_deadline_observer::native_finish::tests::serial_tx_resets_and_overlapping_or_saturated_tx_is_invalid",
    "auth::session_deadline_observer::native_finish::tests::duplicate_or_out_of_order_marker_never_fabricates_completion",
    "auth::session_deadline_observer::native_finish::tests::delayed_worker_updates_only_original_owner_and_not_copied_terminal",
    "auth::session_deadline_observer::native_finish::tests::closed_numeric_line_and_poisoned_record_cannot_expose_payload",
    "sqlite::session::native_finish_tests::real_pool_commit_ack_follows_original_checkout_drop",
    "sqlite::session::native_finish_tests::old_commit_gate_is_after_native_return_and_before_checkout",
    "sqlite::session::native_finish_tests::denied_commit_records_err_then_actual_cleanup_ok_or_err",
    "sqlite::session::native_finish_tests::eof_cleanup_retains_reason_and_retirement_without_retry",
    "sqlite::session::native_finish_tests::old_management_panic_has_unseen_commit_and_after_finish_cleanup",
    "sqlite::session::native_finish_tests::precommit_expired_observer_never_invents_commit_entry",
))


def accepts(raw, exit_code=0):
    if len(raw) > MAX_BYTES or b"\x1b" in raw:
        return False
    try:
        text = raw.decode("utf-8", errors="strict").replace("\r\n", "\n")
    except UnicodeError:
        return False
    return ("\r" not in text and type(exit_code) is int
            and inventory(text, exit_code, REQUIRED))


def capture(stream):
    # Drain the complete pipe, including overflow, without retaining extra raw.
    raw = bytearray()
    while True:
        chunk = stream.read(8192)
        if not chunk:
            return bytes(raw)
        remaining = MAX_BYTES + 1 - len(raw)
        if remaining:
            raw.extend(chunk[:remaining])


def main():
    try:
        passed = len(sys.argv) == 1 and accepts(capture(sys.stdin.buffer))
    except (OSError, ValueError):
        passed = False
    print("session_native_finish_inventory=" + ("passed" if passed else "invalid"))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
