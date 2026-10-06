//! TaskScope-owned result handles. Result delivery always follows actual child join.

use std::{
    cell::Cell,
    collections::HashMap,
    future::Future,
    marker::PhantomData,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};
use tokio::{
    sync::oneshot,
    task::{AbortHandle, Id, JoinError, JoinSet},
};

const LIVE: u8 = 0;
const RECEIVING: u8 = 1;
const RECEIVED: u8 = 2;
const ABANDONED: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskFailureKind {
    Panicked,
    Cancelled,
    LegacyError,
    Internal,
}
// Keep the native legacy Error opaque and structured, rather than reconstructing
// kind/message from Display text. Fault clones share this cause; cost is unmeasured.
#[derive(Clone, Debug)]
struct LegacyCause(Arc<crate::Error>);
impl PartialEq for LegacyCause {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(&self.0.kind) == std::mem::discriminant(&other.0.kind)
            && self.0.message == other.0.message
    }
}
impl Eq for LegacyCause {}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Fault {
    kind: TaskFailureKind,
    message: String,
    cause: Option<LegacyCause>,
}
impl Fault {
    fn scope_error(&self) -> crate::Error {
        self.legacy_cause()
            .cloned()
            .unwrap_or_else(|| crate::Error::internal(self.message.clone()))
    }
    fn legacy_cause(&self) -> Option<&crate::Error> {
        self.cause.as_ref().map(|cause| cause.0.as_ref())
    }
}
/// An opaque child failure. Receiving it does not acknowledge or clear a scope fault.
///
/// ```compile_fail
/// use nagi_runtime::TaskFailure;
/// fn copy_failure(failure: TaskFailure) { let _ = failure.clone(); }
/// ```
#[derive(Debug)]
pub struct TaskFailure(Fault);
impl TaskFailure {
    pub fn kind(&self) -> TaskFailureKind {
        self.0.kind
    }
    pub fn message(&self) -> &str {
        &self.0.message
    }
}
#[cfg(test)]
#[derive(Debug)]
enum Exit<E> {
    Body(E),
    Task(crate::Error),
}
/// A single-use result handle owned by the TaskScope that spawned it.
/// Rust Drop abandons the output; its scope still owns cancellation and join.
/// Nagi additionally checks scope membership and explicit receive/discard.
///
/// ```compile_fail
/// use nagi_runtime::Task;
/// fn copy_handle(task: Task<u8>) { let _ = task.clone(); }
/// ```
/// ```compile_fail
/// use nagi_runtime::{Task, TaskScope};
/// fn receive_twice(scope: &mut TaskScope, task: Task<u8>) {
///     drop(scope.receive(task));
///     drop(scope.receive(task));
/// }
/// ```
/// ```compile_fail
/// use nagi_runtime::Task;
/// fn require_sync<T: Sync>() {}
/// require_sync::<Task<u8>>();
/// ```
pub struct Task<T> {
    owner: Arc<()>,
    ticket: u64,
    receiver: Option<oneshot::Receiver<T>>,
    receipt: Arc<AtomicU8>,
    unshared: PhantomData<Cell<()>>,
}
#[cfg(test)]
impl<T> Task<T> {
    fn output_ready(&self) -> bool {
        self.receiver.as_ref().is_some_and(|rx| !rx.is_empty())
    }
}
impl<T> Drop for Task<T> {
    fn drop(&mut self) {
        if self.receipt.load(Ordering::Acquire) != RECEIVED {
            // Publish abandonment before dropping a buffered arbitrary T.
            self.receipt.store(ABANDONED, Ordering::Release);
        }
    }
}
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
enum NativeKey {
    Real(Id),
    #[cfg(test)]
    Fake(u64),
}
struct Entry {
    receipt: Arc<AtomicU8>,
    joined: bool,
    requested_cancel: bool,
    abort: Option<AbortHandle>,
}
enum ChildExit {
    Success,
    Legacy(crate::Error),
}
#[cfg(test)]
#[derive(Debug)]
struct Stats {
    joined: usize,
    entries: usize,
    unjoined: usize,
    failed: bool,
}
/// Owns every child join, while typed Task handles own only their output receiver.
/// Child faults remain sticky after receive and every later join/cancel drains
/// actual native joins before returning an Error. Drop only requests abort.
pub struct TaskScope {
    identity: Arc<()>,
    tasks: JoinSet<ChildExit>,
    entries: HashMap<u64, Entry>,
    native_keys: HashMap<Id, NativeKey>,
    unjoined: HashMap<NativeKey, u64>,
    next_ticket: u64,
    joined: usize,
    primary: Option<Fault>,
    related: Vec<Fault>,
}
impl Default for TaskScope {
    fn default() -> Self {
        Self::new()
    }
}
impl TaskScope {
    pub fn new() -> Self {
        Self {
            identity: Arc::new(()),
            tasks: JoinSet::new(),
            entries: HashMap::new(),
            native_keys: HashMap::new(),
            unjoined: HashMap::new(),
            next_ticket: 0,
            joined: 0,
            primary: None,
            related: vec![],
        }
    }
    fn register(
        &mut self,
        future: impl Future<Output = ChildExit> + Send + 'static,
        receipt: Arc<AtomicU8>,
        fake: Option<u64>,
    ) -> u64 {
        let ticket = self.next_ticket;
        self.next_ticket = self
            .next_ticket
            .checked_add(1)
            .expect("task ticket exhausted");
        let abort = self.tasks.spawn(future);
        let id = abort.id();
        #[cfg(test)]
        let key = fake.map_or(NativeKey::Real(id), NativeKey::Fake);
        #[cfg(not(test))]
        let key = {
            debug_assert!(fake.is_none());
            NativeKey::Real(id)
        };
        assert!(
            !self.unjoined.contains_key(&key),
            "duplicate unjoined native key"
        );
        self.native_keys.insert(id, key);
        self.unjoined.insert(key, ticket);
        let requested_cancel = self.primary.is_some();
        if requested_cancel {
            abort.abort();
        }
        self.entries.insert(
            ticket,
            Entry {
                receipt,
                joined: false,
                requested_cancel,
                abort: Some(abort),
            },
        );
        ticket
    }
    pub fn spawn_value<T: Send + 'static>(
        &mut self,
        future: impl Future<Output = T> + Send + 'static,
    ) -> Task<T> {
        self.spawn_value_inner(future, (), None)
    }
    #[cfg(test)]
    fn spawn_with_cleanup<T: Send + 'static, D: Send + 'static>(
        &mut self,
        future: impl Future<Output = T> + Send + 'static,
        cleanup: D,
    ) -> Task<T> {
        self.spawn_value_inner(future, cleanup, None)
    }
    #[cfg(test)]
    fn spawn_fake_id<T: Send + 'static>(
        &mut self,
        future: impl Future<Output = T> + Send + 'static,
        fake: u64,
    ) -> Task<T> {
        self.spawn_value_inner(future, (), Some(fake))
    }
    fn spawn_value_inner<T: Send + 'static, D: Send + 'static>(
        &mut self,
        future: impl Future<Output = T> + Send + 'static,
        cleanup: D,
        fake: Option<u64>,
    ) -> Task<T> {
        let (sender, receiver) = oneshot::channel();
        let receipt = Arc::new(AtomicU8::new(LIVE));
        let ticket = self.register(
            async move {
                let _cleanup = cleanup;
                let value = future.await;
                // A failed send destroys T in the child, without converting business Err.
                let _ = sender.send(value);
                ChildExit::Success
            },
            Arc::clone(&receipt),
            fake,
        );
        Task {
            owner: Arc::clone(&self.identity),
            ticket,
            receiver: Some(receiver),
            receipt,
            unshared: PhantomData,
        }
    }
    pub fn spawn(
        &mut self,
        future: impl Future<Output = Result<(), crate::Error>> + Send + 'static,
    ) {
        self.spawn_legacy_with_cleanup(future, ());
    }
    fn spawn_legacy_with_cleanup<D: Send + 'static>(
        &mut self,
        future: impl Future<Output = Result<(), crate::Error>> + Send + 'static,
        cleanup: D,
    ) {
        self.register(
            async move {
                let _cleanup = cleanup;
                match future.await {
                    Ok(()) => ChildExit::Success,
                    Err(error) => ChildExit::Legacy(error),
                }
            },
            Arc::new(AtomicU8::new(ABANDONED)),
            None,
        );
    }
    #[cfg(test)]
    fn spawn_missing_output<T: Send + 'static>(&mut self) -> Task<T> {
        let (sender, receiver) = oneshot::channel();
        let receipt = Arc::new(AtomicU8::new(LIVE));
        let ticket = self.register(
            async move {
                drop(sender);
                ChildExit::Success
            },
            Arc::clone(&receipt),
            None,
        );
        Task {
            owner: Arc::clone(&self.identity),
            ticket,
            receiver: Some(receiver),
            receipt,
            unshared: PhantomData,
        }
    }
    fn request_abort(&mut self) {
        for ticket in self.unjoined.values() {
            self.entries
                .get_mut(ticket)
                .expect("unjoined entry")
                .requested_cancel = true;
        }
        self.tasks.abort_all();
    }
    fn fault(&mut self, fault: Fault) {
        if self.primary.is_none() {
            self.primary = Some(fault);
            self.request_abort();
        } else {
            // Existing children were aborted at primary publication. New children
            // are registered as requested-cancelled, so later causes need no sweep.
            self.related.push(fault);
        }
    }
    fn internal(&mut self, message: &str) {
        self.fault(Fault {
            kind: TaskFailureKind::Internal,
            message: message.into(),
            cause: None,
        });
    }
    // The common record/receive/discard path touches one ticket only.
    fn retire_ticket(&mut self, ticket: u64) {
        if self
            .entries
            .get(&ticket)
            .is_some_and(|entry| entry.joined && entry.receipt.load(Ordering::Acquire) >= RECEIVED)
        {
            self.entries.remove(&ticket);
        }
    }
    // A joined handle can be dropped outside a scope operation. Sweep once when
    // drain reaches its end; records contain no arbitrary T or finished Future.
    fn retire_abandoned(&mut self) {
        self.entries
            .retain(|_, entry| !entry.joined || entry.receipt.load(Ordering::Acquire) < RECEIVED);
    }
    // Only this TaskScope consumes native join results. Ready -> accounting has no await,
    // no arbitrary result T destruction, and no user callbacks.
    fn record(&mut self, result: Result<(Id, ChildExit), JoinError>) {
        let id = match &result {
            Ok((id, _)) => *id,
            Err(error) => error.id(),
        };
        let key = self
            .native_keys
            .remove(&id)
            .expect("native ID registered before join polling");
        let ticket = self
            .unjoined
            .remove(&key)
            .expect("native ID maps only to an unjoined ticket");
        let entry = self.entries.get_mut(&ticket).expect("live join entry");
        entry.joined = true;
        // Do not retain native AbortHandles after join: Tokio may reuse native IDs.
        entry.abort = None;
        let requested_cancel = entry.requested_cancel;
        self.joined += 1;
        let mut deferred_join_error = None;
        let fault = match result {
            Ok((_, ChildExit::Success)) => None,
            Ok((_, ChildExit::Legacy(error))) => {
                let message = error.to_string();
                Some(Fault {
                    kind: TaskFailureKind::LegacyError,
                    message,
                    cause: Some(LegacyCause(Arc::new(error))),
                })
            }
            Err(error) => {
                let fault = if error.is_panic() {
                    Some(Fault {
                        kind: TaskFailureKind::Panicked,
                        message: "child panicked".into(),
                        cause: None,
                    })
                } else if error.is_cancelled() && requested_cancel {
                    None
                } else {
                    Some(Fault {
                        kind: TaskFailureKind::Cancelled,
                        message: error.to_string(),
                        cause: None,
                    })
                };
                // Preserve arbitrary panic payload until actual join/cause publication.
                deferred_join_error = Some(error);
                fault
            }
        };
        if let Some(fault) = fault {
            self.fault(fault);
        }
        self.retire_ticket(ticket);
        // Native panic payload destruction is an arbitrary Rust boundary, after record.
        drop(deferred_join_error);
    }
    async fn observe_one(&mut self) -> bool {
        if let Some(result) = self.tasks.join_next_with_id().await {
            self.record(result);
            true
        } else {
            false
        }
    }
    async fn drain(&mut self) {
        while self.observe_one().await {}
        self.retire_abandoned();
    }
    pub fn receive<'a, T: 'a>(
        &'a mut self,
        mut task: Task<T>,
    ) -> impl Future<Output = Result<T, TaskFailure>> + 'a {
        task.receipt.store(RECEIVING, Ordering::Release);
        async move {
            if !Arc::ptr_eq(&self.identity, &task.owner) {
                self.internal("Task belongs to a different TaskScope");
                self.drain().await;
                return Err(TaskFailure(self.primary.clone().unwrap()));
            }
            loop {
                if self.primary.is_some() {
                    if !self.tasks.is_empty() {
                        self.drain().await;
                    }
                    let fault = self.primary.clone().unwrap();
                    let ticket = task.ticket;
                    // No unjoined child remains. Abandon this receipt and retire
                    // only its ticket; repeated sticky receives must not scan
                    // every other live output record.
                    drop(task);
                    self.retire_ticket(ticket);
                    return Err(TaskFailure(fault));
                }
                if self
                    .entries
                    .get(&task.ticket)
                    .is_some_and(|entry| entry.joined)
                {
                    break;
                }
                if !self.observe_one().await {
                    self.internal("missing target join entry");
                }
            }
            match task.receiver.as_mut().unwrap().try_recv() {
                Ok(value) => {
                    task.receipt.store(RECEIVED, Ordering::Release);
                    self.retire_ticket(task.ticket);
                    Ok(value)
                }
                Err(_) => {
                    self.internal("native success without typed output");
                    self.drain().await;
                    Err(TaskFailure(self.primary.clone().unwrap()))
                }
            }
        }
    }
    /// Abandons a result without stopping the child or releasing join ownership.
    pub fn discard<T>(&mut self, task: Task<T>) {
        let correct = Arc::ptr_eq(&self.identity, &task.owner);
        let ticket = task.ticket;
        drop(task);
        if correct {
            self.retire_ticket(ticket);
        } else {
            self.internal("discard belongs to a different TaskScope");
        }
    }
    pub async fn join(&mut self) -> Result<(), crate::Error> {
        self.drain().await;
        self.primary
            .as_ref()
            .map_or(Ok(()), |fault| Err(fault.scope_error()))
    }
    pub async fn cancel(&mut self) -> Result<(), crate::Error> {
        self.request_abort();
        self.join().await
    }
    #[cfg(test)]
    async fn finish<E>(&mut self, body: Result<(), E>) -> Result<(), Exit<E>> {
        match body {
            Err(error) => {
                let _ = self.cancel().await;
                Err(Exit::Body(error))
            }
            Ok(()) => self.join().await.map_err(Exit::Task),
        }
    }
    #[cfg(test)]
    fn stats(&self) -> Stats {
        Stats {
            joined: self.joined,
            entries: self.entries.len(),
            unjoined: self.unjoined.len(),
            failed: self.primary.is_some(),
        }
    }
}
impl Drop for TaskScope {
    fn drop(&mut self) {
        self.request_abort();
    }
}

#[cfg(test)]
mod tests {
    use super::{Exit, Task, TaskFailureKind, TaskScope};
    use crate::Error;
    use futures_util::FutureExt;
    use std::{
        future::{pending, Future},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Condvar, Mutex,
        },
        time::Duration,
    };
    use tokio::sync::{oneshot, Notify};

    #[tokio::test]
    async fn sticky_receive_retires_its_joined_ticket() {
        let mut scope = TaskScope::new();
        let handles: Vec<_> = (0..128).map(|value| scope.spawn_value(async move { value })).collect();
        scope.join().await.unwrap();
        scope.spawn(async { Err(Error::invalid("retirement fault")) });
        assert!(scope.join().await.is_err());
        assert_eq!(scope.stats().entries, 128);
        for (index, handle) in handles.into_iter().enumerate() {
            assert_eq!(scope.receive(handle).await.unwrap_err().message(), "Invalid: retirement fault");
            assert_eq!(scope.stats().entries, 127 - index);
        }
        assert!(scope.join().await.is_err());
        assert_eq!(scope.stats().entries, 0);
    }

    fn assert_scope_error(error: &Error, failure: &super::TaskFailure) {
        assert!(matches!(error.kind, crate::ErrorKind::Internal));
        assert_eq!(error.message, failure.message());
    }
    #[derive(Default)]
    struct Signal {
        flag: AtomicBool,
        changed: Notify,
    }
    impl Signal {
        fn set(&self) {
            self.flag.store(true, Ordering::SeqCst);
            self.changed.notify_waiters();
        }
        async fn wait(&self) {
            loop {
                let changed = self.changed.notified();
                if self.flag.load(Ordering::SeqCst) {
                    return;
                }
                changed.await;
            }
        }
    }
    // child Futureの同期Dropを止める。parentは別runtime threadでgateを解放する。
    #[derive(Default)]
    struct DropGate {
        entered: Signal,
        released: Mutex<bool>,
        changed: Condvar,
        completed: Signal,
        watchdog_failed: AtomicBool,
    }
    impl DropGate {
        fn release(&self) {
            *self.released.lock().unwrap() = true;
            self.changed.notify_all();
        }
        fn block(&self) {
            self.entered.set();
            let released = self.released.lock().unwrap();
            let (released, timeout) = self
                .changed
                .wait_timeout_while(released, Duration::from_secs(10), |ready| !*ready)
                .unwrap();
            if !*released || timeout.timed_out() {
                self.watchdog_failed.store(true, Ordering::SeqCst);
            }
            self.completed.set();
        }
    }
    struct Cleanup(Arc<DropGate>);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            self.0.block();
        }
    }
    struct Release(Arc<DropGate>);
    impl Drop for Release {
        fn drop(&mut self) {
            self.0.release();
            if !std::thread::panicking() {
                assert!(
                    !self.0.watchdog_failed.load(Ordering::SeqCst),
                    "Drop gate watchdog expired"
                );
            }
        }
    }
    async fn watch<F: Future>(future: F) -> Result<F::Output, &'static str> {
        match tokio::time::timeout(
            Duration::from_secs(10),
            std::panic::AssertUnwindSafe(future).catch_unwind(),
        )
        .await
        {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(_)) => Err("bridge observation panicked"),
            Err(_) => Err("bridge watchdog expired"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn heterogeneous_results_and_business_error_keep_sibling_healthy() {
        let mut scope = TaskScope::new();
        let started = Arc::new(Signal::default());
        let (release, receiver) = oneshot::channel();
        let child_started = Arc::clone(&started);
        let sibling = scope.spawn_value(async move {
            child_started.set();
            receiver.await.unwrap();
            88_u64
        });
        let business = scope.spawn_value(async { Err::<String, u8>(9) });
        let unit = scope.spawn_value(async {});
        let owned = scope.spawn_value(async { vec![String::from("owned")] });
        let ready = watch(started.wait()).await;
        let rejection = watch(scope.receive(business)).await;
        let observed_unit = watch(scope.receive(unit)).await;
        let observed_owned = watch(scope.receive(owned)).await;
        let before_release = scope.stats();
        let released = release.send(());
        let observed_sibling = watch(scope.receive(sibling)).await;
        let joined = watch(scope.join()).await;
        assert!(ready.is_ok() && released.is_ok());
        assert!(matches!(rejection, Ok(Ok(Err(9)))));
        assert!(matches!(observed_unit, Ok(Ok(()))));
        assert_eq!(observed_owned.unwrap().unwrap(), vec!["owned"]);
        assert!(!before_release.failed);
        assert_eq!(before_release.unjoined, 1);
        assert_eq!(observed_sibling.unwrap().unwrap(), 88);
        assert!(matches!(joined, Ok(Ok(()))));
        assert_eq!(scope.stats().joined, 4);
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn receiver_notification_cannot_replace_actual_join() {
        let mut scope = TaskScope::new();
        let gate = Arc::new(DropGate::default());
        let _release = Release(Arc::clone(&gate));
        let task = scope.spawn_with_cleanup(async { 21_u64 }, Cleanup(Arc::clone(&gate)));
        let entered = watch(gate.entered.wait()).await;
        let output_ready = task.output_ready();
        let mut receiving = Box::pin(scope.receive(task));
        let observed = futures_util::poll!(&mut receiving);
        let stayed_pending = observed.is_pending();
        drop(receiving);
        gate.release();
        let completed = watch(gate.completed.wait()).await;
        let joined = watch(scope.join()).await;
        assert!(entered.is_ok() && output_ready);
        assert!(completed.is_ok() && matches!(joined, Ok(Ok(()))));
        assert_eq!(scope.stats().joined, 1);
        assert_eq!(scope.stats().entries, 0);
        assert!(
            stayed_pending,
            "typed notification arrived before child Future Drop/join"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn received_fault_remains_sticky_at_scope_exit() {
        let mut scope = TaskScope::new();
        let task: Task<()> = scope.spawn_value(async { panic!("controlled child fault") });
        let received = watch(scope.receive(task)).await;
        let exit = watch(scope.join()).await;
        let repeated = watch(scope.join()).await;
        let received = received.unwrap().unwrap_err();
        assert_eq!(received.kind(), TaskFailureKind::Panicked);
        assert_eq!(scope.primary.as_ref().unwrap(), &received.0);
        assert_scope_error(&exit.unwrap().unwrap_err(), &received);
        assert_scope_error(&repeated.unwrap().unwrap_err(), &received);
        assert_eq!(scope.stats().joined, 1);
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fault_return_waits_for_requested_sibling_cleanup_and_actual_join() {
        let mut scope = TaskScope::new();
        let gate = Arc::new(DropGate::default());
        let _release = Release(Arc::clone(&gate));
        let started = Arc::new(Signal::default());
        let child_started = Arc::clone(&started);
        let sibling = scope.spawn_with_cleanup(
            async move {
                child_started.set();
                pending::<()>().await;
            },
            Cleanup(Arc::clone(&gate)),
        );
        let ready = watch(started.wait()).await;
        let failed: Task<()> = scope.spawn_value(async { panic!("primary") });
        let mut receiving = Box::pin(scope.receive(failed));
        let observed = tokio::select! {
            entered = watch(gate.entered.wait()) => entered.is_ok(),
            _ = &mut receiving => false,
        };
        drop(receiving); // 生存TaskScopeのfault/drain記録は失われてはならない。
        let interrupted = scope.stats();
        scope.discard(sibling);
        gate.release();
        let resumed = watch(scope.join()).await;
        assert!(ready.is_ok() && observed);
        assert!(interrupted.failed && interrupted.unjoined != 0);
        assert!(matches!(
            resumed.unwrap().unwrap_err().kind,
            crate::ErrorKind::Internal
        ));
        assert_eq!(
            scope.primary.as_ref().unwrap().kind,
            TaskFailureKind::Panicked
        );
        assert_eq!(scope.stats().joined, 2);
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unpolled_and_pending_receive_drop_keep_scope_join_responsibility() {
        let mut scope = TaskScope::new();
        let unpolled = scope.spawn_value(pending::<u64>());
        drop(scope.receive(unpolled));
        let target = scope.spawn_value(pending::<u64>());
        let other = scope.spawn_value(async { 7_u64 });
        // targetは完了不能なので、受取がPendingになった後もTaskScopeへ責任が残る。
        let mut receiving = Box::pin(scope.receive(target));
        let stayed_pending = futures_util::poll!(&mut receiving).is_pending();
        drop(receiving);
        let other_value = watch(scope.receive(other)).await;
        let cancelled = watch(scope.cancel()).await;
        assert!(stayed_pending);
        assert_eq!(other_value.unwrap().unwrap(), 7);
        assert!(matches!(cancelled, Ok(Ok(()))));
        assert_eq!(scope.stats().joined, 3);
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelled_drain_can_resume_after_child_drop_gate() {
        let mut scope = TaskScope::new();
        let gate = Arc::new(DropGate::default());
        let _release = Release(Arc::clone(&gate));
        let started = Arc::new(Signal::default());
        let child_started = Arc::clone(&started);
        let task = scope.spawn_with_cleanup(
            async move {
                child_started.set();
                pending::<()>().await;
            },
            Cleanup(Arc::clone(&gate)),
        );
        let ready = watch(started.wait()).await;
        scope.discard(task);
        let mut cancelling = Box::pin(scope.cancel());
        let observed = tokio::select! {
            entered = watch(gate.entered.wait()) => entered.is_ok(),
            _ = &mut cancelling => false,
        };
        drop(cancelling);
        let interrupted = scope.stats();
        gate.release();
        let resumed = watch(scope.cancel()).await;
        let exit = watch(scope.join()).await;
        assert!(ready.is_ok() && observed);
        assert_eq!(interrupted.joined, 0);
        assert_eq!(interrupted.unjoined, 1);
        assert!(matches!(resumed, Ok(Ok(()))) && matches!(exit, Ok(Ok(()))));
        assert_eq!(scope.stats().joined, 1);
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test]
    async fn discard_keeps_fault_and_legacy_error_observation() {
        let mut scope = TaskScope::new();
        let fault: Task<()> = scope.spawn_value(async { panic!("discarded child still faults") });
        scope.discard(fault);
        let failed = watch(scope.join()).await;
        assert!(matches!(
            failed.unwrap().unwrap_err().kind,
            crate::ErrorKind::Internal
        ));
        assert_eq!(
            scope.primary.as_ref().unwrap().kind,
            TaskFailureKind::Panicked
        );
        assert_eq!(scope.stats().entries, 0);
        let mut legacy = TaskScope::new();
        legacy.spawn(async { Err(Error::invalid("legacy terminal")) });
        let failed = watch(legacy.join()).await;
        assert!(matches!(
            failed.unwrap().unwrap_err().kind,
            crate::ErrorKind::Invalid
        ));
        assert_eq!(
            legacy.primary.as_ref().unwrap().kind,
            TaskFailureKind::LegacyError
        );
        assert!(legacy.stats().failed);
        assert_eq!(legacy.stats().joined, 1);
    }

    #[tokio::test]
    async fn fake_native_id_reuse_does_not_mix_unreceived_tickets_and_retire_records() {
        let mut scope = TaskScope::new();
        let first = scope.spawn_fake_id(async { String::from("first") }, 55);
        let joined_first = watch(scope.observe_one()).await;
        let retained = scope.stats();
        let second = scope.spawn_fake_id(async { 42_u64 }, 55);
        let second_value = watch(scope.receive(second)).await;
        let first_value = watch(scope.receive(first)).await;
        let exit = watch(scope.join()).await;
        assert!(joined_first.unwrap());
        assert_eq!(retained.joined, 1);
        assert_eq!(retained.entries, 1);
        assert_eq!(retained.unjoined, 0);
        assert_eq!(second_value.unwrap().unwrap(), 42);
        assert_eq!(first_value.unwrap().unwrap(), "first");
        assert!(matches!(exit, Ok(Ok(()))));
        assert_eq!(scope.stats().entries, 0);
        assert_eq!(scope.stats().joined, 2);
        // 完了履歴を保持しない。業務Err/値の受取を反復してもentryは退役する。
        for value in 0..32_u64 {
            let task = scope.spawn_value(async move { value });
            assert_eq!(watch(scope.receive(task)).await.unwrap().unwrap(), value);
            assert_eq!(scope.stats().entries, 0);
        }
        assert_eq!(scope.stats().joined, 34);
    }

    #[tokio::test]
    async fn target_other_than_first_join_and_body_error_primary_are_preserved() {
        let mut scope = TaskScope::new();
        let first = scope.spawn_value(async { 11_u64 });
        let observed = watch(scope.observe_one()).await;
        let second = scope.spawn_value(async { 12_u64 });
        let second_value = watch(scope.receive(second)).await;
        let first_value = watch(scope.receive(first)).await;
        assert!(observed.unwrap());
        assert_eq!(second_value.unwrap().unwrap(), 12);
        assert_eq!(first_value.unwrap().unwrap(), 11);
        scope.spawn(async { Err(Error::invalid("secondary legacy fault")) });
        // Observe the secondary failure before body Err, so precedence is not vacuous.
        assert!(watch(scope.observe_one()).await.unwrap());
        assert!(scope.stats().failed);
        let finished = watch(scope.finish(Err::<(), _>(String::from("original body error")))).await;
        assert!(
            matches!(finished, Ok(Err(Exit::Body(message))) if message == "original body error")
        );
        assert_eq!(scope.stats().joined, 3);
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn parent_drop_requests_abort_but_does_not_report_child_destruction_complete() {
        let mut scope = TaskScope::new();
        let gate = Arc::new(DropGate::default());
        let _release = Release(Arc::clone(&gate));
        let started = Arc::new(Signal::default());
        let child_started = Arc::clone(&started);
        let task = scope.spawn_with_cleanup(
            async move {
                child_started.set();
                pending::<()>().await;
            },
            Cleanup(Arc::clone(&gate)),
        );
        let ready = watch(started.wait()).await;
        drop(scope);
        let entered = watch(gate.entered.wait()).await;
        let was_complete = gate.completed.flag.load(Ordering::SeqCst);
        drop(task);
        gate.release();
        let completed = watch(gate.completed.wait()).await;
        assert!(ready.is_ok() && entered.is_ok() && completed.is_ok());
        assert!(
            !was_complete,
            "TaskScope Drop pretended to join a blocked child"
        );
    }

    #[derive(Debug)]
    struct Payload {
        events: Arc<Mutex<Vec<(&'static str, std::thread::ThreadId)>>>,
        label: &'static str,
    }
    impl Clone for Payload {
        fn clone(&self) -> Self {
            panic!("bridge must not clone payload")
        }
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            self.events
                .lock()
                .unwrap()
                .push((self.label, std::thread::current().id()));
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn payload_drop_locations_distinguish_failed_send_buffer_and_received_value() {
        let mut scope = TaskScope::new();
        let events = Arc::new(Mutex::new(vec![]));
        let parent = std::thread::current().id();
        let (release, receiver) = oneshot::channel();
        let child_events = Arc::clone(&events);
        let failed_send = scope.spawn_value(async move {
            receiver.await.unwrap();
            Payload {
                events: child_events,
                label: "child failed send",
            }
        });
        scope.discard(failed_send);
        let released = release.send(());
        let joined = watch(scope.join()).await;
        let gate = Arc::new(DropGate::default());
        let _release = Release(Arc::clone(&gate));
        let child_events = Arc::clone(&events);
        let buffered = scope.spawn_with_cleanup(
            async move {
                Payload {
                    events: child_events,
                    label: "parent buffer",
                }
            },
            Cleanup(Arc::clone(&gate)),
        );
        let entered = watch(gate.entered.wait()).await;
        let ready = buffered.output_ready();
        scope.discard(buffered);
        gate.release();
        let buffer_joined = watch(scope.join()).await;
        let child_events = Arc::clone(&events);
        let received = scope.spawn_value(async move {
            Payload {
                events: child_events,
                label: "parent received",
            }
        });
        let value = watch(scope.receive(received)).await;
        if let Ok(Ok(value)) = value {
            drop(value);
        } else {
            panic!("payload receive failed")
        }
        let exit = watch(scope.join()).await;
        assert!(released.is_ok() && entered.is_ok() && ready);
        assert!(
            matches!(joined, Ok(Ok(())))
                && matches!(buffer_joined, Ok(Ok(())))
                && matches!(exit, Ok(Ok(())))
        );
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].0, "child failed send");
        assert_ne!(events[0].1, parent);
        assert_eq!(events[1], ("parent buffer", parent));
        assert_eq!(events[2], ("parent received", parent));
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn requested_cancel_does_not_hide_already_returned_legacy_error() {
        let mut scope = TaskScope::new();
        let gate = Arc::new(DropGate::default());
        let _release = Release(Arc::clone(&gate));
        scope.spawn_legacy_with_cleanup(
            async { Err(Error::invalid("already failed")) },
            Cleanup(Arc::clone(&gate)),
        );
        let entered = watch(gate.entered.wait()).await;
        let mut cancelling = Box::pin(scope.cancel());
        let stayed_pending = futures_util::poll!(&mut cancelling).is_pending();
        drop(cancelling);
        gate.release();
        let resumed = watch(scope.cancel()).await;
        assert!(entered.is_ok() && stayed_pending);
        assert!(matches!(
            resumed.unwrap().unwrap_err().kind,
            crate::ErrorKind::Invalid
        ));
        assert_eq!(
            scope.primary.as_ref().unwrap().kind,
            TaskFailureKind::LegacyError
        );
        assert_eq!(scope.stats().joined, 1);
    }

    #[tokio::test]
    async fn wrong_owner_and_missing_output_are_protocol_errors_without_stolen_join() {
        let mut original = TaskScope::new();
        let mut wrong = TaskScope::new();
        let task = original.spawn_value(async { 1_u64 });
        let rejected = watch(wrong.receive(task)).await;
        let original_joined = watch(original.join()).await;
        assert_eq!(
            rejected.unwrap().unwrap_err().kind(),
            TaskFailureKind::Internal
        );
        assert!(matches!(original_joined, Ok(Ok(()))));
        assert_eq!(original.stats().joined, 1);
        assert_eq!(wrong.stats().joined, 0);
        let mut original_discard = TaskScope::new();
        let mut wrong_discard = TaskScope::new();
        let task = original_discard.spawn_value(async { 2_u64 });
        wrong_discard.discard(task);
        let rejected = watch(wrong_discard.join()).await;
        let joined = watch(original_discard.join()).await;
        assert!(matches!(
            rejected.unwrap().unwrap_err().kind,
            crate::ErrorKind::Internal
        ));
        assert_eq!(
            wrong_discard.primary.as_ref().unwrap().kind,
            TaskFailureKind::Internal
        );
        assert!(matches!(joined, Ok(Ok(()))));
        assert_eq!(original_discard.stats().joined, 1);
        assert_eq!(wrong_discard.stats().joined, 0);
        let mut missing = TaskScope::new();
        let task = missing.spawn_missing_output::<u64>();
        let rejected = watch(missing.receive(task)).await;
        let repeated = watch(missing.join()).await;
        assert_eq!(
            rejected.unwrap().unwrap_err().kind(),
            TaskFailureKind::Internal
        );
        assert!(matches!(
            repeated.unwrap().unwrap_err().kind,
            crate::ErrorKind::Internal
        ));
        assert_eq!(missing.stats().entries, 0);
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn requested_cancel_preserves_panic_and_unrequested_cancel_is_fault() {
        let mut scope = TaskScope::new();
        let gate = Arc::new(DropGate::default());
        let _release = Release(Arc::clone(&gate));
        let task: Task<()> = scope.spawn_with_cleanup(
            async { panic!("panic already unwinding before abort request") },
            Cleanup(Arc::clone(&gate)),
        );
        let entered = watch(gate.entered.wait()).await;
        scope.discard(task);
        let mut cancelling = Box::pin(scope.cancel());
        let stayed_pending = futures_util::poll!(&mut cancelling).is_pending();
        drop(cancelling);
        gate.release();
        let fault = watch(scope.cancel()).await;
        assert!(entered.is_ok() && stayed_pending);
        assert!(matches!(
            fault.unwrap().unwrap_err().kind,
            crate::ErrorKind::Internal
        ));
        assert_eq!(
            scope.primary.as_ref().unwrap().kind,
            TaskFailureKind::Panicked
        );
        assert_eq!(scope.stats().joined, 1);
        assert_eq!(scope.stats().entries, 0);
        let mut external = TaskScope::new();
        let task = external.spawn_value(pending::<u64>());
        // Private native seam: cancellation not requested by TaskScope.
        external.entries[&task.ticket]
            .abort
            .as_ref()
            .unwrap()
            .abort();
        let fault = watch(external.receive(task)).await;
        let exit = watch(external.join()).await;
        assert_eq!(
            fault.unwrap().unwrap_err().kind(),
            TaskFailureKind::Cancelled
        );
        assert!(matches!(
            exit.unwrap().unwrap_err().kind,
            crate::ErrorKind::Internal
        ));
        assert_eq!(
            external.primary.as_ref().unwrap().kind,
            TaskFailureKind::Cancelled
        );
        assert_eq!(external.stats().joined, 1);
    }

    #[tokio::test]
    async fn spawn_after_fault_keeps_input_evaluation_and_actual_join_responsibility() {
        let mut scope = TaskScope::new();
        let task: Task<()> = scope.spawn_value(async { panic!("sticky before new spawn") });
        let primary = watch(scope.receive(task)).await.unwrap().unwrap_err();
        let evaluated = Arc::new(AtomicBool::new(false));
        let input = {
            evaluated.store(true, Ordering::SeqCst);
            9_u64
        };
        let task = scope.spawn_value(async move { input });
        let before = scope.stats();
        let rejected = watch(scope.receive(task)).await;
        let finished = watch(scope.finish(Ok::<(), String>(()))).await;
        assert!(evaluated.load(Ordering::SeqCst));
        assert_eq!(before.joined, 1);
        assert_eq!(before.unjoined, 1, "new spawn must not fake an actual join");
        assert_eq!(rejected.unwrap().unwrap_err().0, primary.0);
        assert!(
            matches!(finished, Ok(Err(Exit::Task(error))) if error.message == primary.message())
        );
        assert_eq!(scope.stats().joined, 2);
        assert_eq!(scope.stats().entries, 0);
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn legacy_structured_cause_preserves_kind_message_and_primary_after_later_fault() {
        let mut scope = TaskScope::new();
        let first = Arc::new(DropGate::default());
        let later = Arc::new(DropGate::default());
        let _release_first = Release(Arc::clone(&first));
        let _release_later = Release(Arc::clone(&later));
        scope.spawn_legacy_with_cleanup(
            async { Err(Error::invalid("original invalid cause")) },
            Cleanup(Arc::clone(&first)),
        );
        scope.spawn_legacy_with_cleanup(
            async {
                Err(Error {
                    kind: crate::ErrorKind::Busy,
                    message: "later busy cause".into(),
                })
            },
            Cleanup(Arc::clone(&later)),
        );
        // Both native futures already returned their Error before the first abort request.
        let first_ready = watch(first.entered.wait()).await;
        let later_ready = watch(later.entered.wait()).await;
        first.release();
        let first_join = watch(scope.observe_one()).await;
        let first_stats = scope.stats();
        later.release();
        let exit = watch(scope.join()).await;
        let repeated = watch(scope.join()).await;
        // Watchdog/cleanup failure is reported only after both Drop gates are released.
        assert!(first_ready.is_ok() && later_ready.is_ok());
        assert!(first_join.unwrap());
        assert_eq!(first_stats.joined, 1);
        assert_eq!(first_stats.unjoined, 1);
        let primary = exit.unwrap().unwrap_err();
        assert!(matches!(primary.kind, crate::ErrorKind::Invalid));
        assert_eq!(primary.message, "original invalid cause");
        let original = scope
            .primary
            .as_ref()
            .unwrap()
            .legacy_cause()
            .expect("legacy original Error was erased");
        assert!(matches!(original.kind, crate::ErrorKind::Invalid));
        assert_eq!(original.message, "original invalid cause");
        let repeated = repeated.unwrap().unwrap_err();
        assert!(matches!(repeated.kind, crate::ErrorKind::Invalid));
        assert_eq!(repeated.message, "original invalid cause");
        let repeated_clone = scope.primary.clone().unwrap();
        let repeated_original = repeated_clone
            .legacy_cause()
            .expect("sticky clone lost Error cause");
        assert!(matches!(repeated_original.kind, crate::ErrorKind::Invalid));
        assert_eq!(repeated_original.message, "original invalid cause");
        assert_eq!(scope.related.len(), 1);
        let later_cause = scope.related[0]
            .legacy_cause()
            .expect("later Error was erased");
        assert!(matches!(later_cause.kind, crate::ErrorKind::Busy));
        assert_eq!(later_cause.message, "later busy cause");
        assert_eq!(scope.stats().joined, 2);
        assert_eq!(scope.stats().entries, 0);
    }
}

#[cfg(test)]
mod cost_tests {
    use super::*;
    use crate::metrics;
    use std::{
        hint::black_box,
        mem::{size_of, size_of_val},
    };

    async fn receive_batch(n: usize) {
        let mut scope = TaskScope::new();
        let handles: Vec<_> = (0..n)
            .map(|value| scope.spawn_value(async move { value }))
            .collect();
        for (value, task) in handles.into_iter().enumerate() {
            assert_eq!(scope.receive(task).await.unwrap(), value);
        }
        scope.join().await.unwrap();
        assert_eq!(scope.entries.len(), 0);
    }
    async fn discard_batch(n: usize) {
        let mut scope = TaskScope::new();
        for value in 0..n {
            let task = scope.spawn_value(async move { value });
            scope.discard(task);
        }
        scope.join().await.unwrap();
        assert_eq!(scope.entries.len(), 0);
    }

    // Explicitly invoked measurement, separate from the 16 lifecycle oracles.
    // Current-thread runtime keeps the allocator's existing calling-thread
    // counter applicable to every child; no new observer/runtime dependency.
    #[test]
    #[ignore = "native cost measurement; run explicitly with --ignored --nocapture"]
    fn native_task_handle_costs() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let mut scope = TaskScope::new();
            let task = scope.spawn_value(async { 1_u64 });
            let receiving = scope.receive(task);
            let receive_bytes = size_of_val(&receiving);
            drop(receiving);
            let joining = scope.join();
            let join_bytes = size_of_val(&joining);
            drop(joining);
            let cancelling = scope.cancel();
            let cancel_bytes = size_of_val(&cancelling);
            drop(cancelling);
            scope.cancel().await.unwrap();
            println!(
                "{}",
                serde_json::json!({
                    "name":"task_handle_native_sizes", "target":std::env::consts::ARCH, "build":"cfg_test",
                    "scope_bytes":size_of::<TaskScope>(), "handle_u64_bytes":size_of::<Task<u64>>(),
                    "entry_bytes":size_of::<Entry>(), "failure_bytes":size_of::<TaskFailure>(),
                    "kind_bytes":size_of::<TaskFailureKind>(),
                    "receive_future_bytes":receive_bytes, "join_future_bytes":join_bytes,
                    "cancel_future_bytes":cancel_bytes,
                    "receive_batch_future_bytes":size_of_val(&receive_batch(128)),
                    "discard_batch_future_bytes":size_of_val(&discard_batch(128)),
                })
            );
        });
        for n in [128, 1024, 8192] {
            metrics::benchmark("taskscope_native_receive", n, || {
                runtime.block_on(receive_batch(n))
            });
            metrics::benchmark("taskscope_native_discard", n, || {
                runtime.block_on(discard_batch(n))
            });
        }
        let n = 1024;
        let ((mut scope, handles), allocation) = metrics::measure(|| {
            runtime.block_on(async {
                let mut scope = TaskScope::new();
                let handles: Vec<_> = (0..n)
                    .map(|_| scope.spawn_value(async { vec![0_u8; 64] }))
                    .collect();
                while !handles.iter().all(Task::output_ready) {
                    tokio::task::yield_now().await;
                }
                (scope, handles)
            })
        });
        println!(
            "{}",
            serde_json::json!({"name":"completed_unjoined", "items":n,
            "entries":scope.entries.len(), "unjoined":scope.unjoined.len(),
            "joinset_entries":scope.tasks.len(), "buffered_payload_bytes":n*64,
            "entries_capacity":scope.entries.capacity(), "allocation":allocation})
        );
        let (_, joined_allocation) = metrics::measure(|| runtime.block_on(scope.join()).unwrap());
        println!(
            "{}",
            serde_json::json!({"name":"joined_unreceived", "items":n,
            "entries":scope.entries.len(), "unjoined":scope.unjoined.len(),
            "joinset_entries":scope.tasks.len(), "buffered_payload_bytes":n*64,
            "entries_capacity":scope.entries.capacity(), "allocation":joined_allocation})
        );
        let (_, dropped_allocation) = metrics::measure(|| drop(handles));
        println!(
            "{}",
            serde_json::json!({"name":"joined_handle_drop_before_scope_operation",
            "entries":scope.entries.len(), "buffered_payload_bytes":0,
            "entries_capacity":scope.entries.capacity(), "allocation":dropped_allocation})
        );
        let (_, retired_allocation) = metrics::measure(|| runtime.block_on(scope.join()).unwrap());
        assert_eq!(scope.entries.len(), 0);
        println!(
            "{}",
            serde_json::json!({"name":"retired_records", "entries":scope.entries.len(),
            "entries_capacity":scope.entries.capacity(), "native_keys_capacity":scope.native_keys.capacity(),
            "unjoined_capacity":scope.unjoined.capacity(), "related_capacity":scope.related.capacity(),
            "allocation":retired_allocation})
        );
        black_box(scope);

        // Completed legacy errors are all already returned before the abort request,
        // so related fault retention is measured independently from cancellation.
        let mut scope = TaskScope::new();
        runtime.block_on(async {
            let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            for _ in 0..128 {
                let completed = Arc::clone(&completed);
                scope.spawn(async move {
                    let error = crate::Error::invalid("retained legacy cause");
                    completed.fetch_add(1, Ordering::Release);
                    Err(error)
                });
            }
            while completed.load(Ordering::Acquire) != 128 {
                tokio::task::yield_now().await;
            }
            scope.join().await.unwrap_err();
        });
        let (_, cloned_allocation) = metrics::measure(|| {
            let clone = scope.primary.clone();
            black_box(&clone);
            drop(clone);
        });
        println!(
            "{}",
            serde_json::json!({"name":"internal_primary_fault_clone", "allocation":cloned_allocation})
        );
        println!(
            "{}",
            serde_json::json!({"name":"related_fault_retention",
            "primary":scope.primary.is_some(), "related":scope.related.len(),
            "related_capacity":scope.related.capacity(), "fault_bytes":size_of::<Fault>(),
            "entries":scope.entries.len()})
        );
    }
}
