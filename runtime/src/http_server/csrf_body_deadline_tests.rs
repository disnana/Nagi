use super::*;

#[test]
fn existing_budget_tuples_select_absolute_deadline_and_status() {
    let admitted = Instant::now();
    for (body_ms, security_ms, body_first) in [(80, 240, true), (240, 80, false), (100, 100, false)]
    {
        let common = admitted + Duration::from_millis(security_ms);
        let selected =
            select_body_deadline(common, admitted, Duration::from_millis(body_ms)).unwrap();
        assert_eq!(
            selected.until,
            common.min(admitted + Duration::from_millis(body_ms))
        );
        assert_eq!(selected.body_first, body_first);
    }
}

#[test]
fn preparation_spends_original_budget_and_never_restarts_common() {
    let admitted = Instant::now();
    let common = admitted + Duration::from_millis(240);
    for (prep_ms, until_ms, body_first) in [
        (0, 80, true),
        (159, 239, true),
        (160, 240, false),
        (200, 240, false),
        (240, 240, false),
        (300, 240, false),
    ] {
        let selected = select_body_deadline(
            common,
            admitted + Duration::from_millis(prep_ms),
            Duration::from_millis(80),
        )
        .unwrap();
        assert_eq!(selected.until, admitted + Duration::from_millis(until_ms));
        assert_eq!(selected.body_first, body_first);
    }
}

#[test]
fn original_credential_common_can_only_shorten_body_window() {
    let admitted = Instant::now();
    let security_until = admitted + Duration::from_millis(240);
    let credential_until = admitted + Duration::from_millis(100);
    let selected = select_body_deadline(
        security_until.min(credential_until),
        admitted + Duration::from_millis(30),
        Duration::from_millis(80),
    )
    .unwrap();
    assert_eq!(selected.until, credential_until);
    assert!(!selected.body_first);
}

#[test]
fn exact_tie_and_one_nanosecond_neighbors_are_strict() {
    let started = Instant::now();
    let body_until = started + Duration::from_millis(100);
    for (common, expected) in [
        (body_until - Duration::from_nanos(1), false),
        (body_until, false),
        (body_until + Duration::from_nanos(1), true),
    ] {
        let selected = select_body_deadline(common, started, Duration::from_millis(100)).unwrap();
        assert_eq!(selected.body_first, expected);
        assert_eq!(selected.until, common.min(body_until));
    }
}

#[test]
fn unrepresentable_body_deadline_remains_rejected() {
    let started = Instant::now();
    assert!(select_body_deadline(started, started, Duration::MAX).is_none());
}
