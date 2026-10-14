use super::*;
use std::{
    future::{pending, Future},
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll, Waker},
};
use tokio::sync::oneshot;

struct DeliveryWakeCounter(AtomicUsize);
impl std::task::Wake for DeliveryWakeCounter {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn sf06_paused_delivery_retains_actual_handle_quota_and_join_bit() {
    struct DropProbe(Arc<AtomicUsize>);
    impl Future for DropProbe {
        type Output = ();
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            Poll::Pending
        }
    }
    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let live = runtime();
    let closed_runtime = runtime();
    let closed = closed_runtime.handle().clone();
    drop(closed_runtime);
    let (mut owner, sender) = Owner::checked(limits()).unwrap();
    let id = owner.fixture_admit().unwrap();
    let seal = Arc::clone(&owner.jobs[0].seal);
    let drops = Arc::new(AtomicUsize::new(0));
    let (delivery, waiting) = GateFreeFuture::new(DropProbe(Arc::clone(&drops)));
    let registered = {
        let _entered = closed.enter();
        owner
            .try_spawn_delivery(id, TaskKind::JobRoot, waiting, &delivery, false)
            .map_err(|(error, _)| error)
    };
    let wake = Arc::new(DeliveryWakeCounter(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&wake));
    let mut cx = Context::from_waker(&waker);
    let polled = owner.poll_parent(&mut cx);
    let before = (
        owner.used_tasks(TaskKind::JobRoot),
        owner
            .jobs
            .first()
            .is_some_and(|job| job.tasks[0].handle.is_some()),
        seal.completion.load(Ordering::Acquire) & ROOT_ACTUAL_JOINED,
        drops.load(Ordering::SeqCst),
    );
    let delivered = delivery.finish(registered).is_ok();
    live.block_on(owner.close_and_drain());
    let after = (owner.used_jobs(), drops.load(Ordering::SeqCst));
    drop(sender);
    assert!(delivered);
    assert_eq!(after, (0, 1));
    assert_eq!(
        before,
        (1, true, 0, 0),
        "pre-ack handle/quota/join/drop snapshot"
    );
    assert!(polled.is_pending());
}

#[test]
fn sf06_delivery_register_recheck_and_independent_broadcast_wakes() {
    let receipt = DeliveryReceipt::new();
    let recheck = Arc::new(DeliveryWakeCounter(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&recheck));
    let cx = Context::from_waker(&waker);
    assert!(receipt.ready_with_after_register(&cx, || receipt.settle()));
    assert_eq!(recheck.0.load(Ordering::SeqCst), 1);

    let (owner, sender) = Owner::checked(limits()).unwrap();
    let receipt = DeliveryReceipt::new();
    receipt.shared.set(Arc::clone(&owner.shared)).unwrap();
    let counters: Vec<_> = (0..5)
        .map(|_| Arc::new(DeliveryWakeCounter(AtomicUsize::new(0))))
        .collect();
    let wakers: Vec<_> = counters
        .iter()
        .map(|counter| Waker::from(Arc::clone(counter)))
        .collect();
    let mut events_a = owner.shared.events_tx.subscribe();
    let mut events_b = owner.shared.events_tx.subscribe();
    let mut events_cancelled = owner.shared.events_tx.subscribe();
    let mut watch_a = Box::pin(events_a.changed());
    let mut watch_b = Box::pin(events_b.changed());
    let mut cancelled = Box::pin(events_cancelled.changed());
    let mut stop_a = Box::pin(owner.shared.stop_notify.notified());
    let mut stop_b = Box::pin(owner.shared.stop_notify.notified());
    stop_a.as_mut().enable();
    stop_b.as_mut().enable();
    assert!(watch_a
        .as_mut()
        .poll(&mut Context::from_waker(&wakers[0]))
        .is_pending());
    assert!(watch_b
        .as_mut()
        .poll(&mut Context::from_waker(&wakers[1]))
        .is_pending());
    assert!(cancelled
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_pending());
    drop(cancelled);
    assert!(stop_a
        .as_mut()
        .poll(&mut Context::from_waker(&wakers[2]))
        .is_pending());
    assert!(stop_b
        .as_mut()
        .poll(&mut Context::from_waker(&wakers[3]))
        .is_pending());
    assert!(!receipt.ready(&Context::from_waker(Waker::noop())));
    // Replace a cancelled bare waiter; broadcasts must remain independent.
    assert!(!receipt.ready(&Context::from_waker(&wakers[4])));
    receipt.settle();
    assert!(counters
        .iter()
        .all(|counter| counter.0.load(Ordering::SeqCst) >= 1));
    assert!(receipt.ready(&cx));
    assert!(watch_a
        .as_mut()
        .poll(&mut Context::from_waker(&wakers[0]))
        .is_ready());
    assert!(watch_b
        .as_mut()
        .poll(&mut Context::from_waker(&wakers[1]))
        .is_ready());
    assert!(stop_a
        .as_mut()
        .poll(&mut Context::from_waker(&wakers[2]))
        .is_ready());
    assert!(stop_b
        .as_mut()
        .poll(&mut Context::from_waker(&wakers[3]))
        .is_ready());
    drop(sender);
}

#[test]
fn sf06_failed_send_blocking_drop_retains_debt_and_cancelled_bare_drain() {
    struct BlockingDrop {
        entered: std::sync::mpsc::SyncSender<()>,
        release: std::sync::mpsc::Receiver<()>,
        drops: Arc<AtomicUsize>,
    }
    impl Future for BlockingDrop {
        type Output = ();
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            Poll::Pending
        }
    }
    impl Drop for BlockingDrop {
        fn drop(&mut self) {
            let _ = self.entered.send(());
            let _ = self.release.recv_timeout(std::time::Duration::from_secs(5));
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }
    let live = runtime();
    let dead = runtime();
    let closed = dead.handle().clone();
    drop(dead);
    let (mut owner, sender) = Owner::checked(limits()).unwrap();
    let id = owner.fixture_admit().unwrap();
    let seal = Arc::clone(&owner.jobs[0].seal);
    let drops = Arc::new(AtomicUsize::new(0));
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let (delivery, waiting) = GateFreeFuture::new(BlockingDrop {
        entered: entered_tx,
        release: release_rx,
        drops: Arc::clone(&drops),
    });
    let receipt = Arc::clone(&delivery.receipt);
    let registered = {
        let _entered = closed.enter();
        owner
            .try_spawn_delivery(id, TaskKind::JobRoot, waiting, &delivery, false)
            .map_err(|(error, _)| error)
    };
    let worker = std::thread::spawn(move || delivery.finish(registered).is_ok());
    let entered = entered_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .is_ok();
    let wake = Arc::new(DeliveryWakeCounter(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&wake));
    let mut cx = Context::from_waker(&waker);
    let mut abandoned = Box::pin(owner.drain_all());
    let first_pending = abandoned.as_mut().poll(&mut cx).is_pending();
    drop(abandoned);
    let before = (
        owner.used_tasks(TaskKind::JobRoot),
        owner
            .jobs
            .first()
            .is_some_and(|job| job.tasks[0].handle.is_some()),
        seal.completion.load(Ordering::Acquire) & ROOT_ACTUAL_JOINED,
        drops.load(Ordering::SeqCst),
        owner.task_is_finished(id, TaskKind::JobRoot),
        receipt.is_settled(),
    );
    let mut resumed = Box::pin(owner.drain_all());
    let resumed_pending = resumed.as_mut().poll(&mut cx).is_pending();
    wake.0.store(0, Ordering::SeqCst);
    let _ = release_tx.send(());
    let delivered = worker.join().unwrap_or(false);
    let wake_count = wake.0.load(Ordering::SeqCst);
    live.block_on(resumed);
    drop(sender);
    assert!(entered && first_pending && resumed_pending && delivered);
    assert_eq!(before, (1, true, 0, 0, false, false));
    assert!(
        wake_count >= 1,
        "settlement must wake a resumed bare drain before re-poll"
    );
    assert_eq!(owner.used_jobs(), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn sf06_delivery_drop_panic_does_not_acknowledge() {
    struct PanicDrop;
    impl Future for PanicDrop {
        type Output = ();
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            Poll::Pending
        }
    }
    impl Drop for PanicDrop {
        fn drop(&mut self) {
            panic!("original Drop fixture panic");
        }
    }
    let (delivery, waiting) = GateFreeFuture::new(PanicDrop);
    let receipt = Arc::clone(&delivery.receipt);
    drop(waiting);
    let panicked =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| delivery.finish(Ok(())))).is_err();
    assert!(panicked);
    assert!(!receipt.is_settled());
}

#[test]
fn sf06_stop_expiry_and_fault_cannot_join_before_delivery() {
    let live = runtime();
    let dead = runtime();
    let closed = dead.handle().clone();
    drop(dead);
    let mut snapshots = Vec::new();
    for condition in 0..3 {
        let (mut owner, sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        let seal = Arc::clone(&owner.jobs[0].seal);
        let (delivery, waiting) = GateFreeFuture::new(pending::<()>());
        let registered = {
            let _entered = closed.enter();
            owner
                .try_spawn_delivery(id, TaskKind::JobRoot, waiting, &delivery, false)
                .map_err(|(error, _)| error)
        };
        match condition {
            0 => owner.shared.request_stop(),
            1 => owner.jobs[0].end = Some(std::time::Instant::now()),
            _ => {
                owner.latch_fault(Fault::PoisonedGate);
                owner.close();
            }
        }
        let pending = owner
            .poll_parent(&mut Context::from_waker(Waker::noop()))
            .is_pending();
        snapshots.push((
            pending,
            owner.used_tasks(TaskKind::JobRoot),
            owner.jobs[0].tasks[0].handle.is_some(),
            seal.completion.load(Ordering::Acquire) & ROOT_ACTUAL_JOINED,
            owner.jobs[0].cancel_requested,
        ));
        let delivered = delivery.finish(registered).is_ok();
        live.block_on(owner.close_and_drain());
        drop(sender);
        assert!(delivered);
        assert_eq!(owner.used_jobs(), 0);
    }
    assert_eq!(snapshots, vec![(true, 1, true, 0, true); 3]);
}

#[test]
fn sf06_receipt_tracks_exact_reused_task_slot() {
    runtime().block_on(async {
        let (mut owner, sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        must_spawn(&mut owner, id, TaskKind::JobRoot, pending::<()>());
        let (first, waiting) = GateFreeFuture::new(async {});
        let old_receipt = Arc::clone(&first.receipt);
        let registered = owner
            .try_spawn_delivery(id, TaskKind::DnsChild, waiting, &first, false)
            .map_err(|(error, _)| error);
        let pending_before = owner
            .poll_parent(&mut Context::from_waker(Waker::noop()))
            .is_pending();
        let denied = matches!(
            owner.try_spawn(id, TaskKind::DnsChild, async {}),
            Err((LedgerError::Capacity, _))
        );
        let old_slot = Arc::clone(&owner.jobs[0].tasks[1].handle);
        let old_generation = old_slot.identity.get().unwrap().generation;
        let first_delivered = first.finish(registered).is_ok();
        owner.join_matching(Some(id), true).await;
        let (second, waiting) = GateFreeFuture::new(async {});
        let new_receipt = Arc::clone(&second.receipt);
        let registered = owner
            .try_spawn_delivery(id, TaskKind::DnsChild, waiting, &second, false)
            .map_err(|(error, _)| error);
        let current_slot = &owner.jobs[0].tasks[1].handle;
        assert!(!Arc::ptr_eq(&old_slot, current_slot));
        assert!(current_slot.identity.get().unwrap().generation > old_generation);
        assert_eq!(current_slot.identity.get().unwrap().key.id, id);
        old_receipt.settle();
        let _ = owner.poll_parent(&mut Context::from_waker(Waker::noop()));
        let before = (
            owner.used_tasks(TaskKind::DnsChild),
            owner.jobs[0].tasks.len(),
            owner.jobs[0].tasks[1].handle.is_some(),
            Arc::ptr_eq(
                owner.jobs[0].tasks[1].delivery.as_ref().unwrap(),
                &new_receipt,
            ),
            new_receipt.is_settled(),
        );
        let second_delivered = second.finish(registered).is_ok();
        owner.close_and_drain().await;
        drop(sender);
        assert!(pending_before && denied && first_delivered && second_delivered);
        assert_eq!(before, (1, 2, true, true, false));
        assert_eq!(owner.used_jobs(), 0);
    });
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn limits() -> Limits {
    Limits {
        jobs: 1,
        queue: 1,
        job_roots: 1,
        dns_roots: 1,
        dns_children: 1,
        http_drivers: 1,
        waiters: 1,
        waiter_bytes: 4,
        senders: 2,
    }
}

fn fixture_key(owner: &Owner, id: u64) -> JobKey {
    let job = owner.jobs.iter().find(|job| job.id == id).unwrap();
    JobKey {
        id,
        seal: Arc::clone(&job.seal),
    }
}

fn must_spawn<F: Future<Output = ()> + Send + 'static>(
    owner: &mut Owner,
    id: u64,
    kind: TaskKind,
    future: F,
) {
    if let Err((reason, rejected)) = owner.try_spawn(id, kind, future) {
        drop(rejected);
        panic!("tracked spawn rejected: {reason:?}");
    }
}

#[test]
fn reported_complete_is_not_join_observation() {
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        let (reported_tx, reported_rx) = oneshot::channel();
        must_spawn(&mut owner, id, TaskKind::JobRoot, async move {
            let _ = reported_tx.send(());
            pending::<()>().await;
        });
        reported_rx.await.unwrap();
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 1);
        assert_eq!(owner.used_jobs(), 1);
        owner.request_cancel(id).unwrap();
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 1);
        owner.drain_all().await;
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 0);
        assert_eq!(owner.used_jobs(), 0);
    });
}

#[test]
fn finished_task_keeps_quota_until_join_is_observed() {
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        must_spawn(&mut owner, id, TaskKind::JobRoot, async {});
        for _ in 0..32 {
            if owner.task_is_finished(id, TaskKind::JobRoot) {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(owner.task_is_finished(id, TaskKind::JobRoot));
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 1);
        owner.drain_all().await;
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 0);
        assert_eq!(owner.used_jobs(), 0);
    });
}

#[test]
fn cancelled_drain_future_retains_owned_handle_and_quota() {
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        let (release_tx, release_rx) = oneshot::channel::<()>();
        must_spawn(&mut owner, id, TaskKind::JobRoot, async move {
            let _ = release_rx.await;
        });
        let mut drain = Box::pin(owner.drain_all());
        let mut cx = Context::from_waker(Waker::noop());
        assert!(matches!(drain.as_mut().poll(&mut cx), Poll::Pending));
        drop(drain);
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 1);
        release_tx.send(()).unwrap();
        owner.drain_all().await;
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 0);
    });
}

#[test]
fn rejected_future_is_never_polled_and_pending_task_is_joined_on_cancel() {
    struct CountPolls(Arc<AtomicUsize>);
    impl Future for CountPolls {
        type Output = ();
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Poll::Pending
        }
    }
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        let first = Arc::new(AtomicUsize::new(0));
        must_spawn(&mut owner, id, TaskKind::JobRoot, CountPolls(first.clone()));
        let rejected = Arc::new(AtomicUsize::new(0));
        let (reason, rejected_future) = owner
            .try_spawn(id, TaskKind::JobRoot, CountPolls(rejected.clone()))
            .unwrap_err();
        assert_eq!(reason, LedgerError::Capacity);
        drop(rejected_future);
        assert_eq!(rejected.load(Ordering::SeqCst), 0);
        owner.request_cancel(id).unwrap();
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 1);
        owner.drain_all().await;
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 0);
    });
}

#[test]
fn empty_unpolled_fixture_job_holds_then_returns_finite_reservation() {
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let _id = owner.fixture_admit().unwrap();
        assert_eq!(owner.used_jobs(), 1);
        assert_eq!(owner.fixture_admit().unwrap_err(), LedgerError::Capacity);
        owner.close_and_drain().await;
        assert_eq!(owner.used_jobs(), 0);
        assert!(owner.is_closing());
    });
}

#[test]
fn registered_waiter_count_and_bytes_are_bounded_and_drop_releases() {
    let (owner, _sender) = Owner::checked(limits()).unwrap();
    let waiter = owner.try_register_waiter(vec![7; 4]).unwrap();
    let (reason, payload) = owner.try_register_waiter(vec![9]).unwrap_err();
    assert_eq!(reason, LedgerError::Capacity);
    assert_eq!(payload, vec![9]);
    assert_eq!(owner.registered_waiters(), (1, 4));
    drop(waiter);
    assert_eq!(owner.registered_waiters(), (0, 0));
    let (reason, payload) = owner.try_register_waiter(vec![3; 5]).unwrap_err();
    assert_eq!(reason, LedgerError::Capacity);
    assert_eq!(payload.len(), 5);
    assert_eq!(owner.registered_waiters(), (0, 0));
    let waiter = owner.try_register_waiter(vec![1; 4]).unwrap();
    drop(waiter);
}

#[test]
fn last_sender_eof_closes_waiters_but_owner_still_joins_tasks() {
    runtime().block_on(async {
        let (mut owner, sender) = Owner::checked(limits()).unwrap();
        let second = sender.try_clone().unwrap();
        let mut waiter = owner.try_register_waiter(vec![0; 2]).unwrap();
        let id = owner.fixture_admit().unwrap();
        must_spawn(&mut owner, id, TaskKind::JobRoot, pending::<()>());
        drop(sender);
        assert!(!owner.is_closing());
        drop(second);
        waiter.wait_closed().await;
        assert!(owner.is_closing());
        assert_eq!(owner.registered_waiters(), (0, 0));
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 1);
        owner.close_and_drain().await;
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 0);
    });
}

#[test]
fn sticky_first_fault_survives_successful_root_join() {
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        must_spawn(&mut owner, id, TaskKind::JobRoot, async {});
        owner.latch_fault(Fault::ProviderDenied);
        owner.latch_fault(Fault::TaskPanic);
        assert_eq!(owner.fault(), Some(Fault::ProviderDenied));
        let (_, rejected) = owner
            .try_spawn(id, TaskKind::DnsRoot, async {})
            .unwrap_err();
        drop(rejected);
        owner.drain_all().await;
        assert_eq!(owner.fault(), Some(Fault::ProviderDenied));
    });
}

#[test]
fn dns_drain_joins_dns_root_without_waiting_for_job_root() {
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        must_spawn(&mut owner, id, TaskKind::JobRoot, pending::<()>());
        must_spawn(&mut owner, id, TaskKind::DnsRoot, async {});
        let key = fixture_key(&owner, id);
        owner.freeze_dns(&key).unwrap();
        owner.drain_dns(id).await.unwrap();
        assert_eq!(owner.used_tasks(TaskKind::DnsRoot), 0);
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 1);
        let (_, rejected) = owner
            .try_spawn(id, TaskKind::DnsChild, async {})
            .unwrap_err();
        drop(rejected);
        owner.request_cancel(id).unwrap();
        owner.drain_all().await;
    });
}

#[test]
fn dns_drain_observes_panicked_job_root_while_dns_child_is_pending() {
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        let (started_tx, started_rx) = oneshot::channel();
        must_spawn(&mut owner, id, TaskKind::DnsChild, async move {
            let _ = started_tx.send(());
            pending::<()>().await;
        });
        started_rx.await.unwrap();
        let (root_started_tx, root_started_rx) = oneshot::channel();
        let (release_root_tx, release_root_rx) = oneshot::channel::<()>();
        must_spawn(&mut owner, id, TaskKind::JobRoot, async move {
            let _ = root_started_tx.send(());
            release_root_rx.await.unwrap();
            panic!("DNS-stage job root panic fixture")
        });
        root_started_rx.await.unwrap();
        let key = fixture_key(&owner, id);
        owner.freeze_dns(&key).unwrap();
        let mut drain = Box::pin(owner.drain_dns(id));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(matches!(drain.as_mut().poll(&mut cx), Poll::Pending));
        release_root_tx.send(()).unwrap();
        assert_eq!(drain.await, Err(LedgerError::Faulted));
        assert_eq!(owner.fault(), Some(Fault::TaskPanic));
        assert_eq!(owner.used_tasks(TaskKind::DnsChild), 0);
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 1);
        assert!(owner.jobs[0]
            .tasks
            .iter()
            .any(|task| { task.kind == TaskKind::JobRoot && task.handle.is_some() }));
        owner.drain_all().await;
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 0);
    });
}

#[test]
fn dns_drain_observes_unexpected_job_root_cancel_while_dns_child_is_pending() {
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        let (started_tx, started_rx) = oneshot::channel();
        must_spawn(&mut owner, id, TaskKind::DnsChild, async move {
            let _ = started_tx.send(());
            pending::<()>().await;
        });
        started_rx.await.unwrap();
        let (root_started_tx, root_started_rx) = oneshot::channel();
        must_spawn(&mut owner, id, TaskKind::JobRoot, async move {
            let _ = root_started_tx.send(());
            pending::<()>().await;
        });
        root_started_rx.await.unwrap();
        let abort_root = owner.jobs[0]
            .tasks
            .iter()
            .find(|task| task.kind == TaskKind::JobRoot)
            .unwrap()
            .handle
            .state()
            .as_ref()
            .unwrap()
            .abort_handle();
        let key = fixture_key(&owner, id);
        owner.freeze_dns(&key).unwrap();
        let mut drain = Box::pin(owner.drain_dns(id));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(matches!(drain.as_mut().poll(&mut cx), Poll::Pending));
        abort_root.abort();
        assert_eq!(drain.await, Err(LedgerError::Faulted));
        assert_eq!(owner.fault(), Some(Fault::UnexpectedCancel));
        assert_eq!(owner.used_tasks(TaskKind::DnsChild), 0);
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 1);
        assert!(owner.jobs[0]
            .tasks
            .iter()
            .any(|task| { task.kind == TaskKind::JobRoot && task.handle.is_some() }));
        owner.drain_all().await;
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 0);
    });
}

#[test]
fn dns_drain_retains_healthy_completed_job_root_for_final_drain() {
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        let (release_tx, release_rx) = oneshot::channel::<()>();
        must_spawn(&mut owner, id, TaskKind::DnsChild, async move {
            let _ = release_rx.await;
        });
        must_spawn(&mut owner, id, TaskKind::JobRoot, async {});
        for _ in 0..32 {
            if owner.task_is_finished(id, TaskKind::JobRoot) {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(owner.task_is_finished(id, TaskKind::JobRoot));
        let key = fixture_key(&owner, id);
        owner.freeze_dns(&key).unwrap();
        release_tx.send(()).unwrap();
        assert_eq!(owner.drain_dns(id).await, Ok(()));
        assert_eq!(owner.fault(), None);
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 1);
        assert!(owner.jobs[0]
            .tasks
            .iter()
            .any(|task| { task.kind == TaskKind::JobRoot && task.handle.is_some() }));
        owner.drain_all().await;
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 0);
        assert_eq!(owner.used_jobs(), 0);
    });
}

#[test]
fn dns_drain_observes_job_root_future_drop_panic_after_ready() {
    struct ReadyThenPanicOnDrop(oneshot::Receiver<()>);
    impl Future for ReadyThenPanicOnDrop {
        type Output = ();
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            Pin::new(&mut self.get_mut().0).poll(cx).map(|_| ())
        }
    }
    impl Drop for ReadyThenPanicOnDrop {
        fn drop(&mut self) {
            panic!("JobRoot Future Drop panic after Ready")
        }
    }
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        let (child_started_tx, child_started_rx) = oneshot::channel();
        must_spawn(&mut owner, id, TaskKind::DnsChild, async move {
            let _ = child_started_tx.send(());
            pending::<()>().await;
        });
        child_started_rx.await.unwrap();
        let (release_root_tx, release_root_rx) = oneshot::channel();
        must_spawn(
            &mut owner,
            id,
            TaskKind::JobRoot,
            ReadyThenPanicOnDrop(release_root_rx),
        );
        let key = fixture_key(&owner, id);
        owner.freeze_dns(&key).unwrap();
        let mut drain = Box::pin(owner.drain_dns(id));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(matches!(drain.as_mut().poll(&mut cx), Poll::Pending));
        release_root_tx.send(()).unwrap();
        assert_eq!(drain.await, Err(LedgerError::Faulted));
        assert_eq!(owner.fault(), Some(Fault::TaskPanic));
        assert_eq!(owner.used_tasks(TaskKind::DnsChild), 0);
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 1);
        assert!(owner.jobs[0]
            .tasks
            .iter()
            .any(|task| { task.kind == TaskKind::JobRoot && task.handle.is_some() }));
        owner.drain_all().await;
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 0);
        assert_eq!(owner.used_jobs(), 0);
    });
}

#[test]
fn task_panic_is_classified_after_join_and_quota_is_returned() {
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        // Registration order must not make a pending child hide a root panic.
        must_spawn(&mut owner, id, TaskKind::DnsChild, pending::<()>());
        must_spawn(&mut owner, id, TaskKind::JobRoot, async {
            panic!("fixture panic")
        });
        // The child record survives even when the root panics.
        for _ in 0..32 {
            if owner.task_is_finished(id, TaskKind::JobRoot) {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(owner.task_is_finished(id, TaskKind::JobRoot));
        // The owner must abort and join the pending child after observing panic.
        owner.drain_all().await;
        assert_eq!(owner.fault(), Some(Fault::TaskPanic));
        assert_eq!(owner.used_jobs(), 0);
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 0);
        assert_eq!(owner.used_tasks(TaskKind::DnsChild), 0);
    });
}

#[test]
fn close_requests_abort_without_releasing_task_slot() {
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        must_spawn(&mut owner, id, TaskKind::JobRoot, pending::<()>());
        owner.close();
        assert!(owner.is_closing());
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 1);
        owner.drain_all().await;
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 0);
    });
}

#[test]
fn cancelled_job_rejects_new_root_and_http_driver_without_polling() {
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        owner.request_cancel(id).unwrap();
        for kind in [TaskKind::JobRoot, TaskKind::HttpDriver] {
            let polls = Arc::new(AtomicUsize::new(0));
            let count = Arc::clone(&polls);
            let (reason, rejected) = owner
                .try_spawn(id, kind, async move {
                    count.fetch_add(1, Ordering::SeqCst);
                })
                .unwrap_err();
            assert_eq!(reason, LedgerError::Closing);
            drop(rejected);
            tokio::task::yield_now().await;
            assert_eq!(polls.load(Ordering::SeqCst), 0);
            assert_eq!(owner.used_tasks(kind), 0);
        }
        owner.drain_all().await;
        assert_eq!(owner.used_jobs(), 0);
    });
}

#[test]
fn http_driver_slot_requires_its_own_join_observation() {
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        must_spawn(&mut owner, id, TaskKind::HttpDriver, async {});
        assert_eq!(owner.used_tasks(TaskKind::HttpDriver), 1);
        owner.drain_all().await;
        assert_eq!(owner.used_tasks(TaskKind::HttpDriver), 0);
        assert_eq!(owner.used_jobs(), 0);
    });
}

#[test]
fn joined_dns_record_is_reused_before_http_child_registration() {
    runtime().block_on(async {
        let (mut owner, _sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        must_spawn(&mut owner, id, TaskKind::JobRoot, pending::<()>());
        must_spawn(&mut owner, id, TaskKind::DnsRoot, async {});
        let key = fixture_key(&owner, id);
        owner.freeze_dns(&key).unwrap();
        owner.drain_dns(id).await.unwrap();
        assert_eq!(owner.fixture_task_records(id), 2);
        must_spawn(&mut owner, id, TaskKind::HttpDriver, async {});
        assert_eq!(owner.fixture_task_records(id), 2);
        owner.request_cancel(id).unwrap();
        owner.drain_all().await;
    });
}

#[test]
fn finite_limits_and_sender_cap_are_checked() {
    let mut invalid = limits();
    invalid.waiter_bytes = 0;
    assert_eq!(
        Owner::checked(invalid).unwrap_err(),
        LedgerError::InvalidLimits
    );
    let (owner, sender) = Owner::checked(limits()).unwrap();
    let second = sender.try_clone().unwrap();
    assert_eq!(sender.try_clone().unwrap_err(), LedgerError::Capacity);
    drop(second);
    drop(sender);
    assert!(owner.is_closing());
}

#[test]
fn sf06_h0_launch_publication_wakes_independent_waiters_and_retains_receipt() {
    let live = runtime();
    let closed_runtime = runtime();
    let closed = closed_runtime.handle().clone();
    drop(closed_runtime);
    let (mut owner, sender) = Owner::checked(limits()).unwrap();
    let job = owner.fixture_admit().unwrap();
    let (delivery, waiting) = GateFreeFuture::new(pending::<()>());
    let (launch, waiting) = {
        let _entered = closed.enter();
        owner
            .stage(
                job,
                TaskKind::JobRoot,
                waiting,
                false,
                Some(Arc::clone(&delivery.receipt)),
                PreparedLaunch::new(TaskKind::JobRoot),
            )
            .unwrap_or_else(|_| panic!("stage fixture"))
    };
    let owner = Arc::new(Mutex::new(owner));
    let counters: Vec<_> = (0..4)
        .map(|_| Arc::new(DeliveryWakeCounter(AtomicUsize::new(0))))
        .collect();
    let wakers: Vec<_> = counters
        .iter()
        .map(|counter| Waker::from(Arc::clone(counter)))
        .collect();
    let mut events_a = launch.shared.events_tx.subscribe();
    let mut events_b = launch.shared.events_tx.subscribe();
    let mut watch_a = Box::pin(events_a.changed());
    let mut watch_b = Box::pin(events_b.changed());
    let mut stopped = Box::pin(launch.shared.stop_notify.notified());
    stopped.as_mut().enable();
    assert!(watch_a
        .as_mut()
        .poll(&mut Context::from_waker(&wakers[0]))
        .is_pending());
    assert!(watch_b
        .as_mut()
        .poll(&mut Context::from_waker(&wakers[1]))
        .is_pending());
    assert!(stopped
        .as_mut()
        .poll(&mut Context::from_waker(&wakers[2]))
        .is_pending());
    launch.slot.register(&Context::from_waker(Waker::noop()));
    launch.slot.register(&Context::from_waker(&wakers[3]));
    launch.run(&owner, waiting);
    assert!(counters
        .iter()
        .all(|counter| counter.0.load(Ordering::SeqCst) > 0));
    assert!(matches!(*launch.slot.state(), Launch::Running(_)));
    assert!(!delivery.receipt.is_settled());
    {
        let mut owner = owner.lock().unwrap();
        owner.close();
        assert!(owner
            .poll_parent(&mut Context::from_waker(Waker::noop()))
            .is_pending());
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 1);
    }
    // A cancelled waiter and its replacement retain the exact receipt debt.
    let mut draining = Box::pin(std::future::poll_fn(|cx| {
        owner.lock().unwrap().poll_parent(cx)
    }));
    assert!(draining
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_pending());
    drop(draining);
    assert!(delivery.finish(Ok(())).is_ok());
    live.block_on(std::future::poll_fn(|cx| {
        owner.lock().unwrap().poll_parent(cx)
    }));
    assert!(matches!(*launch.slot.state(), Launch::ActualJoined));
    assert_eq!(owner.lock().unwrap().used_jobs(), 0);
    drop(sender);
}
