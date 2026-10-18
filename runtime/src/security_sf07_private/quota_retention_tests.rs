//! Real Vec receipt oracles; the original nine quota cases remain untouched.
use super::*;

fn independent_bytes(capacity: usize) -> usize {
    size_of::<Quota>() + 2 * size_of::<usize>() + capacity * size_of::<ResourceState>()
}

#[test]
fn receipt_covers_same_quota_mutex_and_actual_vector_capacity() {
    let minimum = independent_bytes(2);
    assert_eq!(Quota::retained_catalog_minimum(2), Ok(minimum));
    let (owner, receipt) = Quota::checked_catalog_retained(
        &[ResourceLimit::new(1), ResourceLimit::new(7)],
        2,
        minimum,
    )
    .unwrap();
    let state = owner.state.lock().unwrap();
    let actual = state.resources.capacity();
    assert_eq!(state.resources.len(), 2);
    assert_eq!(receipt.bytes(), independent_bytes(actual));
    assert_eq!(receipt.bytes(), minimum);
    drop(state);
    let id = owner.resource_id(0).unwrap();
    let held = try_reserve(&owner, &[Claim::new(id, 1)]).unwrap();
    assert!(matches!(
        try_reserve(&owner, &[Claim::new(id, 1)]),
        Err(QuotaError::Capacity)
    ));
    drop(held);
    assert!(try_reserve(&owner, &[Claim::new(id, 1)]).is_ok());
}

#[test]
fn short_catalog_budget_refused_before_existing_constructor_allocates() {
    let limits = [ResourceLimit::new(1), ResourceLimit::new(7)];
    let (accepted, positive) = crate::metrics::measure(|| {
        Quota::checked_catalog_retained(&limits, 2, independent_bytes(2))
    });
    assert!(accepted.is_ok());
    assert!(
        positive.allocations > 0,
        "catalog allocation meter positive control"
    );
    drop(accepted);
    let (result, meter) = crate::metrics::measure(|| {
        Quota::checked_catalog_retained(&limits, 2, independent_bytes(2) - 1)
    });
    assert_eq!(result.err(), Some(StartupError::Invalid));
    assert_eq!(meter.allocations, 0);
    assert_eq!(meter.allocated_bytes, 0);
}

#[test]
fn actual_resource_vector_excess_capacity_refused_before_publish() {
    let mut resources = Vec::new();
    resources.try_reserve_exact(3).unwrap();
    resources.push(ResourceState {
        capacity: 1,
        used: 0,
    });
    resources.push(ResourceState {
        capacity: 7,
        used: 0,
    });
    assert!(resources.capacity() > resources.len());
    let owner = Quota {
        state: Mutex::new(QuotaState {
            resources,
            faulted: false,
        }),
    };
    let (result, meter) = crate::metrics::measure(|| owner.catalog_retention(2, usize::MAX));
    assert_eq!(result, Err(StartupError::Allocation));
    assert_eq!(meter.allocations, 0);
}
