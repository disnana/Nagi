//! Private tracked DNS phase nested below the existing retained ledger.
//! Trusted parent retains/drains its monitor and JobRoot independently.
use super::parent_drain::RunnerOwnerHandle;
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
    runner: RunnerOwnerHandle,
    key: JobKey,
    end: Instant,
    started: Instant,
    endpoint: SocketAddr,
    book: Mutex<Book>,
    stop: watch::Sender<bool>,
}
#[derive(Clone)]
pub(super) struct DnsPhase(Arc<PhaseState>);
fn live_root(job: &JobRecord) -> bool {
    job.seal.root_registered()
        && job.tasks.iter().any(|task| {
            task.kind == TaskKind::JobRoot
                && task
                    .handle
                    .state()
                    .as_ref()
                    .is_some_and(|handle| !handle.is_finished())
                && task
                    .root_signal
                    .as_ref()
                    .is_some_and(|signal| signal.state.load(Ordering::Acquire) == ROOT_RUNNING)
        })
}
impl DnsPhase {
    // A non-counted runner handle and the exact admitted seal travel together.
    fn for_job(
        runner: &RunnerOwnerHandle,
        key: &JobKey,
        resolve_end: Instant,
        endpoint: SocketAddr,
    ) -> Result<Self, DnsFailure> {
        if endpoint.port() == 0 || endpoint.ip().is_unspecified() || endpoint.ip().is_multicast() {
            return Err(DnsFailure::Configuration);
        }
        let end = {
            let state = runner.owner.lock().map_err(|_| LedgerError::Faulted)?;
            let index = state.key_index(key)?;
            if state.is_closing() {
                return Err(LedgerError::Closing.into());
            }
            if state.fault().is_some() {
                return Err(LedgerError::Faulted.into());
            }
            let job = &state.jobs[index];
            if job.cancel_requested || job.dns_frozen || !job.reserved_roots[1] || !live_root(job) {
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
            runner: RunnerOwnerHandle {
                owner: Arc::clone(&runner.owner),
            },
            key: key.clone(),
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
    // Identity validation happens before any fault or fresh-record mutation.
    // Receipt cleanup can still run after the bound root has stopped.
    fn checked_owner(&self) -> Result<std::sync::MutexGuard<'_, Owner>, DnsFailure> {
        let (mut owner, poisoned) = match self.0.runner.owner.lock() {
            Ok(owner) => (owner, false),
            Err(poison) => (poison.into_inner(), true),
        };
        owner.key_index(&self.0.key)?;
        if poisoned {
            owner.latch_fault(Fault::PoisonedGate);
            owner.close();
        }
        Ok(owner)
    }
    // Owner -> Book -> OPEN/BUSY, with no await across the native primitive.
    fn gate_with_after_entry<T>(
        &self,
        after_entry: impl FnOnce(),
        operation: impl FnOnce(&mut Owner, usize, &mut Book) -> Result<T, DnsFailure>,
    ) -> Result<T, DnsFailure> {
        let mut owner = self.checked_owner()?;
        let index = owner.key_index(&self.0.key)?;
        if owner.is_closing() {
            return Err(LedgerError::Closing.into());
        }
        if owner.fault().is_some() {
            return Err(LedgerError::Faulted.into());
        }
        let job = &owner.jobs[index];
        if job.cancel_requested || job.dns_frozen || !live_root(job) {
            return Err(LedgerError::PhaseClosed.into());
        }
        if self.0.end <= Instant::now() || job.end.is_none_or(|end| end <= Instant::now()) {
            return Err(LedgerError::Expired.into());
        }
        let mut book = match self.0.book.lock() {
            Ok(book) => book,
            Err(_) => {
                owner.latch_fault(Fault::PoisonedGate);
                owner.close();
                return Err(LedgerError::Faulted.into());
            }
        };
        if book.closed {
            return Err(LedgerError::PhaseClosed.into());
        }
        let shared = Arc::clone(&owner.shared);
        let _entry = shared.begin_enqueue()?;
        after_entry();
        // Root completion can race independently while Book is unavailable.
        // This is the final entry observation, not a total order with later completion.
        let job = &owner.jobs[index];
        if !live_root(job) {
            return Err(LedgerError::PhaseClosed.into());
        }
        if self.0.end <= Instant::now() || job.end.is_none_or(|end| end <= Instant::now()) {
            return Err(LedgerError::Expired.into());
        }
        operation(&mut owner, index, &mut book)
    }
    fn gate<T>(
        &self,
        operation: impl FnOnce(&mut Owner, usize, &mut Book) -> Result<T, DnsFailure>,
    ) -> Result<T, DnsFailure> {
        self.gate_with_after_entry(|| {}, operation)
    }
    fn fail(&self) {
        let Ok(mut owner) = self.checked_owner() else {
            return;
        };
        owner.latch_fault(Fault::ProviderDenied);
        owner.shared.notify();
        drop(owner);
        let _ = self.close();
    }
    fn spawn<F: Future<Output = ()> + Send + 'static>(
        &self,
        kind: TaskKind,
        future: F,
    ) -> Result<(), DnsFailure> {
        self.spawn_with_after_entry(kind, future, || {})
    }
    fn spawn_with_after_entry<F: Future<Output = ()> + Send + 'static>(
        &self,
        kind: TaskKind,
        future: F,
        after_entry: impl FnOnce(),
    ) -> Result<(), DnsFailure> {
        if !kind.dns() {
            drop(future);
            return Err(DnsFailure::Configuration);
        }
        let mut stop = self.0.stop.subscribe();
        let end = self.0.end;
        let wrapped = async move {
            tokio::select! {
                biased;
                _=async {if !*stop.borrow() {let _=stop.changed().await;}}=>{}
                _=tokio::time::sleep_until(tokio::time::Instant::from_std(end))=>{}
                _=future=>{}
            }
        };
        let (delivery, waiting) = GateFreeFuture::new(wrapped);
        let mut pending = Some(waiting);
        let prepared = PreparedLaunch::new(kind);
        let mut duplicate_root = false;
        let result = self.gate_with_after_entry(after_entry, |owner, index, book| {
            if kind == TaskKind::DnsRoot
                && (book.root_started || !owner.jobs[index].reserved_roots[1])
            {
                duplicate_root = true;
                return Err(LedgerError::PhaseClosed.into());
            }
            if kind == TaskKind::DnsChild && !book.root_started {
                return Err(LedgerError::PhaseClosed.into());
            }
            match owner.stage(
                self.0.key.id(),
                kind,
                pending.take().expect("one admission"),
                true,
                Some(Arc::clone(&delivery.receipt)),
                prepared,
            ) {
                Ok(staged) => Ok(staged),
                Err((error, rejected)) => {
                    pending = Some(rejected);
                    Err(error.into())
                }
            }
        });
        let result = result.map(|(launch, waiting)| {
            let poisoned_slot = launch.launch(waiting);
            let (cancel, poisoned) = {
                let (mut owner, poisoned_owner) = match self.0.runner.owner.lock() {
                    Ok(owner) => (owner, false),
                    Err(poison) => (poison.into_inner(), true),
                };
                let mut cancel = launch.publish(&mut owner, poisoned_slot || poisoned_owner);
                let (mut book, poisoned_book) = match self.0.book.lock() {
                    Ok(book) => (book, false),
                    Err(poison) => (poison.into_inner(), true),
                };
                if kind == TaskKind::DnsRoot {
                    book.root_started = true;
                }
                if poisoned_book {
                    owner.latch_fault(Fault::PoisonedGate);
                    cancel = true;
                }
                (cancel, poisoned_slot || poisoned_owner || poisoned_book)
            };
            if poisoned {
                launch.shared.request_stop();
            }
            if cancel {
                if let Some(handle) = launch.slot.state().as_ref() {
                    handle.abort();
                }
            }
            launch.slot.wake(&launch.shared);
        });
        drop(pending); // Only an empty receiver; all lifecycle guards are gone.
        let delivery_result = match result {
            Ok(()) => Ok(()),
            Err(DnsFailure::Ledger(error)) => Err(error),
            Err(_) => Err(LedgerError::PhaseClosed),
        };
        // Preserve the original DNS failure, and destroy/send the original
        // wrapped Future only after Owner, Book and Entry have been released.
        drop(delivery.finish(delivery_result));
        if duplicate_root
            || matches!(
                result,
                Err(DnsFailure::Ledger(
                    LedgerError::Capacity | LedgerError::NoRuntime | LedgerError::Allocation
                ))
            )
        {
            self.fail();
        }
        result
    }
    fn begin_io(&self) -> Result<IoTicket, DnsFailure> {
        self.gate(|_, _, book| {
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
        match self.gate(|_, _, book| Ok(operation(book))) {
            Ok(polled) => polled,
            Err(_) => Poll::Ready(Err(io::Error::other("DNS phase unavailable"))),
        }
    }
    fn close(&self) -> Result<(), DnsFailure> {
        let mut owner = self.checked_owner()?;
        let mut book = match self.0.book.lock() {
            Ok(book) => book,
            Err(poison) => {
                owner.latch_fault(Fault::PoisonedGate);
                owner.close();
                poison.into_inner()
            }
        };
        book.closed = true;
        owner.freeze_dns(&self.0.key)?;
        owner.shared.notify();
        drop(book);
        drop(owner);
        self.0.stop.send_replace(true);
        Ok(())
    }
    async fn finish(&self) -> Result<(), DnsFailure> {
        let mut events = self.checked_owner()?.shared.events_tx.subscribe();
        self.close()?;
        // Stored real joins remain parent-retained if this waiter is dropped.
        loop {
            let joined = std::future::poll_fn(|cx| {
                let mut owner = match self.checked_owner() {
                    Ok(owner) => owner,
                    Err(error) => return Poll::Ready(Err(error)),
                };
                let index = match owner.key_index(&self.0.key) {
                    Ok(index) => index,
                    Err(error) => return Poll::Ready(Err(error.into())),
                };
                owner.observe_root_faults(self.0.key.id());
                let expected_cancel = owner.jobs[index].cancel_requested;
                let mut fault = None;
                let mut pending = false;
                let mut observed = false;
                for task in &mut owner.jobs[index].tasks {
                    if !task.kind.dns() {
                        continue;
                    }
                    if !task.delivery_ready(cx) {
                        pending = true;
                        continue;
                    }
                    task.handle.register(cx);
                    let mut launch_state = task.handle.state();
                    if let Some(handle) = launch_state.as_mut() {
                        match Pin::new(handle).poll(cx) {
                            Poll::Pending => pending = true,
                            Poll::Ready(result) => {
                                observed = true;
                                *launch_state = Launch::ActualJoined;
                                if let Err(error) = result {
                                    if error.is_panic() {
                                        fault.get_or_insert(Fault::TaskPanic);
                                    } else if !expected_cancel {
                                        fault.get_or_insert(Fault::UnexpectedCancel);
                                    }
                                }
                            }
                        }
                    } else if !launch_state.is_none() {
                        pending = true;
                    }
                }
                for kind in [TaskKind::DnsRoot, TaskKind::DnsChild] {
                    owner.used_tasks[kind.index()] = owner
                        .jobs
                        .iter()
                        .flat_map(|job| &job.tasks)
                        .filter(|task| task.kind == kind && task.handle.outstanding())
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
            // Receipt is posted only to the exact still-retained JobKey.
            if let Ok(mut owner) = phase.checked_owner() {
                if poisoned {
                    owner.latch_fault(Fault::PoisonedGate);
                    owner.close();
                }
                owner.shared.notify();
            }
        }
    }
}
struct StopOnDrop(DnsPhase);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        let _ = self.0.close();
    }
}
#[path = "hickory.rs"]
mod hickory;
#[path = "tokio_io.rs"]
mod tokio_io;
pub(super) use hickory::{resolve, DrainedResolution, FamilyOutcome, ResolverStartup};
#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
