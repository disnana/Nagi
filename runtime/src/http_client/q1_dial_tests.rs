use super::super::{AddressError, InternalPolicy, ServiceIdentity};
use super::*;
use std::{net::IpAddr, str::FromStr};

const ID: ServiceIdentity = ServiceIdentity {
    service: 11,
    purpose: 17,
    generation: 23,
    trust: 31,
};
const PORT: u16 = 8443;

fn ip(text: &str) -> IpAddr {
    IpAddr::from_str(text).unwrap()
}

fn v4(values: &[&str]) -> Vec<Ipv4Addr> {
    values
        .iter()
        .map(|value| match ip(value) {
            IpAddr::V4(address) => address,
            IpAddr::V6(_) => panic!("not IPv4: {value}"),
        })
        .collect()
}

fn v6(values: &[&str]) -> Vec<Ipv6Addr> {
    values
        .iter()
        .map(|value| match ip(value) {
            IpAddr::V6(address) => address,
            IpAddr::V4(_) => panic!("not IPv6: {value}"),
        })
        .collect()
}

fn policy(max_candidates: usize) -> InternalPolicy {
    InternalPolicy::checked(ID, PORT, max_candidates, Vec::new()).unwrap()
}

fn brand(id: u64) -> OperationBrand {
    OperationBrand::from_admitted_operation(NonZeroU64::new(id).unwrap())
}

#[allow(clippy::too_many_arguments)]
fn checked(
    now: Instant,
    ipv4: Result<Vec<Ipv4Addr>, ResolverFailure>,
    ipv6: Result<Vec<Ipv6Addr>, ResolverFailure>,
    ttl_chain: &[u32],
    resolver_valid_until: Option<Instant>,
    original_resolve_end: Instant,
    operation_end: Instant,
    max_candidates: usize,
    operation_id: u64,
) -> Result<CheckedResolution, Q1Error> {
    SuccessfulLookup::from_provider_results(
        brand(operation_id),
        ipv4,
        ipv6,
        ttl_chain.to_vec(),
        resolver_valid_until,
        original_resolve_end,
        operation_end,
    )?
    .check(&policy(max_candidates), ID, now)
}

#[allow(clippy::too_many_arguments)]
fn plan(
    now: Instant,
    ipv4: &[&str],
    ipv6: &[&str],
    ttl_chain: &[u32],
    resolver_valid_until: Option<Instant>,
    original_resolve_end: Instant,
    operation_end: Instant,
    max_candidates: usize,
    max_attempts: usize,
    connect_budget: Duration,
    preferred: PreferredFamily,
    operation_id: u64,
) -> Result<DialPlan, Q1Error> {
    let resolution = checked(
        now,
        Ok(v4(ipv4)),
        Ok(v6(ipv6)),
        ttl_chain,
        resolver_valid_until,
        original_resolve_end,
        operation_end,
        max_candidates,
        operation_id,
    )?;
    DialPlan::checked(
        resolution,
        AttemptOrder::Interleave { preferred },
        max_attempts,
        connect_budget,
        now,
    )
}

fn ends(now: Instant) -> (Instant, Instant, Instant) {
    (
        now + Duration::from_secs(20),
        now + Duration::from_secs(30),
        now + Duration::from_secs(40),
    )
}

fn one_plan(now: Instant) -> DialPlan {
    let (ttl_end, resolve_end, operation_end) = ends(now);
    plan(
        now,
        &["8.8.4.4", "8.8.8.8"],
        &["2606:4700:4700::8844", "2606:4700:4700::8888"],
        &[30],
        Some(ttl_end),
        resolve_end,
        operation_end,
        8,
        4,
        Duration::from_secs(12),
        PreferredFamily::V4,
        71,
    )
    .unwrap()
}

fn clean_child() -> AttemptCleanup {
    AttemptCleanup::SocketDisposedAndChildJoined
}

fn expect_stop<T>(result: Result<T, PlanStop>, expected: PlanStop) {
    match result {
        Err(actual) => assert_eq!(actual, expected),
        Ok(_) => panic!("expected stop {expected:?}"),
    }
}

fn expect_q1<T>(result: Result<T, Q1Error>, expected: Q1Error) {
    match result {
        Err(actual) => assert_eq!(actual, expected),
        Ok(_) => panic!("expected Q1 error {expected:?}"),
    }
}

#[test]
fn f01_every_candidate_is_checked_before_any_plan_exists() {
    let now = Instant::now();
    let (_, resolve_end, operation_end) = ends(now);
    let error = checked(
        now,
        Ok(v4(&["8.8.8.8"])),
        Ok(v6(&["2001:db8::1"])),
        &[30],
        Some(now + Duration::from_secs(20)),
        resolve_end,
        operation_end,
        8,
        71,
    )
    .unwrap_err();
    assert_eq!(error, Q1Error::Address(AddressError::Disallowed));
}

#[test]
fn f02_observed_duplicates_count_before_deduplication() {
    let now = Instant::now();
    let (_, resolve_end, operation_end) = ends(now);
    let error = checked(
        now,
        Ok(v4(&["8.8.8.8", "8.8.8.8", "8.8.8.8"])),
        Ok(Vec::new()),
        &[30],
        Some(now + Duration::from_secs(20)),
        resolve_end,
        operation_end,
        2,
        71,
    )
    .unwrap_err();
    assert_eq!(error, Q1Error::Address(AddressError::TooManyCandidates));
}

#[test]
fn f03_explicit_family_order_is_interleaved_and_finite() {
    let now = Instant::now();
    let (ttl_end, resolve_end, operation_end) = ends(now);
    let mut dial = plan(
        now,
        &["8.8.8.8", "8.8.4.4"],
        &["2606:4700:4700::8888", "2606:4700:4700::8844"],
        &[30],
        Some(ttl_end),
        resolve_end,
        operation_end,
        8,
        3,
        Duration::from_secs(9),
        PreferredFamily::V6,
        71,
    )
    .unwrap();
    assert_eq!(dial.candidate_count_for_test(), 3);
    let expected = [
        SocketAddr::new(ip("2606:4700:4700::8844"), PORT),
        SocketAddr::new(ip("8.8.4.4"), PORT),
        SocketAddr::new(ip("2606:4700:4700::8888"), PORT),
    ];
    let control = AttemptControl::default();
    for (index, address) in expected.into_iter().enumerate() {
        let attempt = dial
            .next_attempt(now + Duration::from_secs(index as u64), &control)
            .unwrap();
        assert_eq!(attempt.address(), address);
        if index < 2 {
            dial.fail_after_cleanup(
                attempt,
                AttemptFailure::Io(ErrorKind::ConnectionRefused),
                clean_child(),
                now + Duration::from_secs(index as u64 + 1),
                &control,
            )
            .unwrap();
        } else {
            dial.connected_after_peer_check(
                attempt,
                now + Duration::from_secs(index as u64 + 1),
                &control,
            )
            .unwrap();
        }
    }
    expect_stop(
        dial.next_attempt(now + Duration::from_secs(3), &control),
        PlanStop::Finished,
    );
}

#[test]
fn f04_only_the_adopted_connect_error_kinds_are_retryable() {
    for kind in [
        ErrorKind::ConnectionRefused,
        ErrorKind::NetworkUnreachable,
        ErrorKind::HostUnreachable,
    ] {
        assert_eq!(
            classify_connect_io(kind),
            ConnectRetryability::RetryNextCandidate
        );
    }
    for kind in [
        ErrorKind::TimedOut,
        ErrorKind::ConnectionReset,
        ErrorKind::ConnectionAborted,
        ErrorKind::PermissionDenied,
        ErrorKind::InvalidInput,
        ErrorKind::AddrNotAvailable,
        ErrorKind::OutOfMemory,
        ErrorKind::Other,
        ErrorKind::WouldBlock,
    ] {
        assert_eq!(
            classify_connect_io(kind),
            ConnectRetryability::Terminal,
            "{kind:?}"
        );
    }
}

#[test]
fn f05_slot_expiry_is_distinct_from_os_timeout() {
    let now = Instant::now();
    let control = AttemptControl::default();
    let mut slot_plan = one_plan(now);
    let first = slot_plan.next_attempt(now, &control).unwrap();
    let first_end = first.slot_end();
    slot_plan
        .fail_after_cleanup(
            first,
            AttemptFailure::SlotExpired,
            clean_child(),
            first_end,
            &control,
        )
        .unwrap();
    let second = slot_plan.next_attempt(first_end, &control).unwrap();
    assert_ne!(second.address(), SocketAddr::new(ip("8.8.4.4"), PORT));

    let mut os_timeout_plan = one_plan(now);
    let attempt = os_timeout_plan.next_attempt(now, &control).unwrap();
    expect_stop(
        os_timeout_plan.fail_after_cleanup(
            attempt,
            AttemptFailure::Io(ErrorKind::TimedOut),
            clean_child(),
            now + Duration::from_secs(1),
            &control,
        ),
        PlanStop::FatalConnect,
    );
}

#[test]
fn f06_absolute_slot_end_does_not_restart_and_equality_skips_native_start() {
    let now = Instant::now();
    let control = AttemptControl::default();
    let mut dial = one_plan(now);
    let first = dial.next_attempt(now, &control).unwrap();
    assert_eq!(first.slot_end(), now + Duration::from_secs(3));
    dial.fail_after_cleanup(
        first,
        AttemptFailure::Io(ErrorKind::ConnectionRefused),
        clean_child(),
        now + Duration::from_secs(1),
        &control,
    )
    .unwrap();
    let second = dial
        .next_attempt(now + Duration::from_secs(1), &control)
        .unwrap();
    assert_eq!(second.slot_end(), now + Duration::from_secs(6));

    let (ttl_end, resolve_end, operation_end) = ends(now);
    let mut late = plan(
        now,
        &["8.8.4.4", "8.8.8.8"],
        &[],
        &[30],
        Some(ttl_end),
        resolve_end,
        operation_end,
        4,
        2,
        Duration::from_secs(10),
        PreferredFamily::V4,
        71,
    )
    .unwrap();
    let at_first_slot_end = late
        .next_attempt(now + Duration::from_secs(5), &control)
        .unwrap();
    assert_eq!(
        at_first_slot_end.address(),
        SocketAddr::new(ip("8.8.8.8"), PORT)
    );
    assert_eq!(at_first_slot_end.slot_end(), now + Duration::from_secs(10));
}

#[test]
fn f07_positive_ttl_expiry_after_cleanup_blocks_the_next_address() {
    let now = Instant::now();
    let ttl_end = now + Duration::from_secs(2);
    let mut dial = plan(
        now,
        &["8.8.4.4", "8.8.8.8"],
        &[],
        &[10],
        Some(ttl_end),
        now + Duration::from_secs(20),
        now + Duration::from_secs(30),
        4,
        2,
        Duration::from_secs(10),
        PreferredFamily::V4,
        71,
    )
    .unwrap();
    let attempt = dial.next_attempt(now, &AttemptControl::default()).unwrap();
    expect_stop(
        dial.fail_after_cleanup(
            attempt,
            AttemptFailure::Io(ErrorKind::ConnectionRefused),
            clean_child(),
            ttl_end,
            &AttemptControl::default(),
        ),
        PlanStop::ResolutionExpired,
    );
}

#[test]
fn f08_close_cancel_and_first_fault_are_sticky_terminal_gates() {
    let now = Instant::now();
    let mut closing = AttemptControl::default();
    closing.close();
    closing.cancel();
    expect_stop(one_plan(now).next_attempt(now, &closing), PlanStop::Closing);

    let mut cancelled = AttemptControl::default();
    cancelled.cancel();
    expect_stop(
        one_plan(now).next_attempt(now, &cancelled),
        PlanStop::Cancelled,
    );

    let mut faulty = AttemptControl::default();
    faulty.latch_fault(7);
    faulty.latch_fault(9);
    faulty.close();
    expect_stop(one_plan(now).next_attempt(now, &faulty), PlanStop::Fault(7));
}

#[test]
fn f09_an_attempt_cannot_advance_until_cleanup_is_complete() {
    let now = Instant::now();
    let control = AttemptControl::default();
    let mut dial = one_plan(now);
    let attempt = dial.next_attempt(now, &control).unwrap();
    expect_stop(
        dial.fail_after_cleanup(
            attempt,
            AttemptFailure::Io(ErrorKind::ConnectionRefused),
            AttemptCleanup::Incomplete,
            now + Duration::from_secs(1),
            &control,
        ),
        PlanStop::CleanupIncomplete,
    );
    expect_stop(
        dial.next_attempt(now + Duration::from_secs(1), &control),
        PlanStop::Finished,
    );

    let mut no_child = one_plan(now);
    let attempt = no_child.next_attempt(now, &control).unwrap();
    no_child
        .fail_after_cleanup(
            attempt,
            AttemptFailure::Io(ErrorKind::ConnectionRefused),
            AttemptCleanup::SocketDisposedAndProvenNoChild,
            now + Duration::from_secs(1),
            &control,
        )
        .unwrap();
    assert!(no_child
        .next_attempt(now + Duration::from_secs(1), &control)
        .is_ok());
}

#[test]
fn f10_retries_keep_identity_operation_port_and_original_end() {
    let now = Instant::now();
    let (_, _, operation_end) = ends(now);
    let mut dial = one_plan(now);
    assert_eq!(dial.identity_for_test(), ID);
    assert_eq!(dial.operation_id_for_test(), 71);
    assert_eq!(dial.operation_end_for_test(), now + Duration::from_secs(40));

    let control = AttemptControl::default();
    let first = dial.next_attempt(now, &control).unwrap();
    assert_eq!(first.address().port(), PORT);
    dial.fail_after_cleanup(
        first,
        AttemptFailure::Io(ErrorKind::ConnectionRefused),
        clean_child(),
        now + Duration::from_secs(1),
        &control,
    )
    .unwrap();
    let second = dial
        .next_attempt(now + Duration::from_secs(1), &control)
        .unwrap();
    assert_eq!(second.address().port(), PORT);
    assert_eq!(dial.identity_for_test(), ID);
    assert_eq!(dial.operation_id_for_test(), 71);
    assert_eq!(dial.operation_end_for_test(), operation_end);
    expect_stop(
        dial.next_attempt(operation_end, &control),
        PlanStop::OperationExpired,
    );
}

#[test]
fn f11_tls_and_http_stages_seal_retry_and_allow_one_enqueue() {
    let now = Instant::now();
    let control = AttemptControl::default();
    let mut dial = one_plan(now);
    let attempt = dial.next_attempt(now, &control).unwrap();
    dial.connected_after_peer_check(attempt, now + Duration::from_secs(1), &control)
        .unwrap();
    expect_stop(
        dial.next_attempt(now + Duration::from_secs(1), &control),
        PlanStop::Finished,
    );
    expect_q1(
        dial.take_http_enqueue_ticket(now + Duration::from_secs(1), &control),
        Q1Error::NotConnected,
    );
    dial.start_tls(now + Duration::from_secs(2), &control)
        .unwrap();
    dial.finish_tls(now + Duration::from_secs(3), &control)
        .unwrap();
    let enqueue = dial
        .take_http_enqueue_ticket(now + Duration::from_secs(4), &control)
        .unwrap();
    assert_eq!(enqueue.operation_id(), 71);
    expect_q1(
        dial.take_http_enqueue_ticket(now + Duration::from_secs(4), &control),
        Q1Error::EnqueueAlreadyIssued,
    );
    dial.finish_http(enqueue).unwrap();
    expect_q1(
        dial.take_http_enqueue_ticket(now + Duration::from_secs(5), &control),
        Q1Error::NotConnected,
    );
}

#[test]
fn f12_peer_mismatch_is_terminal_even_with_another_checked_address() {
    let now = Instant::now();
    let control = AttemptControl::default();
    for failure in [
        AttemptFailure::Capacity,
        AttemptFailure::PeerMismatch,
        AttemptFailure::TlsFailure,
        AttemptFailure::Unknown,
    ] {
        let mut dial = one_plan(now);
        let attempt = dial.next_attempt(now, &control).unwrap();
        expect_stop(
            dial.fail_after_cleanup(
                attempt,
                failure,
                clean_child(),
                now + Duration::from_secs(1),
                &control,
            ),
            PlanStop::FatalConnect,
        );
    }
}

#[test]
fn pure_f13_any_required_family_error_rejects_the_whole_lookup() {
    let now = Instant::now();
    let (_, resolve_end, operation_end) = ends(now);
    assert_eq!(
        checked(
            now,
            Ok(v4(&["8.8.8.8"])),
            Err(ResolverFailure::Timeout),
            &[30],
            Some(now + Duration::from_secs(20)),
            resolve_end,
            operation_end,
            8,
            71,
        )
        .unwrap_err(),
        Q1Error::FamilyFailure
    );
    assert_eq!(
        checked(
            now,
            Err(ResolverFailure::ServFail),
            Ok(v6(&["2606:4700:4700::8888"])),
            &[30],
            Some(now + Duration::from_secs(20)),
            resolve_end,
            operation_end,
            8,
            71,
        )
        .unwrap_err(),
        Q1Error::FamilyFailure
    );
}

#[test]
fn pure_f14_attempt_configuration_is_explicit_and_finite() {
    let now = Instant::now();
    let (ttl_end, resolve_end, operation_end) = ends(now);
    let resolution = checked(
        now,
        Ok(v4(&["8.8.8.8"])),
        Ok(Vec::new()),
        &[30],
        Some(ttl_end),
        resolve_end,
        operation_end,
        8,
        71,
    )
    .unwrap();
    assert_eq!(
        DialPlan::checked(
            resolution,
            AttemptOrder::Interleave {
                preferred: PreferredFamily::V4
            },
            0,
            Duration::from_secs(10),
            now,
        )
        .unwrap_err(),
        Q1Error::InvalidAttemptLimit
    );

    let resolution = checked(
        now,
        Ok(v4(&["8.8.8.8"])),
        Ok(Vec::new()),
        &[30],
        Some(ttl_end),
        resolve_end,
        operation_end,
        8,
        71,
    )
    .unwrap();
    assert_eq!(
        DialPlan::checked(
            resolution,
            AttemptOrder::Interleave {
                preferred: PreferredFamily::V4
            },
            1,
            Duration::ZERO,
            now,
        )
        .unwrap_err(),
        Q1Error::InvalidConnectBudget
    );
}

#[test]
fn z01_zero_ttl_from_successful_provider_results_bypasses_cache() {
    let now = Instant::now();
    let (_, resolve_end, operation_end) = ends(now);
    let resolution = checked(
        now,
        Ok(v4(&["8.8.8.8"])),
        Ok(v6(&["2606:4700:4700::8888"])),
        &[30, 0, 30],
        None,
        resolve_end,
        operation_end,
        8,
        71,
    )
    .unwrap();
    assert_eq!(
        resolution.ticket.cache_disposition(),
        CacheDisposition::Bypass
    );
    assert_eq!(resolution.ticket.boundary(), resolve_end);
}

#[test]
fn z02_only_successful_family_results_can_create_a_ticket() {
    let now = Instant::now();
    let (_, resolve_end, operation_end) = ends(now);
    for (ipv4, ipv6) in [
        (
            Err(ResolverFailure::Other),
            Ok(v6(&["2606:4700:4700::8888"])),
        ),
        (Ok(v4(&["8.8.8.8"])), Err(ResolverFailure::ServFail)),
    ] {
        assert_eq!(
            checked(
                now,
                ipv4,
                ipv6,
                &[0],
                None,
                resolve_end,
                operation_end,
                8,
                71,
            )
            .unwrap_err(),
            Q1Error::FamilyFailure
        );
    }
}

#[test]
fn z03_ticket_is_consumed_once_into_one_operation_plan() {
    let now = Instant::now();
    let (_, resolve_end, operation_end) = ends(now);
    let resolution = checked(
        now,
        Ok(v4(&["8.8.8.8", "8.8.4.4"])),
        Ok(Vec::new()),
        &[0],
        None,
        resolve_end,
        operation_end,
        8,
        555,
    )
    .unwrap();
    let mut dial = DialPlan::checked(
        resolution,
        AttemptOrder::Interleave {
            preferred: PreferredFamily::V4,
        },
        2,
        Duration::from_secs(10),
        now,
    )
    .unwrap();
    assert_eq!(dial.operation_id_for_test(), 555);
    let attempt = dial.next_attempt(now, &AttemptControl::default()).unwrap();
    assert_eq!(attempt.address().port(), PORT);
    assert_eq!(dial.operation_id_for_test(), 555);
}

#[test]
fn z04_original_resolution_end_blocks_a_late_first_attempt() {
    let now = Instant::now();
    let resolve_end = now + Duration::from_secs(1);
    let operation_end = now + Duration::from_secs(30);
    let checked_at = now;
    let resolution = checked(
        checked_at,
        Ok(v4(&["8.8.8.8"])),
        Ok(Vec::new()),
        &[0],
        None,
        resolve_end,
        operation_end,
        8,
        71,
    )
    .unwrap();
    let dial = DialPlan::checked(
        resolution,
        AttemptOrder::Interleave {
            preferred: PreferredFamily::V4,
        },
        1,
        Duration::from_secs(10),
        resolve_end,
    )
    .unwrap_err();
    assert_eq!(dial, Q1Error::Address(AddressError::Expired));
}

#[test]
fn z05_zero_ttl_failover_stays_inside_one_original_plan() {
    let now = Instant::now();
    let resolve_end = now + Duration::from_secs(8);
    let operation_end = now + Duration::from_secs(20);
    let resolution = checked(
        now,
        Ok(v4(&["8.8.4.4", "8.8.8.8"])),
        Ok(Vec::new()),
        &[0],
        None,
        resolve_end,
        operation_end,
        8,
        81,
    )
    .unwrap();
    let mut dial = DialPlan::checked(
        resolution,
        AttemptOrder::Interleave {
            preferred: PreferredFamily::V4,
        },
        2,
        Duration::from_secs(6),
        now,
    )
    .unwrap();
    assert_eq!(dial.operation_id_for_test(), 81);
    let control = AttemptControl::default();
    let first = dial.next_attempt(now, &control).unwrap();
    dial.fail_after_cleanup(
        first,
        AttemptFailure::Io(ErrorKind::ConnectionRefused),
        clean_child(),
        now + Duration::from_secs(1),
        &control,
    )
    .unwrap();
    let second = dial
        .next_attempt(now + Duration::from_secs(1), &control)
        .unwrap();
    assert_eq!(
        second.slot_end(),
        resolve_end.min(now + Duration::from_secs(6))
    );
    assert_eq!(dial.operation_id_for_test(), 81);
}

#[test]
fn z06_zero_ttl_equality_expires_without_grace_or_reset() {
    let now = Instant::now();
    let resolve_end = now + Duration::from_secs(5);
    let operation_end = now + Duration::from_secs(20);
    let resolution = checked(
        now,
        Ok(v4(&["8.8.8.8", "8.8.4.4"])),
        Ok(Vec::new()),
        &[0],
        None,
        resolve_end,
        operation_end,
        8,
        91,
    )
    .unwrap();
    let mut dial = DialPlan::checked(
        resolution,
        AttemptOrder::Interleave {
            preferred: PreferredFamily::V4,
        },
        2,
        Duration::from_secs(10),
        now,
    )
    .unwrap();
    expect_stop(
        dial.next_attempt(resolve_end, &AttemptControl::default()),
        PlanStop::ResolutionExpired,
    );
}

#[test]
fn z07_positive_ttl_first_poll_semantics_remain_unchanged() {
    let now = Instant::now();
    let ttl_end = now + Duration::from_secs(2);
    let (_, _, operation_end) = ends(now);
    let mut dial = plan(
        now,
        &["8.8.8.8", "8.8.4.4"],
        &[],
        &[30],
        Some(ttl_end),
        now + Duration::from_secs(20),
        operation_end,
        8,
        2,
        Duration::from_secs(10),
        PreferredFamily::V4,
        101,
    )
    .unwrap();
    let control = AttemptControl::default();
    let attempt = dial.next_attempt(now, &control).unwrap();
    dial.connected_after_peer_check(attempt, ttl_end + Duration::from_secs(1), &control)
        .unwrap();
    let tls_started = ttl_end + Duration::from_secs(1);
    dial.start_tls(tls_started, &control).unwrap();
    dial.finish_tls(tls_started + Duration::from_secs(1), &control)
        .unwrap();
    assert_eq!(
        dial.take_http_enqueue_ticket(tls_started + Duration::from_secs(2), &control)
            .unwrap()
            .operation_id(),
        101
    );
}

#[test]
fn f14_late_tls_stages_ignore_connect_and_positive_ttl_ends_after_tcp() {
    let now = Instant::now();
    let ttl_end = now + Duration::from_secs(1);
    let operation_end = now + Duration::from_secs(40);
    let mut dial = plan(
        now,
        &["8.8.8.8"],
        &[],
        &[30],
        Some(ttl_end),
        now + Duration::from_secs(30),
        operation_end,
        4,
        1,
        Duration::from_secs(2),
        PreferredFamily::V4,
        73,
    )
    .unwrap();
    let control = AttemptControl::default();
    let attempt = dial.next_attempt(now, &control).unwrap();
    // The operation polled before TTL expiry. Later TLS and HTTP stages use only
    // terminal control and the original operation end, not DNS TTL/connect_end.
    dial.connected_after_peer_check(attempt, ttl_end, &control)
        .unwrap();
    let stage_now = now + Duration::from_secs(3);
    dial.start_tls(stage_now, &control).unwrap();
    dial.finish_tls(stage_now + Duration::from_secs(1), &control)
        .unwrap();
    let ticket = dial
        .take_http_enqueue_ticket(stage_now + Duration::from_secs(2), &control)
        .unwrap();
    assert_eq!(ticket.operation_id(), 73);
}

#[test]
fn pure_z08_missing_positive_expiry_and_empty_chain_fail_closed() {
    let now = Instant::now();
    let (_, resolve_end, operation_end) = ends(now);
    assert_eq!(
        checked(
            now,
            Ok(v4(&["8.8.8.8"])),
            Ok(Vec::new()),
            &[30],
            None,
            resolve_end,
            operation_end,
            8,
            111,
        )
        .unwrap_err(),
        Q1Error::MissingPositiveExpiry
    );
    assert_eq!(
        checked(
            now,
            Ok(v4(&["8.8.8.8"])),
            Ok(Vec::new()),
            &[],
            None,
            resolve_end,
            operation_end,
            8,
            111,
        )
        .unwrap_err(),
        Q1Error::EmptyTtlChain
    );

    let resolve_end = now + Duration::from_secs(1);
    assert_eq!(
        checked(
            resolve_end + Duration::from_secs(1),
            Ok(v4(&["8.8.8.8"])),
            Ok(Vec::new()),
            &[30],
            Some(now + Duration::from_secs(30)),
            resolve_end,
            operation_end,
            8,
            111,
        )
        .unwrap_err(),
        Q1Error::Address(AddressError::Expired)
    );
}

#[test]
fn review_q1_foreign_plan_attempt_cannot_complete_same_index() {
    let now = Instant::now();
    let control = AttemptControl::default();
    let mut owner = one_plan(now);
    let mut other = one_plan(now);
    let owned_attempt = owner.next_attempt(now, &control).unwrap();
    let other_attempt = other.next_attempt(now, &control).unwrap();
    assert_eq!(owned_attempt.address(), other_attempt.address());
    assert_eq!(owned_attempt.slot_end(), other_attempt.slot_end());
    expect_stop(
        other.connected_after_peer_check(owned_attempt, now + Duration::from_secs(1), &control),
        PlanStop::InvalidState,
    );
}

#[test]
fn review_q1_foreign_plan_attempt_cannot_advance_retry_cursor() {
    let now = Instant::now();
    let control = AttemptControl::default();
    let mut owner = one_plan(now);
    let mut other = one_plan(now);
    let foreign = owner.next_attempt(now, &control).unwrap();
    let local = other.next_attempt(now, &control).unwrap();
    assert_eq!(foreign.address(), local.address());
    assert_eq!(foreign.slot_end(), local.slot_end());
    expect_stop(
        other.fail_after_cleanup(
            foreign,
            AttemptFailure::Io(ErrorKind::ConnectionRefused),
            clean_child(),
            now + Duration::from_secs(1),
            &control,
        ),
        PlanStop::InvalidState,
    );
}

#[test]
fn review_q1_tls_start_observes_close_after_peer_check() {
    let now = Instant::now();
    let mut control = AttemptControl::default();
    let mut dial = one_plan(now);
    let attempt = dial.next_attempt(now, &control).unwrap();
    dial.connected_after_peer_check(attempt, now + Duration::from_secs(1), &control)
        .unwrap();
    control.close();
    expect_q1(
        dial.start_tls(now + Duration::from_secs(2), &control),
        Q1Error::PlanStopped(PlanStop::Closing),
    );
}

#[test]
fn review_q1_tls_finish_observes_cancel_after_tls_start() {
    let now = Instant::now();
    let mut control = AttemptControl::default();
    let mut dial = one_plan(now);
    let attempt = dial.next_attempt(now, &control).unwrap();
    dial.connected_after_peer_check(attempt, now + Duration::from_secs(1), &control)
        .unwrap();
    dial.start_tls(now + Duration::from_secs(2), &control)
        .unwrap();
    control.cancel();
    expect_q1(
        dial.finish_tls(now + Duration::from_secs(3), &control),
        Q1Error::PlanStopped(PlanStop::Cancelled),
    );
}

#[test]
fn review_q1_http_ticket_observes_first_fault_after_tls() {
    let now = Instant::now();
    let mut control = AttemptControl::default();
    let mut dial = one_plan(now);
    let attempt = dial.next_attempt(now, &control).unwrap();
    dial.connected_after_peer_check(attempt, now + Duration::from_secs(1), &control)
        .unwrap();
    dial.start_tls(now + Duration::from_secs(2), &control)
        .unwrap();
    dial.finish_tls(now + Duration::from_secs(3), &control)
        .unwrap();
    control.latch_fault(29);
    expect_q1(
        dial.take_http_enqueue_ticket(now + Duration::from_secs(4), &control),
        Q1Error::PlanStopped(PlanStop::Fault(29)),
    );
}

#[test]
fn review_q1_tls_and_enqueue_stages_observe_original_operation_end() {
    let start = Instant::now();
    let operation_end = start + Duration::from_millis(80);
    let control = AttemptControl::default();
    let mut start_plan = plan(
        start,
        &["8.8.4.4", "8.8.8.8"],
        &[],
        &[30],
        Some(start + Duration::from_secs(30)),
        start + Duration::from_secs(60),
        operation_end,
        4,
        2,
        Duration::from_secs(1),
        PreferredFamily::V4,
        71,
    )
    .unwrap();
    let mut finish_plan = plan(
        start,
        &["8.8.4.4", "8.8.8.8"],
        &[],
        &[30],
        Some(start + Duration::from_secs(30)),
        start + Duration::from_secs(60),
        operation_end,
        4,
        2,
        Duration::from_secs(1),
        PreferredFamily::V4,
        71,
    )
    .unwrap();
    let mut enqueue_plan = plan(
        start,
        &["8.8.4.4", "8.8.8.8"],
        &[],
        &[30],
        Some(start + Duration::from_secs(30)),
        start + Duration::from_secs(60),
        operation_end,
        4,
        2,
        Duration::from_secs(1),
        PreferredFamily::V4,
        71,
    )
    .unwrap();
    for dial in [&mut start_plan, &mut finish_plan, &mut enqueue_plan] {
        let attempt = dial.next_attempt(start, &control).unwrap();
        dial.connected_after_peer_check(attempt, start + Duration::from_millis(1), &control)
            .unwrap();
    }
    finish_plan
        .start_tls(start + Duration::from_millis(2), &control)
        .unwrap();
    enqueue_plan
        .start_tls(start + Duration::from_millis(2), &control)
        .unwrap();
    enqueue_plan
        .finish_tls(start + Duration::from_millis(3), &control)
        .unwrap();
    expect_q1(
        start_plan.start_tls(operation_end, &control),
        Q1Error::PlanStopped(PlanStop::OperationExpired),
    );
    expect_q1(
        finish_plan.finish_tls(operation_end, &control),
        Q1Error::PlanStopped(PlanStop::OperationExpired),
    );
    expect_q1(
        enqueue_plan.take_http_enqueue_ticket(operation_end, &control),
        Q1Error::PlanStopped(PlanStop::OperationExpired),
    );
}
