//! Private, bounded NoWait quota core for SF07.
//!
//! The production HTTP producer uses the private retained catalogue and atomic
//! reservation path. No public Nagi quota API is exposed.

use super::OwnedStateLock as Mutex;
use std::mem::size_of;
use std::sync::Arc;

const MAX_CLAIMS: usize = 8;
const MAX_CATALOG_ITEMS: usize = (u16::MAX as usize) + 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResourceId(u16);

#[derive(Clone, Copy, Debug)]
pub(crate) struct ResourceLimit {
    capacity: usize,
}

impl ResourceLimit {
    pub(crate) const fn new(capacity: usize) -> Self {
        Self { capacity }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Claim {
    resource: ResourceId,
    amount: usize,
}

impl Claim {
    pub(crate) const fn new(resource: ResourceId, amount: usize) -> Self {
        Self { resource, amount }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartupError {
    Invalid,
    Allocation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QuotaError {
    Invalid,
    Capacity,
    Fault,
}

struct ResourceState {
    capacity: usize,
    used: usize,
}

struct QuotaState {
    resources: Vec<ResourceState>,
    faulted: bool,
}

pub(crate) struct Quota {
    state: Mutex<QuotaState>,
}

#[derive(Clone, Copy)]
struct FixedClaims {
    items: [Claim; MAX_CLAIMS],
    len: usize,
}

pub(crate) struct Reservation {
    owner: Arc<Quota>,
    claims: FixedClaims,
}

impl Quota {
    pub(crate) fn checked_catalog(
        limits: &[ResourceLimit],
        max_catalog: usize,
    ) -> Result<Arc<Self>, StartupError> {
        if limits.is_empty()
            || max_catalog == 0
            || limits.len() > max_catalog
            || limits.len() > MAX_CATALOG_ITEMS
            || limits.iter().any(|limit| limit.capacity == 0)
        {
            return Err(StartupError::Invalid);
        }

        checked_catalog_bytes(limits.len(), size_of::<ResourceState>())?;

        let mut resources = Vec::new();
        resources
            .try_reserve_exact(limits.len())
            .map_err(|_| StartupError::Allocation)?;
        for limit in limits {
            resources.push(ResourceState {
                capacity: limit.capacity,
                used: 0,
            });
        }

        Ok(Arc::new(Self {
            state: Mutex::new(QuotaState {
                resources,
                faulted: false,
            }),
        }))
    }

    /// Resource IDs are minted only from this immutable startup catalog.
    pub(crate) fn resource_id(&self, index: usize) -> Option<ResourceId> {
        let state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.faulted = true;
                return None;
            }
        };
        if index < state.resources.len() && index <= u16::MAX as usize {
            Some(ResourceId(index as u16))
        } else {
            None
        }
    }
}

// SF07_U2_RETAINED_CATALOG_BEGIN
// One checked receipt for the existing Quota allocation. No second quota.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CatalogRetention {
    bytes: usize,
}
impl CatalogRetention {
    pub(crate) fn bytes(self) -> usize {
        self.bytes
    }
}
impl Quota {
    pub(crate) fn retained_catalog_minimum(count: usize) -> Result<usize, StartupError> {
        if count == 0 || count > MAX_CATALOG_ITEMS {
            return Err(StartupError::Invalid);
        }
        let payload = checked_catalog_bytes(count, size_of::<ResourceState>())?;
        size_of::<Self>()
            .checked_add(2 * size_of::<usize>()) // Arc control block
            .and_then(|fixed| fixed.checked_add(payload))
            .filter(|bytes| *bytes <= isize::MAX as usize)
            .ok_or(StartupError::Invalid)
    }
    pub(crate) fn checked_catalog_retained(
        limits: &[ResourceLimit],
        max_catalog: usize,
        retained_limit: usize,
    ) -> Result<(Arc<Self>, CatalogRetention), StartupError> {
        // Reject impossible retention before the existing constructor allocates.
        let minimum = Self::retained_catalog_minimum(limits.len())?;
        if minimum > retained_limit {
            return Err(StartupError::Invalid);
        }
        let owner = Self::checked_catalog(limits, max_catalog)?;
        // Observe actual Vec capacity before this owner can be published.
        let receipt = owner.catalog_retention(limits.len(), retained_limit)?;
        Ok((owner, receipt))
    }
    fn catalog_retention(
        &self,
        planned_count: usize,
        retained_limit: usize,
    ) -> Result<CatalogRetention, StartupError> {
        let state = self.state.lock().map_err(|_| StartupError::Invalid)?;
        if state.resources.len() != planned_count || planned_count == 0 {
            return Err(StartupError::Invalid);
        }
        let capacity = state.resources.capacity();
        // try_reserve_exact is not assumed to report exactly the request.
        if capacity > planned_count {
            return Err(StartupError::Allocation);
        }
        let bytes = Self::retained_catalog_minimum(capacity)?;
        if bytes > retained_limit {
            return Err(StartupError::Invalid);
        }
        Ok(CatalogRetention { bytes })
    }
}
#[cfg(test)]
#[path = "quota_retention_tests.rs"]
mod retained_catalog_tests;
// SF07_U2_RETAINED_CATALOG_END

fn checked_catalog_bytes(count: usize, item_size: usize) -> Result<usize, StartupError> {
    count
        .checked_mul(item_size)
        .filter(|bytes| *bytes <= isize::MAX as usize)
        .ok_or(StartupError::Invalid)
}

/// Atomically reserves all claims or changes no counter. There is no wait path.
pub(crate) fn try_reserve(owner: &Arc<Quota>, claims: &[Claim]) -> Result<Reservation, QuotaError> {
    if claims.is_empty() || claims.len() > MAX_CLAIMS {
        return Err(QuotaError::Invalid);
    }

    let mut fixed = FixedClaims {
        items: [Claim::new(ResourceId(0), 1); MAX_CLAIMS],
        len: claims.len(),
    };
    for (index, claim) in claims.iter().copied().enumerate() {
        if claim.amount == 0 {
            return Err(QuotaError::Invalid);
        }
        for prior in claims.iter().take(index) {
            if prior.resource == claim.resource {
                return Err(QuotaError::Invalid);
            }
        }
        fixed.items[index] = claim;
    }

    // Keep a stable resource-index order without allocating on the request path.
    for index in 1..fixed.len {
        let mut cursor = index;
        while cursor > 0 && fixed.items[cursor - 1].resource.0 > fixed.items[cursor].resource.0 {
            fixed.items.swap(cursor - 1, cursor);
            cursor -= 1;
        }
    }

    let mut state = match owner.state.lock() {
        Ok(state) => state,
        Err(poisoned) => {
            let mut state = poisoned.into_inner();
            state.faulted = true;
            return Err(QuotaError::Fault);
        }
    };
    if state.faulted {
        return Err(QuotaError::Fault);
    }

    for claim in fixed.items.iter().take(fixed.len) {
        let Some(resource) = state.resources.get(usize::from(claim.resource.0)) else {
            state.faulted = true;
            return Err(QuotaError::Fault);
        };
        let Some(new_used) = resource.used.checked_add(claim.amount) else {
            return Err(QuotaError::Capacity);
        };
        if new_used > resource.capacity {
            return Err(QuotaError::Capacity);
        }
    }

    for claim in fixed.items.iter().take(fixed.len) {
        let resource = &mut state.resources[usize::from(claim.resource.0)];
        resource.used += claim.amount;
    }

    Ok(Reservation {
        owner: Arc::clone(owner),
        claims: fixed,
    })
}

impl Drop for Reservation {
    fn drop(&mut self) {
        let mut state = match self.owner.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.faulted = true;
                state
            }
        };

        for claim in self.claims.items.iter().take(self.claims.len) {
            match state.resources.get_mut(usize::from(claim.resource.0)) {
                Some(resource) => match resource.used.checked_sub(claim.amount) {
                    Some(new_used) => resource.used = new_used,
                    None => state.faulted = true,
                },
                None => state.faulted = true,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        checked_catalog_bytes, try_reserve, Claim, Quota, QuotaError, ResourceId, ResourceLimit,
        StartupError,
    };
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::{Arc, Barrier};

    #[test]
    fn q01_two_resource_partial_full_is_atomic() {
        let owner = Quota::checked_catalog(&[ResourceLimit::new(1), ResourceLimit::new(4)], 2)
            .expect("valid fixed catalog");
        let count = owner.resource_id(0).expect("count id");
        let bytes = owner.resource_id(1).expect("byte id");

        let oversized = [Claim::new(count, 1), Claim::new(bytes, 5)];
        assert!(matches!(
            try_reserve(&owner, &oversized),
            Err(QuotaError::Capacity)
        ));

        let exact = [Claim::new(count, 1), Claim::new(bytes, 4)];
        let reservation = try_reserve(&owner, &exact).expect("failed claim changed no counter");
        assert!(matches!(
            try_reserve(&owner, &[Claim::new(count, 1)]),
            Err(QuotaError::Capacity)
        ));
        drop(reservation);
        let after_release = [Claim::new(count, 1), Claim::new(bytes, 4)];
        assert!(try_reserve(&owner, &after_release).is_ok());
    }

    #[test]
    fn startup_rejects_zero_and_empty_catalog() {
        assert_eq!(
            Quota::checked_catalog(&[ResourceLimit::new(0)], 1).err(),
            Some(StartupError::Invalid)
        );
        assert_eq!(
            Quota::checked_catalog(&[], 1).err(),
            Some(StartupError::Invalid)
        );
        assert_eq!(
            Quota::checked_catalog(&[ResourceLimit::new(1)], 0).err(),
            Some(StartupError::Invalid)
        );
    }

    #[test]
    fn q03_catalog_byte_product_rejects_overflow_and_isize_boundary() {
        assert_eq!(
            checked_catalog_bytes(usize::MAX, 2),
            Err(StartupError::Invalid)
        );
        assert_eq!(
            checked_catalog_bytes((isize::MAX as usize) + 1, 1),
            Err(StartupError::Invalid)
        );
        assert_eq!(checked_catalog_bytes(3, 4), Ok(12));
    }

    #[test]
    fn q04_duplicate_is_invalid_but_bad_resource_id_faults_closed() {
        let owner =
            Quota::checked_catalog(&[ResourceLimit::new(2)], 1).expect("valid fixed catalog");
        let id = owner.resource_id(0).expect("catalog resource ID");
        assert_eq!(
            try_reserve(&owner, &[Claim::new(id, 1), Claim::new(id, 1)]).err(),
            Some(QuotaError::Invalid)
        );
        {
            let state = owner.state.lock().expect("unpoisoned test state");
            assert_eq!(state.resources[0].used, 0);
            assert!(!state.faulted);
        }
        let supported = try_reserve(&owner, &[Claim::new(id, 1)])
            .expect("duplicate rejection leaves canonical claim usable");
        drop(supported);

        let invalid_owner =
            Quota::checked_catalog(&[ResourceLimit::new(2)], 1).expect("independent fixed catalog");
        let valid_id = invalid_owner.resource_id(0).expect("catalog resource ID");
        let invalid_id = ResourceId(1); // catalog.len(), forged only within this private unit test
        assert_eq!(
            try_reserve(
                &invalid_owner,
                &[Claim::new(valid_id, 1), Claim::new(invalid_id, 1)]
            )
            .err(),
            Some(QuotaError::Fault)
        );
        {
            let state = invalid_owner.state.lock().expect("mutex remains usable");
            assert_eq!(state.resources[0].used, 0);
            assert!(state.faulted);
        }
        assert_eq!(
            try_reserve(&invalid_owner, &[Claim::new(valid_id, 1)]).err(),
            Some(QuotaError::Fault)
        );
    }

    #[test]
    fn claim_sum_overflow_is_capacity_without_counter_mutation() {
        let owner = Quota::checked_catalog(&[ResourceLimit::new(usize::MAX)], 1)
            .expect("valid maximum-capacity resource");
        let id = owner.resource_id(0).expect("catalog resource ID");
        let first =
            try_reserve(&owner, &[Claim::new(id, usize::MAX - 1)]).expect("large in-range claim");
        assert_eq!(
            try_reserve(&owner, &[Claim::new(id, 2)]).err(),
            Some(QuotaError::Capacity)
        );
        assert_eq!(
            owner.state.lock().expect("unpoisoned").resources[0].used,
            usize::MAX - 1
        );
        drop(first);
        let maximum = try_reserve(&owner, &[Claim::new(id, usize::MAX)])
            .expect("Drop restores full representable capacity");
        drop(maximum);
    }

    #[test]
    fn q05_exact_count_and_bytes_boundary_releases_one_claim() {
        let owner = Quota::checked_catalog(&[ResourceLimit::new(2), ResourceLimit::new(4)], 2)
            .expect("valid two-resource catalog");
        let count = owner.resource_id(0).expect("count ID");
        let bytes = owner.resource_id(1).expect("bytes ID");
        let first = try_reserve(&owner, &[Claim::new(count, 1), Claim::new(bytes, 2)])
            .expect("first exact partial claim");
        let second = try_reserve(&owner, &[Claim::new(count, 1), Claim::new(bytes, 2)])
            .expect("second exact partial claim");
        assert_eq!(
            try_reserve(&owner, &[Claim::new(count, 1), Claim::new(bytes, 1)]).err(),
            Some(QuotaError::Capacity)
        );
        {
            let state = owner.state.lock().expect("unpoisoned test state");
            assert_eq!(state.resources[0].used, 2);
            assert_eq!(state.resources[1].used, 4);
        }
        drop(first);
        let replacement = try_reserve(&owner, &[Claim::new(count, 1), Claim::new(bytes, 2)])
            .expect("one drop returns exactly one claim");
        assert_eq!(
            try_reserve(&owner, &[Claim::new(count, 1)]).err(),
            Some(QuotaError::Capacity)
        );
        drop((second, replacement));
    }

    #[test]
    fn q06_moved_reservation_releases_once_at_final_drop() {
        let owner =
            Quota::checked_catalog(&[ResourceLimit::new(1)], 1).expect("valid fixed catalog");
        let id = owner.resource_id(0).expect("catalog resource ID");
        let reservation = try_reserve(&owner, &[Claim::new(id, 1)]).expect("reserved");
        let moved_once = reservation;
        let moved_twice = moved_once;
        let moved_thrice = moved_twice;
        assert_eq!(
            try_reserve(&owner, &[Claim::new(id, 1)]).err(),
            Some(QuotaError::Capacity)
        );
        drop(moved_thrice);
        let after_release = try_reserve(&owner, &[Claim::new(id, 1)])
            .expect("one final Drop releases the sole reservation");
        drop(after_release);
    }

    #[test]
    fn q09_poisoned_disposal_releases_claim_and_keeps_fault_latched() {
        let owner =
            Quota::checked_catalog(&[ResourceLimit::new(1)], 1).expect("valid fixed catalog");
        let id = owner.resource_id(0).expect("catalog resource ID");
        let reservation = try_reserve(&owner, &[Claim::new(id, 1)]).expect("reserved");
        let poisoned_owner = Arc::clone(&owner);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _guard = poisoned_owner.state.lock().expect("initial lock");
            panic!("controlled poison for disposal oracle");
        }));
        assert!(panic.is_err());
        drop(reservation);
        let state = match owner.state.lock() {
            Ok(_) => panic!("mutex poison must remain sticky"),
            Err(poisoned) => poisoned.into_inner(),
        };
        assert!(state.faulted);
        assert_eq!(state.resources[0].used, 0);
        drop(state);
        assert_eq!(
            try_reserve(&owner, &[Claim::new(id, 1)]).err(),
            Some(QuotaError::Fault)
        );
    }

    #[test]
    fn q13_two_simultaneous_claims_have_one_winner() {
        let owner =
            Quota::checked_catalog(&[ResourceLimit::new(1)], 1).expect("valid fixed catalog");
        let id = owner.resource_id(0).expect("catalog resource ID");
        let barrier = Arc::new(Barrier::new(3));
        let threads = (0..2)
            .map(|_| {
                let owner = Arc::clone(&owner);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    try_reserve(&owner, &[Claim::new(id, 1)])
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();

        let mut winner = None;
        let mut capacity_results = 0;
        for thread in threads {
            match thread.join().expect("claim worker completed") {
                Ok(reservation) => {
                    assert!(winner.replace(reservation).is_none(), "only one winner");
                }
                Err(QuotaError::Capacity) => capacity_results += 1,
                Err(error) => panic!("unexpected concurrent quota error: {error:?}"),
            }
        }
        assert_eq!(capacity_results, 1);
        let reservation = winner.expect("one claim accepted");
        assert_eq!(owner.state.lock().expect("unpoisoned").resources[0].used, 1);
        drop(reservation);
        let after_drop = try_reserve(&owner, &[Claim::new(id, 1)])
            .expect("winner drop restores the single slot");
        drop(after_drop);
    }
}
