use super::{Event, EventKind, Options, RestartPolicy};
use crate::{Error, ErrorKind};
use std::{
    any::Any,
    fmt,
    future::{poll_fn, Future},
    panic::{catch_unwind, AssertUnwindSafe},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    sync::{broadcast, Notify},
    task::{AbortHandle, JoinHandle},
};

pub(super) type ChildFuture = Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Registered,
    Running,
    Stopping,
    Cleaning,
    Complete,
}

#[derive(Clone, Copy, Debug)]
#[cfg(test)]
pub(crate) struct Snapshot {
    pub phase: Phase,
    pub active_children: usize,
    pub retained_child_records: usize,
    pub retained_cleanup_record: bool,
    pub owner_present: bool,
    pub context_destroyed: bool,
}

pub(super) enum Outcome {
    Normal,
    Failed(Error, bool),
    Panicked,
}

pub(super) struct Record {
    abort: AbortHandle,
    join: JoinHandle<Outcome>,
}

pub(super) struct Slot {
    pub name: Arc<str>,
    pub policy: RestartPolicy,
    pub generation: i64,
    pub pending_restart: bool,
    inserting: bool,
    record: Option<Record>,
}

struct Ledger {
    phase: Phase,
    slots: Vec<Slot>,
    cleanup: Option<Record>,
    cleanup_inserting: bool,
    cleanup_future: Option<ChildFuture>,
    terminal: Option<Result<(), Error>>,
    first_failure: Option<Error>,
}

pub(super) struct Group {
    ledger: Mutex<Ledger>,
    pub drainer: tokio::sync::Mutex<()>,
    pub changed: Notify,
    pub started: AtomicBool,
    pub spawning_done: AtomicBool,
    pub stop: AtomicBool,
    pub owner_present: AtomicBool,
    owner_released: AtomicBool,
    active: AtomicUsize,
    cleanup_started: AtomicBool,
    cleanup_finished: AtomicBool,
    context_destroyed: AtomicBool,
    events: Mutex<Option<broadcast::Sender<Arc<Event>>>>,
    origin: OnceLock<tokio::runtime::Handle>,
    pub options: Options,
}

impl Group {
    pub fn new<C: Send + Sync + 'static>(context: Arc<C>, options: Options) -> Arc<Self> {
        let (events, _) = broadcast::channel(options.events);
        let group = Arc::new(Self {
            ledger: Mutex::new(Ledger {
                phase: Phase::Registered,
                slots: Vec::with_capacity(options.children),
                cleanup: None,
                cleanup_inserting: false,
                cleanup_future: None,
                terminal: None,
                first_failure: None,
            }),
            drainer: tokio::sync::Mutex::new(()),
            changed: Notify::new(),
            started: AtomicBool::new(false),
            spawning_done: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            owner_present: AtomicBool::new(true),
            owner_released: AtomicBool::new(false),
            active: AtomicUsize::new(0),
            cleanup_started: AtomicBool::new(false),
            cleanup_finished: AtomicBool::new(false),
            context_destroyed: AtomicBool::new(false),
            events: Mutex::new(Some(events)),
            origin: OnceLock::new(),
            options,
        });
        let weak = Arc::downgrade(&group);
        group.ledger.lock().unwrap().cleanup_future = Some(Box::pin(async move {
            let mut context = context;
            let mut delay = Duration::from_millis(1);
            loop {
                match Arc::try_unwrap(context) {
                    Ok(owned) => {
                        // A last-owner Db/native Drop may block here. The
                        // context guard remains retained until it really ends.
                        drop(owned);
                        if let Some(group) = weak.upgrade() {
                            group.context_destroyed.store(true, Ordering::Release);
                        }
                        return Ok(());
                    }
                    Err(remaining) => {
                        context = remaining;
                        tokio::time::sleep(delay).await;
                        delay = (delay * 2).min(Duration::from_millis(25));
                    }
                }
            }
        }));
        group
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Event>> {
        if let Some(events) = self.events.lock().unwrap().as_ref() {
            events.subscribe()
        } else {
            let (sender, receiver) = broadcast::channel(1);
            drop(sender);
            receiver
        }
    }

    pub fn set_origin(&self, handle: tokio::runtime::Handle) -> Result<(), Error> {
        self.origin
            .set(handle)
            .map_err(|_| Error::invalid("supervisor can only run once"))
    }

    pub fn publish(&self, event: Event) {
        if let Some(events) = self.events.lock().unwrap().as_ref() {
            let _ = events.send(Arc::new(event));
        }
    }

    pub fn child_event(&self, index: usize, kind: EventKind, message: &str) {
        let (name, generation) = {
            let ledger = self.ledger.lock().unwrap();
            (
                Arc::clone(&ledger.slots[index].name),
                ledger.slots[index].generation,
            )
        };
        self.publish(Event::new(
            index as i64 + 1,
            &name,
            generation,
            kind,
            message,
        ));
    }

    pub fn register(&self, name: &str, policy: RestartPolicy) -> Result<usize, Error> {
        let mut ledger = self.ledger.lock().unwrap();
        if self.started.load(Ordering::Acquire) || self.stop.load(Ordering::Acquire) {
            return Err(Error::invalid("supervisor registration is sealed"));
        }
        if name.is_empty() || name.len() > 128 {
            return Err(Error::invalid("child name must contain 1..128 UTF-8 bytes"));
        }
        if ledger.slots.len() >= self.options.children {
            return Err(Error::invalid("supervisor child limit reached"));
        }
        if ledger.slots.iter().any(|slot| slot.name.as_ref() == name) {
            return Err(Error::invalid("duplicate supervisor child name"));
        }
        let index = ledger.slots.len();
        ledger.slots.push(Slot {
            name: Arc::from(name),
            policy,
            generation: 0,
            pending_restart: false,
            record: None,
            inserting: false,
        });
        Ok(index)
    }

    pub fn spawn(self: &Arc<Self>, index: usize, inner: ChildFuture) -> Result<i64, Error> {
        let mut ledger = self.ledger.lock().unwrap();
        let slot = &mut ledger.slots[index];
        let generation = slot
            .generation
            .checked_add(1)
            .ok_or_else(|| Error::internal("actor generation exhausted"))?;
        slot.generation = generation;
        slot.pending_restart = false;
        slot.inserting = true;
        let name = Arc::clone(&slot.name);
        self.active.fetch_add(1, Ordering::AcqRel);
        drop(ledger);
        // Publish STARTING before any new child can publish STARTED. No mutex
        // is held through spawn: a closed origin runtime may immediately drop
        // the supplied future, including arbitrary state/context destructors.
        self.publish(Event::new(
            index as i64 + 1,
            &name,
            generation,
            EventKind::STARTING,
            "",
        ));
        let join = self
            .origin
            .get()
            .expect("running origin runtime")
            .spawn(RetainedFuture::new(
                inner,
                Arc::clone(self),
                GuardKind::Child,
            ));
        let abort = join.abort_handle();
        let mut ledger = self.ledger.lock().unwrap();
        let slot = &mut ledger.slots[index];
        if self.stop.load(Ordering::Acquire) {
            abort.abort();
        }
        slot.record = Some(Record { abort, join });
        slot.inserting = false;
        drop(ledger);
        self.changed.notify_waiters();
        Ok(generation)
    }

    pub fn policy(&self, index: usize) -> RestartPolicy {
        self.ledger.lock().unwrap().slots[index].policy
    }

    pub fn mark_running(&self) {
        let mut ledger = self.ledger.lock().unwrap();
        if !self.stop.load(Ordering::Acquire) {
            ledger.phase = Phase::Running;
        }
    }

    pub fn failure_event(&self, index: usize, error: &Error, truncated: bool) {
        let (name, generation) = {
            let ledger = self.ledger.lock().unwrap();
            (
                Arc::clone(&ledger.slots[index].name),
                ledger.slots[index].generation,
            )
        };
        let mut event = Event::new(
            index as i64 + 1,
            &name,
            generation,
            EventKind::FAILED,
            &error.message,
        );
        event.truncated |= truncated;
        self.publish(event);
    }

    pub fn pending_restart(&self, index: usize, pending: bool) {
        self.ledger.lock().unwrap().slots[index].pending_restart = pending;
    }

    pub fn has_service_work(&self) -> bool {
        self.ledger
            .lock()
            .unwrap()
            .slots
            .iter()
            .any(|slot| slot.record.is_some() || slot.pending_restart || slot.inserting)
    }

    pub fn remember_failure(&self, error: Error) {
        let error = bounded_error(error);
        let mut ledger = self.ledger.lock().unwrap();
        if ledger.first_failure.is_none() {
            ledger.first_failure = Some(error);
        }
    }

    pub fn request_stop(self: &Arc<Self>) {
        let first = !self.stop.swap(true, Ordering::AcqRel);
        {
            let mut ledger = self.ledger.lock().unwrap();
            if matches!(ledger.phase, Phase::Running | Phase::Registered) {
                ledger.phase = Phase::Stopping;
            }
            for slot in &mut ledger.slots {
                slot.pending_restart = false;
                if let Some(record) = &slot.record {
                    record.abort.abort();
                }
            }
        }
        if first {
            self.publish(Event::new(0, "", 0, EventKind::SHUTDOWN, ""));
        }
        self.changed.notify_waiters();
        self.maybe_cleanup();
    }

    pub fn release_owner_context(self: &Arc<Self>) {
        self.owner_released.store(true, Ordering::Release);
        self.maybe_cleanup();
    }

    pub fn maybe_cleanup(self: &Arc<Self>) {
        if !self.started.load(Ordering::Acquire)
            || !self.spawning_done.load(Ordering::Acquire)
            || !self.stop.load(Ordering::Acquire)
            || !self.owner_released.load(Ordering::Acquire)
            || self.active.load(Ordering::Acquire) != 0
        {
            return;
        }
        if self.cleanup_started.swap(true, Ordering::AcqRel) {
            return;
        }
        let mut ledger = self.ledger.lock().unwrap();
        let inner = ledger
            .cleanup_future
            .take()
            .expect("one context-cleanup slot");
        ledger.phase = Phase::Cleaning;
        ledger.cleanup_inserting = true;
        drop(ledger);
        // A foreign-runtime Control can initiate this phase. It must not move
        // cleanup onto its own short-lived runtime or give it abort authority.
        let join = self
            .origin
            .get()
            .expect("running origin runtime")
            .spawn(RetainedFuture::new(
                inner,
                Arc::clone(self),
                GuardKind::Context,
            ));
        let mut ledger = self.ledger.lock().unwrap();
        ledger.cleanup = Some(Record {
            abort: join.abort_handle(),
            join,
        });
        ledger.cleanup_inserting = false;
        drop(ledger);
        self.changed.notify_waiters();
    }

    pub fn poll_finished(
        &self,
        cx: &mut Context<'_>,
    ) -> Poll<(usize, Result<Outcome, tokio::task::JoinError>)> {
        let mut ledger = self.ledger.lock().unwrap();
        for (index, slot) in ledger.slots.iter_mut().enumerate() {
            if let Some(record) = slot.record.as_mut() {
                if let Poll::Ready(result) = Pin::new(&mut record.join).poll(cx) {
                    drop(slot.record.take());
                    // JoinError owns possible arbitrary native panic payload;
                    // return it to the elected observer outside this mutex.
                    return Poll::Ready((index, result));
                }
            }
        }
        Poll::Pending
    }

    #[cfg(test)]
    pub fn snapshot(&self) -> Snapshot {
        let ledger = self.ledger.lock().unwrap();
        Snapshot {
            phase: ledger.phase,
            active_children: self.active.load(Ordering::Acquire),
            retained_child_records: ledger
                .slots
                .iter()
                .filter(|slot| slot.record.is_some() || slot.inserting)
                .count(),
            retained_cleanup_record: ledger.cleanup.is_some() || ledger.cleanup_inserting,
            owner_present: self.owner_present.load(Ordering::Acquire),
            context_destroyed: self.context_destroyed.load(Ordering::Acquire),
        }
    }

    pub async fn wait_terminal(self: &Arc<Self>) -> Result<(), Error> {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let result = tokio::select! {
              _ = &mut changed => continue,
              result = poll_fn(|cx| {
                if let Poll::Ready(finished) = self.poll_finished(cx) {
                    return Poll::Ready(Some(finished));
                }
                let mut ledger = self.ledger.lock().unwrap();
                let cleanup_result = ledger.cleanup.as_mut().and_then(|record|
                    match Pin::new(&mut record.join).poll(cx) {
                        Poll::Pending => None,
                        Poll::Ready(result) => Some(result),
                    });
                if let Some(result) = cleanup_result {
                    drop(ledger.cleanup.take());
                    drop(ledger);
                    // The final slot has no actor index; result processed
                    // outside all locks on the same elected observer lease.
                    return Poll::Ready(Some((usize::MAX, result)));
                }
                if ledger.terminal.is_some() {
                    return Poll::Ready(None::<(usize, Result<Outcome, tokio::task::JoinError>)>);
                }
                if self.stop.load(Ordering::Acquire)
                    && ledger.slots.iter().all(|slot| slot.record.is_none() && !slot.inserting)
                    && ledger.cleanup.is_none()
                    && !ledger.cleanup_inserting
                    && self.cleanup_finished.load(Ordering::Acquire)
                {
                    let result = ledger.first_failure.take().map_or(Ok(()), Err);
                    ledger.phase = Phase::Complete;
                    ledger.terminal = Some(result);
                    drop(ledger);
                    drop(self.events.lock().unwrap().take());
                    self.changed.notify_waiters();
                    Poll::Ready(None)
                } else { Poll::Pending }
              }) => result,
            };
            match result {
                None => {
                    return self
                        .ledger
                        .lock()
                        .unwrap()
                        .terminal
                        .as_ref()
                        .unwrap()
                        .clone()
                }
                Some((index, result)) => {
                    match result {
                        Ok(Outcome::Normal) => {}
                        Ok(Outcome::Failed(error, truncated)) => {
                            if index != usize::MAX {
                                self.failure_event(index, &error, truncated);
                            }
                            self.remember_failure(error);
                        }
                        Ok(Outcome::Panicked) => {
                            if index != usize::MAX {
                                self.child_event(
                                    index,
                                    EventKind::PANICKED,
                                    "registered child panicked",
                                );
                            }
                            self.remember_failure(Error::internal("registered child panicked"));
                        }
                        Err(error) if error.is_cancelled() => {
                            if index == usize::MAX {
                                self.remember_failure(Error::internal("origin runtime canceled owned context cleanup before destruction"));
                            }
                        }
                        Err(error) => {
                            self.remember_failure(Error::internal("native child cleanup panicked"));
                            destroy_panic(error.into_panic());
                        }
                    }
                    if index != usize::MAX {
                        self.child_event(index, EventKind::STOPPED, "");
                    }
                }
            }
        }
    }

    pub fn unstarted_cleanup(self: &Arc<Self>) {
        // Drop of an unstarted owner does not start any background task. The
        // retained context future is dropped outside the mutex; no success
        // report is published because native external Arc owners can remain.
        let inner = self.ledger.lock().unwrap().cleanup_future.take();
        drop(inner);
        self.owner_present.store(false, Ordering::Release);
        drop(self.events.lock().unwrap().take());
    }
}

pub(super) fn bounded_error(error: Error) -> Error {
    let (message, _) = super::bounded_text(&error.message, 1024);
    Error {
        kind: error.kind,
        message,
    }
}

pub(super) fn incomplete() -> Error {
    Error {
        kind: ErrorKind::Busy,
        message: "supervisor shutdown incomplete".into(),
    }
}

// Do not recurse or forget an owned follow-on payload. A finite native panic
// chain is destroyed under the same retained guard. A never-ending native
// destructor remains noncooperative/accounted just like a never-yielding poll.
pub(super) fn destroy_panic(mut payload: Box<dyn Any + Send>) {
    loop {
        match catch_unwind(AssertUnwindSafe(|| drop(payload))) {
            Ok(()) => break,
            Err(next) => payload = next,
        }
    }
}

pub(super) fn destroy_owned<T>(value: T) -> bool {
    match catch_unwind(AssertUnwindSafe(|| drop(value))) {
        Ok(()) => false,
        Err(payload) => {
            destroy_panic(payload);
            true
        }
    }
}

enum GuardKind {
    Child,
    Context,
}

struct RetentionGuard {
    group: Arc<Group>,
    kind: GuardKind,
}

impl Drop for RetentionGuard {
    fn drop(&mut self) {
        match self.kind {
            GuardKind::Child => {
                let previous = self.group.active.fetch_sub(1, Ordering::AcqRel);
                debug_assert!(previous > 0);
                self.group.maybe_cleanup();
            }
            GuardKind::Context => self.group.cleanup_finished.store(true, Ordering::Release),
        }
        self.group.changed.notify_waiters();
    }
}

struct RetainedFuture {
    inner: Option<ChildFuture>,
    guard: Option<RetentionGuard>,
}

impl RetainedFuture {
    fn new(inner: ChildFuture, group: Arc<Group>, kind: GuardKind) -> Self {
        Self {
            inner: Some(inner),
            guard: Some(RetentionGuard { group, kind }),
        }
    }

    fn destroy_inner(&mut self) -> bool {
        let Some(inner) = self.inner.take() else {
            return false;
        };
        match catch_unwind(AssertUnwindSafe(|| drop(inner))) {
            Ok(()) => false,
            Err(payload) => {
                destroy_panic(payload);
                true
            }
        }
    }
}

impl Future for RetainedFuture {
    type Output = Outcome;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Outcome> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.inner
                .as_mut()
                .expect("completed child polled")
                .as_mut()
                .poll(cx)
        }));
        match result {
            Ok(Poll::Pending) => Poll::Pending,
            Ok(Poll::Ready(result)) => {
                let drop_panicked = self.destroy_inner();
                let outcome = if drop_panicked {
                    Outcome::Panicked
                } else {
                    match result {
                        Ok(()) => Outcome::Normal,
                        Err(error) => {
                            let (message, truncated) = super::bounded_text(&error.message, 1024);
                            Outcome::Failed(
                                Error {
                                    kind: error.kind,
                                    message,
                                },
                                truncated,
                            )
                        }
                    }
                };
                drop(self.guard.take());
                Poll::Ready(outcome)
            }
            Err(payload) => {
                self.destroy_inner();
                destroy_panic(payload);
                drop(self.guard.take());
                Poll::Ready(Outcome::Panicked)
            }
        }
    }
}

impl Drop for RetainedFuture {
    fn drop(&mut self) {
        if self.destroy_inner() {
            if let Some(guard) = &self.guard {
                guard
                    .group
                    .remember_failure(Error::internal("registered child destructor panicked"));
            }
        }
        drop(self.guard.take());
    }
}

pub(super) struct SpawningGuard(pub Arc<Group>);
impl Drop for SpawningGuard {
    fn drop(&mut self) {
        self.0.spawning_done.store(true, Ordering::Release);
        self.0.maybe_cleanup();
    }
}

impl fmt::Debug for Group {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SupervisorGroup")
            .field("started", &self.started.load(Ordering::Acquire))
            .field("stopping", &self.stop.load(Ordering::Acquire))
            .field("active_children", &self.active.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}
