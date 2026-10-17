//! Direct existing-owner oracles, including their first lock and final owner.
use super::{indexed, quota};
use std::{
    mem::size_of,
    sync::{atomic::AtomicU64, Arc},
};

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn quota_cold_receipt_matches_allocated_payload_and_claim_keeps_final_owner() {
    assert_send_sync::<quota::Quota>();
    assert_send_sync::<quota::Reservation>();
    super::calibrate_allocation_meter();
    let limits = [quota::ResourceLimit::new(1), quota::ResourceLimit::new(7)];
    let minimum = quota::Quota::retained_catalog_minimum(2).unwrap();
    let ((owner, receipt), create) = crate::metrics::measure(|| {
        quota::Quota::checked_catalog_retained(&limits, 2, minimum).unwrap()
    });
    assert_eq!(receipt.bytes(), minimum);
    // Vec + Arc only, including the constructor's cold capacity-check lock.
    assert_eq!(create.allocations, 2);
    assert_eq!(create.reallocations, 0);
    assert_eq!(create.allocated_bytes, receipt.bytes() as u64);
    assert_eq!(create.deallocations, 0);
    let id = owner.resource_id(0).unwrap();
    let held = quota::try_reserve(&owner, &[quota::Claim::new(id, 1)]).unwrap();
    let weak = Arc::downgrade(&owner);
    drop(owner);
    assert!(weak.upgrade().is_some());
    let ((), final_payload) = crate::metrics::measure(|| drop(held));
    assert!(weak.upgrade().is_none());
    assert_eq!(final_payload.allocations, 0);
    assert_eq!(final_payload.deallocations, 1); // resource Vec, at last strong owner
    let ((), final_control) = crate::metrics::measure(|| drop(weak));
    assert_eq!(final_control.allocations, 0);
    assert_eq!(final_control.deallocations, 1); // test Weak held the Arc control block
}

#[test]
fn rate_cold_receipt_contains_state_and_short_budget_refuses_before_allocation() {
    assert_send_sync::<indexed::RateOwner>();
    super::calibrate_allocation_meter();
    let rule = indexed::Rule::new(1, 1, indexed::Slots::NoSubject, 1);
    let work = rule.checked_layout().unwrap().work;
    let minimum = indexed::RateOwner::minimum_fixed_bytes(1).unwrap();
    let limits = indexed::Limits {
        count: 1,
        bytes: minimum,
        work,
    };
    let (owner, create) =
        crate::metrics::measure(|| indexed::RateOwner::checked_rate(&[rule], limits).unwrap());
    assert_eq!(owner.retained_bytes(), minimum);
    // The existing static brand credit and inline owner are not heap payload.
    let vector_payload = minimum - size_of::<indexed::RateOwner>() - size_of::<AtomicU64>();
    assert_eq!(create.allocations, 1);
    assert_eq!(create.reallocations, 0);
    assert_eq!(create.allocated_bytes, vector_payload as u64);
    assert_eq!(create.deallocations, 0);
    let ((), release) = crate::metrics::measure(|| drop(owner));
    assert_eq!(release.allocations, 0);
    assert_eq!(release.deallocations, 1);
    let (refused, short) = crate::metrics::measure(|| {
        indexed::RateOwner::checked_rate(
            &[rule],
            indexed::Limits {
                bytes: minimum - 1,
                ..limits
            },
        )
    });
    assert_eq!(refused.err(), Some(indexed::StartupError::Invalid));
    assert_eq!(short.allocations, 0);
    assert_eq!(short.allocated_bytes, 0);
}
