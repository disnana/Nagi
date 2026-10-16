use super::OwnedStateLock;
use std::{
    mem::size_of,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Barrier,
    },
};

struct Payload<'a> {
    value: usize,
    drops: &'a AtomicUsize,
}
impl Drop for Payload<'_> {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn cold_first_lock_unlock_and_drop_have_no_owned_heap() {
    crate::security_sf07_private::calibrate_allocation_meter();
    let drops = AtomicUsize::new(0);
    let ((), meter) = crate::metrics::measure(|| {
        let owner = OwnedStateLock::new(Payload {
            value: 0,
            drops: &drops,
        });
        let mut state = owner.lock().unwrap();
        state.value = 7;
        drop(state);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(owner);
    });
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(meter.allocations, 0);
    assert_eq!(meter.reallocations, 0);
    assert_eq!(meter.allocated_bytes, 0);
    assert_eq!(meter.deallocations, 0);
}

#[test]
fn arc_receipt_covers_cold_lock_and_last_owner_releases_payload_once() {
    crate::security_sf07_private::calibrate_allocation_meter();
    let drops = AtomicUsize::new(0);
    let (owner, create) = crate::metrics::measure(|| {
        let owner = Arc::new(OwnedStateLock::new(Payload {
            value: 0,
            drops: &drops,
        }));
        drop(owner.lock().unwrap());
        owner
    });
    let receipt = size_of::<OwnedStateLock<Payload<'_>>>() + 2 * size_of::<usize>();
    assert_eq!(create.allocations, 1);
    assert_eq!(create.reallocations, 0);
    assert_eq!(create.allocated_bytes, receipt as u64);
    assert_eq!(create.deallocations, 0);
    let last = Arc::clone(&owner);
    drop(owner);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let ((), release) = crate::metrics::measure(|| drop(last));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(release.allocations, 0);
    assert_eq!(release.deallocations, 1);
}

#[test]
fn fresh_concurrent_writers_keep_all_updates_and_one_final_payload() {
    const WORKERS: usize = 4;
    const UPDATES: usize = 128;
    let drops = AtomicUsize::new(0);
    let owner = Arc::new(OwnedStateLock::new(Payload {
        value: 0,
        drops: &drops,
    }));
    let start = Barrier::new(WORKERS + 1);
    std::thread::scope(|scope| {
        for _ in 0..WORKERS {
            let owner = Arc::clone(&owner);
            let start = &start;
            scope.spawn(move || {
                start.wait();
                // No prior lock/warmup: the first writes race on a fresh owner.
                for _ in 0..UPDATES {
                    let mut state = owner.lock().unwrap();
                    let before = state.value;
                    std::thread::yield_now();
                    state.value = before + 1;
                }
            });
        }
        start.wait();
    });
    assert_eq!(owner.lock().unwrap().value, WORKERS * UPDATES);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(owner);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    // Thread/Parker initialization is deliberately not a zero-allocation claim.
}

#[test]
fn writer_poison_recovers_for_disposal_and_stays_sticky_until_drop() {
    let drops = AtomicUsize::new(0);
    let owner = OwnedStateLock::new(Payload {
        value: 0,
        drops: &drops,
    });
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let mut state = owner.lock().unwrap();
        state.value = 7;
        panic!("controlled owned writer poison");
    }))
    .is_err());
    let mut state = match owner.lock() {
        Ok(_) => panic!("writer poison must remain observable"),
        Err(poisoned) => poisoned.into_inner(),
    };
    assert_eq!(state.value, 7);
    state.value = 8;
    drop(state);
    assert!(owner.lock().is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(owner);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[cfg(target_os = "macos")]
#[test]
fn cold_std_mutex_control_retains_native_heap_until_owner_drop() {
    crate::security_sf07_private::calibrate_allocation_meter();
    let (owner, first) = crate::metrics::measure(|| {
        let owner = std::sync::Mutex::new(0usize);
        *owner.lock().unwrap() = 7;
        owner
    });
    // Source-bound control for the inspected 1.88/current macOS backend.
    // This is observed payload, never a private ABI constant used in receipts.
    assert!(first.allocations > 0);
    assert!(first.allocated_bytes > 0);
    assert_eq!(first.deallocations, 0);
    let ((), release) = crate::metrics::measure(|| drop(owner));
    assert_eq!(release.allocations, 0);
    assert_eq!(release.deallocations, first.allocations);
}
