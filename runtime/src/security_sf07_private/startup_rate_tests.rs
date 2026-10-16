//! Small owned configuration tests. These explicit values are fixture inputs,
//! not public defaults, a measured deployment profile or a whole-RSS claim.
use super::*;

fn capacity(bytes: i64, work: i64) -> Capacity {
    Capacity::checked(8, 4, 8192, bytes, work).unwrap()
}
fn public() -> Record {
    Record {
        policy: Policy::checked(1, 60_000).unwrap(),
        subject_support: SubjectSupport::None,
    }
}
fn protected(slots: i64) -> Record {
    Record {
        policy: Policy::checked(1, 60_000)
            .unwrap()
            .with_subject(1, 60_000, slots, 60_000)
            .unwrap(),
        subject_support: SubjectSupport::Verified,
    }
}
fn plan(records: &[Record]) -> Plan {
    Plan::checked(records, capacity(4 * 1024 * 1024, 10_000), 0)
        .unwrap()
        .unwrap()
}
fn at(plan: &Plan, binding: Binding, subject: Option<i64>) -> RateDecision {
    let (rule, key) = match subject {
        None => (binding.route, FixedKey::Route),
        Some(subject) => {
            let (rule, domain) = binding.subject.unwrap();
            (rule, FixedKey::subject(domain, subject))
        }
    };
    plan.owner.charge_at_with_test_clock(rule, key, 0)
}

#[test]
fn all_five_capacity_values_are_explicit_positive_and_representable() {
    for index in 0..5 {
        for invalid in [0, -1] {
            let mut input = [4, 2, 8, 1_000_000, 10_000];
            input[index] = invalid;
            assert!(Capacity::checked(input[0], input[1], input[2], input[3], input[4]).is_err());
        }
    }
    assert!(Capacity::checked(4, 65_537, 8, 1_000_000, 10_000).is_err());
    assert!(Capacity::checked(4, 65_536, 8, 1_000_000, 10_000).is_ok());
}

#[test]
fn policy_time_burst_and_duplicate_subject_fail_before_allocation() {
    for value in [0, -1, i64::MAX] {
        assert!(Policy::checked(value, 1).is_err());
        assert!(Policy::checked(1, value).is_err());
    }
    assert!(Policy::checked(1, 1)
        .unwrap()
        .with_subject(1, 1, i64::MAX, 1)
        .is_err());
    assert!(Policy::checked(1, 1)
        .unwrap()
        .with_subject(1, 1, 1, 1)
        .unwrap()
        .with_subject(1, 1, 1, 1)
        .is_err());
}

#[test]
fn aggregate_rules_domains_and_slots_each_have_their_own_limit() {
    let records = [public(), protected(2)];
    for input in [(2, 1, 2), (3, 1, 1)] {
        let limits = Capacity::checked(input.0, input.1, input.2, 1_000_000, 10_000).unwrap();
        assert!(Plan::checked(&records, limits, 0).is_err());
    }
    let records = [protected(1), protected(1)];
    let limits = Capacity::checked(4, 1, 2, 1_000_000, 10_000).unwrap();
    assert!(Plan::checked(&records, limits, 0).is_err());
    let limits = Capacity::checked(4, 2, 2, 1_000_000, 10_000).unwrap();
    assert!(Plan::checked(&records, limits, 0).is_ok());
}

#[test]
fn subject_support_is_checked_at_startup_not_silently_skipped() {
    let mut record = protected(1);
    record.subject_support = SubjectSupport::None;
    assert!(Plan::checked(&[record], capacity(1_000_000, 10_000), 0).is_err());
}

#[test]
fn metadata_byte_refusal_occurs_before_any_vec_or_owner_allocation() {
    let records = [protected(1)];
    let limits = capacity(1, 10_000);
    let (rejected, measured) =
        crate::metrics::measure(|| Plan::checked(&records, limits, 0).is_err());
    assert!(rejected);
    assert_eq!(measured.allocations, 0);
}

#[test]
fn request_work_covers_route_plus_subject_not_only_the_largest_charge() {
    let records = [protected(1)];
    let route = records[0].policy.route.checked_layout().unwrap().work;
    let subject = records[0]
        .policy
        .subject
        .unwrap()
        .checked_layout()
        .unwrap()
        .work;
    let largest = i64::try_from(route.max(subject)).unwrap();
    assert!(Plan::checked(&records, capacity(1_000_000, largest), 0).is_err());
    let total = route.checked_add(subject).unwrap();
    let limits = capacity(1_000_000, i64::try_from(total).unwrap());
    let owner = Plan::checked(&records, limits, 0).unwrap().unwrap();
    assert_eq!(owner.request_work(), total);
}

#[test]
fn route_limited_and_subject_capacity_are_distinct_without_cross_route_sharing() {
    let owner = plan(&[protected(1), protected(1)]);
    let first = owner.binding(0).unwrap();
    let second = owner.binding(1).unwrap();
    assert_eq!(at(&owner, first, None), RateDecision::Charged);
    assert!(matches!(
        at(&owner, first, None),
        RateDecision::Limited { .. }
    ));
    assert_eq!(at(&owner, first, Some(7)), RateDecision::Charged);
    assert!(matches!(
        at(&owner, first, Some(7)),
        RateDecision::Limited { .. }
    ));
    assert_eq!(at(&owner, first, Some(8)), RateDecision::Capacity);
    assert_eq!(at(&owner, second, Some(7)), RateDecision::Charged);
    assert!(owner.binding(2).is_none());
}

#[test]
fn existing_branded_rule_ids_reject_other_owner_without_a_second_brand_system() {
    let first = plan(&[public()]);
    let second = plan(&[public()]);
    assert_eq!(
        second.charge_route(first.binding(0).unwrap()),
        RateDecision::Fault
    );
}

#[test]
fn closing_uses_the_existing_owner_for_both_charge_stages() {
    let owner = plan(&[protected(1)]);
    let binding = owner.binding(0).unwrap();
    owner.close();
    assert_eq!(owner.charge_route(binding), RateDecision::Closing);
    assert_eq!(owner.charge_subject(binding, 7), RateDecision::Closing);
}

#[test]
fn empty_catalog_creates_no_rate_owner_and_no_implicit_rule() {
    let limits = Capacity::checked(1, 1, 1, 1, 1).unwrap();
    let (absent, measured) =
        crate::metrics::measure(|| Plan::checked(&[], limits, 0).unwrap().is_none());
    assert!(absent);
    assert_eq!(measured.allocations, 0);
}

#[test]
fn caller_catalog_storage_is_charged_and_overflow_refused() {
    let records = [public()];
    assert!(Plan::checked(&records, capacity(1_000_000, 10_000), usize::MAX).is_err());
    let first = Plan::checked(&records, capacity(1_000_000, 10_000), 0)
        .unwrap()
        .unwrap();
    let second = Plan::checked(&records, capacity(1_000_000, 10_000), 4096)
        .unwrap()
        .unwrap();
    assert_eq!(second.retained_bytes(), first.retained_bytes() + 4096);
}

#[test]
fn finite_8192_subject_slots_are_not_rejected_by_a_hidden_4096_cap() {
    let owner = plan(&[protected(8192)]);
    assert!(owner.retained_bytes() <= 4 * 1024 * 1024);
    assert_eq!(
        at(&owner, owner.binding(0).unwrap(), Some(7)),
        RateDecision::Charged
    );
}
