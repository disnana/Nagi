//! Private SF06 retained lifecycle ledger. Production admission is closed behind
//! the parent registration and actual Auth bridge; no public client API exists.

use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll, Waker},
};
use tokio::{sync::watch, task::JoinHandle};

const MAX_SLOTS: usize = 64;
const MAX_WAITER_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Limits {
    pub(crate) jobs: usize,
    pub(crate) queue: usize,
    pub(crate) job_roots: usize,
    pub(crate) dns_roots: usize,
    pub(crate) dns_children: usize,
    pub(crate) http_drivers: usize,
    pub(crate) waiters: usize,
    pub(crate) waiter_bytes: usize,
    pub(crate) senders: usize,
}

impl Limits {
    fn valid(self) -> bool {
        [
            self.jobs,
            self.queue,
            self.job_roots,
            self.dns_roots,
            self.dns_children,
            self.http_drivers,
            self.waiters,
            self.senders,
        ]
        .into_iter()
        .all(|n| (1..=MAX_SLOTS).contains(&n))
            && (1..=MAX_WAITER_BYTES).contains(&self.waiter_bytes)
    }
    fn task_cap(self, kind: TaskKind) -> usize {
        match kind {
            TaskKind::JobRoot => self.job_roots,
            TaskKind::DnsRoot => self.dns_roots,
            TaskKind::DnsChild => self.dns_children,
            TaskKind::HttpDriver => self.http_drivers,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LedgerError {
    InvalidLimits,
    Capacity,
    Closing,
    Faulted,
    UnknownJob,
    NoRuntime,
    Allocation,
    PhaseClosed,
    Expired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Fault {
    ProviderDenied,
    TaskPanic,
    UnexpectedCancel,
    PoisonedGate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaskKind {
    JobRoot,
    DnsRoot,
    DnsChild,
    HttpDriver,
}

impl TaskKind {
    fn index(self) -> usize {
        match self {
            Self::JobRoot => 0,
            Self::DnsRoot => 1,
            Self::DnsChild => 2,
            Self::HttpDriver => 3,
        }
    }
    fn dns(self) -> bool {
        matches!(self, Self::DnsRoot | Self::DnsChild)
    }
}

#[derive(Debug)]
struct WaiterRecord {
    id: u64,
    payload: Vec<u8>,
}

#[derive(Debug)]
struct WaiterBook {
    records: Vec<WaiterRecord>,
    bytes: usize,
    next_id: u64,
}

#[derive(Debug)]
struct Shared {
    senders: AtomicUsize,
    closing: AtomicBool,
    closed_tx: watch::Sender<bool>,
    waiters: Mutex<WaiterBook>,
    limits: Limits,
    events_tx: watch::Sender<bool>,
    #[cfg(test)]
    monitor_panic: AtomicBool,
}

impl Shared {
    // All waiter admission and close transitions use this short synchronous gate.
    fn notify(&self) {
        self.events_tx.send_modify(|value| *value = !*value);
    }
    fn mark_closing(&self) {
        {
            let mut book = self
                .waiters
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if self.closing.swap(true, Ordering::AcqRel) {
                return;
            }
            book.records.clear();
            book.bytes = 0;
        }
        self.closed_tx.send_replace(true);
        self.notify();
    }
}

#[derive(Debug)]
pub(crate) struct ClientSender {
    shared: Arc<Shared>,
    owner: Option<Arc<Mutex<Owner>>>,
}

impl ClientSender {
    fn try_clone(&self) -> Result<Self, LedgerError> {
        loop {
            if self.shared.closing.load(Ordering::Acquire) {
                return Err(LedgerError::Closing);
            }
            let count = self.shared.senders.load(Ordering::Acquire);
            if count == 0 || count >= self.shared.limits.senders {
                return Err(LedgerError::Capacity);
            }
            if self
                .shared
                .senders
                .compare_exchange_weak(count, count + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                if self.shared.closing.load(Ordering::Acquire) {
                    self.shared.senders.fetch_sub(1, Ordering::AcqRel);
                    return Err(LedgerError::Closing);
                }
                return Ok(Self {
                    shared: Arc::clone(&self.shared),
                    owner: self.owner.clone(),
                });
            }
        }
    }
}

impl Drop for ClientSender {
    fn drop(&mut self) {
        if self.shared.senders.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.shared.mark_closing();
        }
    }
}

#[derive(Debug)]
struct WaiterTicket {
    id: u64,
    shared: Arc<Shared>,
    closed_rx: watch::Receiver<bool>,
}

impl WaiterTicket {
    async fn wait_closed(&mut self) {
        loop {
            if *self.closed_rx.borrow_and_update() {
                return;
            }
            if self.closed_rx.changed().await.is_err() {
                return;
            }
        }
    }
}

impl Drop for WaiterTicket {
    fn drop(&mut self) {
        let mut book = self
            .shared
            .waiters
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(index) = book.records.iter().position(|record| record.id == self.id) {
            let record = book.records.swap_remove(index);
            book.bytes -= record.payload.len();
        }
    }
}

#[derive(Debug)]
struct TrackedTask {
    kind: TaskKind,
    handle: Option<JoinHandle<()>>,
    root_signal: Option<Arc<RootSignal>>,
    root_fault_seen: bool,
}

const ROOT_RUNNING: u8 = 0;
const ROOT_COMPLETED: u8 = 1;
const ROOT_PANICKED: u8 = 2;
const ROOT_CANCELLED: u8 = 3;

// A DNS-phase drain cannot join JobRoot: it still owns the whole operation.
// This bounded signal reports only terminal failure; the JoinHandle and its
// quota stay in JobRecord until the final outer drain observes the join.
#[derive(Debug)]
struct RootSignal {
    state: AtomicU8,
    dns_waker: Mutex<Option<Waker>>,
}

impl RootSignal {
    fn new() -> Self {
        Self {
            state: AtomicU8::new(ROOT_RUNNING),
            dns_waker: Mutex::new(None),
        }
    }

    fn register(&self, cx: &Context<'_>) {
        let mut waker = self
            .dns_waker
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if !waker.as_ref().is_some_and(|old| old.will_wake(cx.waker())) {
            *waker = Some(cx.waker().clone());
        }
    }

    fn clear_waker(&self) {
        self.dns_waker
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take();
    }

    fn terminal_fault(&self) -> Option<Fault> {
        match self.state.load(Ordering::Acquire) {
            ROOT_PANICKED => Some(Fault::TaskPanic),
            ROOT_CANCELLED => Some(Fault::UnexpectedCancel),
            _ => None,
        }
    }

    fn finish(&self, state: u8) {
        if self
            .state
            .compare_exchange(ROOT_RUNNING, state, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let waker = self
                .dns_waker
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .take();
            if let Some(waker) = waker {
                waker.wake();
            }
        }
    }
}

struct ObservedFuture<F> {
    inner: Option<Pin<Box<F>>>,
    root_signal: Arc<RootSignal>,
}

impl<F: Future<Output = ()>> Future for ObservedFuture<F> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            this.inner
                .as_mut()
                .expect("JobRoot Future polled after completion")
                .as_mut()
                .poll(cx)
        })) {
            Ok(Poll::Pending) => Poll::Pending,
            Ok(Poll::Ready(())) => {
                // Tokio drops the Future after Ready. Its destructor can panic,
                // so inspect that terminal step before reporting healthy root.
                let inner = this.inner.take().expect("JobRoot Future missing");
                if let Err(panic) =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(inner)))
                {
                    this.root_signal.finish(ROOT_PANICKED);
                    std::panic::resume_unwind(panic);
                }
                this.root_signal.finish(ROOT_COMPLETED);
                Poll::Ready(())
            }
            Err(panic) => {
                this.root_signal.finish(ROOT_PANICKED);
                std::panic::resume_unwind(panic)
            }
        }
    }
}

impl<F> Drop for ObservedFuture<F> {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(inner))) {
                Ok(()) => self.root_signal.finish(ROOT_CANCELLED),
                Err(panic) => {
                    self.root_signal.finish(ROOT_PANICKED);
                    // A poll panic is already propagating. Do not let a second
                    // destructor panic abort the process before Tokio can join.
                    if !std::thread::panicking() {
                        std::panic::resume_unwind(panic);
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
struct JobRecord {
    id: u64,
    tasks: Vec<TrackedTask>,
    dns_frozen: bool,
    cancel_requested: bool,
    end: Option<std::time::Instant>,
    payload_bytes: usize,
    queued: Option<admission::Command>,
    reserved_roots: [bool; 2],
    started: bool,
}

#[derive(Debug)]
struct Owner {
    shared: Arc<Shared>,
    limits: Limits,
    jobs: Vec<JobRecord>,
    used_queue: usize,
    used_tasks: [usize; 4],
    next_job_id: u64,
    first_fault: Option<Fault>,
    reserved_roots: [usize; 2],
    retained_job_bytes: usize,
    job_byte_limit: usize,
}

impl Owner {
    fn checked(limits: Limits) -> Result<(Self, ClientSender), LedgerError> {
        if !limits.valid() {
            return Err(LedgerError::InvalidLimits);
        }
        let mut jobs = Vec::new();
        jobs.try_reserve_exact(limits.jobs)
            .map_err(|_| LedgerError::Allocation)?;
        let mut waiter_records = Vec::new();
        waiter_records
            .try_reserve_exact(limits.waiters)
            .map_err(|_| LedgerError::Allocation)?;
        let (closed_tx, _) = watch::channel(false);
        let (events_tx, _) = watch::channel(false);
        let shared = Arc::new(Shared {
            senders: AtomicUsize::new(1),
            closing: AtomicBool::new(false),
            closed_tx,
            waiters: Mutex::new(WaiterBook {
                records: waiter_records,
                bytes: 0,
                next_id: 1,
            }),
            limits,
            events_tx,
            #[cfg(test)]
            monitor_panic: AtomicBool::new(false),
        });
        let sender = ClientSender {
            shared: Arc::clone(&shared),
            owner: None,
        };
        Ok((
            Self {
                shared,
                limits,
                jobs,
                used_queue: 0,
                used_tasks: [0; 4],
                next_job_id: 1,
                first_fault: None,
                reserved_roots: [0; 2],
                retained_job_bytes: 0,
                job_byte_limit: MAX_WAITER_BYTES,
            },
            sender,
        ))
    }

    // Old unprotected constructor remains test-only. Production uses admission.
    #[cfg(test)]
    fn fixture_admit(&mut self) -> Result<u64, LedgerError> {
        if self.is_closing() {
            return Err(LedgerError::Closing);
        }
        if self.first_fault.is_some() {
            return Err(LedgerError::Faulted);
        }
        if self.jobs.len() >= self.limits.jobs || self.used_queue >= self.limits.queue {
            return Err(LedgerError::Capacity);
        }
        self.jobs
            .try_reserve(1)
            .map_err(|_| LedgerError::Allocation)?;
        let id = self.next_job_id;
        self.next_job_id = id.checked_add(1).ok_or(LedgerError::Capacity)?;
        self.jobs.push(JobRecord {
            id,
            tasks: Vec::new(),
            dns_frozen: false,
            cancel_requested: false,
            end: None,
            payload_bytes: 0,
            queued: None,
            reserved_roots: [false; 2],
            started: true,
        });
        self.used_queue += 1;
        Ok(id)
    }

    // Synchronous check/spawn/register with exclusive owner access; no await and
    // no native task on denial. The rejected future is returned to the caller.
    fn try_spawn<F: Future<Output = ()> + Send + 'static>(
        &mut self,
        id: u64,
        kind: TaskKind,
        future: F,
    ) -> Result<(), (LedgerError, F)> {
        if self.is_closing() {
            return Err((LedgerError::Closing, future));
        }
        if self.first_fault.is_some() {
            return Err((LedgerError::Faulted, future));
        }
        let Some(index) = self.jobs.iter().position(|job| job.id == id) else {
            return Err((LedgerError::UnknownJob, future));
        };
        if self.jobs[index]
            .end
            .is_some_and(|end| end <= std::time::Instant::now())
        {
            self.request_cancel(id).expect("retained job");
            return Err((LedgerError::Expired, future));
        }
        if self.jobs[index].cancel_requested {
            return Err((LedgerError::Closing, future));
        }
        if self.jobs[index].dns_frozen && kind.dns() {
            return Err((LedgerError::PhaseClosed, future));
        }
        let reserved = if kind.index() < 2 {
            self.reserved_roots[kind.index()]
                - usize::from(self.jobs[index].reserved_roots[kind.index()])
        } else {
            0
        };
        if self.used_tasks[kind.index()] + reserved >= self.limits.task_cap(kind) {
            return Err((LedgerError::Capacity, future));
        }
        // Join-completed records are reused. Outstanding counts alone are not
        // a bound on this Vec if one job runs sequential child tasks for long.
        let reusable = self.jobs[index]
            .tasks
            .iter()
            .position(|task| task.handle.is_none());
        if reusable.is_none() {
            let max_records = self.limits.job_roots
                + self.limits.dns_roots
                + self.limits.dns_children
                + self.limits.http_drivers;
            if self.jobs[index].tasks.len() >= max_records {
                return Err((LedgerError::Capacity, future));
            }
            if self.jobs[index].tasks.try_reserve(1).is_err() {
                return Err((LedgerError::Allocation, future));
            }
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return Err((LedgerError::NoRuntime, future));
        };
        let root_signal = (kind == TaskKind::JobRoot).then(|| Arc::new(RootSignal::new()));
        let handle = if let Some(signal) = &root_signal {
            runtime.spawn(ObservedFuture {
                inner: Some(Box::pin(future)),
                root_signal: Arc::clone(signal),
            })
        } else {
            runtime.spawn(future)
        };
        let tracked = TrackedTask {
            kind,
            handle: Some(handle),
            root_signal,
            root_fault_seen: false,
        };
        if let Some(slot) = reusable {
            self.jobs[index].tasks[slot] = tracked;
        } else {
            self.jobs[index].tasks.push(tracked);
        }
        if kind.index() < 2 && self.jobs[index].reserved_roots[kind.index()] {
            self.jobs[index].reserved_roots[kind.index()] = false;
            self.reserved_roots[kind.index()] -= 1;
        }
        self.jobs[index].started = true; // actual handle is stored under this gate
        self.used_tasks[kind.index()] += 1;
        self.shared.notify();
        Ok(())
    }

    fn request_cancel(&mut self, id: u64) -> Result<(), LedgerError> {
        let job = self
            .jobs
            .iter_mut()
            .find(|job| job.id == id)
            .ok_or(LedgerError::UnknownJob)?;
        job.cancel_requested = true;
        job.queued = None;
        for task in &mut job.tasks {
            if let Some(handle) = task.handle.as_ref() {
                handle.abort();
            }
        }
        self.shared.notify();
        Ok(())
    }

    fn freeze_dns(&mut self, id: u64) -> Result<(), LedgerError> {
        let job = self
            .jobs
            .iter_mut()
            .find(|job| job.id == id)
            .ok_or(LedgerError::UnknownJob)?;
        job.dns_frozen = true;
        Ok(())
    }

    async fn drain_dns(&mut self, id: u64) -> Result<(), LedgerError> {
        let job = self
            .jobs
            .iter()
            .find(|job| job.id == id)
            .ok_or(LedgerError::UnknownJob)?;
        if !job.dns_frozen {
            return Err(LedgerError::PhaseClosed);
        }
        self.join_matching(Some(id), true).await;
        if let Some(job) = self.jobs.iter().find(|job| job.id == id) {
            for task in &job.tasks {
                if let Some(signal) = &task.root_signal {
                    signal.clear_waker();
                }
            }
        }
        self.observe_root_faults(id);
        if self.first_fault.is_some() {
            Err(LedgerError::Faulted)
        } else {
            Ok(())
        }
    }

    async fn drain_all(&mut self) {
        self.join_matching(None, false).await;
    }

    async fn close_and_drain(&mut self) {
        self.close();
        self.drain_all().await;
    }

    // Polling &mut JoinHandle stored in each record keeps cancellation safe for
    // ownership, and observes a completed root even if a pending child was
    // registered first. A dropped drain future never detaches a handle.
    async fn join_matching(&mut self, only_job: Option<u64>, dns_only: bool) {
        enum Joined {
            Task(usize, usize, Result<(), tokio::task::JoinError>),
            RootFault(usize, usize, Fault),
        }
        while self.jobs.iter().any(|job| {
            (only_job.is_none() || only_job == Some(job.id))
                && job
                    .tasks
                    .iter()
                    .any(|task| task.handle.is_some() && (!dns_only || task.kind.dns()))
        }) {
            let joined = std::future::poll_fn(|cx| {
                for (job_index, job) in self.jobs.iter_mut().enumerate() {
                    if only_job.is_some_and(|id| job.id != id) {
                        continue;
                    }
                    if dns_only {
                        for (task_index, task) in job.tasks.iter().enumerate() {
                            if task.kind == TaskKind::JobRoot && !task.root_fault_seen {
                                if let Some(signal) = &task.root_signal {
                                    signal.register(cx);
                                    if let Some(fault) = signal.terminal_fault() {
                                        return Poll::Ready(Joined::RootFault(
                                            job_index, task_index, fault,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    for (task_index, task) in job.tasks.iter_mut().enumerate() {
                        if dns_only && !task.kind.dns() {
                            continue;
                        }
                        if let Some(handle) = task.handle.as_mut() {
                            if let Poll::Ready(result) = Pin::new(handle).poll(cx) {
                                return Poll::Ready(Joined::Task(job_index, task_index, result));
                            }
                        }
                    }
                }
                Poll::Pending
            })
            .await;
            let Joined::Task(job_index, task_index, result) = joined else {
                if let Joined::RootFault(job_index, task_index, fault) = joined {
                    self.record_root_fault(job_index, task_index, fault);
                }
                continue;
            };
            let kind = self.jobs[job_index].tasks[task_index].kind;
            self.jobs[job_index].tasks[task_index].handle = None;
            self.used_tasks[kind.index()] -= 1;
            if let Err(join_error) = result {
                let unexpected = !self.jobs[job_index].cancel_requested;
                if join_error.is_panic() {
                    self.latch_fault(Fault::TaskPanic);
                } else if unexpected {
                    self.latch_fault(Fault::UnexpectedCancel);
                }
                if join_error.is_panic() || unexpected {
                    self.jobs[job_index].cancel_requested = true;
                    for sibling in &self.jobs[job_index].tasks {
                        if let Some(handle) = sibling.handle.as_ref() {
                            handle.abort();
                        }
                    }
                }
            }
        }
        if !dns_only {
            let previous = self.jobs.len();
            self.release_finished_jobs();
            debug_assert!(self.jobs.len() <= previous);
        }
    }

    fn record_root_fault(&mut self, job_index: usize, task_index: usize, fault: Fault) {
        self.jobs[job_index].tasks[task_index].root_fault_seen = true;
        if fault == Fault::TaskPanic || !self.jobs[job_index].cancel_requested {
            self.latch_fault(fault);
            self.jobs[job_index].cancel_requested = true;
            for sibling in &self.jobs[job_index].tasks {
                if let Some(handle) = sibling.handle.as_ref() {
                    handle.abort();
                }
            }
        }
    }

    fn observe_root_faults(&mut self, id: u64) {
        let Some(job_index) = self.jobs.iter().position(|job| job.id == id) else {
            return;
        };
        for task_index in 0..self.jobs[job_index].tasks.len() {
            let task = &self.jobs[job_index].tasks[task_index];
            if task.kind == TaskKind::JobRoot && !task.root_fault_seen {
                if let Some(fault) = task
                    .root_signal
                    .as_ref()
                    .and_then(|signal| signal.terminal_fault())
                {
                    self.record_root_fault(job_index, task_index, fault);
                }
            }
        }
    }

    fn close(&mut self) {
        self.shared.mark_closing();
        for job in &mut self.jobs {
            job.cancel_requested = true;
            job.queued = None;
            for task in &job.tasks {
                if let Some(handle) = task.handle.as_ref() {
                    handle.abort();
                }
            }
        }
    }

    fn try_register_waiter(
        &self,
        payload: Vec<u8>,
    ) -> Result<WaiterTicket, (LedgerError, Vec<u8>)> {
        let mut book = self
            .shared
            .waiters
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if self.is_closing() {
            return Err((LedgerError::Closing, payload));
        }
        let Some(total) = book.bytes.checked_add(payload.len()) else {
            return Err((LedgerError::Capacity, payload));
        };
        if book.records.len() >= self.limits.waiters || total > self.limits.waiter_bytes {
            return Err((LedgerError::Capacity, payload));
        }
        if book.records.try_reserve(1).is_err() {
            return Err((LedgerError::Allocation, payload));
        }
        let id = book.next_id;
        let Some(next_id) = id.checked_add(1) else {
            return Err((LedgerError::Capacity, payload));
        };
        book.next_id = next_id;
        book.bytes = total;
        book.records.push(WaiterRecord { id, payload });
        Ok(WaiterTicket {
            id,
            shared: Arc::clone(&self.shared),
            closed_rx: self.shared.closed_tx.subscribe(),
        })
    }

    fn registered_waiters(&self) -> (usize, usize) {
        let book = self
            .shared
            .waiters
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        (book.records.len(), book.bytes)
    }
    fn used_jobs(&self) -> usize {
        self.jobs.len()
    }
    fn used_tasks(&self, kind: TaskKind) -> usize {
        self.used_tasks[kind.index()]
    }
    #[cfg(test)]
    fn fixture_task_records(&self, id: u64) -> usize {
        self.jobs
            .iter()
            .find(|job| job.id == id)
            .map_or(0, |job| job.tasks.len())
    }
    fn task_is_finished(&self, id: u64, kind: TaskKind) -> bool {
        self.jobs
            .iter()
            .find(|job| job.id == id)
            .is_some_and(|job| {
                job.tasks.iter().any(|task| {
                    task.kind == kind && task.handle.as_ref().is_some_and(JoinHandle::is_finished)
                })
            })
    }
    fn is_closing(&self) -> bool {
        self.shared.closing.load(Ordering::Acquire)
    }
    fn latch_fault(&mut self, fault: Fault) {
        self.first_fault.get_or_insert(fault);
    }
    fn fault(&self) -> Option<Fault> {
        self.first_fault
    }
}

impl Owner {
    // Capacity is released only after every actual JoinHandle is observed.
    // Queued/unstarted accepted work retains the record until start or cancel.
    fn release_finished_jobs(&mut self) {
        for index in (0..self.jobs.len()).rev() {
            let job = &self.jobs[index];
            if job.tasks.iter().any(|task| task.handle.is_some())
                || (!job.cancel_requested && (!job.started || job.queued.is_some()))
            {
                continue;
            }
            let job = self.jobs.swap_remove(index);
            for kind in 0..2 {
                if job.reserved_roots[kind] {
                    self.reserved_roots[kind] -= 1;
                }
            }
            self.retained_job_bytes -= job.payload_bytes;
            self.used_queue -= 1;
            self.shared.notify();
        }
    }

    // The parent monitor polls stored Tokio joins, never an inner native future.
    // It may be cancelled; the same records and handles remain parent-retained.
    fn poll_parent(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        if self.is_closing() {
            self.close();
        }
        let now = std::time::Instant::now();
        for index in 0..self.jobs.len() {
            if !self.jobs[index].cancel_requested
                && self.jobs[index].end.is_some_and(|end| end <= now)
            {
                self.request_cancel(self.jobs[index].id)
                    .expect("retained job");
            }
        }
        let mut fault = None;
        for job in &mut self.jobs {
            for task in &mut job.tasks {
                if let Some(handle) = &mut task.handle {
                    if let Poll::Ready(result) = Pin::new(handle).poll(cx) {
                        task.handle = None;
                        self.used_tasks[task.kind.index()] -= 1;
                        if let Err(error) = result {
                            if error.is_panic() {
                                fault.get_or_insert(Fault::TaskPanic);
                            } else if !job.cancel_requested {
                                fault.get_or_insert(Fault::UnexpectedCancel);
                            }
                        }
                        self.shared.notify();
                    }
                }
            }
        }
        if let Some(fault) = fault {
            self.latch_fault(fault);
            self.close();
        }
        self.release_finished_jobs();
        if self.is_closing() && self.jobs.is_empty() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
    fn next_end(&self) -> Option<std::time::Instant> {
        self.jobs
            .iter()
            .filter(|job| !job.cancel_requested)
            .filter_map(|job| job.end)
            .min()
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        // Abort is only a request. A trusted parent must retain this owner and
        // await close_and_drain; synchronous Drop is not a join oracle.
        self.close();
    }
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;

// Only a retained ParentDrain can offer production registration. The ledger
// constructor itself and the queue/command factories are not exported.
#[path = "admission.rs"]
mod admission;
#[path = "parent_drain.rs"]
mod parent_drain;
pub(crate) use parent_drain::ParentDrain;

#[path = "tracked_dns.rs"]
mod tracked_dns;
