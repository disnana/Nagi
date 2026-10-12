use super::*;
use std::time::{Duration, Instant};

pub(super) fn fixture() -> (Arc<Mutex<Owner>>, ClientSender, u64) {
    let (mut owner, sender) = Owner::checked(Limits {
        jobs: 2,
        queue: 2,
        job_roots: 2,
        dns_roots: 2,
        dns_children: 2,
        http_drivers: 2,
        waiters: 2,
        waiter_bytes: 128,
        senders: 4,
    })
    .unwrap();
    let id = owner.fixture_admit().unwrap();
    owner.jobs.iter_mut().find(|j| j.id == id).unwrap().end =
        Some(Instant::now() + Duration::from_secs(2));
    owner
        .jobs
        .iter_mut()
        .find(|j| j.id == id)
        .unwrap()
        .reserved_roots[1] = true;
    owner.reserved_roots[1] += 1;
    owner
        .try_spawn(id, TaskKind::JobRoot, std::future::pending())
        .unwrap_or_else(|_| panic!("fixture root"));
    let owner = Arc::new(Mutex::new(owner));
    let mut sender = sender;
    sender.owner = Some(owner.clone());
    (owner, sender, id)
}

fn phase(sender: &ClientSender, id: u64) -> DnsPhase {
    let phase = DnsPhase::for_job(
        sender.owner.as_ref().unwrap(),
        id,
        Instant::now() + Duration::from_secs(1),
        "127.0.0.1:53".parse().unwrap(),
    )
    .unwrap();
    phase
        .spawn(TaskKind::DnsRoot, std::future::pending())
        .unwrap();
    phase
}
pub(super) async fn cleanup(owner: &Arc<Mutex<Owner>>) {
    owner.lock().unwrap().close();
    std::future::poll_fn(|cx| owner.lock().unwrap().poll_parent(cx)).await;
}

#[tokio::test]
async fn healthy_job_root_survives_dns_only_drain() {
    let (owner, sender, id) = fixture();
    let phase = phase(&sender, id);
    phase.finish().await.unwrap();
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::DnsRoot), 0);
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::JobRoot), 1);
    assert_eq!(owner.lock().unwrap().used_jobs(), 1);
    assert!(owner.lock().unwrap().fault().is_none());
    cleanup(&owner).await;
    drop(sender);
}

#[tokio::test]
async fn spawn_refusal_is_sticky_and_rejected_future_is_unpolled_outside_gate() {
    let (owner, sender, id) = fixture();
    let phase = phase(&sender, id);
    for _ in 0..2 {
        phase
            .spawn(TaskKind::DnsChild, std::future::pending())
            .unwrap();
    }
    struct Rejected {
        owner: Arc<Mutex<Owner>>,
        polled: Arc<AtomicBool>,
        outside: Arc<AtomicBool>,
    }
    impl Future for Rejected {
        type Output = ();
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            self.polled.store(true, Ordering::Release);
            Poll::Pending
        }
    }
    impl Drop for Rejected {
        fn drop(&mut self) {
            self.outside
                .store(self.owner.try_lock().is_ok(), Ordering::Release);
        }
    }
    let polled = Arc::new(AtomicBool::new(false));
    let outside = Arc::new(AtomicBool::new(false));
    assert!(phase
        .spawn(
            TaskKind::DnsChild,
            Rejected {
                owner: owner.clone(),
                polled: polled.clone(),
                outside: outside.clone()
            }
        )
        .is_err());
    assert!(!polled.load(Ordering::Acquire));
    assert!(outside.load(Ordering::Acquire));
    assert_eq!(owner.lock().unwrap().fault(), Some(Fault::ProviderDenied));
    assert!(phase.finish().await.is_err());
    cleanup(&owner).await;
    drop(sender);
}

#[tokio::test]
async fn first_native_poll_is_zero_after_close_expiry_or_cancel() {
    for mode in 0..3 {
        let (owner, sender, id) = fixture();
        let phase = phase(&sender, id);
        match mode {
            0 => phase.close(),
            1 => {
                owner
                    .lock()
                    .unwrap()
                    .jobs
                    .iter_mut()
                    .find(|j| j.id == id)
                    .unwrap()
                    .end = Some(Instant::now())
            }
            _ => owner.lock().unwrap().request_cancel(id).unwrap(),
        }
        let polls = AtomicUsize::new(0);
        let result = phase.poll_io(|_| {
            polls.fetch_add(1, Ordering::Relaxed);
            Poll::Ready(Ok(()))
        });
        assert!(matches!(result, Poll::Ready(Err(_))));
        assert_eq!(polls.load(Ordering::Relaxed), 0);
        assert!(phase.begin_io().is_err());
        let drained = phase.finish().await;
        if mode == 0 {
            drained.unwrap();
        } else {
            assert!(drained.is_err());
        }
        cleanup(&owner).await;
        drop(sender);
    }
}

#[tokio::test]
async fn pending_and_unpolled_drain_drop_keeps_same_join_records() {
    let (owner, sender, id) = fixture();
    let phase = phase(&sender, id);
    drop(phase.finish()); // An unpolled waiter cannot claim a join or release.
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::DnsRoot), 1);
    let mut waiting = Box::pin(phase.finish());
    std::future::poll_fn(|cx| {
        assert!(waiting.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(waiting);
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::DnsRoot), 1);
    phase.finish().await.unwrap();
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::DnsRoot), 0);
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::JobRoot), 1);
    cleanup(&owner).await;
    drop(sender);
}

#[tokio::test]
async fn early_child_panic_cannot_become_success() {
    let (owner, sender, id) = fixture();
    let phase = phase(&sender, id);
    let (tx, rx) = tokio::sync::oneshot::channel();
    phase
        .spawn(TaskKind::DnsChild, async move {
            let _ = tx.send(());
            panic!("owned DNS child fault");
        })
        .unwrap();
    rx.await.unwrap();
    assert!(phase.finish().await.is_err());
    assert_eq!(owner.lock().unwrap().fault(), Some(Fault::TaskPanic));
    cleanup(&owner).await;
    drop(sender);
}

#[tokio::test]
async fn sender_eof_closes_admission_and_still_joins_dns() {
    let (owner, sender, id) = fixture();
    let phase = phase(&sender, id);
    drop(sender);
    let count = AtomicUsize::new(0);
    assert!(matches!(
        phase.poll_io(|_| {
            count.fetch_add(1, Ordering::Relaxed);
            Poll::Ready(Ok(()))
        }),
        Poll::Ready(Err(_))
    ));
    assert_eq!(count.load(Ordering::Relaxed), 0);
    assert!(phase.finish().await.is_err());
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::DnsRoot), 0);
    cleanup(&owner).await;
}

#[tokio::test]
async fn close_receipts_do_not_reopen_native_start_capacity() {
    let (owner, sender, id) = fixture();
    let phase = phase(&sender, id);
    for _ in 0..IO_STARTS {
        drop(phase.begin_io().unwrap());
    }
    assert_eq!(phase.0.book.lock().unwrap().io_live, 0);
    assert_eq!(phase.0.book.lock().unwrap().io_closed, IO_STARTS);
    assert!(phase.begin_io().is_err());
    phase.finish().await.unwrap();
    cleanup(&owner).await;
    drop(sender);
}

#[tokio::test]
async fn retained_io_ticket_prevents_success_without_close_receipt() {
    let (owner, sender, id) = fixture();
    let phase = phase(&sender, id);
    let ticket = phase.begin_io().unwrap();
    assert!(phase.finish().await.is_err());
    assert_eq!(owner.lock().unwrap().fault(), Some(Fault::ProviderDenied));
    drop(ticket);
    cleanup(&owner).await;
    drop(sender);
}

#[tokio::test]
async fn one_original_dns_reservation_cannot_start_two_roots() {
    let (owner, sender, id) = fixture();
    let first = DnsPhase::for_job(
        &owner,
        id,
        Instant::now() + Duration::from_secs(1),
        "127.0.0.1:53".parse().unwrap(),
    )
    .unwrap();
    let second = DnsPhase::for_job(
        &owner,
        id,
        Instant::now() + Duration::from_secs(1),
        "127.0.0.1:53".parse().unwrap(),
    )
    .unwrap();
    first
        .spawn(TaskKind::DnsRoot, std::future::pending())
        .unwrap();
    assert!(second
        .spawn(TaskKind::DnsRoot, std::future::pending())
        .is_err());
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::DnsRoot), 1);
    assert!(first.finish().await.is_err());
    cleanup(&owner).await;
    drop(sender);
}

#[tokio::test]
async fn dropped_operation_guard_stops_but_does_not_claim_join() {
    let (owner, sender, id) = fixture();
    let phase = phase(&sender, id);
    drop(StopOnDrop(phase.clone()));
    assert!(phase.0.book.lock().unwrap().closed);
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::DnsRoot), 1);
    phase.finish().await.unwrap();
    cleanup(&owner).await;
    drop(sender);
}

#[tokio::test]
async fn actual_parent_monitor_and_dns_waiter_converge_on_same_join_records() {
    let mut parent = ParentDrain::new(1).unwrap();
    let (client_id, sender) = parent
        .register(
            Limits {
                jobs: 2,
                queue: 2,
                job_roots: 2,
                dns_roots: 2,
                dns_children: 2,
                http_drivers: 2,
                waiters: 2,
                waiter_bytes: 128,
                senders: 4,
            },
            128,
        )
        .unwrap();
    let owner = sender.owner.as_ref().unwrap().clone();
    // Trusted cfg(test) job fixture inside an actual registered parent. No Auth
    // stub is invoked, and this is not proof of production protected admission.
    let id = {
        let mut state = owner.lock().unwrap();
        let id = state.fixture_admit().unwrap();
        let job = state.jobs.iter_mut().find(|j| j.id == id).unwrap();
        job.end = Some(Instant::now() + Duration::from_secs(2));
        job.reserved_roots[1] = true;
        state.reserved_roots[1] += 1;
        state
            .try_spawn(id, TaskKind::JobRoot, std::future::pending())
            .unwrap_or_else(|_| panic!("fixture root"));
        id
    };
    let phase = phase(&sender, id);
    phase.spawn(TaskKind::DnsChild, async {}).unwrap();
    tokio::task::yield_now().await;
    phase.finish().await.unwrap();
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::DnsRoot), 0);
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::DnsChild), 0);
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::JobRoot), 1);
    parent.request_close(client_id).unwrap();
    parent.shutdown().await;
    assert!(parent.records.is_empty());
    assert_eq!(owner.lock().unwrap().used_jobs(), 0);
    drop(sender);
}

#[tokio::test]
async fn poisoned_dns_book_cannot_publish_a_healthy_drain() {
    let (owner, sender, id) = fixture();
    let phase = phase(&sender, id);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _held = phase.0.book.lock().unwrap();
        panic!("owned phase gate poison");
    }));
    assert!(caught.is_err());
    assert!(phase.finish().await.is_err());
    assert_eq!(owner.lock().unwrap().fault(), Some(Fault::PoisonedGate));
    cleanup(&owner).await;
    drop(sender);
}

#[tokio::test]
async fn owned_udp_primitive_gate_stops_sends_and_closes_before_join() {
    use tokio::net::UdpSocket;
    let server = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let endpoint = server.local_addr().unwrap();
    let (owner, sender, id) = fixture();
    let phase = DnsPhase::for_job(
        &owner,
        id,
        Instant::now() + Duration::from_secs(1),
        endpoint,
    )
    .unwrap();
    let child_phase = phase.clone();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    phase
        .spawn(TaskKind::DnsRoot, async move {
            let ticket = child_phase.begin_io().unwrap();
            let native = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0)));
            tokio::pin!(native);
            let raw = std::future::poll_fn(|cx| child_phase.poll_io(|_| native.as_mut().poll(cx)))
                .await
                .unwrap();
            let io = tokio_io::TrackedUdp::new(raw, ticket, child_phase.clone());
            std::future::poll_fn(|cx| io.poll_send_to(cx, b"q", endpoint))
                .await
                .unwrap();
            let _ = ready_tx.send(());
            std::future::pending::<()>().await;
            drop(io);
        })
        .unwrap();
    ready_rx.await.unwrap();
    let mut bytes = [0; 8];
    let (n, _) = server.recv_from(&mut bytes).await.unwrap();
    assert_eq!(&bytes[..n], b"q");
    assert_eq!(phase.0.book.lock().unwrap().io_live, 1);
    phase.finish().await.unwrap();
    assert_eq!(phase.0.book.lock().unwrap().io_live, 0);
    assert_eq!(phase.0.book.lock().unwrap().io_closed, 1);
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::DnsRoot), 0);
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::JobRoot), 1);
    assert!(phase.begin_io().is_err());
    assert!(matches!(
        phase.poll_io(|_| panic!("late primitive poll forbidden")),
        Poll::<io::Result<()>>::Ready(Err(_))
    ));
    cleanup(&owner).await;
    drop(sender);
    drop(server);
}

#[tokio::test]
async fn stopped_dns_must_observe_actual_child_join_before_success() {
    let (owner, sender, id) = fixture();
    let dropped = Arc::new(AtomicBool::new(false));
    struct DropReceipt(Arc<AtomicBool>);
    impl Drop for DropReceipt {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let receipt = DropReceipt(dropped.clone());
    let phase = DnsPhase::for_job(
        &owner,
        id,
        Instant::now() + Duration::from_secs(1),
        "127.0.0.1:53".parse().unwrap(),
    )
    .unwrap();
    phase
        .spawn(TaskKind::DnsRoot, std::future::pending())
        .unwrap();
    phase
        .spawn(TaskKind::DnsChild, async move {
            let _receipt = receipt;
            std::future::pending::<()>().await;
        })
        .unwrap_or_else(|_| panic!("fixture spawn"));
    phase.finish().await.unwrap();
    let used = owner.lock().unwrap().used_tasks(TaskKind::DnsChild);
    let saw_drop = dropped.load(Ordering::Acquire);
    // Even a RED test cleans the same retained handle before its assertion.
    owner.lock().unwrap().close();
    std::future::poll_fn(|cx| owner.lock().unwrap().poll_parent(cx)).await;
    drop(sender);
    assert_eq!(
        used, 0,
        "freeze alone must not return success with a retained child"
    );
    assert!(saw_drop, "native future disposal must precede success");
}

#[tokio::test]
async fn followup_known_parent_or_user_cancel_is_not_unexpected_fault() {
    for close_parent in [false, true] {
        let (owner, sender, id) = fixture();
        let phase = phase(&sender, id);
        phase
            .spawn(TaskKind::DnsChild, std::future::pending())
            .unwrap();
        if close_parent {
            owner.lock().unwrap().close();
        } else {
            owner.lock().unwrap().request_cancel(id).unwrap();
        }
        let result = phase.finish().await;
        assert_eq!(result, Err(DnsFailure::Ledger(LedgerError::Closing)));
        assert_eq!(owner.lock().unwrap().fault(), None);
        assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::DnsRoot), 0);
        assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::DnsChild), 0);
        cleanup(&owner).await;
        drop(sender);
    }
}
#[tokio::test]
async fn followup_first_observed_panic_precedes_later_unexpected_abort() {
    let (owner, sender, id) = fixture();
    let phase = phase(&sender, id);
    phase
        .spawn(TaskKind::DnsChild, async {
            panic!("owned child failure");
        })
        .unwrap();
    phase
        .spawn(TaskKind::DnsChild, std::future::pending())
        .unwrap();
    {
        let state = owner.lock().unwrap();
        let job = state.jobs.iter().find(|j| j.id == id).unwrap();
        job.tasks
            .iter()
            .filter(|t| t.kind == TaskKind::DnsChild)
            .nth(1)
            .unwrap()
            .handle
            .as_ref()
            .unwrap()
            .abort();
    }
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let ready = {
                let state = owner.lock().unwrap();
                state
                    .jobs
                    .iter()
                    .find(|j| j.id == id)
                    .unwrap()
                    .tasks
                    .iter()
                    .filter(|t| t.kind == TaskKind::DnsChild)
                    .all(|t| t.handle.as_ref().unwrap().is_finished())
            };
            if ready {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(phase.finish().await.is_err());
    assert_eq!(owner.lock().unwrap().fault(), Some(Fault::TaskPanic));
    cleanup(&owner).await;
    drop(sender);
}
