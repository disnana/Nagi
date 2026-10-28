// Fault injection and lifecycle observation only. No supervisor, restart loop,
// scheduling, extra Tokio task, custom runtime, or replacement mailbox.
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::time::Duration;
use tokio::sync::Notify;

struct Counts {
    attempts: AtomicUsize,
    active: AtomicUsize,
    maximum: AtomicUsize,
    dropped: AtomicUsize,
    siblings: AtomicUsize,
    sibling_active: AtomicUsize,
    sibling_dropped: AtomicUsize,
    contexts: AtomicUsize,
    factories: AtomicUsize,
    states: AtomicUsize,
    changed: Notify,
}
impl Counts {
    const fn new() -> Self {
        Self { attempts: AtomicUsize::new(0), active: AtomicUsize::new(0),
            maximum: AtomicUsize::new(0), dropped: AtomicUsize::new(0),
            siblings: AtomicUsize::new(0), sibling_active: AtomicUsize::new(0),
            sibling_dropped: AtomicUsize::new(0), contexts: AtomicUsize::new(0),
            factories: AtomicUsize::new(0), states: AtomicUsize::new(0), changed: Notify::const_new() }
    }
}
static COUNTS: [Counts; 7] = [const { Counts::new() }; 7];
fn counts(slot: i64) -> &'static Counts { &COUNTS[usize::try_from(slot).unwrap()] }
struct Attempt(&'static Counts);
impl Drop for Attempt {
    fn drop(&mut self) { self.0.active.fetch_sub(1, SeqCst); self.0.dropped.fetch_add(1, SeqCst); }
}
struct Sibling(&'static Counts);
impl Drop for Sibling {
    fn drop(&mut self) { self.0.sibling_active.fetch_sub(1, SeqCst); self.0.sibling_dropped.fetch_add(1, SeqCst); }
}
async fn sibling_entered(slot: i64) {
    let c = counts(slot);
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let changed = c.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if c.sibling_active.load(SeqCst) == 1 { break; }
            changed.await;
        }
    }).await.expect("real sibling was not polled");
}
pub async fn attempt(slot: i64, mode: i64) -> Result<(), nagi_runtime::Error> {
    sibling_entered(slot).await;
    let c = counts(slot);
    c.attempts.fetch_add(1, SeqCst);
    let active = c.active.fetch_add(1, SeqCst) + 1;
    c.maximum.fetch_max(active, SeqCst);
    let _guard = Attempt(c);
    if mode == 1 { panic!("owned public Supervisor controlled panic"); }
    if mode == 2 { Ok(()) } else { Err(nagi_runtime::Error::internal("owned repeated child failure")) }
}
pub async fn hold(slot: i64) -> Result<(), nagi_runtime::Error> {
    let c = counts(slot);
    c.siblings.fetch_add(1, SeqCst);
    assert_eq!(c.sibling_active.fetch_add(1, SeqCst), 0, "duplicate sibling task");
    let _guard = Sibling(c);
    c.changed.notify_waiters();
    std::future::pending().await
}
pub async fn parent_failure(slot: i64) -> Result<(), nagi_runtime::Error> {
    sibling_entered(slot).await;
    Err(nagi_runtime::Error::internal("owned parent scope cancellation"))
}
pub fn factory_started(slot: i64) { counts(slot).factories.fetch_add(1, SeqCst); }
pub fn context_dropped(slot: i64) { counts(slot).contexts.fetch_add(1, SeqCst); }
pub fn state_dropped(slot: i64) { counts(slot).states.fetch_add(1, SeqCst); }
pub fn check_clean(slot: i64, attempts: i64, siblings: i64, factories: i64, states: i64) {
    let c = counts(slot);
    assert_eq!(c.attempts.load(SeqCst), attempts as usize);
    assert_eq!(c.dropped.load(SeqCst), attempts as usize);
    assert_eq!(c.maximum.load(SeqCst), usize::from(attempts > 0));
    assert_eq!(c.active.load(SeqCst), 0);
    assert_eq!(c.siblings.load(SeqCst), siblings as usize);
    assert_eq!(c.sibling_dropped.load(SeqCst), siblings as usize);
    assert_eq!(c.sibling_active.load(SeqCst), 0);
    assert_eq!(c.contexts.load(SeqCst), 1);
    assert_eq!(c.factories.load(SeqCst), factories as usize);
    assert_eq!(c.states.load(SeqCst), states as usize);
}
