use super::super::parent_drain::{ParentDrain, ParentError};
use super::*;
use std::{future::pending, sync::atomic::AtomicUsize, task::Waker, time::Duration};

fn limits() -> Limits {
    Limits {
        jobs: 1,
        queue: 1,
        job_roots: 1,
        dns_roots: 1,
        dns_children: 1,
        http_drivers: 1,
        waiters: 1,
        waiter_bytes: 16,
        senders: 3,
    }
}
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
fn make_grant(end: Instant) -> (auth::LeaseOwner, auth::Grant<()>) {
    let owner = auth::LeaseOwner::new(end).unwrap();
    let identity = auth::VerifiedIdentity::from_verified(7, end).unwrap();
    let scope = auth::AuthScope::bind(identity, Arc::clone(&owner.lease)).unwrap();
    (owner, auth::Grant::from_authorized(scope, 91).unwrap())
}
fn owner(sender: &ClientSender) -> &Arc<Mutex<Owner>> {
    sender.owner.as_ref().unwrap()
}
async fn admit(sender: &ClientSender) -> (auth::LeaseOwner, u64) {
    let end = Instant::now() + Duration::from_secs(10);
    let (lease, grant) = make_grant(end);
    (lease, sender.submit(grant, vec![1, 2], end).await.unwrap())
}
async fn pending_child(sender: &ClientSender, id: u64) {
    let (started, ready) = tokio::sync::oneshot::channel();
    assert!(sender
        .spawn(id, TaskKind::DnsChild, async move {
            let _ = started.send(());
            pending::<()>().await;
        })
        .is_ok());
    ready.await.unwrap();
}

#[test]
fn production_registration_is_finite_and_requires_runtime_before_monitor_spawn() {
    let mut parent = ParentDrain::new(1).unwrap();
    assert!(matches!(
        parent.register(limits(), 16),
        Err(ParentError::NoRuntime)
    ));
    assert!(parent.records.is_empty());
    runtime().block_on(async {
        let (_, sender) = parent.register(limits(), 16).unwrap();
        assert!(matches!(
            parent.register(limits(), 16),
            Err(ParentError::Capacity)
        ));
        assert_eq!(parent.records.len(), 1);
        drop(sender);
        parent.shutdown().await;
        assert!(parent.records.is_empty());
        assert!(matches!(
            parent.register(limits(), 16),
            Err(ParentError::Closing)
        ));
    });
}

#[test]
fn eof_monitor_autonomously_joins_inner_before_parent_slot_release() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (id, sender) = parent.register(limits(), 16).unwrap();
        let (_lease, job) = admit(&sender).await;
        pending_child(&sender, job).await;
        let retained = Arc::clone(owner(&sender));
        drop(sender);
        tokio::time::timeout(Duration::from_secs(2), async {
            while !parent.records[0].monitor.as_ref().unwrap().is_finished() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(retained.lock().unwrap().used_tasks(TaskKind::DnsChild), 0);
        assert_eq!(retained.lock().unwrap().used_jobs(), 0);
        assert_eq!(parent.records.len(), 1); // outer join has not yet been observed
        assert_eq!(parent.reap_next().await, Some(id));
        assert!(parent.records.is_empty());
    });
}

#[test]
fn unpolled_admission_and_cancelled_capacity_wait_release_finite_waiter_bytes() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (_, sender) = parent.register(limits(), 16).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        let (_lease, grant) = make_grant(end);
        drop(sender.submit(grant, vec![3], end));
        assert_eq!(owner(&sender).lock().unwrap().registered_waiters(), (0, 0));
        assert_eq!(owner(&sender).lock().unwrap().used_jobs(), 0);
        let (_lease, first) = admit(&sender).await;
        let (_lease2, grant) = make_grant(end);
        let mut wait = Box::pin(sender.submit(grant, vec![4, 5, 6], end));
        assert!(wait
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
        assert_eq!(owner(&sender).lock().unwrap().registered_waiters(), (1, 3));
        let (_lease3, grant) = make_grant(end);
        assert!(matches!(
            sender.submit(grant, vec![7], end).await,
            Err(AdmissionError::Ledger(LedgerError::Capacity))
        ));
        drop(wait);
        assert_eq!(owner(&sender).lock().unwrap().registered_waiters(), (0, 0));
        sender.cancel(first).unwrap();
        parent.shutdown().await;
    });
}

#[test]
fn invalidation_during_real_capacity_wait_prevents_auth_enqueue() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (_, sender) = parent.register(limits(), 16).unwrap();
        let (_first_lease, first) = admit(&sender).await;
        let end = Instant::now() + Duration::from_secs(10);
        let (lease, grant) = make_grant(end);
        let mut wait = Box::pin(sender.submit(grant, vec![9], end));
        assert!(wait.as_mut().poll(&mut Context::from_waker(Waker::noop())).is_pending());
        drop(lease);
        sender.cancel(first).unwrap();
        let result = tokio::time::timeout(Duration::from_secs(2), wait).await.unwrap();
        assert!(matches!(result, Err(AdmissionError::Auth(ref e)) if e.kind() == auth::FailureKind::Expired));
        {
            let state = owner(&sender).lock().unwrap();
            assert_eq!(state.used_jobs(), 0);
            assert_eq!(state.used_queue, 0);
            assert_eq!(state.reserved_roots, [0, 0]);
            assert_eq!(state.registered_waiters(), (0, 0));
        }
        parent.shutdown().await;
    });
}

#[test]
fn actual_grant_bound_target_and_minimum_original_end_survive_permitfirst_invalidation() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (_, sender) = parent.register(limits(), 16).unwrap();
        let now = Instant::now();
        let (lease, grant) = make_grant(now + Duration::from_secs(10));
        let whole = now + Duration::from_secs(5);
        let job = sender.submit(grant, vec![8], whole).await.unwrap();
        drop(lease);
        let command = sender.take_command().unwrap().unwrap();
        assert_eq!(
            (command.id, command.subject, command.resource),
            (job, 7, 91)
        );
        assert_eq!(command.end, whole);
        assert_eq!(command.payload, [8]);
        let accepted = AtomicUsize::new(0);
        sender
            .phase(job, || {
                accepted.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
        sender.cancel(job).unwrap();
        assert!(sender
            .phase(job, || {
                accepted.fetch_add(1, Ordering::SeqCst);
            })
            .is_err());
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
        parent.shutdown().await;
    });
}

#[test]
fn whole_expiry_and_close_never_start_late_native_phase() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (id, sender) = parent.register(limits(), 16).unwrap();
        let now = Instant::now();
        let (_lease, grant) = make_grant(now + Duration::from_secs(10));
        assert!(matches!(
            sender.submit(grant, vec![1], now).await,
            Err(AdmissionError::Ledger(LedgerError::Expired))
        ));
        assert_eq!(owner(&sender).lock().unwrap().used_jobs(), 0);
        let (_lease, job) = admit(&sender).await;
        // Existing private fixture authority fixes an elapsed original job ceiling.
        owner(&sender).lock().unwrap().jobs[0].end = Some(Instant::now());
        let starts = AtomicUsize::new(0);
        assert!(sender
            .phase(job, || {
                starts.fetch_add(1, Ordering::SeqCst);
            })
            .is_err());
        parent.request_close(id).unwrap();
        assert!(sender
            .phase(job, || {
                starts.fetch_add(1, Ordering::SeqCst);
            })
            .is_err());
        assert_eq!(starts.load(Ordering::SeqCst), 0);
        parent.shutdown().await;
    });
}

#[test]
fn unexpected_monitor_abort_keeps_retained_inner_join_and_sticky_fault() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (id, sender) = parent.register(limits(), 16).unwrap();
        let (_lease, job) = admit(&sender).await;
        pending_child(&sender, job).await;
        parent.records[0].monitor.as_ref().unwrap().abort();
        assert_eq!(parent.reap_next().await, Some(id));
        assert_eq!(
            owner(&sender)
                .lock()
                .unwrap()
                .used_tasks(TaskKind::DnsChild),
            0
        );
        assert_eq!(owner(&sender).lock().unwrap().used_jobs(), 0);
        assert!(parent.closing_for_test());
        assert!(sender.phase(job, || panic!("late native")).is_err());
        parent.shutdown().await;
    });
}

#[test]
fn parent_loop_observes_fault_after_healthy_older_client_and_closes_both() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(2).unwrap();
        let (first, healthy) = parent.register(limits(), 16).unwrap();
        let (second, failing) = parent.register(limits(), 16).unwrap();
        let (_first_lease, first_job) = admit(&healthy).await;
        pending_child(&healthy, first_job).await;
        let (_second_lease, second_job) = admit(&failing).await;
        assert!(failing
            .spawn(second_job, TaskKind::JobRoot, async {
                panic!("owned root failure");
            })
            .is_ok());
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), parent.reap_next())
                .await
                .unwrap(),
            Some(second)
        );
        assert!(owner(&healthy).lock().unwrap().is_closing());
        assert!(healthy.phase(first_job, || panic!("late native")).is_err());
        assert_eq!(parent.reap_next().await, Some(first));
        assert!(parent.records.is_empty());
        assert!(matches!(
            parent.register(limits(), 16),
            Err(ParentError::Closing)
        ));
    });
}

#[test]
fn unpolled_parent_drain_retains_outer_and_inner_until_shutdown() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (id, sender) = parent.register(limits(), 16).unwrap();
        let (_lease, job) = admit(&sender).await;
        pending_child(&sender, job).await;
        drop(parent.drain_client(id));
        assert_eq!(parent.records.len(), 1);
        assert_eq!(
            owner(&sender)
                .lock()
                .unwrap()
                .used_tasks(TaskKind::DnsChild),
            1
        );
        parent.shutdown().await;
        assert!(parent.records.is_empty());
        assert_eq!(
            owner(&sender)
                .lock()
                .unwrap()
                .used_tasks(TaskKind::DnsChild),
            0
        );
    });
}

#[test]
fn accepted_unpolled_inner_capture_drop_is_joined_before_capacity_reuse() {
    struct Guard(Arc<AtomicUsize>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (_, sender) = parent.register(limits(), 16).unwrap();
        let (_lease, job) = admit(&sender).await;
        let drops = Arc::new(AtomicUsize::new(0));
        let polls = Arc::new(AtomicUsize::new(0));
        let guard = Guard(Arc::clone(&drops));
        let polled = Arc::clone(&polls);
        assert!(sender
            .spawn(job, TaskKind::DnsChild, async move {
                let _guard = guard;
                polled.fetch_add(1, Ordering::SeqCst);
                pending::<()>().await;
            })
            .is_ok());
        sender.cancel(job).unwrap();
        assert_eq!(
            owner(&sender)
                .lock()
                .unwrap()
                .used_tasks(TaskKind::DnsChild),
            1
        );
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        parent.shutdown().await;
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        assert_eq!(
            owner(&sender)
                .lock()
                .unwrap()
                .used_tasks(TaskKind::DnsChild),
            0
        );
    });
}

#[test]
fn pending_parent_drain_drop_retains_quota_until_actual_inner_drop_and_join() {
    struct SlowDrop {
        entered: std::sync::mpsc::Sender<()>,
        release: std::sync::mpsc::Receiver<()>,
        done: Arc<AtomicBool>,
    }
    impl Drop for SlowDrop {
        fn drop(&mut self) {
            self.entered.send(()).unwrap();
            self.release.recv_timeout(Duration::from_secs(2)).unwrap();
            self.done.store(true, Ordering::SeqCst);
        }
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (id, sender) = parent.register(limits(), 16).unwrap();
        let (_lease, job) = admit(&sender).await;
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let done = Arc::new(AtomicBool::new(false));
        let slow = SlowDrop {
            entered: entered_tx,
            release: release_rx,
            done: Arc::clone(&done),
        };
        let (started, ready) = tokio::sync::oneshot::channel();
        assert!(sender
            .spawn(job, TaskKind::DnsChild, async move {
                let _slow = slow;
                let _ = started.send(());
                pending::<()>().await;
            })
            .is_ok());
        ready.await.unwrap();
        // Outer termination cannot take the retained inner owner with it.
        parent.records[0].monitor.as_ref().unwrap().abort();
        tokio::time::timeout(Duration::from_secs(2), async {
            while !parent.records[0].monitor.as_ref().unwrap().is_finished() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let mut drain = Box::pin(parent.drain_client(id));
        assert!(drain
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(drain);
        assert_eq!(parent.records.len(), 1);
        assert!(parent.records[0].monitor.is_none()); // outer actual join was observed
        assert_eq!(
            owner(&sender)
                .lock()
                .unwrap()
                .used_tasks(TaskKind::DnsChild),
            1
        );
        assert_eq!(owner(&sender).lock().unwrap().used_jobs(), 1);
        assert!(!done.load(Ordering::SeqCst));
        assert!(matches!(
            parent.register(limits(), 16),
            Err(ParentError::Closing)
        ));
        release_tx.send(()).unwrap();
        parent.drain_client(id).await.unwrap();
        assert!(done.load(Ordering::SeqCst));
        assert!(parent.records.is_empty());
        assert_eq!(
            owner(&sender)
                .lock()
                .unwrap()
                .used_tasks(TaskKind::DnsChild),
            0
        );
    });
}

#[test]
fn outer_monitor_panic_is_actual_join_error_and_parent_still_drains_inner() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (id, sender) = parent.register(limits(), 16).unwrap();
        let (_lease, job) = admit(&sender).await;
        pending_child(&sender, job).await;
        {
            let state = owner(&sender).lock().unwrap();
            state.shared.monitor_panic.store(true, Ordering::Release);
            state.shared.notify();
        }
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), parent.reap_next())
                .await
                .unwrap(),
            Some(id)
        );
        assert!(parent.closing_for_test());
        assert!(parent.monitor_panic_for_test());
        assert_eq!(
            owner(&sender)
                .lock()
                .unwrap()
                .used_tasks(TaskKind::DnsChild),
            0
        );
        assert!(sender.phase(job, || panic!("late native")).is_err());
        assert!(parent.records.is_empty());
    });
}

#[test]
fn actual_absolute_timer_expires_queued_work_and_releases_only_after_monitor_observation() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (id, sender) = parent.register(limits(), 16).unwrap();
        let (_lease, grant) = make_grant(Instant::now() + Duration::from_secs(10));
        let end = Instant::now() + Duration::from_millis(250);
        let job = sender.submit(grant, vec![1], end).await.unwrap();
        let mut events = sender.shared.events_tx.subscribe();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if owner(&sender).lock().unwrap().used_jobs() == 0 {
                    break;
                }
                events.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        assert!(Instant::now() >= end);
        assert!(sender.phase(job, || panic!("expired native")).is_err());
        assert_eq!(owner(&sender).lock().unwrap().reserved_roots, [0, 0]);
        assert_eq!(parent.records.len(), 1); // parent registration persists
        parent.request_close(id).unwrap();
        parent.shutdown().await;
        assert!(parent.records.is_empty());
    });
}

#[test]
fn poisoned_client_gate_is_fail_closed_but_retained_parent_can_join_cleanup() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (id, sender) = parent.register(limits(), 16).unwrap();
        let (_lease, job) = admit(&sender).await;
        pending_child(&sender, job).await;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = owner(&sender).lock().unwrap();
            panic!("private gate fixture poison");
        }));
        assert!(result.is_err());
        assert!(sender.phase(job, || panic!("poisoned native")).is_err());
        sender.shared.notify();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), parent.reap_next())
                .await
                .unwrap(),
            Some(id)
        );
        assert!(parent.closing_for_test());
        assert!(parent.records.is_empty());
        let state = owner(&sender).lock().unwrap_err().into_inner();
        assert_eq!(state.fault(), Some(Fault::PoisonedGate));
        assert_eq!(state.used_tasks(TaskKind::DnsChild), 0);
    });
}

#[test]
fn popped_command_retains_reservations_until_first_actual_task_registration() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (_, sender) = parent.register(limits(), 16).unwrap();
        let (_lease, job) = admit(&sender).await;
        let command = sender.take_command().unwrap().unwrap();
        assert_eq!(command.id, job);
        {
            let mut state = owner(&sender).lock().unwrap();
            assert!(state
                .poll_parent(&mut Context::from_waker(Waker::noop()))
                .is_pending());
            assert_eq!(state.used_jobs(), 1);
            assert_eq!(state.reserved_roots, [1, 1]);
        }
        assert!(sender
            .spawn(job, TaskKind::JobRoot, pending::<()>())
            .is_ok());
        sender.cancel(job).unwrap();
        parent.shutdown().await;
        assert_eq!(owner(&sender).lock().unwrap().used_jobs(), 0);
    });
}
