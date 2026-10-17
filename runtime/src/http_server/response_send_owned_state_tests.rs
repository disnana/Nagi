use super::super::response_budget::Limits;
use super::*;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::atomic::{AtomicUsize, Ordering},
};
use tokio::sync::Semaphore;

struct NativeDropProbe {
    ledger: Arc<SendLedger>,
    pool: Arc<Semaphore>,
    seen: Arc<AtomicUsize>,
}
impl Future for NativeDropProbe {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        Poll::Pending
    }
}
impl Drop for NativeDropProbe {
    fn drop(&mut self) {
        let state = match self.ledger.state.lock() {
            Ok(_) => panic!("controlled ledger poison must remain sticky"),
            Err(poisoned) => poisoned.into_inner(),
        };
        assert!(!state.closed);
        assert!(state.cached.is_some());
        assert_eq!(self.pool.available_permits(), 0);
        assert_eq!(
            self.ledger.ordinary.text("blocked").err(),
            Some(Diagnostic::Capacity)
        );
        self.seen.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn poisoned_ledger_disposal_keeps_native_cache_and_permit_release_order() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SendLedger>();
    let ordinary = Arc::new(
        Producer::checked_wire(Limits {
            body_bytes: 32,
            user_header_bytes: 128,
            user_header_count: 4,
            managed_header_bytes: 512,
            managed_header_count: 12,
            serialization_writes: 64,
            live_responses: 1,
            retained_bytes: 65536,
        })
        .unwrap(),
    );
    let fallback = Arc::new(
        FallbackLedger::checked(
            FallbackLimits {
                responses: 1,
                retained_bytes: 65536,
                managed_header_bytes: 512,
                managed_header_count: 12,
            },
            1,
        )
        .unwrap(),
    );
    crate::security_sf07_private::calibrate_allocation_meter();
    let (ledger, create) = crate::metrics::measure(|| {
        let ledger = SendLedger::new(Arc::clone(&ordinary), fallback);
        let state = ledger.state.lock().unwrap();
        assert!(!state.closed);
        assert!(state.cached.is_none());
        drop(state);
        ledger
    });
    let receipt = std::mem::size_of::<SendLedger>() + 2 * std::mem::size_of::<usize>();
    assert_eq!(create.allocations, 1);
    assert_eq!(create.reallocations, 0);
    assert_eq!(create.allocated_bytes, receipt as u64);
    assert_eq!(create.deallocations, 0);
    let wire = ordinary.text("").unwrap().into_wire().unwrap();
    drop(wire.transfer_native(&ledger, StatusCode::OK).unwrap());
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _state = ledger.state.lock().unwrap();
        panic!("controlled send ledger writer poison");
    }))
    .is_err());
    let cached = match ledger.state.lock() {
        Ok(_) => panic!("poison must remain observable"),
        Err(poisoned) => Arc::clone(poisoned.into_inner().cached.as_ref().unwrap()),
    };
    assert_eq!(ledger.publish(cached), Err(SendFault::Poisoned));
    let pool = Arc::new(Semaphore::new(1));
    let permit = Arc::clone(&pool).try_acquire_owned().unwrap();
    let seen = Arc::new(AtomicUsize::new(0));
    let driver = NativeDriver {
        native: Some(NativeDropProbe {
            ledger: Arc::clone(&ledger),
            pool: Arc::clone(&pool),
            seen: Arc::clone(&seen),
        }),
        _after_native: AfterNative {
            ledger: Arc::clone(&ledger),
        },
        _permit: permit,
    };
    let weak = Arc::downgrade(&ledger);
    drop(driver);
    assert_eq!(seen.load(Ordering::SeqCst), 1);
    let state = match ledger.state.lock() {
        Ok(_) => panic!("disposal must not clear poison"),
        Err(poisoned) => poisoned.into_inner(),
    };
    assert!(state.closed);
    assert!(state.cached.is_none());
    drop(state);
    assert_eq!(pool.available_permits(), 1);
    assert!(ordinary.text("released").is_ok());
    drop(ledger);
    assert!(weak.upgrade().is_none());
}
