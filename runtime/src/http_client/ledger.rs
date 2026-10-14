//! Private SF06 retained lifecycle ledger. Production admission is closed behind
//! the parent registration and actual Auth bridge; no public client API exists.

use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
    task::{Context, Poll, Waker},
};
use tokio::{
    sync::{watch, Notify},
    task::JoinHandle,
};

const MAX_SLOTS: usize = 64;
const MAX_WAITER_BYTES: usize = 1024 * 1024;
const STOP_REQUESTED: u8 = 1;
const ENQUEUE_ACCEPTED: u8 = 2;

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
    UnresolvedLaunch,
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
    stop_admit: AtomicU8,
    stop_notify: Notify,
    unresolved_launch: AtomicBool,
    waiters: Mutex<WaiterBook>,
    limits: Limits,
    events_tx: watch::Sender<bool>,
    #[cfg(test)]
    monitor_panic: AtomicBool,
}

impl Shared {
    // A synchronous destructor only posts stop. The retained monitor performs
    // the authoritative Owner -> waiter cleanup after taking the Owner gate.
    fn notify(&self) {
        self.events_tx.send_modify(|value| *value = !*value);
    }
    fn request_stop(&self) {
        let old = self.stop_admit.fetch_or(STOP_REQUESTED, Ordering::AcqRel);
        // No caller-owned Owner, waiter, or watch value lock is taken in Drop.
        // Every listener registers before checking the atomic stop bit.
        if old & STOP_REQUESTED == 0 {
            self.stop_notify.notify_waiters();
        }
    }
    fn is_stopping(&self) -> bool {
        self.stop_admit.load(Ordering::Acquire) & STOP_REQUESTED != 0
    }
    // The successful CAS is admission's linearization point relative to a
    // concurrent nonblocking stop request. The Owner gate excludes a second
    // enqueue until this reservation and record transition is complete.
    fn begin_enqueue(&self) -> Result<EntryGuard<'_>, LedgerError> {
        self.stop_admit
            .compare_exchange(0, ENQUEUE_ACCEPTED, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| EntryGuard { shared: self })
            .map_err(|state| {
                if state & STOP_REQUESTED != 0 {
                    LedgerError::Closing
                } else {
                    LedgerError::Faulted
                }
            })
    }
    fn clear_waiters(&self) {
        let mut book = self
            .waiters
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        book.records.clear();
        book.bytes = 0;
    }
}

// The entry is a short synchronous primitive. A stop request after its CAS
// cannot revoke the winner, and clearing BUSY must never clear STOP.
struct EntryGuard<'a> {
    shared: &'a Shared,
}
impl Drop for EntryGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.shared.request_stop();
        }
        self.shared
            .stop_admit
            .fetch_and(!ENQUEUE_ACCEPTED, Ordering::AcqRel);
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
            if self.shared.is_stopping() {
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
                if self.shared.is_stopping() {
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
            self.shared.request_stop();
        }
    }
}

#[derive(Debug)]
struct WaiterTicket {
    id: u64,
    shared: Arc<Shared>,
}

impl WaiterTicket {
    async fn wait_closed(&mut self) {
        loop {
            let notified = self.shared.stop_notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.shared.is_stopping() {
                self.release_registration();
                return;
            }
            notified.await;
        }
    }
    fn release_registration(&self) {
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
impl Drop for WaiterTicket {
    fn drop(&mut self) {
        self.release_registration();
    }
}

#[derive(Debug)]
struct TrackedTask {
    kind: TaskKind,
    handle: Arc<LaunchSlot>,
    root_signal: Option<Arc<RootSignal>>,
    root_fault_seen: bool,
    delivery: Option<Arc<DeliveryReceipt>>,
}

// Only polling the retained real handle Ready may produce ActualJoined.
#[derive(Debug)]
enum Launch<T> {
    Starting,
    Running(JoinHandle<T>),
    Unresolved,
    ActualJoined,
}
impl<T> Launch<T> {
    fn as_ref(&self) -> Option<&JoinHandle<T>> {
        if let Self::Running(handle) = self {
            Some(handle)
        } else {
            None
        }
    }
    fn as_mut(&mut self) -> Option<&mut JoinHandle<T>> {
        if let Self::Running(handle) = self {
            Some(handle)
        } else {
            None
        }
    }
    fn is_some(&self) -> bool {
        matches!(self, Self::Running(_))
    }
    fn is_none(&self) -> bool {
        matches!(self, Self::ActualJoined)
    }
}
#[derive(Debug)]
struct LaunchIdentity {
    key: JobKey,
    generation: u64,
}
#[derive(Debug)]
struct LaunchSlot {
    identity: OnceLock<LaunchIdentity>,
    state: Mutex<Launch<()>>,
    waker: Mutex<Option<Waker>>,
}
impl LaunchSlot {
    fn new() -> Self {
        Self {
            identity: OnceLock::new(),
            state: Mutex::new(Launch::Starting),
            waker: Mutex::new(None),
        }
    }
    fn state(&self) -> std::sync::MutexGuard<'_, Launch<()>> {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }
    fn outstanding(&self) -> bool {
        !self.state().is_none()
    }
    #[cfg(test)]
    fn is_some(&self) -> bool {
        self.state().is_some()
    }
    fn register(&self, cx: &Context<'_>) {
        let mut waker = self
            .waker
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if !waker.as_ref().is_some_and(|old| old.will_wake(cx.waker())) {
            *waker = Some(cx.waker().clone());
        }
    }
    fn wake(&self, shared: &Shared) {
        let waker = self
            .waker
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take();
        shared.notify();
        shared.stop_notify.notify_waiters();
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}
// Direct slot identity survives STOP, expiry and poisoned/stale Owner lookups.
// Each reuse allocates a fresh identity; a launch token can never target reuse.
struct PreparedLaunch {
    slot: Arc<LaunchSlot>,
    root_signal: Option<Arc<RootSignal>>,
    runtime: Result<tokio::runtime::Handle, LedgerError>,
}
impl PreparedLaunch {
    fn new(kind: TaskKind) -> Self {
        Self {
            slot: Arc::new(LaunchSlot::new()),
            root_signal: (kind == TaskKind::JobRoot).then(|| Arc::new(RootSignal::new())),
            runtime: tokio::runtime::Handle::try_current().map_err(|_| LedgerError::NoRuntime),
        }
    }
}
struct StagedLaunch {
    slot: Arc<LaunchSlot>,
    shared: Arc<Shared>,
    kind: TaskKind,
    root_signal: Option<Arc<RootSignal>>,
    runtime: tokio::runtime::Handle,
    #[cfg(test)]
    probe: Option<Arc<Mutex<h0_launch_probe::Inner>>>,
}
enum LaunchFuture<F> {
    Root(ObservedFuture<F>),
    Plain(F),
}
impl StagedLaunch {
    fn launch<F: Future<Output = ()> + Send + 'static>(&self, waiting: F) -> bool {
        // Prepare only the empty wrapper outside every lifecycle gate. Keep
        // the original root/child native frame types and allocation count.
        let future = if let Some(signal) = &self.root_signal {
            LaunchFuture::Root(ObservedFuture {
                inner: Some(Box::pin(waiting)),
                root_signal: Arc::clone(signal),
            })
        } else {
            LaunchFuture::Plain(waiting)
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            #[cfg(test)]
            if self
                .probe
                .as_ref()
                .is_some_and(|probe| probe.lock().unwrap().pre_bind)
            {
                panic!("injected pre-bind launch unwind");
            }
            let handle = match future {
                LaunchFuture::Root(future) => self.runtime.spawn(future),
                LaunchFuture::Plain(future) => self.runtime.spawn(future),
            };
            #[cfg(test)]
            let handle = h0_launch_probe::after_inner_spawn(handle, self.probe.clone());
            handle
        }));
        match result {
            Ok(handle) => {
                // No rejectable lookup, close/abort or external wake before storage.
                let (mut state, poisoned) = match self.slot.state.lock() {
                    Ok(state) => (state, false),
                    Err(poison) => (poison.into_inner(), true),
                };
                *state = Launch::Running(handle);
                drop(state);
                poisoned
            }
            Err(payload) => {
                *self.slot.state() = Launch::Unresolved;
                self.shared.unresolved_launch.store(true, Ordering::Release);
                self.shared.request_stop();
                self.slot.wake(&self.shared);
                std::panic::resume_unwind(payload)
            }
        }
    }
    fn publish(&self, owner: &mut Owner, poisoned: bool) -> bool {
        let identity = self.slot.identity.get().expect("staged launch identity");
        identity.key.seal.registered(self.kind);
        if poisoned {
            owner.latch_fault(Fault::PoisonedGate);
        }
        let stopping = owner.is_closing() || owner.fault().is_some();
        // Debt prevents retirement, so the exact key necessarily remains.
        let job = owner
            .jobs
            .iter_mut()
            .find(|job| job.id == identity.key.id && Arc::ptr_eq(&job.seal, &identity.key.seal));
        let cancel = if let Some(job) = job {
            job.started = true;
            job.cancel_requested
                || stopping
                || job.end.is_some_and(|end| end <= std::time::Instant::now())
        } else {
            return true;
        };
        if cancel {
            // Storage is already authoritative. Apply the whole-job transition
            // before the cancellation bit removes its deadline from the monitor.
            owner
                .request_cancel(identity.key.id)
                .expect("retained staged job");
        }
        cancel
    }

    fn run<F: Future<Output = ()> + Send + 'static>(&self, owner: &Mutex<Owner>, waiting: F) {
        let poisoned_slot = self.launch(waiting);
        let (cancel, poisoned) = {
            let (mut state, poisoned_owner) = match owner.lock() {
                Ok(state) => (state, false),
                Err(poison) => (poison.into_inner(), true),
            };
            let poisoned = poisoned_slot || poisoned_owner;
            (self.publish(&mut state, poisoned), poisoned)
        };
        if poisoned {
            self.shared.request_stop();
        }
        if cancel {
            if let Some(handle) = self.slot.state().as_ref() {
                handle.abort();
            }
        }
        self.slot.wake(&self.shared);
    }
}

// A returned empty-wrapper handle can join before the caller has transferred
// or destroyed its original Future. This receipt holds that separate debt.
// Allocation precedes the lifecycle gates; binding identifies the same Owner
// as the exact stored task. The bare drain has one waker, while independent
// monitor/DNS/fallback listeners use the existing shared broadcast channels.
#[derive(Debug)]
struct DeliveryReceipt {
    settled: AtomicBool,
    waker: Mutex<Option<Waker>>,
    shared: OnceLock<Arc<Shared>>,
}
impl DeliveryReceipt {
    fn new() -> Self {
        Self {
            settled: AtomicBool::new(false),
            waker: Mutex::new(None),
            shared: OnceLock::new(),
        }
    }
    fn is_settled(&self) -> bool {
        self.settled.load(Ordering::Acquire)
    }
    fn ready(&self, cx: &Context<'_>) -> bool {
        self.ready_with_after_register(cx, || {})
    }
    fn ready_with_after_register(&self, cx: &Context<'_>, after_register: impl FnOnce()) -> bool {
        if self.is_settled() {
            return true;
        }
        {
            let mut waker = self
                .waker
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            // Settlement takes this same lock after publishing. Do not leave
            // a newly registered waker behind a completed settlement.
            if self.is_settled() {
                return true;
            }
            if !waker.as_ref().is_some_and(|old| old.will_wake(cx.waker())) {
                *waker = Some(cx.waker().clone());
            }
        }
        after_register();
        self.is_settled()
    }
    // Only GateFreeFuture::finish calls this in production, outside all caller
    // gates and after send succeeds or rejected-Future Drop returns normally.
    fn settle(&self) {
        self.settled.store(true, Ordering::Release);
        let waker = self
            .waker
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take();
        if let Some(shared) = self.shared.get() {
            shared.notify();
            // Parent fallback listens independently of the monitor/watch.
            shared.stop_notify.notify_waiters();
        }
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl TrackedTask {
    fn delivery_ready(&self, cx: &Context<'_>) -> bool {
        self.delivery
            .as_ref()
            .is_none_or(|receipt| receipt.ready(cx))
    }
}

// Keep the caller's Future out of runtime.spawn until its actual JoinHandle is
// retained and every lifecycle gate has been released. A closed runtime may
// synchronously drop the empty receiver, but cannot run the caller's destructor
// under Owner/Book/Entry. This does not recover a spawn that unwinds without
// returning a handle, and neither send nor Future Drop establishes actual join.
struct GateFreeFuture<F> {
    future: F,
    sender: tokio::sync::oneshot::Sender<F>,
    receipt: Arc<DeliveryReceipt>,
}

impl<F: Future<Output = ()> + Send + 'static> GateFreeFuture<F> {
    fn new(future: F) -> (Self, impl Future<Output = ()> + Send + 'static) {
        let (sender, receiver) = tokio::sync::oneshot::channel::<F>();
        let waiting = async move {
            if let Ok(future) = receiver.await {
                future.await;
            }
        };
        (
            Self {
                future,
                sender,
                receipt: Arc::new(DeliveryReceipt::new()),
            },
            waiting,
        )
    }

    // The caller must invoke this only after dropping all lifecycle guards.
    // Rejection returns the original unpolled Future; successful registration
    // keeps the actual handle even when cancellation makes delivery fail.
    fn finish(self, registered: Result<(), LedgerError>) -> Result<(), (LedgerError, F)> {
        match registered {
            Err(error) => Err((error, self.future)),
            Ok(()) => {
                if let Err(future) = self.sender.send(self.future) {
                    // A panic here deliberately leaves unresolved delivery
                    // debt. Arbitrary Rust Drop panic is not recovery/ack.
                    drop(future);
                }
                self.receipt.settle();
                Ok(())
            }
        }
    }
}

const ROOT_REGISTERED: u8 = 1;
const ROOT_ACTUAL_JOINED: u8 = 2;
const DRIVER_REGISTERED: u8 = 4;
const DRIVER_ACTUAL_JOINED: u8 = 8;
const COMMAND_POPPED: u8 = 16;
const COMMAND_DISPOSED: u8 = 32;
const COMMAND_ABANDONED: u8 = 64;

// Fixed completion storage is allocated before the Auth permit is consumed.
// Registration and actual join are distinct; neither a reported result nor
// JoinHandle::is_finished can set an actual-join bit.
#[derive(Debug)]
struct JobSeal {
    completion: AtomicU8,
    stages: AtomicU8,
}
impl JobSeal {
    fn new() -> Self {
        Self {
            completion: AtomicU8::new(0),
            stages: AtomicU8::new(0),
        }
    }
    fn staged(&self, kind: TaskKind) -> bool {
        self.stages.load(Ordering::Acquire) & (1 << kind.index()) != 0
    }
    fn registered(&self, kind: TaskKind) {
        let bit = match kind {
            TaskKind::JobRoot => ROOT_REGISTERED,
            TaskKind::HttpDriver => DRIVER_REGISTERED,
            _ => return,
        };
        self.completion.fetch_or(bit, Ordering::Release);
    }
    fn joined(&self, kind: TaskKind) {
        let bit = match kind {
            TaskKind::JobRoot => ROOT_ACTUAL_JOINED,
            TaskKind::HttpDriver => DRIVER_ACTUAL_JOINED,
            _ => return,
        };
        self.completion.fetch_or(bit, Ordering::Release);
    }
    fn command_popped(&self) {
        self.completion.fetch_or(COMMAND_POPPED, Ordering::Release);
    }
    fn command_disposed(&self, abandoned: bool) {
        let flags = COMMAND_DISPOSED | if abandoned { COMMAND_ABANDONED } else { 0 };
        self.completion.fetch_or(flags, Ordering::Release);
    }
    fn command_retained(&self) -> bool {
        let flags = self.completion.load(Ordering::Acquire);
        flags & COMMAND_POPPED != 0 && flags & COMMAND_DISPOSED == 0
    }
    fn command_abandoned(&self) -> bool {
        self.completion.load(Ordering::Acquire) & COMMAND_ABANDONED != 0
    }
    fn root_registered(&self) -> bool {
        self.completion.load(Ordering::Acquire) & ROOT_REGISTERED != 0
    }
}

/// An opaque identity for one admitted job. The number is only an index into
/// the owner's retained records; the Arc identity authorizes a runner port.
#[derive(Clone, Debug)]
pub(super) struct JobKey {
    id: u64,
    seal: Arc<JobSeal>,
}
impl JobKey {
    pub(super) fn id(&self) -> u64 {
        self.id
    }
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
    seal: Arc<JobSeal>,
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
    next_task_generation: u64,
    first_fault: Option<Fault>,
    reserved_roots: [usize; 2],
    retained_job_bytes: usize,
    job_byte_limit: usize,
    close_applied: bool,
    #[cfg(test)]
    h0_launch_probe: Option<Arc<Mutex<h0_launch_probe::Inner>>>,
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
        let (events_tx, _) = watch::channel(false);
        let shared = Arc::new(Shared {
            senders: AtomicUsize::new(1),
            stop_admit: AtomicU8::new(0),
            stop_notify: Notify::new(),
            unresolved_launch: AtomicBool::new(false),
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
                next_task_generation: 1,
                first_fault: None,
                reserved_roots: [0; 2],
                retained_job_bytes: 0,
                job_byte_limit: MAX_WAITER_BYTES,
                close_applied: false,
                #[cfg(test)]
                h0_launch_probe: None,
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
        let seal = Arc::new(JobSeal::new());
        self.next_job_id = id.checked_add(1).ok_or(LedgerError::Capacity)?;
        self.jobs.push(JobRecord {
            id,
            seal,
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

    fn key_index(&self, key: &JobKey) -> Result<usize, LedgerError> {
        self.jobs
            .iter()
            .position(|job| job.id == key.id && Arc::ptr_eq(&job.seal, &key.seal))
            .ok_or(LedgerError::UnknownJob)
    }

    fn stage<F>(
        &mut self,
        id: u64,
        kind: TaskKind,
        future: F,
        accepted_entry: bool,
        delivery: Option<Arc<DeliveryReceipt>>,
        prepared: PreparedLaunch,
    ) -> Result<(StagedLaunch, F), (LedgerError, F)> {
        if !accepted_entry && self.is_closing() {
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
        if self.jobs[index].seal.command_abandoned() {
            return Err((LedgerError::PhaseClosed, future));
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
            .position(|task| !task.handle.outstanding());
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

        let runtime = match prepared.runtime {
            Ok(runtime) => runtime,
            Err(error) => return Err((error, future)),
        };
        let Some(next_generation) = self.next_task_generation.checked_add(1) else {
            return Err((LedgerError::Capacity, future));
        };
        prepared
            .slot
            .identity
            .set(LaunchIdentity {
                key: JobKey {
                    id,
                    seal: Arc::clone(&self.jobs[index].seal),
                },
                generation: self.next_task_generation,
            })
            .expect("one immutable launch identity");
        self.next_task_generation = next_generation;
        if let Some(receipt) = &delivery {
            receipt
                .shared
                .set(Arc::clone(&self.shared))
                .expect("one delivery registration");
        }
        let tracked = TrackedTask {
            kind,
            handle: Arc::clone(&prepared.slot),
            root_signal: prepared.root_signal.clone(),
            root_fault_seen: false,
            delivery,
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
        self.used_tasks[kind.index()] += 1;
        self.jobs[index]
            .seal
            .stages
            .fetch_or(1 << kind.index(), Ordering::Release);
        Ok((
            StagedLaunch {
                slot: prepared.slot,
                root_signal: prepared.root_signal,
                runtime,
                shared: Arc::clone(&self.shared),
                kind,
                #[cfg(test)]
                probe: self.h0_launch_probe.take(),
            },
            future,
        ))
    }
    // Legacy bare-owner fixtures exercise the same slot transitions. Production
    // stages under its gates and invokes StagedLaunch::run after releasing them.
    #[cfg(test)]
    fn try_spawn<F: Future<Output = ()> + Send + 'static>(
        &mut self,
        id: u64,
        kind: TaskKind,
        future: F,
    ) -> Result<(), (LedgerError, F)> {
        self.try_spawn_record(id, kind, future, false, None)
    }
    #[cfg(test)]
    fn try_spawn_delivery<F, G: Future<Output = ()> + Send + 'static>(
        &mut self,
        id: u64,
        kind: TaskKind,
        waiting: G,
        delivery: &GateFreeFuture<F>,
        accepted_entry: bool,
    ) -> Result<(), (LedgerError, G)> {
        self.try_spawn_record(
            id,
            kind,
            waiting,
            accepted_entry,
            Some(Arc::clone(&delivery.receipt)),
        )
    }
    #[cfg(test)]
    fn try_spawn_record<F: Future<Output = ()> + Send + 'static>(
        &mut self,
        id: u64,
        kind: TaskKind,
        future: F,
        accepted_entry: bool,
        delivery: Option<Arc<DeliveryReceipt>>,
    ) -> Result<(), (LedgerError, F)> {
        let prepared = PreparedLaunch::new(kind);
        let (launch, waiting) = self.stage(id, kind, future, accepted_entry, delivery, prepared)?;
        let poisoned = launch.launch(waiting);
        if launch.publish(self, poisoned) {
            if let Some(handle) = launch.slot.state().as_ref() {
                handle.abort();
            }
        }
        launch.slot.wake(&launch.shared);
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
            if let Some(handle) = task.handle.state().as_ref() {
                handle.abort();
            }
        }
        self.shared.notify();
        Ok(())
    }

    fn freeze_dns(&mut self, key: &JobKey) -> Result<(), LedgerError> {
        let index = self.key_index(key)?;
        self.jobs[index].dns_frozen = true;
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
                    .any(|task| task.handle.outstanding() && (!dns_only || task.kind.dns()))
        }) {
            let joined = std::future::poll_fn(|cx| {
                for (job_index, job) in self.jobs.iter_mut().enumerate() {
                    if only_job.is_some_and(|id| job.id != id) {
                        continue;
                    }
                    if dns_only {
                        for (task_index, task) in job.tasks.iter().enumerate() {
                            if task.kind == TaskKind::JobRoot
                                && !task.root_fault_seen
                                && task.handle.state().is_some()
                            {
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
                        task.handle.register(cx);
                        if !task.delivery_ready(cx) {
                            continue;
                        }
                        if let Some(handle) = task.handle.state().as_mut() {
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
            *self.jobs[job_index].tasks[task_index].handle.state() = Launch::ActualJoined;
            self.jobs[job_index].seal.joined(kind);
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
                        if let Some(handle) = sibling.handle.state().as_ref() {
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
                if let Some(handle) = sibling.handle.state().as_ref() {
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
            if task.kind == TaskKind::JobRoot
                && !task.root_fault_seen
                && task.handle.state().is_some()
            {
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
        self.shared.request_stop();
        if self.close_applied {
            return;
        }
        self.shared.clear_waiters();
        for job in &mut self.jobs {
            job.cancel_requested = true;
            job.queued = None;
            for task in &job.tasks {
                if let Some(handle) = task.handle.state().as_ref() {
                    handle.abort();
                }
            }
        }
        self.close_applied = true;
        self.shared.notify();
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
                    task.kind == kind
                        && task
                            .delivery
                            .as_ref()
                            .is_none_or(|receipt| receipt.is_settled())
                        && task
                            .handle
                            .state()
                            .as_ref()
                            .is_some_and(JoinHandle::is_finished)
                })
            })
    }
    fn is_closing(&self) -> bool {
        self.shared.is_stopping()
    }
    fn latch_fault(&mut self, fault: Fault) {
        self.first_fault.get_or_insert(fault);
    }
    fn fault(&self) -> Option<Fault> {
        self.first_fault.or_else(|| {
            self.shared
                .unresolved_launch
                .load(Ordering::Acquire)
                .then_some(Fault::UnresolvedLaunch)
        })
    }
}

impl Owner {
    // Capacity is released only after every actual JoinHandle is observed.
    // Queued/unstarted accepted work retains the record until start or cancel.
    fn release_finished_jobs(&mut self) {
        for index in (0..self.jobs.len()).rev() {
            let job = &self.jobs[index];
            if job.tasks.iter().any(|task| task.handle.outstanding())
                || job.seal.command_retained()
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
        if self.shared.unresolved_launch.load(Ordering::Acquire) {
            self.latch_fault(Fault::UnresolvedLaunch);
        }
        if self.is_closing() {
            self.close();
        }
        let now = std::time::Instant::now();
        for index in 0..self.jobs.len() {
            if self.jobs[index].seal.command_abandoned() && !self.jobs[index].cancel_requested {
                self.request_cancel(self.jobs[index].id)
                    .expect("retained abandoned job");
            }
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
                if !task.delivery_ready(cx) {
                    continue;
                }
                task.handle.register(cx);
                let mut launch_state = task.handle.state();
                if let Some(handle) = launch_state.as_mut() {
                    if let Poll::Ready(result) = Pin::new(handle).poll(cx) {
                        *launch_state = Launch::ActualJoined;
                        job.seal.joined(task.kind);
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
        // observe actual joins. No destructor takes a waiter or watch lock.
        self.shared.request_stop();
        for job in &self.jobs {
            for task in &job.tasks {
                if let Some(handle) = task.handle.state().as_ref() {
                    handle.abort();
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "h0_launch_probe.rs"]
mod h0_launch_probe;

// Only a retained ParentDrain can offer production registration. The ledger
// constructor itself and the queue/command factories are not exported.
#[path = "admission.rs"]
mod admission;
#[path = "parent_drain.rs"]
mod parent_drain;
pub(crate) use parent_drain::ParentDrain;

#[path = "tracked_dns.rs"]
mod tracked_dns;
