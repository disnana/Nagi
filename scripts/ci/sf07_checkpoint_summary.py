"""Bounded closed Cargo summary; no raw compiler/test text is rendered."""
import re
import sys

MAX_BYTES = 65536
EXPECTED = {
    'wire': (
        'http_server::response_budget::tests::known_oversized_input_is_refused_before_factory_allocation',
        'http_server::response_budget::tests::failed_capacity_check_does_not_allocate_or_mutate_existing_buffer',
        'http_server::response_budget::tests::json_checks_serialized_expansion_not_input_string_length',
        'http_server::response_budget::tests::serializer_failure_has_bounded_static_diagnostic_and_releases_charge',
        'http_server::response_budget::tests::serializer_panic_keeps_unwind_and_releases_factory_charge',
        'http_server::response_budget::tests::serializer_sink_work_bound_is_independent_of_body_size',
        'http_server::response_budget::tests::failed_and_empty_writes_cannot_reset_or_evade_work_bound',
        'http_server::response_budget::tests::transport_frame_clone_and_header_owner_both_retain_core_charge',
        'http_server::response_budget::tests::header_count_byte_bounds_are_checked_before_copy_and_failure_preserves_entries',
        'http_server::response_budget::tests::invalid_zero_overflow_and_impossible_total_profiles_fail_before_publish',
        'http_server::response_budget::wire::tests::native_header_only_and_bytes_frame_clone_retain_one_original_claim',
        'http_server::response_budget::wire::tests::head_empty_body_and_cleared_native_container_keep_header_charge',
        'http_server::response_budget::wire::tests::native_transfer_rejects_unplanned_old_producer_and_foreign_owner',
        'http_server::response_budget::wire::tests::invalid_native_header_conversion_frees_reservation_without_partial_escape',
        'http_server::response_budget::wire::tests::managed_and_user_header_budgets_remain_separate',
        'http_server::response_budget::wire::tests::framework_names_are_fixed_and_each_uses_managed_capacity',
        'http_server::response_budget::wire::tests::native_layout_peak_is_reserved_before_copy_and_checked_before_publish',
        'http_server::response_budget::wire::tests::native_container_capacity_stays_within_pinned_layout_on_duplicates_and_names',
        'http_server::response_budget::wire::tests::fallback_uses_independent_ledger_then_closes_without_another_allocation',
        'http_server::response_budget::wire::tests::fallback_preserves_statuses_and_positive_retry_without_ordinary_serializer',
        'http_server::response_budget::wire::tests::fallback_startup_requires_connection_count_and_complete_minimum_bytes',
        'http_server::response_budget::wire::tests::wire_native_owners_are_send_sync_without_clone_or_raw_map_extraction',
    ),
    'quota': (
        'security_sf07_private::quota::tests::q01_two_resource_partial_full_is_atomic',
        'security_sf07_private::quota::tests::startup_rejects_zero_and_empty_catalog',
        'security_sf07_private::quota::tests::q03_catalog_byte_product_rejects_overflow_and_isize_boundary',
        'security_sf07_private::quota::tests::q04_duplicate_is_invalid_but_bad_resource_id_faults_closed',
        'security_sf07_private::quota::tests::claim_sum_overflow_is_capacity_without_counter_mutation',
        'security_sf07_private::quota::tests::q05_exact_count_and_bytes_boundary_releases_one_claim',
        'security_sf07_private::quota::tests::q06_moved_reservation_releases_once_at_final_drop',
        'security_sf07_private::quota::tests::q09_poisoned_disposal_releases_claim_and_keeps_fault_latched',
        'security_sf07_private::quota::tests::q13_two_simultaneous_claims_have_one_winner',
    ),
}


def classify(raw, mode):
    if mode not in EXPECTED or len(raw) > MAX_BYTES:
        return "invalid"
    raw = raw.replace(b"\r\n", b"\n")
    if b"\r" in raw:
        return "invalid"
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeError:
        return "invalid"
    if re.search(r"(?m)^error(?:\[E[0-9]+\]|:)", text):
        return "compile_failed"
    summaries = re.findall(r"(?m)^test result: ([^\n]+)$", text)
    if len(summaries) != 1:
        return "unavailable"
    if re.fullmatch(r"FAILED\. [0-9]+ passed; [0-9]+ failed; [0-9]+ ignored; [^\n]+", summaries[0]):
        return "test_failed"
    count = len(EXPECTED[mode])
    if not re.fullmatch(rf"ok\. {count} passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9.]+s", summaries[0]):
        return "invalid"
    rows = re.findall(r"(?m)^test ([^ \n]+) \.\.\. ([^\n]+)$", text)
    test_lines = [line for line in text.splitlines() if line.startswith("test ") and not line.startswith("test result:")]
    if len(test_lines) != count or len(rows) != count or {name for name, status in rows} != set(EXPECTED[mode]):
        return "invalid"
    if any(status != "ok" for name, status in rows):
        return "invalid"
    return "passed"


def main():
    mode = sys.argv[1] if len(sys.argv) == 2 else ""
    state = classify(sys.stdin.buffer.read(MAX_BYTES + 1), mode)
    # Both rendered fields are fixed enums, never values taken from the log.
    label = mode if mode in EXPECTED else "invalid"
    print("sf07_checkpoint scope=" + label + " state=" + state)
    return 0 if state == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
