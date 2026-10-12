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
        // Never move handles into the waiting future or hold this lock over await.
        lock_owner(&self.records[index].owner).close();
        std::future::poll_fn(|cx| lock_owner(&self.records[index].owner).poll_parent(cx)).await;
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
