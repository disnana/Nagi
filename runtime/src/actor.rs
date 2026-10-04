//! Generic native supervision with owned state transitions and bounded mailboxes.
//! Registered children run on independent Tokio tasks; messages do not spawn
//! tasks or erase their handler future. See the ownership/deadline contracts in
//! the language reference before automatically retrying a timed-out write.

pub mod charge;
mod lifecycle;
use crate::Error;
pub use charge::{charged_bytes, ChargeError, ChargeOwned, ChargeWalk};
use futures_util::{stream::FuturesUnordered, StreamExt};
use lifecycle::{ChildFuture, Group, Outcome, SpawningGuard};
#[cfg(test)]
pub(crate) use lifecycle::{Phase, Snapshot};
use std::{
    collections::VecDeque,
    fmt,
    future::{poll_fn, Future},
    mem::size_of,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex, Weak,
    },
    time::Duration,
};
use tokio::{
    sync::{broadcast, mpsc, oneshot, watch, OwnedSemaphorePermit, Semaphore, TryAcquireError},
    time::{timeout, timeout_at, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestartPolicy(u8);
impl RestartPolicy {
    pub const TEMPORARY: Self = Self(0);
    pub const TRANSIENT: Self = Self(1);
    pub const PERMANENT: Self = Self(2);
    fn restart(self, failed: bool) -> bool {
        self == Self::PERMANENT || (failed && self == Self::TRANSIENT)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallKind(u8);
impl CallKind {
    pub const NOT_READY: Self = Self(0);
    pub const MAILBOX_FULL: Self = Self(1);
    pub const MAILBOX_TIMEOUT: Self = Self(2);
    pub const MESSAGE_TOO_LARGE: Self = Self(3);
    pub const REPLY_TOO_LARGE: Self = Self(4);
    pub const STOPPED: Self = Self(5);
    pub const RESTARTING: Self = Self(6);
    pub const REPLY_LOST: Self = Self(7);
    pub const REPLY_TIMEOUT: Self = Self(8);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventKind(u8);
impl EventKind {
    pub const STARTING: Self = Self(0);
    pub const STARTED: Self = Self(1);
    pub const FAILED: Self = Self(2);
    pub const PANICKED: Self = Self(3);
    pub const RESTART_SCHEDULED: Self = Self(4);
    pub const STOPPED: Self = Self(5);
    pub const INTENSITY_EXCEEDED: Self = Self(6);
    pub const SHUTDOWN: Self = Self(7);
    pub const LAGGED: Self = Self(8);
    pub const READY: Self = Self(9);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitKind(u8);
impl WaitKind {
    pub const TIMEOUT: Self = Self(0);
    pub const INVALID_TIMEOUT: Self = Self(1);
}
#[derive(Debug)]
pub struct WaitError {
    kind: WaitKind,
    message: &'static str,
}
impl WaitError {
    pub fn kind(&self) -> WaitKind {
        self.kind
    }
    pub fn message(&self) -> &str {
        self.message
    }
}

pub struct CallError {
    kind: CallKind,
    message: &'static str,
}
impl CallError {
    fn new(kind: CallKind, message: &'static str) -> Self {
        Self { kind, message }
    }
    pub fn kind(&self) -> CallKind {
        self.kind
    }
    pub fn message(&self) -> &str {
        self.message
    }
}
impl fmt::Debug for CallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CallError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct Event {
    child_id: i64,
    child_name: String,
    generation: i64,
    kind: EventKind,
    message: String,
    truncated: bool,
    lost_events: i64,
}
impl Event {
    fn new(id: i64, name: &str, generation: i64, kind: EventKind, message: &str) -> Self {
        let (message, truncated) = bounded_text(message, 1024);
        Self {
            child_id: id,
            child_name: name.into(),
            generation,
            kind,
            message,
            truncated,
            lost_events: 0,
        }
    }
    pub fn child_id(&self) -> i64 {
        self.child_id
    }
    pub fn child_name(&self) -> &str {
        &self.child_name
    }
    pub fn generation(&self) -> i64 {
        self.generation
    }
    pub fn kind(&self) -> EventKind {
        self.kind
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn truncated(&self) -> bool {
        self.truncated
    }
    pub fn lost_events(&self) -> i64 {
        self.lost_events
    }
}
impl fmt::Debug for Event {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Event")
            .field("child_id", &self.child_id)
            .field("generation", &self.generation)
            .field("kind", &self.kind)
            .field("truncated", &self.truncated)
            .field("lost_events", &self.lost_events)
            .finish_non_exhaustive()
    }
}

fn bounded_text(text: &str, limit: usize) -> (String, bool) {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].into(), end != text.len())
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    children: usize,
    events: usize,
    restarts: usize,
    window: Duration,
    shutdown: Duration,
    restart_delay: Duration,
}
#[derive(Clone, Copy, Debug)]
pub struct ActorOptions {
    messages: usize,
    message_bytes: u32,
    reply_bytes: usize,
    startup: Duration,
    policy: RestartPolicy,
}

fn positive_duration(value: i64) -> Result<Duration, Error> {
    let milliseconds =
        u64::try_from(value).map_err(|_| Error::invalid("timeout must be positive"))?;
    if milliseconds == 0 {
        return Err(Error::invalid("timeout must be positive"));
    }
    let duration = Duration::from_millis(milliseconds);
    Instant::now()
        .checked_add(duration)
        .ok_or_else(|| Error::invalid("timeout is too large"))?;
    Ok(duration)
}
fn bounded_count(value: i64) -> Result<usize, Error> {
    let value = usize::try_from(value).map_err(|_| Error::invalid("capacity must be positive"))?;
    if !(1..=65_536).contains(&value) {
        return Err(Error::invalid("capacity must be 1..65536"));
    }
    Ok(value)
}

pub fn default_options() -> Options {
    Options {
        children: 64,
        events: 256,
        restarts: 5,
        window: Duration::from_secs(10),
        shutdown: Duration::from_secs(10),
        restart_delay: Duration::from_millis(10),
    }
}
pub fn options(
    children: i64,
    events: i64,
    restarts: i64,
    window_ms: i64,
    shutdown_ms: i64,
) -> Result<Options, Error> {
    let restart_count = usize::try_from(restarts)
        .map_err(|_| Error::invalid("restart count must be nonnegative"))?;
    if restart_count > 65_536 {
        return Err(Error::invalid("restart count exceeds 65536"));
    }
    Ok(Options {
        children: bounded_count(children)?,
        events: bounded_count(events)?,
        restarts: restart_count,
        window: positive_duration(window_ms)?,
        shutdown: positive_duration(shutdown_ms)?,
        restart_delay: Duration::from_millis(10),
    })
}
pub fn restart_delay(mut options: Options, milliseconds: i64) -> Result<Options, Error> {
    let value = u64::try_from(milliseconds)
        .map_err(|_| Error::invalid("restart delay must be nonnegative"))?;
    let duration = Duration::from_millis(value);
    Instant::now()
        .checked_add(duration)
        .ok_or_else(|| Error::invalid("restart delay is too large"))?;
    options.restart_delay = duration;
    Ok(options)
}
pub fn default_actor_options() -> ActorOptions {
    ActorOptions {
        messages: 64,
        message_bytes: 1_048_576,
        reply_bytes: 1_048_576,
        startup: Duration::from_secs(5),
        policy: RestartPolicy::TRANSIENT,
    }
}
pub fn actor_options(
    messages: i64,
    message_bytes: i64,
    reply_bytes: i64,
    startup_ms: i64,
    policy: RestartPolicy,
) -> Result<ActorOptions, Error> {
    fn byte_limit(bytes: i64) -> Result<u32, Error> {
        let bytes = u32::try_from(bytes).map_err(|_| Error::invalid("byte limit must fit u32"))?;
        if bytes == 0 || bytes as usize > Semaphore::MAX_PERMITS {
            return Err(Error::invalid("invalid byte limit"));
        }
        Ok(bytes)
    }
    Ok(ActorOptions {
        messages: bounded_count(messages)?,
        message_bytes: byte_limit(message_bytes)?,
        reply_bytes: byte_limit(reply_bytes)? as usize,
        startup: positive_duration(startup_ms)?,
        policy,
    })
}

type Runner<C> = Arc<dyn Fn(Arc<C>, Arc<Group>, usize) -> ChildFuture + Send + Sync>;
pub struct Supervisor<C: Send + Sync + 'static> {
    context: Option<Arc<C>>,
    group: Arc<Group>,
    specs: Mutex<Vec<Runner<C>>>,
}
pub fn supervisor<C: Send + Sync + 'static>(context: C, options: Options) -> Supervisor<C> {
    let context = Arc::new(context);
    let group = Group::new(Arc::clone(&context), options);
    Supervisor {
        context: Some(context),
        group,
        specs: Mutex::new(Vec::with_capacity(options.children)),
    }
}
impl<C: Send + Sync + 'static> fmt::Debug for Supervisor<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Supervisor")
            .field("group", &self.group)
            .finish_non_exhaustive()
    }
}
impl<C: Send + Sync + 'static> Supervisor<C> {
    fn release_context(&mut self) {
        struct Release(Arc<Group>);
        impl Drop for Release {
            fn drop(&mut self) {
                self.0.release_owner_context();
            }
        }
        let factories = std::mem::take(&mut *self.specs.lock().unwrap());
        let context = self.context.take();
        // Unconditional release also covers a native factory closure's Drop.
        // Keep the lease through its full destruction/panic payload teardown.
        let release = Release(Arc::clone(&self.group));
        if lifecycle::destroy_owned(factories) {
            self.group
                .remember_failure(Error::internal("native factory destructor panicked"));
        }
        if lifecycle::destroy_owned(context) {
            self.group.remember_failure(Error::internal(
                "owner context reference destructor panicked",
            ));
        }
        drop(release);
    }
}
impl<C: Send + Sync + 'static> Drop for Supervisor<C> {
    fn drop(&mut self) {
        self.group.owner_present.store(false, Ordering::Release);
        self.group.request_stop();
        self.release_context();
        if !self.group.started.load(Ordering::Acquire) {
            self.group.unstarted_cleanup();
        }
    }
}

pub struct Control {
    group: Arc<Group>,
    events: tokio::sync::Mutex<broadcast::Receiver<Arc<Event>>>,
}
impl fmt::Debug for Control {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Control")
            .field("group", &self.group)
            .finish_non_exhaustive()
    }
}
fn new_control(group: Arc<Group>) -> Control {
    Control {
        events: tokio::sync::Mutex::new(group.subscribe()),
        group,
    }
}
pub fn control<C: Send + Sync + 'static>(supervisor: &Supervisor<C>) -> Control {
    new_control(Arc::clone(&supervisor.group))
}
pub fn clone_control(control: &Control) -> Control {
    new_control(Arc::clone(&control.group))
}

impl Control {
    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> Snapshot {
        self.group.snapshot()
    }
    #[cfg(test)]
    pub(crate) fn request_stop(&self) {
        self.group.request_stop();
    }
    pub(crate) async fn wait_complete(&self) -> Result<(), Error> {
        if !self.group.started.load(Ordering::Acquire) {
            return Err(Error::invalid("supervisor has not been run"));
        }
        let _observer = self.group.drainer.lock().await;
        self.group.wait_terminal().await
    }
}
pub async fn shutdown(control: &Control) -> Result<(), Error> {
    if !control.group.started.load(Ordering::Acquire) {
        return Err(Error::invalid("supervisor has not been run"));
    }
    control.group.request_stop();
    match timeout(control.group.options.shutdown, control.wait_complete()).await {
        Ok(result) => result,
        Err(_) => Err(lifecycle::incomplete()),
    }
}
pub async fn next_event(control: &Control) -> Result<Option<Event>, Error> {
    Ok(receive_event(control).await)
}
async fn receive_event(control: &Control) -> Option<Event> {
    let mut events = control.events.lock().await;
    let result = loop {
        let changed = control.group.changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        if control.group.started.load(Ordering::Acquire)
            && (control.group.stop.load(Ordering::Acquire)
                || !control.group.owner_present.load(Ordering::Acquire))
        {
            // An event-only surviving observer can reap a canceled owner's
            // retained ledger. Canceling this select never removes join
            // records; the same election is shared with run/shutdown.
            tokio::select! {
                result = events.recv() => break result,
                _ = control.wait_complete() => break events.recv().await,
            }
        } else {
            tokio::select! {
                result = events.recv() => break result,
                _ = &mut changed => {},
            }
        }
    };
    match result {
        Ok(event) => Some((*event).clone()),
        Err(broadcast::error::RecvError::Closed) => None,
        Err(broadcast::error::RecvError::Lagged(lost)) => {
            let mut event = Event::new(0, "", 0, EventKind::LAGGED, "");
            event.lost_events = lost.min(i64::MAX as u64) as i64;
            Some(event)
        }
    }
}

/// Wait at most a positive number of milliseconds, including receiver-lock
/// contention. A timeout does not consume an event or stop the supervisor.
/// Like other Tokio deadlines, this cannot preempt blocking native work.
pub async fn next_event_timeout(
    control: &Control,
    timeout_ms: i64,
) -> Result<Option<Event>, WaitError> {
    let invalid = || WaitError {
        kind: WaitKind::INVALID_TIMEOUT,
        message: "event timeout must be 1..4294967295 ms with a representable deadline",
    };
    let milliseconds = u32::try_from(timeout_ms).map_err(|_| invalid())?;
    if milliseconds == 0 {
        return Err(invalid());
    }
    let duration = Duration::from_millis(u64::from(milliseconds));
    let deadline = Instant::now().checked_add(duration).ok_or_else(invalid)?;
    timeout_at(deadline, receive_event(control))
        .await
        .map_err(|_| WaitError {
            kind: WaitKind::TIMEOUT,
            message: "supervisor event wait deadline exceeded",
        })
}

// Pending -> notified -> inactive. The guard invalidates escaped native
// tokens when their factory finishes or is canceled; the token itself retains
// neither the group nor its user context.
const READY_PENDING: u8 = 0;
const READY_NOTIFIED: u8 = 1;
const READY_INACTIVE: u8 = 2;
struct ReadyState {
    group: Weak<Group>,
    index: usize,
    generation: i64,
    status: AtomicU8,
}
pub struct TaskReady {
    state: Arc<ReadyState>,
}
impl fmt::Debug for TaskReady {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TaskReady")
            .field("generation", &self.state.generation)
            .finish_non_exhaustive()
    }
}
struct ReadyGuard(Arc<ReadyState>);
impl Drop for ReadyGuard {
    fn drop(&mut self) {
        self.0.status.store(READY_INACTIVE, Ordering::Release);
    }
}

/// Publish READY once for this task generation, after application-defined
/// initialization. Duplicate, stopped and no-longer-active tokens are invalid.
pub fn mark_ready(signal: &TaskReady) -> Result<(), Error> {
    let group = signal
        .state
        .group
        .upgrade()
        .ok_or_else(|| Error::invalid("task generation is no longer active"))?;
    group.task_ready(
        signal.state.index,
        signal.state.generation,
        &signal.state.status,
    )
}

pub fn task_with_ready<C, F, Fut>(
    supervisor: &Supervisor<C>,
    name: &str,
    factory: F,
    policy: RestartPolicy,
) -> Result<(), Error>
where
    C: Send + Sync + 'static,
    F: Fn(Arc<C>, TaskReady) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), Error>> + Send + 'static,
{
    let factory = Arc::new(factory);
    let mut specs = supervisor.specs.lock().unwrap();
    let index = supervisor.group.register(name, policy)?;
    debug_assert_eq!(index, specs.len());
    specs.push(Arc::new(move |context, group, index| {
        let factory = Arc::clone(&factory);
        Box::pin(async move {
            if group.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            let state = Arc::new(ReadyState {
                group: Arc::downgrade(&group),
                index,
                generation: group.generation(index),
                status: AtomicU8::new(READY_PENDING),
            });
            let _ready = ReadyGuard(Arc::clone(&state));
            group.child_event(index, EventKind::STARTED, "");
            factory(context, TaskReady { state }).await
        })
    }));
    Ok(())
}

pub fn task<C, F, Fut>(
    supervisor: &Supervisor<C>,
    name: &str,
    factory: F,
    policy: RestartPolicy,
) -> Result<(), Error>
where
    C: Send + Sync + 'static,
    F: Fn(Arc<C>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), Error>> + Send + 'static,
{
    let factory = Arc::new(factory);
    let mut specs = supervisor.specs.lock().unwrap();
    let index = supervisor.group.register(name, policy)?;
    debug_assert_eq!(index, specs.len());
    specs.push(Arc::new(move |context, group, index| {
        let factory = Arc::clone(&factory);
        Box::pin(async move {
            if group.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            group.child_event(index, EventKind::STARTED, "");
            factory(context).await
        })
    }));
    Ok(())
}

pub async fn run<C: Send + Sync + 'static>(mut supervisor: Supervisor<C>) -> Result<(), Error> {
    let origin = tokio::runtime::Handle::try_current()
        .map_err(|_| Error::invalid("Supervisor requires a Tokio runtime"))?;
    supervisor.group.set_origin(origin)?;
    supervisor.group.started.store(true, Ordering::Release);
    let group = Arc::clone(&supervisor.group);
    group.mark_running();
    let spawning = SpawningGuard(Arc::clone(&group));
    let count = supervisor.specs.lock().unwrap().len();
    for index in 0..count {
        spawn_child(&supervisor, index)?;
    }
    drop(spawning);
    let _observer = group.drainer.lock().await;
    let mut timers = FuturesUnordered::new();
    let mut history = VecDeque::<Instant>::with_capacity(group.options.restarts);
    loop {
        if group.stop.load(Ordering::Acquire) {
            drop(timers);
            supervisor.release_context();
            return group.wait_terminal().await;
        }
        if !group.has_service_work() {
            group.request_stop();
            continue;
        }
        enum Wake {
            Finished(usize, Result<Outcome, tokio::task::JoinError>),
            Restart(usize),
            Stop,
        }
        let changed = group.changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        if group.stop.load(Ordering::Acquire) {
            continue;
        }
        let wake = tokio::select! {
            finished = poll_fn(|cx| group.poll_finished(cx)) => Wake::Finished(finished.0, finished.1),
            index = timers.next(), if !timers.is_empty() => Wake::Restart(index.unwrap()),
            _ = &mut changed => Wake::Stop,
        };
        match wake {
            Wake::Stop => {}
            Wake::Restart(index) => {
                if !group.stop.load(Ordering::Acquire) {
                    spawn_child(&supervisor, index)?;
                }
            }
            Wake::Finished(index, outcome) => {
                let (failed, failure) = match outcome {
                    Ok(Outcome::Normal) => (false, None),
                    Ok(Outcome::Failed(error, truncated)) => {
                        group.failure_event(index, &error, truncated);
                        (true, Some(error))
                    }
                    Ok(Outcome::Panicked) => {
                        group.child_event(index, EventKind::PANICKED, "registered child panicked");
                        (true, Some(Error::internal("registered child panicked")))
                    }
                    Err(error) if error.is_cancelled() => {
                        // Native cancellation is distinct from successful
                        // PERMANENT completion and never schedules a restart.
                        if !group.stop.load(Ordering::Acquire) {
                            group.remember_failure(Error::internal(
                                "origin runtime canceled a registered child",
                            ));
                            group.request_stop();
                        }
                        (false, None)
                    }
                    Err(error) => {
                        lifecycle::destroy_panic(error.into_panic());
                        (true, Some(Error::internal("native child cleanup panicked")))
                    }
                };
                if group.stop.load(Ordering::Acquire) {
                    // The record has already been consumed. A real failure
                    // racing explicit shutdown must remain a terminal failure;
                    // cancellation itself is not a restart/failure reason.
                    if let Some(error) = failure {
                        group.remember_failure(error);
                    }
                    group.child_event(index, EventKind::STOPPED, "");
                    continue;
                }
                if !group.policy(index).restart(failed) {
                    if let Some(error) = failure {
                        group.remember_failure(error);
                    }
                    group.child_event(index, EventKind::STOPPED, "");
                    continue;
                }
                let now = Instant::now();
                // Window is (now-window, now]; exact boundary expires.
                while history
                    .front()
                    .is_some_and(|at| now.duration_since(*at) >= group.options.window)
                {
                    history.pop_front();
                }
                if history.len() >= group.options.restarts {
                    group
                        .remember_failure(Error::internal("supervisor restart intensity exceeded"));
                    group.child_event(
                        index,
                        EventKind::INTENSITY_EXCEEDED,
                        "supervisor restart intensity exceeded",
                    );
                    group.request_stop();
                    continue;
                }
                history.push_back(now);
                group.pending_restart(index, true);
                group.child_event(index, EventKind::RESTART_SCHEDULED, "");
                let at = now + group.options.restart_delay;
                timers.push(async move {
                    tokio::time::sleep_until(at).await;
                    index
                });
            }
        }
    }
}

fn spawn_child<C: Send + Sync + 'static>(
    supervisor: &Supervisor<C>,
    index: usize,
) -> Result<(), Error> {
    let runner = Arc::clone(&supervisor.specs.lock().unwrap()[index]);
    let context = Arc::clone(
        supervisor
            .context
            .as_ref()
            .expect("live supervisor context"),
    );
    // Calling the erased private runner only constructs an async body. Actual
    // user factory invocation happens after its native retention guard exists.
    let inner = runner(context, Arc::clone(&supervisor.group), index);
    supervisor.group.spawn(index, inner)?;
    Ok(())
}

pub struct Turn<S, R, E> {
    state: S,
    reply: Result<R, E>,
}
impl<S, R, E> fmt::Debug for Turn<S, R, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Turn").finish_non_exhaustive()
    }
}
pub fn turn<S, R, E>(state: S, reply: Result<R, E>) -> Turn<S, R, E> {
    Turn { state, reply }
}

struct Envelope<M, R, E> {
    message: M,
    reply: ReplyPort<R, E>,
    _count: OwnedSemaphorePermit,
    _bytes: OwnedSemaphorePermit,
}

struct CompletedReply<R, E> {
    completed_at: Instant,
    result: Result<Result<R, E>, CallError>,
}

struct ReplyPort<R, E>(Option<oneshot::Sender<CompletedReply<R, E>>>);

impl<R, E> ReplyPort<R, E> {
    fn send(mut self, result: Result<Result<R, E>, CallError>) {
        let completed = CompletedReply {
            completed_at: Instant::now(),
            result,
        };
        let _ = self
            .0
            .take()
            .expect("one reply per accepted call")
            .send(completed);
    }
}

impl<R, E> Drop for ReplyPort<R, E> {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            // Outer handler failure, native panic and cancellation cannot
            // silently turn a late buffered failure into an on-time reply.
            let _ = sender.send(CompletedReply {
                completed_at: Instant::now(),
                result: Err(CallError::new(
                    CallKind::REPLY_LOST,
                    "actor reply lost; accepted work is not automatically replayed",
                )),
            });
        }
    }
}

async fn receive_reply<R, E>(
    deadline: Instant,
    receiver: oneshot::Receiver<CompletedReply<R, E>>,
) -> Result<Result<R, E>, CallError> {
    // Tokio polls ready receivers before its timer. The production timestamp
    // keeps an early reply valid after caller scheduling delay, while an
    // already-buffered late reply still observes the acceptance deadline.
    let completed = timeout_at(deadline, receiver)
        .await
        .map_err(|_| {
            CallError::new(
                CallKind::REPLY_TIMEOUT,
                "actor reply deadline exceeded; accepted work may have completed",
            )
        })?
        .map_err(|_| {
            CallError::new(
                CallKind::REPLY_LOST,
                "actor reply lost; accepted work is not automatically replayed",
            )
        })?;
    if completed.completed_at >= deadline {
        return Err(CallError::new(
            CallKind::REPLY_TIMEOUT,
            "actor reply was produced after its deadline; accepted work may have completed",
        ));
    }
    completed.result
}
enum ActorState<M, R, E> {
    Pending,
    Restarting,
    Ready(mpsc::Sender<Envelope<M, R, E>>),
    Stopped,
}
impl<M, R, E> Clone for ActorState<M, R, E> {
    fn clone(&self) -> Self {
        match self {
            Self::Pending => Self::Pending,
            Self::Restarting => Self::Restarting,
            Self::Ready(sender) => Self::Ready(sender.clone()),
            Self::Stopped => Self::Stopped,
        }
    }
}
struct ActorHub<M, R, E> {
    ready: watch::Sender<ActorState<M, R, E>>,
    count: Arc<Semaphore>,
    bytes: Arc<Semaphore>,
    group: Arc<Group>,
    options: ActorOptions,
}
struct ActorStatusGuard<M, R, E>(Arc<ActorHub<M, R, E>>);
impl<M, R, E> Drop for ActorStatusGuard<M, R, E> {
    fn drop(&mut self) {
        let next = if self.0.group.stop.load(Ordering::Acquire)
            || self.0.options.policy == RestartPolicy::TEMPORARY
        {
            ActorState::Stopped
        } else {
            ActorState::Restarting
        };
        self.0.ready.send_replace(next);
    }
}

pub struct Actor<M, R, E> {
    hub: Arc<ActorHub<M, R, E>>,
}
impl<M, R, E> fmt::Debug for Actor<M, R, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Actor").finish_non_exhaustive()
    }
}
pub fn clone_actor<M, R, E>(actor: &Actor<M, R, E>) -> Actor<M, R, E> {
    Actor {
        hub: Arc::clone(&actor.hub),
    }
}

pub fn register<S, M, R, E, C, F, FFut, H, HFut>(
    supervisor: &Supervisor<C>,
    name: &str,
    factory: F,
    handler: H,
    options: ActorOptions,
) -> Result<Actor<M, R, E>, Error>
where
    S: Send + 'static,
    M: ChargeOwned + Send + 'static,
    R: ChargeOwned + Send + 'static,
    E: ChargeOwned + Send + 'static,
    C: Send + Sync + 'static,
    F: Fn(Arc<C>) -> FFut + Send + Sync + 'static,
    FFut: Future<Output = Result<S, Error>> + Send + 'static,
    H: Fn(S, M) -> HFut + Send + Sync + 'static,
    HFut: Future<Output = Result<Turn<S, R, E>, Error>> + Send + 'static,
{
    let (ready, _) = watch::channel(ActorState::Pending);
    let hub = Arc::new(ActorHub {
        ready,
        count: Arc::new(Semaphore::new(options.messages)),
        bytes: Arc::new(Semaphore::new(options.message_bytes as usize)),
        group: Arc::clone(&supervisor.group),
        options,
    });
    let factory = Arc::new(factory);
    let handler = Arc::new(handler);
    let child_hub = Arc::clone(&hub);
    let mut specs = supervisor.specs.lock().unwrap();
    let index = supervisor.group.register(name, options.policy)?;
    debug_assert_eq!(index, specs.len());
    specs.push(Arc::new(move |context, group, index| {
        let factory = Arc::clone(&factory);
        let handler = Arc::clone(&handler);
        let hub = Arc::clone(&child_hub);
        Box::pin(async move {
            let _status = ActorStatusGuard(Arc::clone(&hub));
            if group.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            let startup = Instant::now()
                .checked_add(options.startup)
                .ok_or_else(|| Error::invalid("actor startup deadline is not representable"))?;
            let mut state = timeout_at(startup, factory(context))
                .await
                .map_err(|_| Error::internal("actor startup deadline exceeded"))??;
            // Native factory construction/poll can block and return ready
            // after its deadline. Tokio polls a ready operation before its
            // timer; reject that state before publishing admission as ready.
            if Instant::now() >= startup {
                return Err(Error::internal("actor startup deadline exceeded"));
            }
            let (sender, mut receiver) = mpsc::channel::<Envelope<M, R, E>>(options.messages);
            if group.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            hub.ready.send_replace(ActorState::Ready(sender));
            group.child_event(index, EventKind::STARTED, "");
            while let Some(envelope) = receiver.recv().await {
                let Envelope {
                    message,
                    reply,
                    _count,
                    _bytes,
                } = envelope;
                let transition = handler(state, message).await?;
                state = transition.state;
                let response =
                    if charged_bytes(&transition.reply, 0, options.reply_bytes, None).is_ok() {
                        Ok(transition.reply)
                    } else {
                        Err(CallError::new(
                            CallKind::REPLY_TOO_LARGE,
                            "actor business reply exceeds its byte/work limit",
                        ))
                    };
                reply.send(response);
                // The input reservation covers queued and in-flight work,
                // including handler/result validation; caller timeout never
                // releases it while this envelope remains owned here.
                drop((_count, _bytes));
            }
            Ok(())
        })
    }));
    Ok(Actor { hub })
}

impl<M: Send + 'static, R: Send + 'static, E: Send + 'static> Actor<M, R, E> {
    fn refusal(&self) -> CallError {
        if self.hub.group.stop.load(Ordering::Acquire) {
            CallError::new(CallKind::STOPPED, "supervisor is stopping")
        } else {
            match &*self.hub.ready.borrow() {
                ActorState::Restarting => {
                    CallError::new(CallKind::RESTARTING, "actor is restarting")
                }
                _ => CallError::new(CallKind::STOPPED, "actor generation stopped"),
            }
        }
    }

    async fn admission_wait<F: Future>(
        &self,
        deadline: Instant,
        operation: F,
        generation: Option<&mpsc::Sender<Envelope<M, R, E>>>,
    ) -> Result<F::Output, CallError> {
        tokio::pin!(operation);
        loop {
            let changed = self.hub.group.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.hub.group.stop.load(Ordering::Acquire) {
                return Err(self.refusal());
            }
            if Instant::now() >= deadline {
                return Err(CallError::new(
                    CallKind::MAILBOX_TIMEOUT,
                    "actor admission deadline exceeded",
                ));
            }
            if generation.is_some_and(|sender| sender.is_closed()) {
                return Err(self.refusal());
            }
            tokio::select! {
                _ = &mut changed => {},
                _ = async { if let Some(sender) = generation { sender.closed().await } else { std::future::pending::<()>().await } } => {
                    return Err(self.refusal());
                },
                result = timeout_at(deadline, &mut operation) => {
                    let value = result.map_err(|_| CallError::new(CallKind::MAILBOX_TIMEOUT, "actor admission deadline exceeded"))?;
                    if Instant::now() >= deadline {
                        return Err(CallError::new(CallKind::MAILBOX_TIMEOUT, "actor admission deadline exceeded"));
                    }
                    return Ok(value);
                }
            }
        }
    }

    async fn sender(
        &self,
        deadline: Option<Instant>,
    ) -> Result<mpsc::Sender<Envelope<M, R, E>>, CallError> {
        let mut receiver: Option<watch::Receiver<ActorState<M, R, E>>> = None;
        loop {
            if self.hub.group.stop.load(Ordering::Acquire) {
                return Err(self.refusal());
            }
            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                return Err(CallError::new(
                    CallKind::MAILBOX_TIMEOUT,
                    "actor admission deadline exceeded",
                ));
            }
            let status = match &receiver {
                Some(receiver) => receiver.borrow().clone(),
                None => self.hub.ready.borrow().clone(),
            };
            match status {
                ActorState::Ready(sender) if !sender.is_closed() => return Ok(sender),
                ActorState::Stopped => return Err(self.refusal()),
                ActorState::Restarting if deadline.is_none() => {
                    return Err(CallError::new(CallKind::RESTARTING, "actor is restarting"))
                }
                ActorState::Pending if deadline.is_none() => {
                    return Err(CallError::new(
                        CallKind::NOT_READY,
                        "actor factory has not completed",
                    ))
                }
                ActorState::Ready(_) if deadline.is_none() => return Err(self.refusal()),
                _ => {}
            }
            let Some(deadline) = deadline else {
                return Err(self.refusal());
            };
            if let Some(receiver) = receiver.as_mut() {
                self.admission_wait(deadline, receiver.changed(), None)
                    .await?
                    .map_err(|_| self.refusal())?;
            } else {
                // Subscribe only when waiting is necessary, then recheck the
                // current state before awaiting. A transition between the
                // initial borrow and subscribe must never become a missed wake.
                receiver = Some(self.hub.ready.subscribe());
            }
        }
    }
}

pub async fn ready<M: Send + 'static, R: Send + 'static, E: Send + 'static>(
    actor: &Actor<M, R, E>,
    timeout_ms: i64,
) -> Result<(), CallError> {
    let deadline = call_deadline(timeout_ms, CallKind::MAILBOX_TIMEOUT)?;
    actor.sender(deadline).await.map(|_| ())
}
fn call_deadline(milliseconds: i64, kind: CallKind) -> Result<Option<Instant>, CallError> {
    let value = u64::try_from(milliseconds)
        .map_err(|_| CallError::new(kind, "timeout must be nonnegative"))?;
    if value == 0 {
        return Ok(None);
    }
    Instant::now()
        .checked_add(Duration::from_millis(value))
        .map(Some)
        .ok_or_else(|| CallError::new(kind, "timeout is too large"))
}
pub async fn call<M, R, E>(
    actor: &Actor<M, R, E>,
    message: M,
    mailbox_ms: i64,
    reply_ms: i64,
) -> Result<Result<R, E>, CallError>
where
    M: ChargeOwned + Send + 'static,
    R: ChargeOwned + Send + 'static,
    E: ChargeOwned + Send + 'static,
{
    // One admission deadline starts before validation/charging/readiness.
    let deadline = call_deadline(mailbox_ms, CallKind::MAILBOX_TIMEOUT)?;
    let reply = positive_duration(reply_ms).map_err(|_| {
        CallError::new(
            CallKind::REPLY_TIMEOUT,
            "reply timeout must be positive and representable",
        )
    })?;
    let metadata = size_of::<Envelope<M, R, E>>() - size_of::<M>();
    let bytes = charged_bytes(
        &message,
        metadata,
        actor.hub.options.message_bytes as usize,
        deadline.map(Instant::into_std),
    )
    .map_err(|error| {
        if matches!(error, ChargeError::Deadline) {
            CallError::new(
                CallKind::MAILBOX_TIMEOUT,
                "actor admission deadline exceeded during validation",
            )
        } else {
            CallError::new(
                CallKind::MESSAGE_TOO_LARGE,
                "actor message exceeds its byte/depth/work limit",
            )
        }
    })? as u32;
    let sender = actor.sender(deadline).await?;
    // Tokio assigns released permits to existing waiters before making them
    // available to try_acquire. Immediate admission therefore keeps that
    // fairness while avoiding waiter/timer setup when capacity is free.
    let count = match Arc::clone(&actor.hub.count).try_acquire_owned() {
        Ok(permit) => permit,
        Err(TryAcquireError::Closed) => return Err(actor.refusal()),
        Err(TryAcquireError::NoPermits) => match deadline {
            None => {
                return Err(CallError::new(
                    CallKind::MAILBOX_FULL,
                    "actor mailbox count limit reached",
                ))
            }
            Some(deadline) => actor
                .admission_wait(
                    deadline,
                    Arc::clone(&actor.hub.count).acquire_owned(),
                    Some(&sender),
                )
                .await?
                .map_err(|_| actor.refusal())?,
        },
    };
    let byte_permit = match Arc::clone(&actor.hub.bytes).try_acquire_many_owned(bytes) {
        Ok(permit) => permit,
        Err(TryAcquireError::Closed) => return Err(actor.refusal()),
        Err(TryAcquireError::NoPermits) => match deadline {
            None => {
                return Err(CallError::new(
                    CallKind::MAILBOX_FULL,
                    "actor mailbox byte limit reached",
                ))
            }
            Some(deadline) => actor
                .admission_wait(
                    deadline,
                    Arc::clone(&actor.hub.bytes).acquire_many_owned(bytes),
                    Some(&sender),
                )
                .await?
                .map_err(|_| actor.refusal())?,
        },
    };
    if actor.hub.group.stop.load(Ordering::Acquire) || sender.is_closed() {
        return Err(actor.refusal());
    }
    let reservation = match sender.try_reserve_owned() {
        Ok(reservation) => reservation,
        Err(mpsc::error::TrySendError::Closed(_)) => return Err(actor.refusal()),
        Err(mpsc::error::TrySendError::Full(sender)) => match deadline {
            None => {
                return Err(CallError::new(
                    CallKind::MAILBOX_FULL,
                    "actor mailbox queue limit reached",
                ))
            }
            Some(deadline) => {
                let for_wait = sender.clone();
                actor
                    .admission_wait(deadline, sender.reserve_owned(), Some(&for_wait))
                    .await?
                    .map_err(|_| actor.refusal())?
            }
        },
    };
    if actor.hub.group.stop.load(Ordering::Acquire) {
        return Err(actor.refusal());
    }
    let (reply_sender, receiver) = oneshot::channel();
    // This generation-specific send is the acceptance point. Reply deadline
    // begins here; it never silently rebinds to a different incarnation.
    let accepted = Instant::now();
    if deadline.is_some_and(|deadline| accepted >= deadline) {
        return Err(CallError::new(
            CallKind::MAILBOX_TIMEOUT,
            "actor admission deadline exceeded before acceptance",
        ));
    }
    let reply_deadline = accepted.checked_add(reply).ok_or_else(|| {
        CallError::new(
            CallKind::REPLY_TIMEOUT,
            "actor reply deadline is not representable",
        )
    })?;
    reservation.send(Envelope {
        message,
        reply: ReplyPort(Some(reply_sender)),
        _count: count,
        _bytes: byte_permit,
    });
    receive_reply(reply_deadline, receiver).await
}

pub async fn yield_now() {
    tokio::task::yield_now().await;
}

#[cfg(test)]
mod lifecycle_adversarial_tests;

#[cfg(test)]
mod tests;
