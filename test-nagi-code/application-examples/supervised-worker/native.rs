//! Test-only connector: one panic, then a cancellable pending future.
use std::sync::atomic::{AtomicI64, Ordering};

static STARTS: AtomicI64 = AtomicI64::new(0);
static ACTIVE: AtomicI64 = AtomicI64::new(0);
static CLEANUPS: AtomicI64 = AtomicI64::new(0);

struct ConnectorGuard;

impl Drop for ConnectorGuard {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
        CLEANUPS.fetch_add(1, Ordering::SeqCst);
    }
}

pub async fn connector() -> Result<(), nagi_runtime::Error> {
    let attempt = STARTS.fetch_add(1, Ordering::SeqCst) + 1;
    ACTIVE.fetch_add(1, Ordering::SeqCst);
    let _guard = ConnectorGuard;
    if attempt == 1 {
        panic!("supervised-worker: controlled connector panic");
    }
    // Cancellation drops the future and its guard; no extra Tokio runtime.
    std::future::pending::<Result<(), nagi_runtime::Error>>().await
}

pub fn connector_starts() -> i64 {
    STARTS.load(Ordering::SeqCst)
}

pub fn connector_active() -> i64 {
    ACTIVE.load(Ordering::SeqCst)
}

pub fn connector_cleanups() -> i64 {
    CLEANUPS.load(Ordering::SeqCst)
}
