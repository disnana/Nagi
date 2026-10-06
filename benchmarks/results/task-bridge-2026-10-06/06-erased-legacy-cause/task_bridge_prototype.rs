//! ADR012のprivate bridge先行試験。公開Task/checker/旧Scopeの実装ではない。

use std::{
    collections::HashMap,
    future::Future,
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

#[derive(Clone, Debug, PartialEq, Eq)]
enum FaultKind {
    Panicked,
    Cancelled,
    LegacyError,
    Internal,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Fault {
    kind: FaultKind,
    message: String,
}
// RED representation exposes that the original Error is no longer stored.
impl Fault {
    fn legacy_cause(&self) -> Option<&crate::Error> {
        None
    }
}
#[derive(Debug)]
enum Exit<E> {
    Body(E),
    Task(Fault),
}
struct Task<T> {
    owner: Arc<()>,
    ticket: u64,
    receiver: Option<oneshot::Receiver<T>>,
    receipt: Arc<AtomicU8>,
}
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
#[derive(Debug)]
struct Stats {
    joined: usize,
    entries: usize,
    unjoined: usize,
    failed: bool,
}
struct Scope {
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
impl Scope {
    fn new() -> Self {
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
            .expect("private ticket exhausted");
        let abort = self.tasks.spawn(future);
        let id = abort.id();
        let key = fake.map_or(NativeKey::Real(id), NativeKey::Fake);
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
    fn spawn<T: Send + 'static>(
        &mut self,
        future: impl Future<Output = T> + Send + 'static,
    ) -> Task<T> {
        self.spawn_value(future, (), None)
    }
    fn spawn_with_cleanup<T: Send + 'static, D: Send + 'static>(
        &mut self,
        future: impl Future<Output = T> + Send + 'static,
        cleanup: D,
    ) -> Task<T> {
        self.spawn_value(future, cleanup, None)
    }
    fn spawn_fake_id<T: Send + 'static>(
        &mut self,
        future: impl Future<Output = T> + Send + 'static,
        fake: u64,
    ) -> Task<T> {
        self.spawn_value(future, (), Some(fake))
    }
    fn spawn_value<T: Send + 'static, D: Send + 'static>(
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
        }
    }
    fn spawn_legacy(
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
        }
    }
    fn request_abort(&mut self) {
        for entry in self.entries.values_mut().filter(|entry| !entry.joined) {
            entry.requested_cancel = true;
        }
        self.tasks.abort_all();
    }
    fn fault(&mut self, fault: Fault) {
        if self.primary.is_none() {
            self.primary = Some(fault);
        } else {
            self.related.push(fault);
        }
        self.request_abort();
    }
    fn internal(&mut self, message: &str) {
        self.fault(Fault {
            kind: FaultKind::Internal,
            message: message.into(),
        });
    }
    fn retire(&mut self) {
        self.entries
            .retain(|_, entry| !entry.joined || entry.receipt.load(Ordering::Acquire) < RECEIVED);
    }
    // Only this Scope consumes native join results. Ready -> accounting has no await,
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
        let fault = match &result {
            Ok((_, ChildExit::Success)) => None,
            Ok((_, ChildExit::Legacy(error))) => Some(Fault {
                kind: FaultKind::LegacyError,
                message: error.to_string(),
            }),
            Err(error) if error.is_panic() => Some(Fault {
                kind: FaultKind::Panicked,
                message: "child panicked".into(),
            }),
            Err(error) if error.is_cancelled() && requested_cancel => None,
            Err(error) => Some(Fault {
                kind: FaultKind::Cancelled,
                message: error.to_string(),
            }),
        };
        if let Some(fault) = fault {
            self.fault(fault);
        }
        self.retire();
        // Native panic payload destruction is an arbitrary Rust boundary, after record.
        drop(result);
    }
    async fn observe_one(&mut self) -> bool {
        if let Some(result) = self.tasks.join_next_with_id().await {
            self.record(result);
            true
        } else {
            self.retire();
            false
        }
    }
    async fn drain(&mut self) {
        while self.observe_one().await {}
    }
    fn receive<'a, T: 'a>(
        &'a mut self,
        mut task: Task<T>,
    ) -> impl Future<Output = Result<T, Fault>> + 'a {
        task.receipt.store(RECEIVING, Ordering::Release);
        async move {
            if !Arc::ptr_eq(&self.identity, &task.owner) {
                self.internal("Task belongs to a different Scope");
                self.drain().await;
                return Err(self.primary.clone().unwrap());
            }
            loop {
                if self.primary.is_some() {
                    self.drain().await;
                    let fault = self.primary.clone().unwrap();
                    return Err(fault);
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
                    self.retire();
                    Ok(value)
                }
                Err(_) => {
                    self.internal("native success without typed output");
                    self.drain().await;
                    Err(self.primary.clone().unwrap())
                }
            }
        }
    }
    fn discard<T>(&mut self, task: Task<T>) -> Result<(), Fault> {
        let correct = Arc::ptr_eq(&self.identity, &task.owner);
        drop(task);
        self.retire();
        if correct {
            Ok(())
        } else {
            self.internal("discard belongs to a different Scope");
            Err(self.primary.clone().unwrap())
        }
    }
    async fn join(&mut self) -> Result<(), Fault> {
        self.drain().await;
        self.primary.clone().map_or(Ok(()), Err)
    }
    async fn cancel(&mut self) -> Result<(), Fault> {
        self.request_abort();
        self.join().await
    }
    async fn finish<E>(&mut self, body: Result<(), E>) -> Result<(), Exit<E>> {
        match body {
            Err(error) => {
                let _ = self.cancel().await;
                Err(Exit::Body(error))
            }
            Ok(()) => self.join().await.map_err(Exit::Task),
        }
    }
    fn stats(&self) -> Stats {
        Stats {
            joined: self.joined,
            entries: self.entries.len(),
            unjoined: self.unjoined.len(),
            failed: self.primary.is_some(),
        }
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        self.request_abort();
    }
}

#[cfg(test)]
mod tests {
    use super::{Exit, FaultKind, Scope, Task};
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
        let mut scope = Scope::new();
        let started = Arc::new(Signal::default());
        let (release, receiver) = oneshot::channel();
        let child_started = Arc::clone(&started);
        let sibling = scope.spawn(async move {
            child_started.set();
            receiver.await.unwrap();
            88_u64
        });
        let business = scope.spawn(async { Err::<String, u8>(9) });
        let unit = scope.spawn(async {});
        let owned = scope.spawn(async { vec![String::from("owned")] });
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
        let mut scope = Scope::new();
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
        let mut scope = Scope::new();
        let task: Task<()> = scope.spawn(async { panic!("controlled child fault") });
        let received = watch(scope.receive(task)).await;
        let exit = watch(scope.join()).await;
        let repeated = watch(scope.join()).await;
        let received = received.unwrap().unwrap_err();
        assert_eq!(received.kind, FaultKind::Panicked);
        assert_eq!(exit.unwrap().unwrap_err(), received);
        assert_eq!(repeated.unwrap().unwrap_err(), received);
        assert_eq!(scope.stats().joined, 1);
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fault_return_waits_for_requested_sibling_cleanup_and_actual_join() {
        let mut scope = Scope::new();
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
        let failed: Task<()> = scope.spawn(async { panic!("primary") });
        let mut receiving = Box::pin(scope.receive(failed));
        let observed = tokio::select! {
            entered = watch(gate.entered.wait()) => entered.is_ok(),
            _ = &mut receiving => false,
        };
        drop(receiving); // 生存Scopeのfault/drain記録は失われてはならない。
        let interrupted = scope.stats();
        scope.discard(sibling).unwrap();
        gate.release();
        let resumed = watch(scope.join()).await;
        assert!(ready.is_ok() && observed);
        assert!(interrupted.failed && interrupted.unjoined != 0);
        assert_eq!(resumed.unwrap().unwrap_err().kind, FaultKind::Panicked);
        assert_eq!(scope.stats().joined, 2);
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unpolled_and_pending_receive_drop_keep_scope_join_responsibility() {
        let mut scope = Scope::new();
        let unpolled = scope.spawn(pending::<u64>());
        drop(scope.receive(unpolled));
        let target = scope.spawn(pending::<u64>());
        let other = scope.spawn(async { 7_u64 });
        // targetは完了不能なので、受取がPendingになった後もScopeへ責任が残る。
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
        let mut scope = Scope::new();
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
        scope.discard(task).unwrap();
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
        let mut scope = Scope::new();
        let fault: Task<()> = scope.spawn(async { panic!("discarded child still faults") });
        scope.discard(fault).unwrap();
        let failed = watch(scope.join()).await;
        assert_eq!(failed.unwrap().unwrap_err().kind, FaultKind::Panicked);
        assert_eq!(scope.stats().entries, 0);
        let mut legacy = Scope::new();
        legacy.spawn_legacy(async { Err(Error::invalid("legacy terminal")) });
        let failed = watch(legacy.join()).await;
        assert_eq!(failed.unwrap().unwrap_err().kind, FaultKind::LegacyError);
        assert!(legacy.stats().failed);
        assert_eq!(legacy.stats().joined, 1);
    }

    #[tokio::test]
    async fn fake_native_id_reuse_does_not_mix_unreceived_tickets_and_retire_records() {
        let mut scope = Scope::new();
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
            let task = scope.spawn(async move { value });
            assert_eq!(watch(scope.receive(task)).await.unwrap().unwrap(), value);
            assert_eq!(scope.stats().entries, 0);
        }
        assert_eq!(scope.stats().joined, 34);
    }

    #[tokio::test]
    async fn target_other_than_first_join_and_body_error_primary_are_preserved() {
        let mut scope = Scope::new();
        let first = scope.spawn(async { 11_u64 });
        let observed = watch(scope.observe_one()).await;
        let second = scope.spawn(async { 12_u64 });
        let second_value = watch(scope.receive(second)).await;
        let first_value = watch(scope.receive(first)).await;
        assert!(observed.unwrap());
        assert_eq!(second_value.unwrap().unwrap(), 12);
        assert_eq!(first_value.unwrap().unwrap(), 11);
        scope.spawn_legacy(async { Err(Error::invalid("secondary legacy fault")) });
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
        let mut scope = Scope::new();
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
            "Scope Drop pretended to join a blocked child"
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
        let mut scope = Scope::new();
        let events = Arc::new(Mutex::new(vec![]));
        let parent = std::thread::current().id();
        let (release, receiver) = oneshot::channel();
        let child_events = Arc::clone(&events);
        let failed_send = scope.spawn(async move {
            receiver.await.unwrap();
            Payload {
                events: child_events,
                label: "child failed send",
            }
        });
        scope.discard(failed_send).unwrap();
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
        scope.discard(buffered).unwrap();
        gate.release();
        let buffer_joined = watch(scope.join()).await;
        let child_events = Arc::clone(&events);
        let received = scope.spawn(async move {
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
        let mut scope = Scope::new();
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
        assert_eq!(resumed.unwrap().unwrap_err().kind, FaultKind::LegacyError);
        assert_eq!(scope.stats().joined, 1);
    }

    #[tokio::test]
    async fn wrong_owner_and_missing_output_are_protocol_errors_without_stolen_join() {
        let mut original = Scope::new();
        let mut wrong = Scope::new();
        let task = original.spawn(async { 1_u64 });
        let rejected = watch(wrong.receive(task)).await;
        let original_joined = watch(original.join()).await;
        assert_eq!(rejected.unwrap().unwrap_err().kind, FaultKind::Internal);
        assert!(matches!(original_joined, Ok(Ok(()))));
        assert_eq!(original.stats().joined, 1);
        assert_eq!(wrong.stats().joined, 0);
        let mut missing = Scope::new();
        let task = missing.spawn_missing_output::<u64>();
        let rejected = watch(missing.receive(task)).await;
        let repeated = watch(missing.join()).await;
        assert_eq!(rejected.unwrap().unwrap_err().kind, FaultKind::Internal);
        assert_eq!(repeated.unwrap().unwrap_err().kind, FaultKind::Internal);
        assert_eq!(missing.stats().entries, 0);
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn requested_cancel_preserves_panic_and_unrequested_cancel_is_fault() {
        let mut scope = Scope::new();
        let gate = Arc::new(DropGate::default());
        let _release = Release(Arc::clone(&gate));
        let task: Task<()> = scope.spawn_with_cleanup(
            async { panic!("panic already unwinding before abort request") },
            Cleanup(Arc::clone(&gate)),
        );
        let entered = watch(gate.entered.wait()).await;
        scope.discard(task).unwrap();
        let mut cancelling = Box::pin(scope.cancel());
        let stayed_pending = futures_util::poll!(&mut cancelling).is_pending();
        drop(cancelling);
        gate.release();
        let fault = watch(scope.cancel()).await;
        assert!(entered.is_ok() && stayed_pending);
        assert_eq!(fault.unwrap().unwrap_err().kind, FaultKind::Panicked);
        assert_eq!(scope.stats().joined, 1);
        assert_eq!(scope.stats().entries, 0);
        let mut external = Scope::new();
        let task = external.spawn(pending::<u64>());
        // Private native seam: cancellation not requested by Scope.
        external.entries[&task.ticket]
            .abort
            .as_ref()
            .unwrap()
            .abort();
        let fault = watch(external.receive(task)).await;
        let exit = watch(external.join()).await;
        assert_eq!(fault.unwrap().unwrap_err().kind, FaultKind::Cancelled);
        assert_eq!(exit.unwrap().unwrap_err().kind, FaultKind::Cancelled);
        assert_eq!(external.stats().joined, 1);
    }

    #[tokio::test]
    async fn spawn_after_fault_keeps_input_evaluation_and_actual_join_responsibility() {
        let mut scope = Scope::new();
        let task: Task<()> = scope.spawn(async { panic!("sticky before new spawn") });
        let primary = watch(scope.receive(task)).await.unwrap().unwrap_err();
        let evaluated = Arc::new(AtomicBool::new(false));
        let input = {
            evaluated.store(true, Ordering::SeqCst);
            9_u64
        };
        let task = scope.spawn(async move { input });
        let before = scope.stats();
        let rejected = watch(scope.receive(task)).await;
        let finished = watch(scope.finish(Ok::<(), String>(()))).await;
        assert!(evaluated.load(Ordering::SeqCst));
        assert_eq!(before.joined, 1);
        assert_eq!(before.unjoined, 1, "new spawn must not fake an actual join");
        assert_eq!(rejected.unwrap().unwrap_err(), primary);
        assert!(matches!(finished, Ok(Err(Exit::Task(fault))) if fault == primary));
        assert_eq!(scope.stats().joined, 2);
        assert_eq!(scope.stats().entries, 0);
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn legacy_structured_cause_preserves_kind_message_and_primary_after_later_fault() {
        let mut scope = Scope::new();
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
        let original = primary
            .legacy_cause()
            .expect("legacy original Error was erased");
        assert!(matches!(original.kind, crate::ErrorKind::Invalid));
        assert_eq!(original.message, "original invalid cause");
        let repeated = repeated.unwrap().unwrap_err();
        let repeated_original = repeated
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
