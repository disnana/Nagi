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

#[test]
fn branded_runner_rejects_foreign_same_id_at_every_port() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(2).unwrap();
        let (a, sender_a) = parent.register(limits(), 16).unwrap();
        let (b, sender_b) = parent.register(limits(), 16).unwrap();
        let (_lease_a, id_a) = admit(&sender_a).await;
        let (_lease_b, id_b) = admit(&sender_b).await;
        assert_eq!(id_a, id_b);
        let runner_a = parent.runner_owner(a).unwrap();
        let runner_b = parent.runner_owner(b).unwrap();
        let key_a = runner_a.take_command().unwrap().unwrap().key;
        let key_b = runner_b.take_command().unwrap().unwrap().key;
        assert!(matches!(
            runner_a.spawn(&key_b, TaskKind::JobRoot, async {}),
            Err((LedgerError::UnknownJob, _))
        ));
        assert_eq!(
            owner(&sender_a)
                .lock()
                .unwrap()
                .used_tasks(TaskKind::JobRoot),
            0
        );
        let starts = AtomicUsize::new(0);
        assert_eq!(
            runner_a.phase(&key_b, || starts.fetch_add(1, Ordering::SeqCst)),
            Err(LedgerError::UnknownJob)
        );
        assert_eq!(runner_a.cancel(&key_b), Err(LedgerError::UnknownJob));
        assert_eq!(starts.load(Ordering::SeqCst), 0);
        assert!(runner_a
            .spawn(&key_a, TaskKind::JobRoot, pending::<()>())
            .is_ok());
        assert_eq!(
            runner_a.phase(&key_a, || starts.fetch_add(1, Ordering::SeqCst)),
            Ok(0)
        );
        runner_a.cancel(&key_a).unwrap();
        runner_b.cancel(&key_b).unwrap();
        parent.shutdown().await;
    });
}

#[test]
fn old_key_cannot_authorize_reused_numeric_slot() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (client, sender) = parent.register(limits(), 16).unwrap();
        let runner = parent.runner_owner(client).unwrap();
        let (_lease, first) = admit(&sender).await;
        let old = runner.take_command().unwrap().unwrap().key;
        assert_eq!(old.id(), first);
        runner.cancel(&old).unwrap();
        {
            let mut state = owner(&sender).lock().unwrap();
            state.release_finished_jobs();
            assert_eq!(state.used_jobs(), 0);
            state.next_job_id = first; // Private ABA fixture; production never resets IDs.
        }
        let (_lease, second) = admit(&sender).await;
        assert_eq!(first, second);
        let fresh = runner.take_command().unwrap().unwrap().key;
        assert!(matches!(
            runner.spawn(&old, TaskKind::JobRoot, async {}),
            Err((LedgerError::UnknownJob, _))
        ));
        assert_eq!(runner.phase(&old, || ()), Err(LedgerError::UnknownJob));
        assert_eq!(runner.cancel(&old), Err(LedgerError::UnknownJob));
        assert_eq!(
            owner(&sender).lock().unwrap().used_tasks(TaskKind::JobRoot),
            0
        );
        assert!(runner
            .spawn(&fresh, TaskKind::JobRoot, pending::<()>())
            .is_ok());
        runner.cancel(&fresh).unwrap();
        parent.shutdown().await;
    });
}

#[test]
fn runner_handle_is_not_counted_and_last_sender_closes_eof() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (client, sender) = parent.register(limits(), 16).unwrap();
        let runner = parent.runner_owner(client).unwrap();
        assert_eq!(sender.shared.senders.load(Ordering::Acquire), 1);
        let (_lease, _) = admit(&sender).await;
        let key = runner.take_command().unwrap().unwrap().key;
        assert!(runner
            .spawn(&key, TaskKind::JobRoot, pending::<()>())
            .is_ok());
        assert!(key.seal.root_registered());
        assert_eq!(
            key.seal.completion.load(Ordering::Acquire) & ROOT_ACTUAL_JOINED,
            0
        );
        drop(sender);
        assert_eq!(
            runner
                .owner
                .lock()
                .unwrap()
                .shared
                .senders
                .load(Ordering::Acquire),
            0
        );
        assert_eq!(runner.phase(&key, || ()), Err(LedgerError::Closing));
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), parent.reap_next())
                .await
                .unwrap(),
            Some(client)
        );
        assert_ne!(
            key.seal.completion.load(Ordering::Acquire) & ROOT_ACTUAL_JOINED,
            0
        );
        assert_eq!(
            runner.owner.lock().unwrap().used_tasks(TaskKind::JobRoot),
            0
        );
        assert_eq!(runner.owner.lock().unwrap().used_jobs(), 0);
    });
}

#[test]
fn drop_posts_stop_without_owner_or_waiter_lock() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (_, sender) = parent.register(limits(), 16).unwrap();
        let retained = Arc::clone(owner(&sender));
        let shared = Arc::clone(&sender.shared);
        let owner_gate = retained.lock().unwrap();
        let waiter_gate = shared.waiters.lock().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            drop(sender);
            tx.send(()).unwrap();
        });
        let timely = rx.recv_timeout(Duration::from_secs(1)).is_ok();
        drop(waiter_gate);
        drop(owner_gate);
        thread.join().unwrap();
        assert!(timely, "last Sender Drop blocked on Owner/waiter lock");
        assert!(shared.is_stopping());
        parent.shutdown().await;
    });
}

#[test]
fn parent_drop_posts_stop_and_pending_grant_cannot_enqueue() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (_, sender) = parent.register(limits(), 16).unwrap();
        let (_first_lease, first) = admit(&sender).await;
        let end = Instant::now() + Duration::from_secs(10);
        let (_second_lease, grant) = make_grant(end);
        let mut waiting = Box::pin(sender.submit(grant, vec![9], end));
        assert!(waiting
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
        let retained = Arc::clone(owner(&sender));
        let gate = retained.lock().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            drop(parent);
            tx.send(()).unwrap();
        });
        let timely = rx.recv_timeout(Duration::from_secs(1)).is_ok();
        drop(gate);
        thread.join().unwrap();
        assert!(timely, "ParentDrain Drop blocked on Owner lock");
        assert!(sender.shared.is_stopping());
        assert!(matches!(
            waiting.await,
            Err(AdmissionError::Ledger(LedgerError::Closing))
        ));
        assert_eq!(retained.lock().unwrap().next_job_id, first + 1); // No second enqueue.
        drop(sender);
    });
}

#[test]
fn stop_after_consumed_real_auth_permit_refuses_record_publication() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (_, sender) = parent.register(limits(), 16).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        let (_lease, grant) = make_grant(end);
        let mut reservation = std::cell::OnceCell::new();
        {
            let mut state = owner(&sender).lock().unwrap();
            let (key, tasks) = state.reserve_command(1).unwrap();
            reservation.get_or_init(|| Reservation {
                key,
                payload: vec![4],
                tasks,
            });
            let answer =
                grant.submit_private(&mut reservation, end, |subject, resource, bound, slot| {
                    state.shared.request_stop();
                    state.enqueue(subject, resource, bound, slot)
                });
            assert!(matches!(answer, Ok(Err(LedgerError::Closing))));
            assert!(reservation.get().is_some());
            assert_eq!(state.used_jobs(), 0);
            assert_eq!(state.used_queue, 0);
            assert_eq!(state.reserved_roots, [0, 0]);
        }
        drop(reservation);
        parent.shutdown().await;
    });
}

#[test]
fn last_sender_drop_does_not_wait_for_watch_value_borrow() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (_, sender) = parent.register(limits(), 16).unwrap();
        let receiver = sender.shared.events_tx.subscribe();
        let watch_guard = receiver.borrow();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            drop(sender);
            tx.send(()).unwrap();
        });
        let completed_without_borrow = rx.recv_timeout(Duration::from_secs(1)).is_ok();
        drop(watch_guard);
        worker.join().unwrap();
        parent.shutdown().await;
        assert!(
            completed_without_borrow,
            "Sender Drop waited on watch borrow"
        );
    });
}

#[test]
fn owner_destructor_posts_stop_without_waiting_for_waiter_gate() {
    let (owner, sender) = Owner::checked(limits()).unwrap();
    let shared = Arc::clone(&sender.shared);
    let waiter_gate = shared.waiters.lock().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        drop(owner);
        tx.send(()).unwrap();
    });
    let completed_without_gate = rx.recv_timeout(Duration::from_secs(1)).is_ok();
    drop(waiter_gate);
    worker.join().unwrap();
    drop(sender);
    assert!(completed_without_gate, "Owner Drop waited on waiter gate");
    assert!(shared.is_stopping());
}

#[test]
fn entry_guard_preserves_stop_after_a_winning_cas() {
    let (owner, sender) = Owner::checked(limits()).unwrap();
    let entry = owner.shared.begin_enqueue().unwrap();
    owner.shared.request_stop();
    assert_eq!(
        owner.shared.stop_admit.load(Ordering::Acquire),
        STOP_REQUESTED | ENQUEUE_ACCEPTED
    );
    drop(entry);
    assert_eq!(
        owner.shared.stop_admit.load(Ordering::Acquire),
        STOP_REQUESTED
    );
    assert!(matches!(
        owner.shared.begin_enqueue(),
        Err(LedgerError::Closing)
    ));
    drop(sender);
    drop(owner);
}

#[test]
fn unwinding_entry_guard_closes_without_clearing_stop() {
    let (owner, sender) = Owner::checked(limits()).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _entry = owner.shared.begin_enqueue().unwrap();
        panic!("fixture after accepted entry");
    }));
    assert!(result.is_err());
    assert_eq!(
        owner.shared.stop_admit.load(Ordering::Acquire),
        STOP_REQUESTED
    );
    assert!(matches!(
        owner.shared.begin_enqueue(),
        Err(LedgerError::Closing)
    ));
    drop(sender);
    drop(owner);
}

#[test]
fn repeated_stop_has_no_self_wake_while_pending_root_is_joined() {
    runtime().block_on(async {
        let (mut owner, sender) = Owner::checked(limits()).unwrap();
        let id = owner.fixture_admit().unwrap();
        assert!(owner
            .try_spawn(id, TaskKind::JobRoot, pending::<()>())
            .is_ok());
        let mut events = owner.shared.events_tx.subscribe();
        owner.close();
        assert!(events.has_changed().unwrap());
        let _ = events.borrow_and_update();
        owner.close();
        owner.close();
        assert!(!events.has_changed().unwrap());

        let shared = Arc::clone(&owner.shared);
        let stopped = shared.stop_notify.notified();
        tokio::pin!(stopped);
        stopped.as_mut().enable();
        shared.request_stop();
        assert!(tokio::time::timeout(Duration::from_millis(20), stopped)
            .await
            .is_err());
        owner.drain_all().await;
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 0);
        assert_eq!(owner.used_jobs(), 0);
        drop(sender);
    });
}

#[test]
fn auth_enqueue_winning_entry_retains_record_after_concurrent_stop() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (_, sender) = parent.register(limits(), 16).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        let (_lease, grant) = make_grant(end);
        let mut reservation = std::cell::OnceCell::new();
        {
            let mut state = owner(&sender).lock().unwrap();
            let (key, tasks) = state.reserve_command(1).unwrap();
            reservation.get_or_init(|| Reservation {
                key,
                payload: vec![4],
                tasks,
            });
            let shared = Arc::clone(&sender.shared);
            let answer =
                grant.submit_private(&mut reservation, end, |subject, resource, bound, slot| {
                    state.enqueue_with_after_entry(subject, resource, bound, slot, || {
                        shared.request_stop();
                    })
                });
            assert!(matches!(answer, Ok(Ok(_))));
            assert!(reservation.get().is_none());
            assert_eq!(state.used_jobs(), 1);
            assert_eq!(state.used_queue, 1);
            assert!(shared.is_stopping());
        }
        drop(reservation);
        parent.shutdown().await;
        assert_eq!(owner(&sender).lock().unwrap().used_jobs(), 0);
        drop(sender);
    });
}

#[test]
fn branded_phase_winner_finishes_once_then_stop_denies_next_entry() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (client, sender) = parent.register(limits(), 16).unwrap();
        let runner = parent.runner_owner(client).unwrap();
        let (_lease, _) = admit(&sender).await;
        let command = runner.take_command().unwrap().unwrap();
        let key = command.key.clone();
        drop(command);
        assert!(runner
            .spawn(&key, TaskKind::JobRoot, pending::<()>())
            .is_ok());
        let shared = Arc::clone(&sender.shared);
        let calls = Arc::new(AtomicUsize::new(0));
        let witness = Arc::clone(&calls);
        assert_eq!(
            runner.phase(&key, || {
                shared.request_stop();
                witness.fetch_add(1, Ordering::SeqCst);
                7
            }),
            Ok(7)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(runner.phase(&key, || 9), Err(LedgerError::Closing));
        assert_eq!(
            key.seal.completion.load(Ordering::Acquire) & ROOT_ACTUAL_JOINED,
            0
        );
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), parent.reap_next())
                .await
                .unwrap(),
            Some(client)
        );
        assert_ne!(
            key.seal.completion.load(Ordering::Acquire) & ROOT_ACTUAL_JOINED,
            0
        );
        assert_eq!(runner.owner.lock().unwrap().used_jobs(), 0);
        drop(sender);
    });
}

#[test]
fn branded_driver_history_rejects_second_driver_after_first_actual_join() {
    runtime().block_on(async {
        let mut parent = ParentDrain::new(1).unwrap();
        let (client, sender) = parent.register(limits(), 16).unwrap();
        let runner = parent.runner_owner(client).unwrap();
        let (_lease, _) = admit(&sender).await;
        let command = runner.take_command().unwrap().unwrap();
        let key = command.key.clone();
        drop(command);
        assert!(runner
            .spawn(&key, TaskKind::JobRoot, pending::<()>())
            .is_ok());
        let mut events = sender.shared.events_tx.subscribe();
        assert!(runner.spawn(&key, TaskKind::HttpDriver, async {}).is_ok());
        tokio::time::timeout(Duration::from_secs(2), async {
            while key.seal.completion.load(Ordering::Acquire) & DRIVER_ACTUAL_JOINED == 0 {
                events
                    .changed()
                    .await
                    .expect("retained monitor event stream");
            }
        })
        .await
        .expect("retained monitor actual driver join");
        assert_ne!(
            key.seal.completion.load(Ordering::Acquire) & DRIVER_ACTUAL_JOINED,
            0
        );
        let rejected = runner.spawn(&key, TaskKind::HttpDriver, async {});
        assert!(matches!(rejected, Err((LedgerError::PhaseClosed, _))));
        drop(rejected);
        runner.cancel(&key).unwrap();
        parent.shutdown().await;
        drop(sender);
    });
}
