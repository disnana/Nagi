//! Private tracked DNS phase nested below the existing retained ledger.
//! Trusted parent retains/drains its monitor and JobRoot independently.
use super::*;
use std::{io, net::SocketAddr, time::Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DnsFailure {
    Ledger(LedgerError),
    Configuration,
    Capacity,
    Answer,
    Empty,
    Resolver,
    ResponseCode(u16),
    Channel,
}
impl From<LedgerError> for DnsFailure {
    fn from(error: LedgerError) -> Self {
        Self::Ledger(error)
    }
}
const IO_STARTS: usize = 16;
const WIRE_BYTES: usize = 128 * 1024;
const RECORDS: usize = 128;
const CNAME_HOPS: usize = 8;
struct Book {
    closed: bool,
    root_started: bool,
    io_started: usize,
    io_live: usize,
    io_closed: usize,
    sent: usize,
    received: usize,
}
struct PhaseState {
    owner: Arc<Mutex<Owner>>,
    id: u64,
    end: Instant,
    started: Instant,
    endpoint: SocketAddr,
    book: Mutex<Book>,
    stop: watch::Sender<bool>,
}
#[derive(Clone)]
pub(super) struct DnsPhase(Arc<PhaseState>);
fn retained_owner(owner: &Mutex<Owner>) -> std::sync::MutexGuard<'_, Owner> {
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
impl DnsPhase {
    // Already admitted command only; no Grant/permit/proof constructor.
    fn for_job(
        retained: &Arc<Mutex<Owner>>,
        id: u64,
        resolve_end: Instant,
        endpoint: SocketAddr,
    ) -> Result<Self, DnsFailure> {
        if endpoint.port() == 0 || endpoint.ip().is_unspecified() || endpoint.ip().is_multicast() {
            return Err(DnsFailure::Configuration);
        }
        // Internal roots retain the parent owner, never a counted sender. EOF
        // must remain observable when the last external sender disappears.
        let owner = retained.clone();
        let end = {
            let state = owner.lock().map_err(|_| LedgerError::Faulted)?;
            if state.is_closing() {
                return Err(LedgerError::Closing.into());
            }
            if state.fault().is_some() {
                return Err(LedgerError::Faulted.into());
            }
            let job = state
                .jobs
                .iter()
                .find(|job| job.id == id)
                .ok_or(LedgerError::UnknownJob)?;
            if job.cancel_requested || job.dns_frozen {
                return Err(LedgerError::PhaseClosed.into());
            }
            if !job.reserved_roots[1] {
                return Err(LedgerError::PhaseClosed.into());
            }
            if !job
                .tasks
                .iter()
                .any(|task| task.kind == TaskKind::JobRoot && task.handle.is_some())
            {
                return Err(LedgerError::PhaseClosed.into());
            }
            let end = job.end.ok_or(LedgerError::Expired)?.min(resolve_end);
            if end <= Instant::now() {
                return Err(LedgerError::Expired.into());
            }
            end
        };
        let (stop, _) = watch::channel(false);
        Ok(Self(Arc::new(PhaseState {
            owner,
            id,
            end,
            started: Instant::now(),
            endpoint,
            book: Mutex::new(Book {
                closed: false,
                root_started: false,
                io_started: 0,
                io_live: 0,
                io_closed: 0,
                sent: 0,
                received: 0,
            }),
            stop,
        })))
    }
    // Always owner then book; never poll whole root/resolver futures here.
    fn gate<T>(
        &self,
        operation: impl FnOnce(&mut Owner, &mut Book) -> Result<T, DnsFailure>,
    ) -> Result<T, DnsFailure> {
        let mut owner = retained_owner(&self.0.owner);
        let mut book = match self.0.book.lock() {
            Ok(book) => book,
            Err(_) => {
                owner.latch_fault(Fault::PoisonedGate);
                owner.close();
                return Err(LedgerError::Faulted.into());
            }
        };
        if owner.is_closing() {
            return Err(LedgerError::Closing.into());
        }
        if owner.fault().is_some() {
            return Err(LedgerError::Faulted.into());
        }
        let job = owner
            .jobs
            .iter()
            .find(|job| job.id == self.0.id)
            .ok_or(LedgerError::UnknownJob)?;
        if book.closed || job.dns_frozen || job.cancel_requested {
            return Err(LedgerError::PhaseClosed.into());
        }
        if self.0.end <= Instant::now() || job.end.is_none_or(|end| end <= Instant::now()) {
            return Err(LedgerError::Expired.into());
        }
        operation(&mut owner, &mut book)
    }
    fn fail(&self) {
        let mut owner = retained_owner(&self.0.owner);
        owner.latch_fault(Fault::ProviderDenied);
        owner.shared.notify();
        drop(owner);
        self.close();
    }
    fn spawn<F: Future<Output = ()> + Send + 'static>(
        &self,
        kind: TaskKind,
        future: F,
    ) -> Result<(), DnsFailure> {
        if !kind.dns() {
            drop(future);
            return Err(DnsFailure::Configuration);
        }
        let mut stop = self.0.stop.subscribe();
        let end = self.0.end;
        let wrapped = async move {
            // Dispose inner native future before normal actual task completion.
            // This stops DNS only; global cancellation would forbid later TCP.
            tokio::select! {
                biased;
                _=async {if !*stop.borrow() {let _=stop.changed().await;}}=>{}
                _=tokio::time::sleep_until(tokio::time::Instant::from_std(end))=>{}
                _=future=>{}
            }
        };
        let mut pending = Some(wrapped);
        let result = self.gate(|owner, book| {
            if (kind == TaskKind::DnsRoot
                && (book.root_started
                    || !owner
                        .jobs
                        .iter()
                        .find(|j| j.id == self.0.id)
                        .expect("checked job")
                        .reserved_roots[1]))
                || (kind == TaskKind::DnsChild && !book.root_started)
            {
                return Err(LedgerError::PhaseClosed.into());
            }
            match owner.try_spawn(self.0.id, kind, pending.take().expect("one admission")) {
                Ok(()) => {
                    if kind == TaskKind::DnsRoot {
                        book.root_started = true;
                    }
                    Ok(())
                }
                Err((error, rejected)) => {
                    pending = Some(rejected);
                    Err(error.into())
                }
            }
        });
        drop(pending); // Rejected future remains unpolled, dropped outside gates.
        if result.is_err() {
            self.fail();
        }
        result
    }
    fn begin_io(&self) -> Result<IoTicket, DnsFailure> {
        self.gate(|_, book| {
            if !book.root_started {
                return Err(LedgerError::PhaseClosed.into());
            }
            if book.io_started >= IO_STARTS {
                return Err(DnsFailure::Capacity);
            }
            book.io_started += 1;
            book.io_live += 1;
            Ok(())
        })?;
        Ok(IoTicket(Some(self.clone())))
    }
    fn poll_io<T>(
        &self,
        operation: impl FnOnce(&mut Book) -> Poll<io::Result<T>>,
    ) -> Poll<io::Result<T>> {
        match self.gate(|_, book| Ok(operation(book))) {
            Ok(polled) => polled,
            Err(_) => Poll::Ready(Err(io::Error::other("DNS phase unavailable"))),
        }
    }
    fn close(&self) {
        let mut owner = retained_owner(&self.0.owner);
        let mut book = match self.0.book.lock() {
            Ok(book) => book,
            Err(poison) => {
                owner.latch_fault(Fault::PoisonedGate);
                owner.close();
                poison.into_inner()
            }
        };
        book.closed = true;
        let _ = owner.freeze_dns(self.0.id);
        owner.shared.notify();
        drop(book);
        drop(owner);
        self.0.stop.send_replace(true);
    }
    async fn finish(&self) -> Result<(), DnsFailure> {
        let mut events = retained_owner(&self.0.owner).shared.events_tx.subscribe();
        self.close();
        // Stored real joins remain parent-retained if this waiter is dropped.
        loop {
            let joined = std::future::poll_fn(|cx| {
                let mut owner = retained_owner(&self.0.owner);
                owner.observe_root_faults(self.0.id);
                let Some(index) = owner.jobs.iter().position(|job| job.id == self.0.id) else {
                    return Poll::Ready(Err(LedgerError::UnknownJob.into()));
                };
                let expected_cancel = owner.jobs[index].cancel_requested;
                let mut fault = None;
                let mut pending = false;
                let mut observed = false;
                for task in &mut owner.jobs[index].tasks {
                    if !task.kind.dns() {
                        continue;
                    }
                    if let Some(handle) = &mut task.handle {
                        match Pin::new(handle).poll(cx) {
                            Poll::Pending => pending = true,
                            Poll::Ready(result) => {
                                observed = true;
                                task.handle = None;
                                if let Err(error) = result {
                                    if error.is_panic() {
                                        fault.get_or_insert(Fault::TaskPanic);
                                    } else if !expected_cancel {
                                        fault.get_or_insert(Fault::UnexpectedCancel);
                                    }
                                }
                            }
                        }
                    }
                }
                for kind in [TaskKind::DnsRoot, TaskKind::DnsChild] {
                    owner.used_tasks[kind.index()] = owner
                        .jobs
                        .iter()
                        .flat_map(|job| &job.tasks)
                        .filter(|task| task.kind == kind && task.handle.is_some())
                        .count();
                }
                if observed {
                    owner.shared.notify();
                }
                if let Some(fault) = fault {
                    owner.latch_fault(fault);
                    owner.close();
                }
                if pending {
                    return Poll::Pending;
                }
                let book = match self.0.book.lock() {
                    Ok(book) => book,
                    Err(poison) => {
                        owner.latch_fault(Fault::PoisonedGate);
                        owner.close();
                        poison.into_inner()
                    }
                };
                if book.io_live != 0 || book.io_closed != book.io_started {
                    owner.latch_fault(Fault::ProviderDenied);
                    return Poll::Ready(Err(LedgerError::Faulted.into()));
                }
                if owner.fault().is_some() {
                    return Poll::Ready(Err(LedgerError::Faulted.into()));
                }
                if owner.is_closing() || owner.jobs[index].cancel_requested {
                    return Poll::Ready(Err(LedgerError::Closing.into()));
                }
                if self.0.end <= Instant::now()
                    || owner.jobs[index]
                        .end
                        .is_none_or(|end| end <= Instant::now())
                {
                    return Poll::Ready(Err(LedgerError::Expired.into()));
                }
                Poll::Ready(Ok(()))
            });
            tokio::select! {
                result=joined=>return result,
                changed=events.changed()=>{if changed.is_err() {return Err(LedgerError::Closing.into());}}
            }
        }
    }
}
struct IoTicket(Option<DnsPhase>);
impl Drop for IoTicket {
    fn drop(&mut self) {
        if let Some(phase) = self.0.take() {
            let (mut book, poisoned) = match phase.0.book.lock() {
                Ok(book) => (book, false),
                Err(poison) => (poison.into_inner(), true),
            };
            book.io_live -= 1;
            book.io_closed += 1;
            drop(book);
            // Raw IO/future was already disposed; starts do not reopen on receipt.
            let mut owner = retained_owner(&phase.0.owner);
            if poisoned {
                owner.latch_fault(Fault::PoisonedGate);
                owner.close();
            }
            owner.shared.notify();
        }
    }
}
struct StopOnDrop(DnsPhase);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.close();
    }
}
#[cfg(test)]
#[path = "hickory.rs"]
mod hickory;
#[path = "tokio_io.rs"]
mod tokio_io;
#[cfg(test)]
pub(super) use hickory::{resolve, DrainedResolution, FamilyOutcome, ResolverStartup};
#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
