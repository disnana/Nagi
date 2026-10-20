//! Test-only, fixed-size observations. This cannot change a clock, gate or timer.
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    OnceLock,
};
use tokio::sync::Notify;

#[derive(Clone, Copy, Debug)]
pub(super) struct Armed {
    pub(super) end: tokio::time::Instant,
    pub(super) status: u16,
    pub(super) body_bytes: usize,
}
#[derive(Debug, Default)]
pub(super) struct Probe {
    pub(super) first: OnceLock<Armed>,
    pub(super) pending: AtomicBool,
    pub(super) expired: AtomicBool,
    pub(super) graceful_before_end: AtomicBool,
    pub(super) listener_closed: AtomicBool,
    pub(super) joins: AtomicUsize,
    changed: Notify,
}
impl Probe {
    pub(super) fn arm(&self, end: tokio::time::Instant, status: u16, body_bytes: usize) {
        if self
            .first
            .set(Armed {
                end,
                status,
                body_bytes,
            })
            .is_ok()
        {
            self.changed.notify_waiters();
        }
    }
    fn first_end(&self, end: Option<tokio::time::Instant>) -> bool {
        self.first.get().is_some_and(|first| Some(first.end) == end)
    }
    pub(super) fn pending(&self, end: Option<tokio::time::Instant>) {
        if self.first_end(end) {
            self.pending.store(true, Ordering::SeqCst);
            self.changed.notify_waiters();
        }
    }
    pub(super) fn expired(&self, end: Option<tokio::time::Instant>) {
        if self.first_end(end) {
            self.expired.store(true, Ordering::SeqCst);
            self.changed.notify_waiters();
        }
    }
    pub(super) fn graceful(&self, end: Option<tokio::time::Instant>) {
        if self.first_end(end) && end.is_some_and(|end| tokio::time::Instant::now() < end) {
            self.graceful_before_end.store(true, Ordering::SeqCst);
        }
    }
    pub(super) fn listener_closed(&self) {
        self.listener_closed.store(true, Ordering::SeqCst);
        self.changed.notify_waiters();
    }
    // Only the retained JoinSet collector calls this, after actual join.
    pub(super) fn joined(&self) {
        self.joins.fetch_add(1, Ordering::SeqCst);
        self.changed.notify_waiters();
    }
    pub(super) async fn wait(&self, ready: impl Fn(&Self) -> bool) {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if ready(self) {
                return;
            }
            changed.await;
        }
    }
}
