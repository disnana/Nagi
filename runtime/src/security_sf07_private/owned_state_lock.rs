//! Private exclusive state lock with an inline owner representation.
//!
//! The inspected Rust 1.88/current Linux/Windows/macOS backends retain no
//! separate per-lock heap; this is not a promise about all future toolchains.
//! Contended thread parking is shared thread/runtime state, not owner storage.
//! Native first-lock and toolchain-update gates must keep checking this boundary.
use std::sync::{LockResult, RwLock, RwLockWriteGuard};

pub(crate) struct OwnedStateLock<T> {
    inner: RwLock<T>,
}

impl<T> OwnedStateLock<T> {
    // The existing private Quota constructors and other owners are test-only
    // until public admission is attached; promote this with those constructors.
    #[cfg(test)]
    pub(crate) const fn new(value: T) -> Self {
        Self {
            inner: RwLock::new(value),
        }
    }

    // All callers acquire exclusive writers, retaining standard writer poison.
    // No reader/upgrade path or independent fault policy is exposed.
    pub(crate) fn lock(&self) -> LockResult<RwLockWriteGuard<'_, T>> {
        self.inner.write()
    }
}

#[cfg(test)]
#[path = "owned_state_lock_tests.rs"]
mod tests;
