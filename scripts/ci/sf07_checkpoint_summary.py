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
        'http_server::response_budget::finalize::tests::security_fallback_catalog_and_last_body_view_keep_finite_credit',
        'http_server::response_budget::wire::tests::closed_transport_and_internal_bodies_keep_independent_finite_fallback_credit',
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


# BrowserLogin comparison profiles are an independent fixed inventory. They do
# not alter the current SF07 native 85-case expectation table.
BROWSER_COUNTER_CASES = (
    "public_login_claims_once_then_named_verifier_issue_and_appended_cookies",
    "input_and_strict_json_rejections_do_not_consume_or_call_verifier",
    "committed_claim_stays_consumed_on_verifier_error_panic_and_deadline",
    "issue_commit_then_handler_err_panic_or_timeout_never_delivers_auth_cookie",
    "wire_body_cap_is_independent_of_db_quota_and_checked_before_claim",
    "browser_profile_refuses_non_post_and_wrong_startup_origin",
    "body_own_deadline_408_security_earlier_or_equal_504_before_claim",
    "shutdown_cancels_actual_request_and_committed_token_cannot_return",
    "closed_native_pool_refuses_before_verifier_and_session_capacity_never_redelivers",
    "json_value_nodes_and_container_depth_accept_exact_bounds",
)
BROWSER_COUNTER_PROFILES = {
    "browser-counter-strict": (BROWSER_COUNTER_CASES[1],),
    "browser-counter-wire": (BROWSER_COUNTER_CASES[4],),
    "browser-counter-parallel": BROWSER_COUNTER_CASES,
}
BROWSER_COUNTER_LIST = "browser-counter-list"
BROWSER_COUNTER_COMPILE = "browser-counter-compile"
BROWSER_COUNTER_PROFILE_NAMES = set(BROWSER_COUNTER_PROFILES) | {
    BROWSER_COUNTER_LIST, BROWSER_COUNTER_COMPILE
}
BROWSER_COUNTER_TEST_ROW = re.compile(
    r"test ([A-Za-z_][A-Za-z0-9_:]*) \.\.\. (ok|FAILED|ignored)\Z"
)
BROWSER_COUNTER_LIST_ROW = re.compile(r"([A-Za-z_][A-Za-z0-9_]*)[:] test\Z")
BROWSER_COUNTER_FOOTER = re.compile(r"([0-9]{1,4}) tests?, ([0-9]{1,4}) benchmarks\Z")
BROWSER_COUNTER_FAMILY_MARKER = "sf03_browser_failure_counter"
BROWSER_COUNTER_CARGO_TEST_FAILURE = (
    "error: test failed, to rerun pass `-p nagi-runtime --test browser_login_public_api`"
)
BROWSER_COUNTER_SECRET_TAINT = re.compile(
    r"(?i)\bsecret\b|\b(?:authorization|cookie|set-cookie|bearer|password|token|"
    r"csrf[-_ ]token|session[-_ ]id)\s*[:=]"
)
BROWSER_COUNTER_STRICT_ROW = re.compile(
    r"sf03_browser_failure_counters_v1 case=strict_json_valid phase=final_response "
    r"status=(?P<status>0|[1-9][0-9]*) status_overflow=(?P<status_overflow>true|false) "
    r"body_bytes=(?P<body_bytes>0|[1-9][0-9]*) body_bytes_overflow=(?P<body_bytes_overflow>true|false) "
    r"set_cookie_count=(?P<set_cookie_count>0|[1-9][0-9]*) "
    r"set_cookie_count_overflow=(?P<set_cookie_count_overflow>true|false) "
    r"auth_cookie=(?P<auth_cookie>true|false) deletion=(?P<deletion>true|false) "
    r"verifier_calls=(?P<verifier_calls>0|[1-9][0-9]*) "
    r"verifier_calls_overflow=(?P<verifier_calls_overflow>true|false) "
    r"handler_calls=(?P<handler_calls>0|[1-9][0-9]*) "
    r"handler_calls_overflow=(?P<handler_calls_overflow>true|false) "
    r"claim_committed=(?P<claim_committed>true|false) "
    r"issue_committed=(?P<issue_committed>true|false) "
    r"mode6_cancel_seen=(?P<mode6_cancel_seen>0|[1-9][0-9]*) "
    r"mode6_cancel_seen_overflow=(?P<mode6_cancel_seen_overflow>true|false)\Z"
)
BROWSER_COUNTER_WIRE_ROW = re.compile(
    r"sf03_browser_failure_counters_v1 case=wire_body_cap phase=exact_body_response "
    r"iteration=(?P<iteration>0|[1-9][0-9]*) "
    r"status=(?P<status>0|[1-9][0-9]*) status_overflow=(?P<status_overflow>true|false) "
    r"body_bytes=(?P<body_bytes>0|[1-9][0-9]*) body_bytes_overflow=(?P<body_bytes_overflow>true|false) "
    r"expected_body_bytes=(?P<expected_body_bytes>0|[1-9][0-9]*) "
    r"expected_body_bytes_overflow=(?P<expected_body_bytes_overflow>true|false) "
    r"body_matches=(?P<body_matches>true|false) "
    r"set_cookie_count=(?P<set_cookie_count>0|[1-9][0-9]*) "
    r"set_cookie_count_overflow=(?P<set_cookie_count_overflow>true|false) "
    r"auth_cookie=(?P<auth_cookie>true|false) deletion=(?P<deletion>true|false) "
    r"verifier_calls=(?P<verifier_calls>0|[1-9][0-9]*) "
    r"verifier_calls_overflow=(?P<verifier_calls_overflow>true|false) "
    r"handler_calls=(?P<handler_calls>0|[1-9][0-9]*) "
    r"handler_calls_overflow=(?P<handler_calls_overflow>true|false) "
    r"claim_committed=(?P<claim_committed>true|false) "
    r"issue_committed=(?P<issue_committed>true|false) "
    r"mode6_cancel_seen=(?P<mode6_cancel_seen>0|[1-9][0-9]*) "
    r"mode6_cancel_seen_overflow=(?P<mode6_cancel_seen_overflow>true|false)\Z"
)
BROWSER_COUNTER_STRICT_RANGES = {
    "status": 999, "body_bytes": 8193, "set_cookie_count": 1023,
    "verifier_calls": 1023, "handler_calls": 1023, "mode6_cancel_seen": 1023,
}
BROWSER_COUNTER_WIRE_RANGES = {
    "iteration": 1, "status": 999, "body_bytes": 8193,
    "expected_body_bytes": 8193, "set_cookie_count": 1023,
    "verifier_calls": 1023, "handler_calls": 1023, "mode6_cancel_seen": 1023,
}
BROWSER_COUNTER_FAILURE_CASES = {
    "strict_json_valid": BROWSER_COUNTER_CASES[1],
    "wire_body_cap": BROWSER_COUNTER_CASES[4],
}


def _browser_counter_text(raw, allow_test_failure=False):
    if len(raw) > MAX_BYTES:
        return None, "invalid"
    raw = raw.replace(b"\r\n", b"\n")
    if b"\r" in raw:
        return None, "invalid"
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeError:
        return None, "invalid"
    for line in text.splitlines():
        if re.match(r"^error(?:\[E[0-9]+\]|:)", line):
            if allow_test_failure and line == BROWSER_COUNTER_CARGO_TEST_FAILURE:
                continue
            return text, "compile_failed"
    return text, None


def classify_browser_counter_list(raw):
    text, state = _browser_counter_text(raw)
    if state:
        return state
    if text is None:
        return "invalid"
    entries = [match.group(1) for line in text.splitlines()
               if (match := BROWSER_COUNTER_LIST_ROW.fullmatch(line))]
    footers = [match.groups() for line in text.splitlines()
               if (match := BROWSER_COUNTER_FOOTER.fullmatch(line))]
    if not entries and not footers:
        return "unavailable"
    if (len(entries) != len(BROWSER_COUNTER_CASES)
            or len(set(entries)) != len(BROWSER_COUNTER_CASES)
            or set(entries) != set(BROWSER_COUNTER_CASES)
            or footers != [(str(len(BROWSER_COUNTER_CASES)), "0")]):
        return "invalid"
    return "passed"


def _browser_counter_sample(line, mode):
    strict_match = BROWSER_COUNTER_STRICT_ROW.fullmatch(line)
    wire_match = BROWSER_COUNTER_WIRE_ROW.fullmatch(line)
    if bool(strict_match) == bool(wire_match):
        return None
    match = strict_match or wire_match
    case = "strict_json_valid" if strict_match else "wire_body_cap"
    allowed = {
        "browser-counter-strict": {"strict_json_valid"},
        "browser-counter-wire": {"wire_body_cap"},
        "browser-counter-parallel": {"strict_json_valid", "wire_body_cap"},
    }.get(mode, set())
    if case not in allowed:
        return None
    ranges = BROWSER_COUNTER_STRICT_RANGES if strict_match else BROWSER_COUNTER_WIRE_RANGES
    sample = {"case": case,
              "phase": "final_response" if strict_match else "exact_body_response"}
    seen = {"case", "phase"}
    for name, raw_value in match.groupdict().items():
        if name in seen:
            return None
        seen.add(name)
        if name in ranges:
            if len(raw_value) > len(str(ranges[name])):
                return None
            value = int(raw_value)
            if value > ranges[name]:
                return None
        elif raw_value in ("true", "false"):
            value = raw_value == "true"
        else:
            return None
        sample[name] = value
    return sample


def _browser_counter_samples(text, mode, cases):
    lines = [line for line in text.splitlines()
             if BROWSER_COUNTER_FAMILY_MARKER in line]
    if not lines:
        return "absent", ()
    if BROWSER_COUNTER_SECRET_TAINT.search(text):
        return "unavailable", ()
    failed = {name for name, status in cases if status == "failed"}
    samples = []
    identities = set()
    for line in lines:
        sample = _browser_counter_sample(line, mode)
        if sample is None or BROWSER_COUNTER_FAILURE_CASES[sample["case"]] not in failed:
            return "unavailable", ()
        identity = (sample["case"], sample.get("iteration"))
        if identity in identities:
            return "unavailable", ()
        identities.add(identity)
        samples.append(sample)
    case_order = {"strict_json_valid": 0, "wire_body_cap": 1}
    samples.sort(key=lambda sample: (case_order[sample["case"]], sample.get("iteration", -1)))
    return "present", tuple(samples)


def _browser_counter_profile_projection(raw, mode):
    expected = BROWSER_COUNTER_PROFILES.get(mode)
    result = {"state": "invalid", "cases": None,
              "counter_state": "unavailable", "samples": ()}
    if expected is None:
        return result
    text, state = _browser_counter_text(raw, allow_test_failure=True)
    if state:
        result["state"] = state
        return result
    if text is None:
        return result
    count = len(expected)
    filtered = len(BROWSER_COUNTER_CASES) - count
    running = re.findall(r"(?m)^running ([0-9]{1,4}) tests?$", text)
    rows = [match.groups() for line in text.splitlines()
            if (match := BROWSER_COUNTER_TEST_ROW.fullmatch(line))]
    test_lines = [line for line in text.splitlines()
                  if line.startswith("test ") and not line.startswith("test result:")]
    summaries = re.findall(r"(?m)^test result: ([^\n]+)$", text)
    if not summaries:
        result["state"] = "unavailable"
        return result
    if (running != [str(count)] or len(test_lines) != count or len(rows) != count
            or {name for name, _ in rows} != set(expected)
            or len({name for name, _ in rows}) != count
            or any(status == "ignored" for _, status in rows)
            or len(summaries) != 1):
        return result
    failures = sum(status == "FAILED" for _, status in rows)
    if failures == 0:
        pattern = (rf"ok\. {count} passed; 0 failed; 0 ignored; 0 measured; "
                   rf"{filtered} filtered out; finished in [0-9.]+s")
        if not re.fullmatch(pattern, summaries[0]):
            return result
    else:
        pattern = (rf"FAILED\. {count-failures} passed; {failures} failed; 0 ignored; "
                   rf"0 measured; {filtered} filtered out; finished in [0-9.]+s")
        if not re.fullmatch(pattern, summaries[0]):
            return result
    failure_footers = text.splitlines().count(BROWSER_COUNTER_CARGO_TEST_FAILURE)
    if failure_footers > 1 or (failure_footers and failures == 0):
        return result
    case_states = {name: status for name, status in rows}
    cases = tuple((name, "failed" if case_states[name] == "FAILED" else "passed")
                  for name in expected)
    result["cases"] = cases
    result["state"] = "test_failed" if failures else "passed"
    if BROWSER_COUNTER_SECRET_TAINT.search(text):
        result["state"] = "invalid"
        return result
    counter_state, samples = _browser_counter_samples(text, mode, cases)
    result["counter_state"] = counter_state
    result["samples"] = samples
    if counter_state == "unavailable":
        result["state"] = "invalid"
    return result


def classify_browser_counter_profile(raw, mode):
    return _browser_counter_profile_projection(raw, mode)["state"]


def classify_browser_counter_compile(raw):
    text, state = _browser_counter_text(raw)
    if state:
        return state
    if text is None:
        return "invalid"
    return "compiled"


def classify(raw, mode):
    if mode == BROWSER_COUNTER_LIST:
        return classify_browser_counter_list(raw)
    if mode == BROWSER_COUNTER_COMPILE:
        return classify_browser_counter_compile(raw)
    if mode in BROWSER_COUNTER_PROFILES:
        return classify_browser_counter_profile(raw, mode)
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


def prelogin_failure_locations(raw):
    """Project only fixed source identifiers and bounded numeric coordinates.

    A failed diagnostic is never an acceptance receipt. Preserve privacy even
    when a panic contains credentials, SQL, machine paths or terminal commands.
    """
    if len(raw) > MAX_BYTES:
        return None
    raw = raw.replace(b"\r\n", b"\n")
    if b"\r" in raw:
        return None
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeError:
        return None
    expected = EXPECTED["prelogin-issuer"][0]
    rows = re.findall(r"(?m)^test ([^ \n]+) \.\.\. ([^\n]+)$", text)
    test_lines = [line for line in text.splitlines()
                  if line.startswith("test ") and not line.startswith("test result:")]
    if rows != [(expected, "FAILED")] or len(test_lines) != 1:
        return None
    if re.findall(r"(?m)^running ([0-9]+) tests?$", text) != ["1"]:
        return None
    summaries = re.findall(r"(?m)^test result: ([^\n]+)$", text)
    if len(summaries) != 1 or not re.fullmatch(
            r"FAILED\. 0 passed; 1 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9.]+s",
            summaries[0]):
        return None
    locations = []
    for source, label in (("runtime/src/auth/prelogin/fixed_wire_owner_tests.rs", "owner"),
                          ("runtime/src/auth/prelogin/tests.rs", "fixture"),
                          ("runtime/src/auth/prelogin.rs", "prelogin")):
        pattern = re.escape(source).replace("/", r"[/\\]") + r":([0-9]{1,6}):([0-9]{1,6}):"
        locations.extend((label, int(line), int(column))
                         for line, column in sorted(set(re.findall(pattern, text)))[:8])
    return tuple(locations)


def _browser_counter_sample_line(sample):
    parts = []
    for name, value in sample.items():
        if isinstance(value, bool):
            rendered = "true" if value else "false"
        else:
            rendered = str(value)
        parts.append(name + "=" + rendered)
    return "sf07_browser_counter_sample " + " ".join(parts)


def main():
    mode = sys.argv[1] if len(sys.argv) == 2 else ""
    raw = sys.stdin.buffer.read(MAX_BYTES + 1)
    label = mode if mode in EXPECTED or mode in BROWSER_COUNTER_PROFILE_NAMES else "invalid"
    if mode in BROWSER_COUNTER_PROFILES:
        projection = _browser_counter_profile_projection(raw, mode)
        state = projection["state"]
        print("sf07_checkpoint scope=" + label + " state=" + state)
        cases = projection["cases"]
        if cases is None:
            print("sf07_browser_cases state=unavailable")
        else:
            print("sf07_browser_cases state=complete count=" + str(len(cases)))
            for name, case_state in cases:
                print("sf07_browser_case name=" + name + " state=" + case_state)
        counter_state = projection["counter_state"]
        if counter_state == "present":
            print("sf07_browser_counters state=present count=" + str(len(projection["samples"])))
            for sample in projection["samples"]:
                print(_browser_counter_sample_line(sample))
        elif counter_state == "absent":
            print("sf07_browser_counters state=absent count=0")
        else:
            print("sf07_browser_counters state=unavailable")
        return 0 if state == "passed" else 1

    state = classify(raw, mode)
    # Both rendered fields are fixed enums, never values taken from the log.
    print("sf07_checkpoint scope=" + label + " state=" + state)
    if mode == "prelogin-issuer" and state == "test_failed":
        locations = prelogin_failure_locations(raw)
        if not locations:
            print("sf07_prelogin_owner_failure state=unavailable")
        else:
            for source, line, column in locations:
                print("sf07_prelogin_owner_failure state=located source=" + source
                      + " line=" + str(line) + " column=" + str(column))
    if mode == "catalog-retained" and state == "test_failed":
        projection = catalog_failure_projection(raw)
        if projection is None:
            print("sf07_catalog_diagnostic state=unavailable")
        else:
            print("sf07_catalog_diagnostic state=classified receipt=" + projection[0]
                  + " preallocation=" + projection[1] + " excess_capacity=" + projection[2])
    return 0 if state == "passed" or state == "compiled" else 1


if __name__ == "__main__":
    raise SystemExit(main())
