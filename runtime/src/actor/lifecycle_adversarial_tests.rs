//! Finite adversarial gates: failures must not leave a forever-blocked Tokio worker.
//! These tests exercise lifecycle ownership, not the actor/message implementation.
//! Native callbacks use TEMPORARY policy so each adversarial factory runs once.

use super::{
    clone_control, control as observe, next_event, options, run, shutdown, supervisor, task,
    Control, Phase, RestartPolicy, Supervisor,
};
use crate::{Error, ErrorKind};
use std::future::{pending, Future};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{oneshot, Barrier, Notify};

const ASSERT_TIMEOUT: Duration = Duration::from_secs(3);
const GATE_FAILSAFE: Duration = Duration::from_secs(10);
const SHUTDOWN_DEADLINE: Duration = Duration::from_millis(25);

struct GateState {
    open: AtomicBool,
    entered: AtomicBool,
    timed_out: AtomicBool,
    changed: Notify,
    mutex: Mutex<()>,
    condvar: Condvar,
}

#[derive(Clone)]
struct Gate(Arc<GateState>);

struct ReleaseOnDrop(Gate);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}

impl Gate {
    fn new() -> (Self, ReleaseOnDrop) {
        let gate = Self(Arc::new(GateState {
            open: AtomicBool::new(false),
            entered: AtomicBool::new(false),
            timed_out: AtomicBool::new(false),
            changed: Notify::new(),
            mutex: Mutex::new(()),
            condvar: Condvar::new(),
        }));
        (gate.clone(), ReleaseOnDrop(gate))
    }

    fn release(&self) {
        // Pair the Condvar predicate transition with its mutex to avoid lost wakeups.
        let _lock = self.0.mutex.lock().unwrap();
        self.0.open.store(true, Ordering::Release);
        self.0.condvar.notify_all();
    }

    fn announce_entry(&self) {
        self.0.entered.store(true, Ordering::Release);
        self.0.changed.notify_waiters();
    }

    async fn wait_entered(&self) {
        tokio::time::timeout(ASSERT_TIMEOUT, async {
            loop {
                let changed = self.0.changed.notified();
                tokio::pin!(changed);
                changed.as_mut().enable();
                if self.0.entered.load(Ordering::Acquire) {
                    break;
                }
                changed.await;
            }
        })
        .await
        .expect("the finite gate was never entered");
    }

    fn wait_native(&self) {
        self.announce_entry();
        let lock = self.0.mutex.lock().unwrap();
        let (_guard, timeout) = self
            .0
            .condvar
            .wait_timeout_while(lock, GATE_FAILSAFE, |_| {
                !self.0.open.load(Ordering::Acquire)
            })
            .unwrap();
        if timeout.timed_out() && !self.0.open.load(Ordering::Acquire) {
            self.0.timed_out.store(true, Ordering::Release);
        }
    }

    fn wait_non_yielding(&self) {
        self.announce_entry();
        let until = Instant::now() + GATE_FAILSAFE;
        while !self.0.open.load(Ordering::Acquire) {
            // This yields to the OS, never to Tokio; cancellation cannot enter the poll.
            std::thread::yield_now();
            if Instant::now() >= until {
                self.0.timed_out.store(true, Ordering::Release);
                break;
            }
        }
    }

    fn assert_released_by_test(&self) {
        assert!(self.0.open.load(Ordering::Acquire));
        assert!(!self.0.timed_out.load(Ordering::Acquire));
    }
}

struct Context {
    drops: Arc<AtomicUsize>,
    drop_gate: Option<Gate>,
}

impl Drop for Context {
    fn drop(&mut self) {
        if let Some(gate) = &self.drop_gate {
            gate.wait_native();
        }
        self.drops.fetch_add(1, Ordering::Release);
    }
}

fn context() -> (Context, Arc<AtomicUsize>) {
    let drops = Arc::new(AtomicUsize::new(0));
    (
        Context {
            drops: drops.clone(),
            drop_gate: None,
        },
        drops,
    )
}

fn new_group<C: Send + Sync + 'static>(context: C, maximum: usize) -> Supervisor<C> {
    let options = options(maximum as i64, 16, 5, 10_000, 25).unwrap();
    supervisor(context, options)
}

fn register_once<C, F, Fut>(group: &Supervisor<C>, name: &str, factory: F) -> Result<(), Error>
where
    C: Send + Sync + 'static,
    F: FnOnce(Arc<C>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), Error>> + Send + 'static,
{
    let factory = Mutex::new(Some(factory));
    task(
        group,
        name,
        move |context| {
            let factory = factory
                .lock()
                .unwrap()
                .take()
                .expect("TEMPORARY test child restarted");
            factory(context)
        },
        RestartPolicy::TEMPORARY,
    )
}

async fn complete(control: &Control) {
    tokio::time::timeout(ASSERT_TIMEOUT, control.wait_complete())
        .await
        .expect("cleanup did not finish after its finite gates were released")
        .expect("an explicit cooperative stop must complete successfully");
    let snapshot = control.snapshot();
    assert_eq!(snapshot.phase, Phase::Complete);
    assert_eq!(snapshot.active_children, 0);
    assert_eq!(snapshot.retained_child_records, 0);
    assert!(!snapshot.retained_cleanup_record);
    assert!(snapshot.context_destroyed);
}

async fn wait_started(control: &Control) {
    tokio::time::timeout(ASSERT_TIMEOUT, async {
        while control.snapshot().phase == Phase::Registered {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the run owner was never polled");
}

async fn assert_incomplete(control: &Control) {
    let error = shutdown(control)
        .await
        .expect_err("gated cleanup must be incomplete");
    assert!(matches!(error.kind, ErrorKind::Busy));
    assert_eq!(error.message, "supervisor shutdown incomplete");
    assert_ne!(control.snapshot().phase, Phase::Complete);
    assert!(
        tokio::time::timeout(SHUTDOWN_DEADLINE, control.wait_complete())
            .await
            .is_err(),
        "wait_complete reported completion while user cleanup was still gated"
    );
}

async fn assert_incomplete_with_independent_timer(control: &Control) {
    // A blocking native Drop can stall its originating runtime's timer driver.
    // Keep an explicitly schedulable observer with its own timer, while polling
    // the same retained join ledger. The cleanup task is already accounted.
    let snapshot = control.snapshot();
    assert_eq!(snapshot.phase, Phase::Cleaning);
    assert!(snapshot.retained_cleanup_record);
    let control = clone_control(control);
    let (done_tx, done_rx) = oneshot::channel();
    let observer = std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap();
            runtime.block_on(assert_incomplete(&control));
        }));
        let _ = done_tx.send(result);
    });
    let result = done_rx
        .await
        .expect("independent observer exited without a result");
    observer.join().unwrap();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

async fn non_yielding_child_is_accounted(native: bool) {
    let (context, drops) = context();
    let (gate, _release) = Gate::new();
    let group = new_group(context, 1);
    let control = observe(&group);
    let child_gate = gate.clone();
    register_once(&group, "non-yielding", move |context| async move {
        let _context = context;
        if native {
            child_gate.wait_native();
        } else {
            child_gate.wait_non_yielding();
        }
        Ok(())
    })
    .unwrap();
    let run = tokio::spawn(run(group));
    gate.wait_entered().await;

    assert_incomplete(&control).await;
    let snapshot = control.snapshot();
    assert_eq!(snapshot.active_children, 1);
    assert_eq!(snapshot.retained_child_records, 1);
    assert!(!snapshot.context_destroyed);
    assert_eq!(drops.load(Ordering::Acquire), 0);
    assert!(!run.is_finished());

    gate.release();
    complete(&control).await;
    tokio::time::timeout(ASSERT_TIMEOUT, run)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(drops.load(Ordering::Acquire), 1);
    gate.assert_released_by_test();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_non_yielding_poll_cannot_be_reported_as_completed_cleanup() {
    non_yielding_child_is_accounted(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_blocking_native_poll_retains_its_child_record_until_it_returns() {
    non_yielding_child_is_accounted(true).await;
}

struct GatedDrop {
    gate: Gate,
    dropped: Arc<AtomicBool>,
}

impl Drop for GatedDrop {
    fn drop(&mut self) {
        self.gate.wait_native();
        self.dropped.store(true, Ordering::Release);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn abort_does_not_complete_before_the_child_future_destructor_finishes() {
    let (context, drops) = context();
    let (gate, _release) = Gate::new();
    let child_dropped = Arc::new(AtomicBool::new(false));
    let (started_tx, started_rx) = oneshot::channel();
    let group = new_group(context, 1);
    let control = observe(&group);
    let destructor = GatedDrop {
        gate: gate.clone(),
        dropped: child_dropped.clone(),
    };
    register_once(&group, "blocking-drop", move |context| async move {
        let _context = context;
        let _destructor = destructor;
        let _ = started_tx.send(());
        pending::<()>().await;
        Ok(())
    })
    .unwrap();
    let run = tokio::spawn(run(group));
    tokio::time::timeout(ASSERT_TIMEOUT, started_rx)
        .await
        .unwrap()
        .unwrap();
    control.request_stop();
    gate.wait_entered().await;

    assert_incomplete(&control).await;
    assert!(!child_dropped.load(Ordering::Acquire));
    assert_eq!(control.snapshot().retained_child_records, 1);
    assert!(!run.is_finished());
    gate.release();
    complete(&control).await;
    run.await.unwrap().unwrap();
    assert!(child_dropped.load(Ordering::Acquire));
    assert_eq!(drops.load(Ordering::Acquire), 1);
    gate.assert_released_by_test();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn canceled_observation_keeps_unfinished_joins_available_to_another_control() {
    let (context, drops) = context();
    let (gate, _release) = Gate::new();
    let group = new_group(context, 1);
    let first_control = observe(&group);
    let second_control = observe(&group);
    let child_gate = gate.clone();
    register_once(&group, "retained-join", move |context| async move {
        let _context = context;
        child_gate.wait_native();
        Ok(())
    })
    .unwrap();
    let run = tokio::spawn(run(group));
    gate.wait_entered().await;
    first_control.request_stop();
    // Release the monitor's drain lease so this observer must actually poll a
    // retained child join, rather than merely wait behind the live run owner.
    run.abort();
    assert!(tokio::time::timeout(ASSERT_TIMEOUT, run)
        .await
        .unwrap()
        .unwrap_err()
        .is_cancelled());
    let mut observer = Box::pin(first_control.wait_complete());
    std::future::poll_fn(|cx| {
        assert!(observer.as_mut().poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(observer);
    assert_incomplete(&second_control).await;
    assert_eq!(second_control.snapshot().retained_child_records, 1);

    gate.release();
    complete(&second_control).await;
    assert_eq!(drops.load(Ordering::Acquire), 1);
    gate.assert_released_by_test();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_surviving_control_can_finish_cleanup_after_the_running_owner_is_dropped() {
    let (context, drops) = context();
    let (gate, _release) = Gate::new();
    let group = new_group(context, 1);
    let control = observe(&group);
    let child_gate = gate.clone();
    register_once(&group, "owner-drop", move |context| async move {
        let _context = context;
        child_gate.wait_native();
        Ok(())
    })
    .unwrap();
    let run = tokio::spawn(run(group));
    gate.wait_entered().await;
    run.abort();
    assert!(tokio::time::timeout(ASSERT_TIMEOUT, run)
        .await
        .unwrap()
        .unwrap_err()
        .is_cancelled());

    assert_incomplete(&control).await;
    let snapshot = control.snapshot();
    assert!(!snapshot.owner_present);
    assert_eq!(snapshot.retained_child_records, 1);
    assert!(!snapshot.context_destroyed);
    gate.release();
    complete(&control).await;
    assert_eq!(drops.load(Ordering::Acquire), 1);
    gate.assert_released_by_test();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn final_context_destruction_is_a_retained_phase_before_completion() {
    let (gate, _release) = Gate::new();
    let drops = Arc::new(AtomicUsize::new(0));
    let context = Context {
        drops: drops.clone(),
        drop_gate: Some(gate.clone()),
    };
    let group = new_group(context, 1);
    let control = observe(&group);
    let run = tokio::spawn(run(group));
    gate.wait_entered().await;

    assert_incomplete(&control).await;
    let snapshot = control.snapshot();
    assert_eq!(snapshot.phase, Phase::Cleaning);
    assert_eq!(snapshot.active_children, 0);
    assert_eq!(snapshot.retained_child_records, 0);
    assert!(snapshot.retained_cleanup_record);
    assert!(!snapshot.context_destroyed);
    assert_eq!(drops.load(Ordering::Acquire), 0);
    assert!(!run.is_finished());

    gate.release();
    complete(&control).await;
    run.await.unwrap().unwrap();
    assert_eq!(drops.load(Ordering::Acquire), 1);
    gate.assert_released_by_test();
}

struct ReentrantDrop {
    request: mpsc::Sender<()>,
    answer: mpsc::Receiver<()>,
    blocked_observer: Arc<AtomicBool>,
}

impl Drop for ReentrantDrop {
    fn drop(&mut self) {
        let replied = self.request.send(()).is_ok()
            && self.answer.recv_timeout(Duration::from_secs(1)).is_ok();
        self.blocked_observer.store(!replied, Ordering::Release);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_destructors_do_not_run_under_the_ledger_or_stop_lock() {
    let (context, drops) = context();
    let group = new_group(context, 1);
    let control = observe(&group);
    let destructor_control = observe(&group);
    let (request_tx, request_rx) = mpsc::channel();
    let (answer_tx, answer_rx) = mpsc::channel();
    let blocked_observer = Arc::new(AtomicBool::new(false));
    let destructor = ReentrantDrop {
        request: request_tx,
        answer: answer_rx,
        blocked_observer: blocked_observer.clone(),
    };
    // Use another OS thread plus a finite Drop wait: a regression reports failure
    // and releases the destructor instead of deadlocking the Cargo test process.
    let observer = std::thread::spawn(move || {
        request_rx.recv_timeout(GATE_FAILSAFE).unwrap();
        let _ = destructor_control.snapshot();
        destructor_control.request_stop();
        let _ = answer_tx.send(());
    });
    let (started_tx, started_rx) = oneshot::channel();
    register_once(&group, "reentrant-drop", move |context| async move {
        let _context = context;
        let _destructor = destructor;
        let _ = started_tx.send(());
        pending::<()>().await;
        Ok(())
    })
    .unwrap();
    let run = tokio::spawn(run(group));
    tokio::time::timeout(ASSERT_TIMEOUT, started_rx)
        .await
        .unwrap()
        .unwrap();

    control.request_stop();
    complete(&control).await;
    run.await.unwrap().unwrap();
    observer.join().unwrap();
    assert!(
        !blocked_observer.load(Ordering::Acquire),
        "a destructor's snapshot/stop observer was blocked until Drop returned"
    );
    assert_eq!(drops.load(Ordering::Acquire), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_factory_constructor_panic_does_not_lose_the_context_cleanup_obligation() {
    let (context, drops) = context();
    let group = new_group(context, 1);
    let control = observe(&group);
    register_once(
        &group,
        "constructor-panic",
        |_| -> std::future::Ready<Result<(), Error>> {
            panic!("finite factory-construction panic")
        },
    )
    .unwrap();
    let run = tokio::spawn(run(group));
    // A fixed implementation may invoke the factory inside its tracked child
    // and return a group error, or observe the panicking run's cleanup guard.
    let result = tokio::time::timeout(ASSERT_TIMEOUT, run)
        .await
        .expect("factory-construction panic left the run task unfinished");
    assert!(result.is_err() || result.unwrap().is_err());
    let result = tokio::time::timeout(ASSERT_TIMEOUT, control.wait_complete())
        .await
        .expect("factory panic permanently prevented the retained cleanup phase");
    assert!(
        result.is_err(),
        "a factory panic must remain a group failure"
    );
    let snapshot = control.snapshot();
    assert_eq!(snapshot.phase, Phase::Complete);
    assert_eq!(snapshot.retained_child_records, 0);
    assert!(!snapshot.retained_cleanup_record);
    assert!(snapshot.context_destroyed);
    assert_eq!(drops.load(Ordering::Acquire), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_during_factory_construction_cancels_every_later_inserted_child() {
    let (context, drops) = context();
    let (gate, _release) = Gate::new();
    let group = new_group(context, 1);
    let control = observe(&group);
    let factory_gate = gate.clone();
    register_once(&group, "construction-race", move |context| {
        // The other worker can request stop before this factory returns
        // its future; no sleeps or probabilistic spawn-loop race required.
        factory_gate.wait_native();
        async move {
            let _context = context;
            pending::<()>().await;
            Ok(())
        }
    })
    .unwrap();
    let mut run = tokio::spawn(run(group));
    gate.wait_entered().await;
    control.request_stop();
    gate.release();
    let result = tokio::time::timeout(ASSERT_TIMEOUT, &mut run).await;
    if result.is_err() {
        // Rescue a missed-insertion regression so it cannot leave normal work
        // running after this test reports failure.
        control.request_stop();
        run.abort();
    }
    assert!(
        result.is_ok(),
        "stop missed a child inserted after its ledger scan"
    );
    result.unwrap().unwrap().unwrap();
    complete(&control).await;
    assert_eq!(drops.load(Ordering::Acquire), 1);
    gate.assert_released_by_test();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unrestarted_child_failure_is_retained_while_a_healthy_sibling_continues() {
    let (context, drops) = context();
    let barrier = Arc::new(Barrier::new(2));
    let group = new_group(context, 2);
    let control = observe(&group);
    let failure_barrier = barrier.clone();
    register_once(&group, "terminal-failure", move |context| async move {
        let _context = context;
        failure_barrier.wait().await;
        Err(Error::internal("finite child failure"))
    })
    .unwrap();
    register_once(&group, "unfinished-sibling", move |context| async move {
        let _context = context;
        barrier.wait().await;
        pending::<()>().await;
        Ok(())
    })
    .unwrap();
    let run = tokio::spawn(run(group));
    wait_started(&control).await;
    let first_result = tokio::time::timeout(SHUTDOWN_DEADLINE, control.wait_complete()).await;
    let sibling_was_active = control.snapshot().active_children == 1;
    // Ending the group returns the retained failure and joins the sibling.
    control.request_stop();
    let terminal = tokio::time::timeout(ASSERT_TIMEOUT, control.wait_complete())
        .await
        .expect("the sibling did not release after the explicit rescue stop");
    assert!(terminal.is_err());
    assert!(run.await.unwrap().is_err());
    assert!(
        first_result.is_err(),
        "one TEMPORARY child failure stopped its healthy sibling"
    );
    assert!(
        sibling_was_active,
        "the healthy sibling did not remain active"
    );
    assert_eq!(control.snapshot().phase, Phase::Complete);
    assert!(control.snapshot().context_destroyed);
    assert_eq!(drops.load(Ordering::Acquire), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_panic_payload_destructor_can_observe_and_stop_its_group_without_a_lock_cycle() {
    let (context, drops) = context();
    let group = new_group(context, 1);
    let control = observe(&group);
    let destructor_control = observe(&group);
    let (request_tx, request_rx) = mpsc::channel();
    let (answer_tx, answer_rx) = mpsc::channel();
    let blocked_observer = Arc::new(AtomicBool::new(false));
    let payload = ReentrantDrop {
        request: request_tx,
        answer: answer_rx,
        blocked_observer: blocked_observer.clone(),
    };
    let observer = std::thread::spawn(move || {
        request_rx.recv_timeout(GATE_FAILSAFE).unwrap();
        let _ = destructor_control.snapshot();
        destructor_control.request_stop();
        let _ = answer_tx.send(());
    });
    register_once(&group, "panic-payload", move |context| async move {
        let _context = context;
        std::panic::panic_any(payload);
        #[allow(unreachable_code)]
        Ok::<(), Error>(())
    })
    .unwrap();
    let run = tokio::spawn(run(group));
    assert!(tokio::time::timeout(ASSERT_TIMEOUT, run)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert!(control.wait_complete().await.is_err());
    observer.join().unwrap();
    assert!(
        !blocked_observer.load(Ordering::Acquire),
        "JoinError panic payload was destroyed under the group's observer lock"
    );
    assert!(control.snapshot().context_destroyed);
    assert_eq!(drops.load(Ordering::Acquire), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn context_owned_by_a_child_panic_payload_is_cleaned_before_final_context_cleanup() {
    let (context, drops) = context();
    let group = new_group(context, 1);
    let control = observe(&group);
    register_once(&group, "context-panic-payload", |context| async move {
        // This Arc is still owned by a registered child's result, not an
        // escaped native callback owner. It must precede final C cleanup.
        std::panic::panic_any(context);
        #[allow(unreachable_code)]
        Ok::<(), Error>(())
    })
    .unwrap();
    let run = tokio::spawn(run(group));
    assert!(tokio::time::timeout(ASSERT_TIMEOUT, run)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert!(control.wait_complete().await.is_err());
    let snapshot = control.snapshot();
    assert_eq!(snapshot.phase, Phase::Complete);
    assert!(
        snapshot.context_destroyed,
        "an owned child panic payload was mistaken for an escaped context"
    );
    assert_eq!(snapshot.retained_child_records, 0);
    assert!(!snapshot.retained_cleanup_record);
    assert_eq!(drops.load(Ordering::Acquire), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_blocking_panic_payload_destructor_remains_a_retained_child_obligation() {
    let (context, drops) = context();
    let (gate, _release) = Gate::new();
    let payload_dropped = Arc::new(AtomicBool::new(false));
    let payload = GatedDrop {
        gate: gate.clone(),
        dropped: payload_dropped.clone(),
    };
    let group = new_group(context, 1);
    let control = observe(&group);
    register_once(&group, "gated-panic-payload", move |context| async move {
        let _context = context;
        std::panic::panic_any(payload);
        #[allow(unreachable_code)]
        Ok::<(), Error>(())
    })
    .unwrap();
    let run = tokio::spawn(run(group));
    gate.wait_entered().await;

    assert_incomplete(&control).await;
    let snapshot = control.snapshot();
    assert_eq!(snapshot.active_children, 1);
    assert_eq!(snapshot.retained_child_records, 1);
    assert!(!snapshot.context_destroyed);
    assert!(!payload_dropped.load(Ordering::Acquire));
    assert!(!run.is_finished());
    gate.release();

    assert!(tokio::time::timeout(ASSERT_TIMEOUT, run)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert!(control.wait_complete().await.is_err());
    assert_eq!(control.snapshot().phase, Phase::Complete);
    assert!(control.snapshot().context_destroyed);
    assert!(payload_dropped.load(Ordering::Acquire));
    assert_eq!(drops.load(Ordering::Acquire), 1);
    gate.assert_released_by_test();

    let (panicked, stopped) = tokio::time::timeout(ASSERT_TIMEOUT, async {
        let mut panicked = 0;
        let mut stopped = 0;
        while let Some(event) = next_event(&control).await.unwrap() {
            if event.child_id() == 1 {
                panicked += usize::from(event.kind() == super::EventKind::PANICKED);
                stopped += usize::from(event.kind() == super::EventKind::STOPPED);
            }
        }
        (panicked, stopped)
    })
    .await
    .expect("terminal panic events were never closed after cleanup");
    assert_eq!(
        panicked, 1,
        "the retained panic cause must be reported once"
    );
    assert_eq!(stopped, 1, "the stopped generation must be reported once");
}

struct ForwardPanic<T: Send + 'static>(Option<T>);

impl<T: Send + 'static> Drop for ForwardPanic<T> {
    fn drop(&mut self) {
        if let Some(next) = self.0.take() {
            std::panic::panic_any(next);
        }
    }
}

struct HeldContextPayload {
    _context: Arc<Context>,
    gate: Gate,
}

impl Drop for HeldContextPayload {
    fn drop(&mut self) {
        self.gate.wait_native();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_finite_follow_on_panic_payload_retains_the_guard_and_context_until_drop_finishes() {
    let (context, drops) = context();
    let (gate, _release) = Gate::new();
    let group = new_group(context, 1);
    let control = observe(&group);
    let payload_gate = gate.clone();
    register_once(&group, "follow-on-payload", move |context| async move {
        let payload = ForwardPanic(Some(HeldContextPayload {
            _context: context,
            gate: payload_gate,
        }));
        std::panic::panic_any(payload);
        #[allow(unreachable_code)]
        Ok::<(), Error>(())
    })
    .unwrap();
    let run = tokio::spawn(run(group));
    gate.wait_entered().await;

    assert_incomplete(&control).await;
    let snapshot = control.snapshot();
    assert_eq!(snapshot.active_children, 1);
    assert_eq!(snapshot.retained_child_records, 1);
    assert!(!snapshot.context_destroyed);
    assert!(!run.is_finished());
    assert_eq!(drops.load(Ordering::Acquire), 0);
    gate.release();

    assert!(tokio::time::timeout(ASSERT_TIMEOUT, run)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert!(control.wait_complete().await.is_err());
    let snapshot = control.snapshot();
    assert_eq!(snapshot.phase, Phase::Complete);
    assert_eq!(snapshot.active_children, 0);
    assert_eq!(snapshot.retained_child_records, 0);
    assert!(!snapshot.retained_cleanup_record);
    assert!(snapshot.context_destroyed);
    assert_eq!(drops.load(Ordering::Acquire), 1);
    gate.assert_released_by_test();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delayed_native_context_owners_then_actual_context_drop_precede_completion() {
    let (gate, _release) = Gate::new();
    let drops = Arc::new(AtomicUsize::new(0));
    let context = Context {
        drops: drops.clone(),
        drop_gate: Some(gate.clone()),
    };
    let group = new_group(context, 1);
    let control = observe(&group);
    let (owner_tx, owner_rx) = oneshot::channel();
    register_once(
        &group,
        "delayed-native-context",
        move |context| async move {
            // Model the temporary Arc held by an aborted HTTP/Scope native child.
            // The test owns that external reference and releases it finitely.
            let _ = owner_tx.send(context);
            pending::<()>().await;
            Ok(())
        },
    )
    .unwrap();
    let run = tokio::spawn(run(group));
    let native_owner = tokio::time::timeout(ASSERT_TIMEOUT, owner_rx)
        .await
        .unwrap()
        .unwrap();
    control.request_stop();
    tokio::time::timeout(ASSERT_TIMEOUT, async {
        loop {
            let snapshot = control.snapshot();
            if snapshot.phase == Phase::Cleaning && snapshot.active_children == 0 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("registered child did not reach retained context cleanup");

    assert_incomplete(&control).await;
    let snapshot = control.snapshot();
    assert!(snapshot.retained_cleanup_record);
    assert!(!snapshot.context_destroyed);
    assert_eq!(drops.load(Ordering::Acquire), 0);
    assert!(!run.is_finished());

    drop(native_owner);
    gate.wait_entered().await;
    assert_incomplete_with_independent_timer(&control).await;
    assert!(control.snapshot().retained_cleanup_record);
    assert!(!control.snapshot().context_destroyed);
    assert_eq!(drops.load(Ordering::Acquire), 0);
    gate.release();

    complete(&control).await;
    tokio::time::timeout(ASSERT_TIMEOUT, run)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(drops.load(Ordering::Acquire), 1);
    gate.assert_released_by_test();
}

struct PanickingFactoryDrop(Arc<AtomicBool>);

impl Drop for PanickingFactoryDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
        panic!("finite native factory-destructor panic");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn panicking_factory_destruction_during_owner_drop_cannot_strand_context_cleanup() {
    let (context, drops) = context();
    let group = new_group(context, 2);
    let control = observe(&group);
    let factory_dropped = Arc::new(AtomicBool::new(false));
    let factory_drop = PanickingFactoryDrop(factory_dropped.clone());
    let factory_finished = Arc::new(AtomicBool::new(false));
    let finished = factory_finished.clone();
    task(
        &group,
        "completed-native-factory",
        move |context| {
            // Keep this native Drop field in the factory, not its future.
            let _retained = &factory_drop;
            let finished = finished.clone();
            async move {
                drop(context);
                finished.store(true, Ordering::Release);
                Ok(())
            }
        },
        RestartPolicy::TEMPORARY,
    )
    .unwrap();
    register_once(&group, "healthy-sibling", |context| async move {
        let _context = context;
        pending::<()>().await;
        Ok(())
    })
    .unwrap();
    let run = tokio::spawn(run(group));
    tokio::time::timeout(ASSERT_TIMEOUT, async {
        loop {
            if factory_finished.load(Ordering::Acquire) && control.snapshot().active_children == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the factory's finished incarnation was still holding its closure");
    assert!(!factory_dropped.load(Ordering::Acquire));

    run.abort();
    assert!(tokio::time::timeout(ASSERT_TIMEOUT, run)
        .await
        .unwrap()
        .is_err());
    let terminal = tokio::time::timeout(ASSERT_TIMEOUT, control.wait_complete())
        .await
        .expect("native factory Drop stranded the final cleanup obligation");
    assert!(terminal.is_err());
    let snapshot = control.snapshot();
    assert_eq!(snapshot.phase, Phase::Complete);
    assert_eq!(snapshot.active_children, 0);
    assert_eq!(snapshot.retained_child_records, 0);
    assert!(!snapshot.retained_cleanup_record);
    assert!(snapshot.context_destroyed);
    assert!(factory_dropped.load(Ordering::Acquire));
    assert_eq!(drops.load(Ordering::Acquire), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_event_only_surviving_control_drains_cleanup_and_observes_stream_close() {
    let (context, drops) = context();
    let group = new_group(context, 1);
    let control = observe(&group);
    let (started_tx, started_rx) = oneshot::channel();
    register_once(&group, "event-only-observer", move |context| async move {
        let _context = context;
        let _ = started_tx.send(());
        pending::<()>().await;
        Ok(())
    })
    .unwrap();
    let run = tokio::spawn(run(group));
    tokio::time::timeout(ASSERT_TIMEOUT, started_rx)
        .await
        .unwrap()
        .unwrap();
    run.abort();
    assert!(tokio::time::timeout(ASSERT_TIMEOUT, run)
        .await
        .unwrap()
        .is_err());

    // Deliberately call neither shutdown nor wait_complete. Event observation
    // must close after real cleanup even though its running monitor is gone.
    tokio::time::timeout(ASSERT_TIMEOUT, async {
        while next_event(&control).await.unwrap().is_some() {}
    })
    .await
    .expect("event-only observation never closed the completed group's stream");
    let snapshot = control.snapshot();
    assert_eq!(snapshot.phase, Phase::Complete);
    assert_eq!(snapshot.active_children, 0);
    assert_eq!(snapshot.retained_child_records, 0);
    assert!(!snapshot.retained_cleanup_record);
    assert!(snapshot.context_destroyed);
    assert_eq!(drops.load(Ordering::Acquire), 1);
}

#[test]
fn origin_runtime_cancellation_does_not_restart_a_permanent_child_on_a_surviving_runtime() {
    let (context, drops) = context();
    let group = new_group(context, 1);
    let control = observe(&group);
    let starts = Arc::new(AtomicUsize::new(0));
    let factory_starts = starts.clone();
    task(
        &group,
        "origin-runtime-child",
        move |context| {
            let starts = factory_starts.clone();
            async move {
                let _context = context;
                starts.fetch_add(1, Ordering::Release);
                pending::<()>().await;
                Ok(())
            }
        },
        RestartPolicy::PERMANENT,
    )
    .unwrap();
    // Keep the public owned run future outside a runtime-owned Tokio task.
    // This requires no private child AbortHandle or lifecycle hook.
    let mut running = Box::pin(run(group));
    let origin = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    origin.block_on(async {
        std::future::poll_fn(|cx| {
            assert!(running.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        tokio::time::timeout(ASSERT_TIMEOUT, async {
            while starts.load(Ordering::Acquire) != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the initial child was never polled on its originating runtime");
    });
    drop(origin);
    assert_eq!(starts.load(Ordering::Acquire), 1);

    let survivor = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    survivor.block_on(async {
        let result = tokio::time::timeout(ASSERT_TIMEOUT, running.as_mut()).await;
        if result.is_err() {
            // Rescue the incorrect restarted generation before asserting.
            let _ = shutdown(&control).await;
            let _ = tokio::time::timeout(ASSERT_TIMEOUT, running.as_mut()).await;
        }
        assert!(
            result.is_ok(),
            "origin cancellation resumed normal service on another runtime"
        );
        assert!(
            result.unwrap().is_err(),
            "origin cancellation was treated as successful normal completion"
        );
        assert_eq!(
            starts.load(Ordering::Acquire),
            1,
            "a canceled generation was restarted"
        );
        assert!(shutdown(&control).await.is_err());
    });
    assert_eq!(drops.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn a_reply_completed_before_deadline_succeeds_when_the_caller_polls_after_deadline() {
    let deadline = tokio::time::Instant::now() - Duration::from_millis(2);
    let (sender, receiver) = oneshot::channel();
    assert!(sender
        .send(super::CompletedReply {
            completed_at: deadline - Duration::from_millis(1),
            result: Ok(Ok::<u64, ()>(42)),
        })
        .is_ok());
    // The consumer is first polled after the deadline, with a buffered reply.
    // Producer completion time decides the outcome, not caller scheduling.
    let reply = super::receive_reply(deadline, receiver).await.unwrap();
    assert_eq!(reply, Ok(42));
}

#[tokio::test]
async fn a_reply_completed_after_deadline_times_out_even_when_already_buffered() {
    let deadline = tokio::time::Instant::now() - Duration::from_millis(2);
    let (sender, receiver) = oneshot::channel();
    assert!(sender
        .send(super::CompletedReply {
            completed_at: deadline + Duration::from_millis(1),
            result: Ok(Ok::<u64, ()>(42)),
        })
        .is_ok());
    let error = super::receive_reply(deadline, receiver).await.unwrap_err();
    assert_eq!(error.kind(), super::CallKind::REPLY_TIMEOUT);
}

#[tokio::test]
async fn a_late_buffered_outer_failure_obeys_the_same_reply_deadline() {
    let deadline = tokio::time::Instant::now() - Duration::from_millis(2);
    let (sender, receiver) = oneshot::channel();
    assert!(sender
        .send(super::CompletedReply::<u64, ()> {
            completed_at: deadline + Duration::from_millis(1),
            result: Err(super::CallError::new(
                super::CallKind::REPLY_LOST,
                "accepted generation failed",
            )),
        })
        .is_ok());
    let error = super::receive_reply(deadline, receiver).await.unwrap_err();
    assert_eq!(error.kind(), super::CallKind::REPLY_TIMEOUT);
}
