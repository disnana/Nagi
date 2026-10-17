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
        'http_server::response_budget::finalize::tests::rejected_user_append_copies_nothing_and_keeps_prior_entries',
        'http_server::response_budget::finalize::tests::foreign_owner_refused_before_plan_allocation_and_original_refunded',
        'http_server::response_budget::finalize::tests::ordinary_s_plus_q_exact_boundary_does_not_require_l_times_q',
        'http_server::response_budget::finalize::tests::head_and_bodyless_statuses_preserve_header_claim_and_security',
        'http_server::response_budget::finalize::tests::auth_304_and_html_encoding_are_invariants_before_conversion',
        'http_server::response_budget::finalize::tests::finite_vary_union_cors_allow_and_preflight_are_planned_once',
        'http_server::response_budget::finalize::tests::complete_plan_refuses_before_native_conversion_and_preserves_peak_credit',
        'http_server::response_budget::finalize::tests::fallback_sf_plus_c_qf_and_f_ge_c_are_independent_exact_boundaries',
        'http_server::response_budget::finalize::tests::every_terminal_reason_keeps_status_and_retry_zero_closes_without_allocation',
        'http_server::response_budget::finalize::tests::original_auth_refusal_deletion_and_cancellation_survive_fallback',
        'http_server::response_budget::finalize::tests::fixed_body_preserves_original_bytes_pointer_until_last_frame_owner',
        'http_server::response_budget::finalize::tests::reserve_before_fake_take_failure_never_reissues_and_fallback_is_independent',
        'http_server::response_budget::finalize::tests::prelogin_owned_cookie_moves_once_without_secret_copy',
        'http_server::response_budget::finalize::tests::private_finalizer_and_framework_ticket_keep_send_sync_layout',
        'http_server::response_budget::finalize::tests::hidden_vec_capacity_and_empty_work_are_checked_before_copy_or_headers',
        'http_server::response_budget::finalize::tests::fallback_sf_includes_actual_quota_receipt_and_refuses_one_byte_over_budget',
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
    'transport': (
        'http_server::response_send::tests::head_and_empty_keep_native_cache_charge_at_ready_until_driver_drop',
        'http_server::response_send::tests::keep_alive_cache_moves_to_same_shared_request_owner_past_native_shutdown',
        'http_server::response_send::tests::pipelined_requests_keep_one_cache_slot_and_multiple_prior_map_owners',
        'http_server::response_send::tests::fallback_is_independent_then_exhaustion_closes_real_transport_without_reallocation',
        'http_server::response_send::tests::foreign_wire_fails_closed_and_native_connection_actually_ends',
        'http_server::response_send::tests::parse_failure_does_not_release_pending_claim_at_ready',
        'http_server::response_send::tests::abort_before_actual_join_and_handler_panic_release_only_on_native_destruction',
        'http_server::response_send::tests::unpolled_and_ready_native_io_drop_before_cache_guard_and_connection_permit',
        'http_server::response_send::tests::prepared_managed_head_and_empty_use_existing_native_drop_chain',
    ),
    'request': ('http_server::request_headers::tests::snapshots_share_actual_map_and_never_retain_original_body', 'http_server::request_headers::tests::sanitized_views_hide_every_credential_accessor_debug_and_child_snapshot', 'http_server::request_headers::tests::ordinary_lookup_keeps_duplicate_case_utf8_and_invalid_name_contracts', 'http_server::request_headers::tests::header_only_last_shared_owner_retains_prior_response_reservation', 'http_server::request_headers::tests::unpolled_future_and_unwind_release_only_their_last_request_owner', 'http_server::request_headers::tests::cancelled_pending_request_keeps_charge_until_actual_task_join', 'http_server::request_headers::tests::fixture_mutation_cannot_copy_shared_map_or_mutate_charged_native_owner', 'http_server::request_headers::tests::actual_native_parts_extensions_live_through_handler_and_drop_after_join'),
    'rate-startup': (
        'security_sf07_private::startup_rate::tests::all_five_capacity_values_are_explicit_positive_and_representable',
        'security_sf07_private::startup_rate::tests::policy_time_burst_and_duplicate_subject_fail_before_allocation',
        'security_sf07_private::startup_rate::tests::aggregate_rules_domains_and_slots_each_have_their_own_limit',
        'security_sf07_private::startup_rate::tests::subject_support_is_checked_at_startup_not_silently_skipped',
        'security_sf07_private::startup_rate::tests::metadata_byte_refusal_occurs_before_any_vec_or_owner_allocation',
        'security_sf07_private::startup_rate::tests::request_work_covers_route_plus_subject_not_only_the_largest_charge',
        'security_sf07_private::startup_rate::tests::route_limited_and_subject_capacity_are_distinct_without_cross_route_sharing',
        'security_sf07_private::startup_rate::tests::existing_branded_rule_ids_reject_other_owner_without_a_second_brand_system',
        'security_sf07_private::startup_rate::tests::closing_uses_the_existing_owner_for_both_charge_stages',
        'security_sf07_private::startup_rate::tests::empty_catalog_creates_no_rate_owner_and_no_implicit_rule',
        'security_sf07_private::startup_rate::tests::caller_catalog_storage_is_charged_and_overflow_refused',
        'security_sf07_private::startup_rate::tests::finite_8192_subject_slots_are_not_rejected_by_a_hidden_4096_cap',
    ),
    'fixed-owner': (
        'auth::csrf::fixed_body::tests::exact_protocol_owner_validates_before_copy_without_hidden_capacity',
        'auth::csrf::fixed_body::tests::scoped_parallel_probes_do_not_confuse_abandoned_registration_or_other_owner',
    ),
    'csrf-issuer': (
        'auth::csrf::tests::fixed_wire_owner_tests::actual_csrf_wire_fixed_owner_outlives_issuer_until_last_bytes_clone',
    ),
    'prelogin-issuer': (
        'auth::prelogin::tests::fixed_wire_owner_tests::actual_prelogin_wire_fixed_owner_outlives_issuer_until_last_bytes_clone',
    ),
    'catalog-retained': (
        'security_sf07_private::quota::retained_catalog_tests::receipt_covers_same_quota_mutex_and_actual_vector_capacity',
        'security_sf07_private::quota::retained_catalog_tests::short_catalog_budget_refused_before_existing_constructor_allocates',
        'security_sf07_private::quota::retained_catalog_tests::actual_resource_vector_excess_capacity_refused_before_publish',
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



def catalog_failure_projection(raw):
    """Render only three fixed case states after a complete failed inventory."""
    if len(raw) > MAX_BYTES:
        return None
    raw = raw.replace(b"\r\n", b"\n")
    if b"\r" in raw:
        return None
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeError:
        return None
    expected = EXPECTED["catalog-retained"]
    if re.findall(r"(?m)^running ([0-9]+) tests$", text) != ["3"]:
        return None
    rows = re.findall(r"(?m)^test ([^ \n]+) \.\.\. ([^\n]+)$", text)
    test_lines = [line for line in text.splitlines()
                  if line.startswith("test ") and not line.startswith("test result:")]
    if len(rows) != 3 or len(test_lines) != 3:
        return None
    if {name for name, _ in rows} != set(expected):
        return None
    if any(state not in ("ok", "FAILED") for _, state in rows):
        return None
    failures = sum(state == "FAILED" for _, state in rows)
    if failures == 0:
        return None
    summaries = re.findall(r"(?m)^test result: ([^\n]+)$", text)
    pattern = (rf"FAILED\. {3-failures} passed; {failures} failed; 0 ignored; "
               r"0 measured; [0-9]+ filtered out; finished in [0-9.]+s")
    if len(summaries) != 1 or not re.fullmatch(pattern, summaries[0]):
        return None
    states = dict(rows)
    return tuple("failed" if states[name] == "FAILED" else "passed" for name in expected)


def main():
    mode = sys.argv[1] if len(sys.argv) == 2 else ""
    raw = sys.stdin.buffer.read(MAX_BYTES + 1)
    state = classify(raw, mode)
    # Both rendered fields are fixed enums, never values taken from the log.
    label = mode if mode in EXPECTED else "invalid"
    print("sf07_checkpoint scope=" + label + " state=" + state)
    if mode == "catalog-retained" and state == "test_failed":
        projection = catalog_failure_projection(raw)
        if projection is None:
            print("sf07_catalog_diagnostic state=unavailable")
        else:
            print("sf07_catalog_diagnostic state=classified receipt=" + projection[0]
                  + " preallocation=" + projection[1] + " excess_capacity=" + projection[2])
    return 0 if state == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
