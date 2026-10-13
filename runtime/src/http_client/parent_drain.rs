//! Finite trusted-parent registrations retain monitor and inner joins independently.
//! A cancelled drain borrows the same parent record. Sync Drop requests stop only.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ParentError {
    InvalidCapacity,
    Capacity,
    Closing,
    NoRuntime,
    Allocation,
    Ledger(LedgerError),
    UnknownClient,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParentFault {
    MonitorPanic,
    UnexpectedMonitorCancel,
    Inner(Fault),
}
pub(super) struct DrainRecord {
    pub(super) id: u64,
    pub(super) owner: Arc<Mutex<Owner>>,
    shared: Arc<Shared>,
    pub(super) monitor: Option<JoinHandle<Option<Fault>>>,
    monitor_cancel_requested: bool,
    monitor_fault: Option<ParentFault>,
}
pub(crate) struct ParentDrain {
    pub(super) records: Vec<DrainRecord>,
    capacity: usize,
    next_id: u64,
    closing: bool,
    first_fault: Option<ParentFault>,
}

// Runner authority is the admitted JobKey, not this non-counted owner handle.
// Holding a runner cannot keep the last ClientSender EOF open.
pub(super) struct RunnerOwnerHandle {
    pub(super) owner: Arc<Mutex<Owner>>,
}

fn lock_owner(owner: &Mutex<Owner>) -> std::sync::MutexGuard<'_, Owner> {
    match owner.lock() {
        Ok(guard) => guard,
        Err(poison) => {
            let mut guard = poison.into_inner();
            guard.latch_fault(Fault::PoisonedGate);
            guard.close();
            guard
        }
    }
}

async fn monitor(
    owner: Arc<Mutex<Owner>>,
    shared: Arc<Shared>,
    mut events: watch::Receiver<bool>,
) -> Option<Fault> {
    loop {
        let stopped = shared.stop_notify.notified();
        tokio::pin!(stopped);
        stopped.as_mut().enable();
        #[cfg(test)]
        if lock_owner(&owner)
            .shared
            .monitor_panic
            .swap(false, Ordering::AcqRel)
        {
            panic!("owned monitor fixture panic");
        }
        let end = lock_owner(&owner).next_end();
        tokio::select! {
            _ = std::future::poll_fn(|cx| lock_owner(&owner).poll_parent(cx)) => {
                return lock_owner(&owner).fault();
            }
            _ = stopped => {}
            changed = events.changed() => {
                if changed.is_err() { lock_owner(&owner).close(); }
            }
            _ = async {
                if let Some(end) = end {
                    tokio::time::sleep_until(tokio::time::Instant::from_std(end)).await;
                } else { std::future::pending::<()>().await; }
            } => {}
        }
    }
}

impl ParentDrain {
    pub(crate) fn new(capacity: usize) -> Result<Self, ParentError> {
        if !(1..=MAX_SLOTS).contains(&capacity) {
            return Err(ParentError::InvalidCapacity);
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(capacity)
            .map_err(|_| ParentError::Allocation)?;
        Ok(Self {
            records,
            capacity,
            next_id: 1,
            closing: false,
            first_fault: None,
        })
    }
    pub(crate) fn register(
        &mut self,
        limits: Limits,
        job_bytes: usize,
    ) -> Result<(u64, ClientSender), ParentError> {
        if self.closing {
            return Err(ParentError::Closing);
        }
        if self.records.len() >= self.capacity {
            return Err(ParentError::Capacity);
        }
        if !(1..=MAX_WAITER_BYTES).contains(&job_bytes) {
            return Err(ParentError::InvalidCapacity);
        }
        let next = self.next_id.checked_add(1).ok_or(ParentError::Capacity)?;
        let runtime = tokio::runtime::Handle::try_current().map_err(|_| ParentError::NoRuntime)?;
        let (mut owner, mut sender) = Owner::checked(limits).map_err(ParentError::Ledger)?;
        owner.job_byte_limit = job_bytes;
        let events = owner.shared.events_tx.subscribe();
        let retained = Arc::new(Mutex::new(owner));
        sender.owner = Some(Arc::clone(&retained));
        let id = self.next_id;
        // Parent registration exists before any monitor/native task is spawned.
        self.records.push(DrainRecord {
            id,
            owner: Arc::clone(&retained),
            shared: Arc::clone(&sender.shared),
            monitor: None,
            monitor_cancel_requested: false,
            monitor_fault: None,
        });
        self.next_id = next;
        let task = runtime.spawn(monitor(retained, Arc::clone(&sender.shared), events));
        self.records.last_mut().expect("registered record").monitor = Some(task);
        Ok((id, sender))
    }
    pub(super) fn runner_owner(&self, id: u64) -> Result<RunnerOwnerHandle, ParentError> {
        let index = self.index(id)?;
        Ok(RunnerOwnerHandle {
            owner: Arc::clone(&self.records[index].owner),
        })
    }
    fn index(&self, id: u64) -> Result<usize, ParentError> {
        self.records
            .iter()
            .position(|record| record.id == id)
            .ok_or(ParentError::UnknownClient)
    }
    pub(crate) fn request_close(&mut self, id: u64) -> Result<(), ParentError> {
        let index = self.index(id)?;
        lock_owner(&self.records[index].owner).close();
        Ok(())
    }
    fn request_close_all(&mut self) {
        self.closing = true;
        for record in &self.records {
            lock_owner(&record.owner).close();
        }
    }
    fn observe_outer(
        &mut self,
        index: usize,
        result: Result<Option<Fault>, tokio::task::JoinError>,
    ) {
        self.records[index].monitor = None;
        let fault = match result {
            Ok(Some(fault)) => Some(ParentFault::Inner(fault)),
            Ok(None) => None,
            Err(error) if error.is_panic() => Some(ParentFault::MonitorPanic),
            Err(_) if !self.records[index].monitor_cancel_requested => {
                Some(ParentFault::UnexpectedMonitorCancel)
            }
            Err(_) => None,
        };
        self.records[index].monitor_fault = fault;
        if let Some(fault) = fault {
            self.first_fault.get_or_insert(fault);
            self.request_close_all();
        }
    }
    async fn finish_record(&mut self, index: usize) {
        // The monitor may already be gone while a caller still owns a popped
        // Command. Subscribe before polling so its Drop cannot strand fallback.
        let owner = Arc::clone(&self.records[index].owner);
        let shared = Arc::clone(&self.records[index].shared);
        lock_owner(&owner).close();
        loop {
            let stopped = shared.stop_notify.notified();
            tokio::pin!(stopped);
            stopped.as_mut().enable();
            tokio::select! {
                _ = std::future::poll_fn(|cx| lock_owner(&owner).poll_parent(cx)) => break,
                _ = stopped => {}
            }
        }
        if let Some(fault) = lock_owner(&self.records[index].owner).fault() {
            self.first_fault.get_or_insert(ParentFault::Inner(fault));
            self.closing = true;
        }
        if self.closing {
            self.request_close_all();
        }
        self.records.swap_remove(index); // both actual join layers have completed
    }
    pub(crate) async fn drain_client(&mut self, id: u64) -> Result<(), ParentError> {
        let index = self.index(id)?;
        if let Some(handle) = &mut self.records[index].monitor {
            let result = handle.await;
            self.observe_outer(index, result);
        }
        self.finish_record(index).await;
        Ok(())
    }
    // A trusted parent drives this bounded event loop. It scans all monitor
    // joins rather than awaiting the oldest healthy client and hiding a fault.
    pub(crate) async fn reap_next(&mut self) -> Option<u64> {
        if self.records.is_empty() {
            return None;
        }
        let joined = std::future::poll_fn(|cx| {
            for (index, record) in self.records.iter_mut().enumerate() {
                if let Some(handle) = &mut record.monitor {
                    if let Poll::Ready(result) = Pin::new(handle).poll(cx) {
                        return Poll::Ready((index, result));
                    }
                } else {
                    return Poll::Ready((index, Ok(None)));
                }
            }
            Poll::Pending
        })
        .await;
        let id = self.records[joined.0].id;
        // A prior cancelled drain may already have observed this monitor join.
        if self.records[joined.0].monitor.is_some() {
            self.observe_outer(joined.0, joined.1);
        }
        self.finish_record(joined.0).await;
        Some(id)
    }
    pub(crate) async fn shutdown(&mut self) {
        self.request_close_all();
        while self.reap_next().await.is_some() {}
    }
}
impl Drop for ParentDrain {
    fn drop(&mut self) {
        for record in &self.records {
            record.shared.request_stop();
        }
        // This only posts stop. A trusted caller must await shutdown for an
        // actual monitor join; detached records cannot prove publication.
    }
}

#[cfg(test)]
impl ParentDrain {
    pub(super) fn monitor_panic_for_test(&self) -> bool {
        self.first_fault == Some(ParentFault::MonitorPanic)
    }
    pub(super) fn closing_for_test(&self) -> bool {
        self.closing && self.first_fault.is_some()
    }
}

#[cfg(test)]
mod fallback_lost_wake_tests {
    use super::*;
    use crate::auth;
    use std::{future::Future, time::Duration};

    struct WakeCounter(AtomicUsize);
    impl std::task::Wake for WakeCounter {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn limits() -> Limits {
        Limits {
            jobs: 1,
            queue: 1,
            job_roots: 1,
            dns_roots: 1,
            dns_children: 1,
            http_drivers: 1,
            waiters: 1,
            waiter_bytes: 16,
            senders: 3,
        }
    }

    async fn paused_external_child(mode: u8, closed: tokio::runtime::Handle) {
        let mut parent = ParentDrain::new(1).unwrap();
        let (id, sender) = parent.register(limits(), 16).unwrap();
        let end = std::time::Instant::now() + Duration::from_secs(10);
        let lease = auth::LeaseOwner::new(end).unwrap();
        let identity = auth::VerifiedIdentity::from_verified(7, end).unwrap();
        let scope = auth::AuthScope::bind(identity, Arc::clone(&lease.lease)).unwrap();
        let grant: auth::Grant<()> = auth::Grant::from_authorized(scope, 91).unwrap();
        sender.submit(grant, vec![1, 2], end).await.unwrap();
        let runner = parent.runner_owner(id).unwrap();
        let mut command = runner.take_command().unwrap().unwrap();
        let key = command.key().clone();
        assert!(runner
            .spawn_root(&mut command, std::future::pending::<()>())
            .is_ok());
        drop(command);
        // The caller is external to every retained root and owns no Command.
        let (delivery, waiting) = GateFreeFuture::new(std::future::pending::<()>());
        let receipt = Arc::clone(&delivery.receipt);
        let registered = {
            let _entered = closed.enter();
            let mut owner = lock_owner(&runner.owner);
            let shared = Arc::clone(&owner.shared);
            let _entry = shared.begin_enqueue().unwrap();
            owner
                .try_spawn_delivery(key.id(), TaskKind::HttpDriver, waiting, &delivery, true)
                .map_err(|(error, _)| error)
        };
        if mode == 1 {
            parent.records[0].monitor.as_ref().unwrap().abort();
            let result = parent.records[0].monitor.as_mut().unwrap().await;
            parent.observe_outer(0, result);
        } else if mode == 2 {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = runner.owner.lock().unwrap();
                panic!("paused delivery gate poison fixture");
            }));
        }
        parent.request_close(id).unwrap();
        // Let the actual root cancellation finish. The child receipt still
        // prevents monitor completion and parent record retirement.
        while !lock_owner(&runner.owner).jobs[0]
            .tasks
            .iter()
            .find(|task| task.kind == TaskKind::JobRoot)
            .is_none_or(|task| task.handle.as_ref().is_none_or(JoinHandle::is_finished))
        {
            tokio::task::yield_now().await;
        }
        let wake = Arc::new(WakeCounter(AtomicUsize::new(0)));
        let waker = Waker::from(Arc::clone(&wake));
        let mut drain = Box::pin(parent.shutdown());
        let pending = drain
            .as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending();
        drop(drain); // cancellation leaves both retained layers intact
        let before = {
            let owner = lock_owner(&runner.owner);
            (
                owner.used_jobs(),
                owner.used_tasks(TaskKind::HttpDriver),
                owner.jobs[0]
                    .tasks
                    .iter()
                    .any(|task| task.kind == TaskKind::HttpDriver && task.handle.is_some()),
                key.seal.completion.load(Ordering::Acquire) & DRIVER_ACTUAL_JOINED,
                receipt.is_settled(),
            )
        };
        let retained = parent.records.len();
        let mut resumed = Box::pin(parent.shutdown());
        let resumed_pending = resumed
            .as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending();
        // Displace the per-receipt waker: fallback must be woken independently
        // by stop_notify, and the running monitor by its shared watch.
        let other = Arc::new(WakeCounter(AtomicUsize::new(0)));
        let other_waker = Waker::from(Arc::clone(&other));
        let displaced = !receipt.ready(&Context::from_waker(&other_waker));
        wake.0.store(0, Ordering::SeqCst);
        let delivered = delivery.finish(registered).is_ok();
        let fallback_wakes = wake.0.load(Ordering::SeqCst);
        resumed.await;
        drop(sender);
        assert!(pending && resumed_pending && displaced && delivered);
        assert_eq!(retained, 1);
        assert_eq!(before, (1, 1, true, 0, false));
        if mode == 1 {
            assert!(
                fallback_wakes >= 1,
                "fallback must wake before re-poll after receipt waker replacement"
            );
        }
        assert!(parent.records.is_empty());
        assert_eq!(lock_owner(&runner.owner).used_jobs(), 0);
        assert_ne!(
            key.seal.completion.load(Ordering::Acquire) & DRIVER_ACTUAL_JOINED,
            0
        );
        if mode == 2 {
            assert_eq!(lock_owner(&runner.owner).fault(), Some(Fault::PoisonedGate));
        }
    }

    fn paused_external_child_case(mode: u8) {
        let dead = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let closed = dead.handle().clone();
        drop(dead);
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(paused_external_child(mode, closed));
    }
    #[test]
    fn sf06_monitor_retains_external_child_paused_delivery() {
        paused_external_child_case(0);
    }
    #[test]
    fn sf06_fallback_retains_external_child_and_wakes_after_cancelled_waiter() {
        paused_external_child_case(1);
    }
    #[test]
    fn sf06_poisoned_owner_retains_external_child_until_delivery() {
        paused_external_child_case(2);
    }

    async fn fallback_after_outer_join(monitor_panics: bool) {
        let mut parent = ParentDrain::new(1).unwrap();
        let (id, sender) = parent.register(limits(), 16).unwrap();
        let end = std::time::Instant::now() + Duration::from_secs(10);
        let lease = auth::LeaseOwner::new(end).unwrap();
        let identity = auth::VerifiedIdentity::from_verified(7, end).unwrap();
        let scope = auth::AuthScope::bind(identity, Arc::clone(&lease.lease)).unwrap();
        let grant: auth::Grant<()> = auth::Grant::from_authorized(scope, 91).unwrap();
        sender.submit(grant, vec![1, 2], end).await.unwrap();
        let runner = parent.runner_owner(id).unwrap();
        let command = runner.take_command().unwrap().unwrap();
        let retained = Arc::clone(&parent.records[0].owner);
        assert_eq!(lock_owner(&retained).used_jobs(), 1);
        if monitor_panics {
            let owner = lock_owner(&retained);
            owner.shared.monitor_panic.store(true, Ordering::Release);
            owner.shared.notify();
        } else {
            parent.records[0].monitor.as_ref().unwrap().abort();
        }
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            parent.records[0].monitor.as_mut().unwrap(),
        )
        .await
        .unwrap();
        parent.observe_outer(0, result);
        let expected = if monitor_panics {
            ParentFault::MonitorPanic
        } else {
            ParentFault::UnexpectedMonitorCancel
        };
        assert_eq!(parent.first_fault, Some(expected));
        let mut drain = Box::pin(parent.reap_next());
        let wake = Arc::new(WakeCounter(AtomicUsize::new(0)));
        let waker = Waker::from(Arc::clone(&wake));
        let mut cx = Context::from_waker(&waker);
        let was_pending = drain.as_mut().poll(&mut cx).is_pending();
        let retained_before = lock_owner(&retained).used_jobs() == 1;
        wake.0.store(0, Ordering::SeqCst);
        std::thread::spawn(move || drop(command)).join().unwrap();
        // Check the synchronous Drop wake before any re-poll can mask its loss.
        let wake_count = wake.0.load(Ordering::SeqCst);
        if wake_count == 0 {
            drop(drain);
            tokio::time::timeout(Duration::from_secs(2), parent.shutdown())
                .await
                .unwrap();
            assert!(parent.records.is_empty());
            assert_eq!(lock_owner(&retained).used_jobs(), 0);
            assert_eq!(parent.first_fault, Some(expected));
            panic!("Command Drop did not wake pending fallback");
        }
        assert!(was_pending);
        assert!(retained_before);
        assert!(wake_count >= 1);
        let completed = tokio::time::timeout(Duration::from_secs(2), &mut drain)
            .await
            .ok();
        drop(drain);
        assert_eq!(completed, Some(Some(id)));
        assert!(parent.records.is_empty());
        assert_eq!(lock_owner(&retained).used_jobs(), 0);
        assert_eq!(parent.first_fault, Some(expected));
        drop(sender);
    }

    #[test]
    fn fallback_lost_wake_after_monitor_abort() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(fallback_after_outer_join(false));
    }

    #[test]
    fn fallback_lost_wake_after_monitor_panic() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(fallback_after_outer_join(true));
    }
}
