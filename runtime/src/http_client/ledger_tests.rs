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
        owner.freeze_dns(id).unwrap();
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
        owner.freeze_dns(id).unwrap();
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
            .as_ref()
            .unwrap()
            .abort_handle();
        owner.freeze_dns(id).unwrap();
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
        owner.freeze_dns(id).unwrap();
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
        owner.freeze_dns(id).unwrap();
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
        owner.freeze_dns(id).unwrap();
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
