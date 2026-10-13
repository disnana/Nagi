use super::*;
use std::time::{Duration, Instant};

#[test]
fn sf06_paused_dns_delivery_retains_io_ticket_until_failed_send_drop() {
    struct TicketFuture(IoTicket);
    impl Future for TicketFuture {
        type Output = ();
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            let _ = &self.0;
            Poll::Pending
        }
    }
    let live = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (owner, sender, phase) = live.block_on(async {
        let (owner, sender, id) = fixture();
        let phase = phase(&sender, id);
        (owner, sender, phase)
    });
    let ticket = phase.begin_io().unwrap();
    let (delivery, waiting) = GateFreeFuture::new(TicketFuture(ticket));
    let closed_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let closed = closed_runtime.handle().clone();
    drop(closed_runtime);
    let registered = {
        let _entered = closed.enter();
        phase.gate(|owner, _, _| {
            owner
                .try_spawn_delivery(
                    phase.0.key.id(),
                    TaskKind::DnsChild,
                    waiting,
                    &delivery,
                    true,
                )
                .map_err(|(error, _)| error.into())
        })
    };
    phase.close().unwrap();
    live.block_on(async {
        while !owner
            .lock()
            .unwrap()
            .task_is_finished(phase.0.key.id(), TaskKind::DnsRoot)
        {
            tokio::task::yield_now().await;
        }
    });
    let mut finishing = Box::pin(phase.finish());
    let mut cx = Context::from_waker(Waker::noop());
    let pre_result = finishing.as_mut().poll(&mut cx);
    let before = {
        let state = owner.lock().unwrap();
        (
            state.used_tasks(TaskKind::DnsChild),
            state.fault(),
            phase.0.book.lock().unwrap().io_live,
        )
    };
    drop(finishing);
    let registered = registered.map_err(|error| match error {
        DnsFailure::Ledger(error) => error,
        _ => LedgerError::PhaseClosed,
    });
    let delivered = delivery.finish(registered).is_ok();
    live.block_on(cleanup(&owner));
    let after = (
        owner.lock().unwrap().used_jobs(),
        phase.0.book.lock().unwrap().io_live,
    );
    drop(sender);
    assert!(delivered);
    assert_eq!(after, (0, 0));
    assert_eq!(
        before,
        (1, None, 1),
        "pre-ack DNS count/fault/raw-IO snapshot"
    );
    assert!(
        pre_result.is_pending(),
        "DNS finish returned before original IoTicket Drop: {pre_result:?}"
    );
}

#[test]
fn sf06_multiple_dns_waiters_and_cancelled_waiter_wake_on_delivery() {
    struct Counter(AtomicUsize);
    impl std::task::Wake for Counter {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let live = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (owner, sender, phase) = live.block_on(async {
        let (owner, sender, id) = fixture();
        let phase = phase(&sender, id);
        (owner, sender, phase)
    });
    let dead = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let closed = dead.handle().clone();
    drop(dead);
    let (delivery, waiting) = GateFreeFuture::new(std::future::pending::<()>());
    let receipt = Arc::clone(&delivery.receipt);
    let registered = {
        let _entered = closed.enter();
        phase.gate(|owner, _, _| {
            owner
                .try_spawn_delivery(
                    phase.0.key.id(),
                    TaskKind::DnsChild,
                    waiting,
                    &delivery,
                    true,
                )
                .map_err(|(error, _)| error.into())
        })
    };
    phase.close().unwrap();
    live.block_on(async {
        while !owner
            .lock()
            .unwrap()
            .task_is_finished(phase.0.key.id(), TaskKind::DnsRoot)
        {
            tokio::task::yield_now().await;
        }
    });
    let counters: Vec<_> = (0..3)
        .map(|_| Arc::new(Counter(AtomicUsize::new(0))))
        .collect();
    let wakers: Vec<_> = counters
        .iter()
        .map(|counter| Waker::from(Arc::clone(counter)))
        .collect();
    let mut cancelled = Box::pin(phase.finish());
    let cancelled_pending = cancelled
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_pending();
    drop(cancelled);
    let mut first = Box::pin(phase.finish());
    let mut second = Box::pin(phase.finish());
    let first_pending = first
        .as_mut()
        .poll(&mut Context::from_waker(&wakers[0]))
        .is_pending();
    let second_pending = second
        .as_mut()
        .poll(&mut Context::from_waker(&wakers[1]))
        .is_pending();
    // The second waiter's initial close posts an event to the first. Consume
    // that event and register both observers again before measuring settlement.
    let first_registered = first
        .as_mut()
        .poll(&mut Context::from_waker(&wakers[0]))
        .is_pending();
    let second_registered = second
        .as_mut()
        .poll(&mut Context::from_waker(&wakers[1]))
        .is_pending();
    // A monitor/bare waiter replaces the receipt's single waker. Both DNS
    // waiters still require the independent shared watch broadcast.
    let monitor_pending = owner
        .lock()
        .unwrap()
        .poll_parent(&mut Context::from_waker(&wakers[2]))
        .is_pending();
    for counter in &counters {
        counter.0.store(0, Ordering::SeqCst);
    }
    let before = (
        owner.lock().unwrap().used_tasks(TaskKind::DnsChild),
        receipt.is_settled(),
    );
    let delivered = delivery
        .finish(registered.map_err(|error| match error {
            DnsFailure::Ledger(error) => error,
            _ => LedgerError::PhaseClosed,
        }))
        .is_ok();
    let woke: Vec<_> = counters
        .iter()
        .map(|counter| counter.0.load(Ordering::SeqCst))
        .collect();
    let results = live.block_on(async { (first.await, second.await) });
    live.block_on(cleanup(&owner));
    drop(sender);
    assert!(
        cancelled_pending
            && first_pending
            && second_pending
            && first_registered
            && second_registered
            && monitor_pending
            && delivered
    );
    assert_eq!(before, (1, false));
    assert!(
        woke.iter().all(|count| *count >= 1),
        "delivery must wake every registered observer: {woke:?}"
    );
    assert!(results.0.is_err() && results.1.is_err());
    assert_eq!(owner.lock().unwrap().used_jobs(), 0);
}

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

pub(super) fn fixture_runner(owner: &Arc<Mutex<Owner>>) -> RunnerOwnerHandle {
    RunnerOwnerHandle {
        owner: Arc::clone(owner),
    }
}
pub(super) fn fixture_key(owner: &Arc<Mutex<Owner>>, id: u64) -> JobKey {
    let state = owner.lock().unwrap();
    let job = state.jobs.iter().find(|job| job.id == id).unwrap();
    JobKey {
        id,
        seal: Arc::clone(&job.seal),
    }
}

fn phase(sender: &ClientSender, id: u64) -> DnsPhase {
    let owner = sender.owner.as_ref().unwrap();
    let runner = fixture_runner(owner);
    let key = fixture_key(owner, id);
    let phase = DnsPhase::for_job(
        &runner,
        &key,
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
            0 => phase.close().unwrap(),
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
    let runner = fixture_runner(&owner);
    let key = fixture_key(&owner, id);
    let first = DnsPhase::for_job(
        &runner,
        &key,
        Instant::now() + Duration::from_secs(1),
        "127.0.0.1:53".parse().unwrap(),
    )
    .unwrap();
    let second = DnsPhase::for_job(
        &runner,
        &key,
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
    let runner = fixture_runner(&owner);
    let key = fixture_key(&owner, id);
    let phase = DnsPhase::for_job(
        &runner,
        &key,
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
    let runner = fixture_runner(&owner);
    let key = fixture_key(&owner, id);
    let phase = DnsPhase::for_job(
        &runner,
        &key,
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

#[cfg(test)]
mod dns_brand_tests {
    use super::super::super::admission::Command;
    use super::super::super::parent_drain::RunnerOwnerHandle;
    use super::*;
    use crate::auth;
    use std::time::Duration;

    fn limits() -> Limits {
        Limits {
            jobs: 2,
            queue: 2,
            job_roots: 2,
            dns_roots: 2,
            dns_children: 2,
            http_drivers: 2,
            waiters: 2,
            waiter_bytes: 32,
            senders: 4,
        }
    }
    fn grant(end: Instant) -> (auth::LeaseOwner, auth::Grant<()>) {
        let lease = auth::LeaseOwner::new(end).unwrap();
        let identity = auth::VerifiedIdentity::from_verified(7, end).unwrap();
        let scope = auth::AuthScope::bind(identity, Arc::clone(&lease.lease)).unwrap();
        (lease, auth::Grant::from_authorized(scope, 91).unwrap())
    }
    async fn admit(
        parent: &ParentDrain,
        client: u64,
        sender: &ClientSender,
    ) -> (RunnerOwnerHandle, Command, JobKey, auth::LeaseOwner) {
        let end = Instant::now() + Duration::from_secs(10);
        let (lease, authorized) = grant(end);
        sender.submit(authorized, vec![1], end).await.unwrap();
        let runner = parent.runner_owner(client).unwrap();
        let mut command = runner.take_command().unwrap().unwrap();
        let key = command.key().clone();
        runner
            .spawn_root(&mut command, std::future::pending())
            .unwrap();
        (runner, command, key, lease)
    }
    fn endpoint() -> SocketAddr {
        "127.0.0.1:53".parse().unwrap()
    }

    #[tokio::test]
    async fn dns_brand_foreign_same_numeric_id_must_not_construct() {
        let mut parent = ParentDrain::new(2).unwrap();
        let (a, sender_a) = parent.register(limits(), 32).unwrap();
        let (b, sender_b) = parent.register(limits(), 32).unwrap();
        let (runner_a, command_a, key_a, _lease_a) = admit(&parent, a, &sender_a).await;
        let (_runner_b, command_b, key_b, _lease_b) = admit(&parent, b, &sender_b).await;
        assert_eq!(key_a.id(), key_b.id());
        let result = DnsPhase::for_job(
            &runner_a,
            &key_b,
            Instant::now() + Duration::from_secs(1),
            endpoint(),
        );
        assert!(matches!(
            result,
            Err(DnsFailure::Ledger(LedgerError::UnknownJob))
        ));
        let own = DnsPhase::for_job(
            &runner_a,
            &key_a,
            Instant::now() + Duration::from_secs(1),
            endpoint(),
        )
        .unwrap();
        assert_eq!(
            runner_a.owner.lock().unwrap().used_tasks(TaskKind::DnsRoot),
            0
        );
        assert_eq!(runner_a.owner.lock().unwrap().fault(), None);
        drop(own);
        drop(command_a);
        drop(command_b);
        parent.shutdown().await;
    }

    #[tokio::test]
    async fn dns_brand_stale_phase_fail_must_not_poison_reused_id() {
        let mut parent = ParentDrain::new(1).unwrap();
        let (client, sender) = parent.register(limits(), 32).unwrap();
        let (runner, command, key, _lease) = admit(&parent, client, &sender).await;
        let old = DnsPhase::for_job(
            &runner,
            &key,
            Instant::now() + Duration::from_secs(1),
            endpoint(),
        )
        .unwrap();
        runner.cancel(&key).unwrap();
        drop(command);
        let owner = Arc::clone(&runner.owner);
        tokio::time::timeout(
            Duration::from_secs(2),
            std::future::poll_fn(|cx| {
                let mut state = owner.lock().unwrap();
                let _ = state.poll_parent(cx);
                if state.jobs.is_empty() {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            }),
        )
        .await
        .unwrap();
        assert!(owner.lock().unwrap().fault().is_none());
        {
            let mut state = owner.lock().unwrap();
            state.next_job_id = key.id(); // private ABA oracle only
        }
        let (runner_new, command_new, fresh, _lease_new) = admit(&parent, client, &sender).await;
        assert_eq!(fresh.id(), key.id());
        old.fail();
        assert!(matches!(
            DnsPhase::for_job(
                &runner_new,
                &key,
                Instant::now() + Duration::from_secs(1),
                endpoint()
            ),
            Err(DnsFailure::Ledger(LedgerError::UnknownJob))
        ));
        assert!(matches!(
            old.spawn(TaskKind::DnsChild, std::future::pending()),
            Err(DnsFailure::Ledger(LedgerError::UnknownJob))
        ));
        struct Rejected {
            owner: Arc<Mutex<Owner>>,
            phase: DnsPhase,
            outside: Arc<AtomicBool>,
            polled: Arc<AtomicBool>,
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
                self.outside.store(
                    self.owner.try_lock().is_ok() && self.phase.0.book.try_lock().is_ok(),
                    Ordering::Release,
                );
            }
        }
        let outside = Arc::new(AtomicBool::new(false));
        let polled = Arc::new(AtomicBool::new(false));
        assert!(matches!(
            old.spawn(
                TaskKind::DnsChild,
                Rejected {
                    owner: Arc::clone(&runner_new.owner),
                    phase: old.clone(),
                    outside: Arc::clone(&outside),
                    polled: Arc::clone(&polled),
                }
            ),
            Err(DnsFailure::Ledger(LedgerError::UnknownJob))
        ));
        assert!(outside.load(Ordering::Acquire));
        assert!(!polled.load(Ordering::Acquire));
        assert!(matches!(
            old.begin_io(),
            Err(DnsFailure::Ledger(LedgerError::UnknownJob))
        ));
        let mut native_polls = 0;
        assert!(matches!(
            old.poll_io(|_| {
                native_polls += 1;
                Poll::Ready(Ok(()))
            }),
            Poll::Ready(Err(_))
        ));
        assert_eq!(native_polls, 0);
        assert_eq!(
            old.close(),
            Err(DnsFailure::Ledger(LedgerError::UnknownJob))
        );
        assert_eq!(
            old.finish().await,
            Err(DnsFailure::Ledger(LedgerError::UnknownJob))
        );
        let (fault, dns_frozen) = {
            let state = runner_new.owner.lock().unwrap();
            let fault = state.fault();
            let dns_frozen = state
                .jobs
                .iter()
                .find(|j| j.id == fresh.id())
                .unwrap()
                .dns_frozen;
            (fault, dns_frozen)
        };
        drop(command_new);
        parent.shutdown().await;
        assert_eq!(fault, None, "stale fail must not poison fresh seal");
        assert!(!dns_frozen, "stale close must not freeze fresh DNS");
        assert_eq!(
            runner_new
                .owner
                .lock()
                .unwrap()
                .used_tasks(TaskKind::DnsRoot),
            0
        );
    }

    #[tokio::test]
    async fn dns_brand_native_poll_must_hold_busy_entry_through_stop() {
        let mut parent = ParentDrain::new(1).unwrap();
        let (client, sender) = parent.register(limits(), 32).unwrap();
        let (runner, command, key, _lease) = admit(&parent, client, &sender).await;
        let phase = DnsPhase::for_job(
            &runner,
            &key,
            Instant::now() + Duration::from_secs(1),
            endpoint(),
        )
        .unwrap();
        phase
            .spawn(TaskKind::DnsRoot, std::future::pending())
            .unwrap();
        let ticket = phase.begin_io().unwrap();
        let shared = Arc::clone(&runner.owner.lock().unwrap().shared);
        let mut observed = 0;
        let first = phase.poll_io(|_| {
            observed = shared.stop_admit.load(Ordering::Acquire);
            shared.request_stop();
            Poll::Ready(Ok(()))
        });
        let mut later_calls = 0;
        let second = phase.poll_io(|_| {
            later_calls += 1;
            Poll::Ready(Ok(()))
        });
        drop(ticket);
        drop(command);
        parent.shutdown().await;
        assert!(matches!(first, Poll::Ready(Ok(()))));
        assert!(matches!(second, Poll::Ready(Err(_))));
        assert_eq!(later_calls, 0);
        assert_eq!(observed & ENQUEUE_ACCEPTED, ENQUEUE_ACCEPTED);
    }
}

#[cfg(test)]
mod dns_brand_extra_tests {
    use super::super::super::admission::Command;
    use super::super::super::parent_drain::RunnerOwnerHandle;
    use super::*;
    use crate::auth;
    use std::time::Duration;

    fn limits() -> Limits {
        Limits {
            jobs: 2,
            queue: 2,
            job_roots: 2,
            dns_roots: 2,
            dns_children: 2,
            http_drivers: 2,
            waiters: 2,
            waiter_bytes: 32,
            senders: 4,
        }
    }
    fn grant(end: Instant) -> (auth::LeaseOwner, auth::Grant<()>) {
        let lease = auth::LeaseOwner::new(end).unwrap();
        let identity = auth::VerifiedIdentity::from_verified(7, end).unwrap();
        let scope = auth::AuthScope::bind(identity, Arc::clone(&lease.lease)).unwrap();
        (lease, auth::Grant::from_authorized(scope, 91).unwrap())
    }
    async fn admit(
        parent: &ParentDrain,
        client: u64,
        sender: &ClientSender,
    ) -> (RunnerOwnerHandle, Command, JobKey, auth::LeaseOwner) {
        let end = Instant::now() + Duration::from_secs(10);
        let (lease, authorized) = grant(end);
        sender.submit(authorized, vec![1], end).await.unwrap();
        let runner = parent.runner_owner(client).unwrap();
        let mut command = runner.take_command().unwrap().unwrap();
        let key = command.key().clone();
        runner
            .spawn_root(&mut command, std::future::pending())
            .unwrap();
        (runner, command, key, lease)
    }
    fn endpoint() -> SocketAddr {
        "127.0.0.1:53".parse().unwrap()
    }

    #[tokio::test]
    async fn dns_brand_unregistered_root_refuses_then_same_key_starts() {
        let mut parent = ParentDrain::new(1).unwrap();
        let (client, sender) = parent.register(limits(), 32).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        let (lease, authorized) = grant(end);
        sender.submit(authorized, vec![1], end).await.unwrap();
        let runner = parent.runner_owner(client).unwrap();
        let mut command = runner.take_command().unwrap().unwrap();
        let key = command.key().clone();
        assert!(matches!(
            DnsPhase::for_job(&runner, &key, end, endpoint()),
            Err(DnsFailure::Ledger(LedgerError::PhaseClosed))
        ));
        assert_eq!(
            runner.owner.lock().unwrap().used_tasks(TaskKind::DnsRoot),
            0
        );
        runner
            .spawn_root(&mut command, std::future::pending())
            .unwrap();
        let phase = DnsPhase::for_job(&runner, &key, end, endpoint()).unwrap();
        phase
            .spawn(TaskKind::DnsRoot, std::future::pending())
            .unwrap();
        drop(phase);
        drop(command);
        parent.shutdown().await;
        assert_eq!(runner.owner.lock().unwrap().used_jobs(), 0);
        assert_eq!(runner.owner.lock().unwrap().fault(), None);
        drop(lease);
    }

    #[tokio::test]
    async fn dns_brand_stop_before_entry_keeps_native_and_fault_zero() {
        let mut parent = ParentDrain::new(1).unwrap();
        let (client, sender) = parent.register(limits(), 32).unwrap();
        let (runner, command, key, _lease) = admit(&parent, client, &sender).await;
        let phase = DnsPhase::for_job(
            &runner,
            &key,
            Instant::now() + Duration::from_secs(1),
            endpoint(),
        )
        .unwrap();
        phase
            .spawn(TaskKind::DnsRoot, std::future::pending())
            .unwrap();
        let shared = Arc::clone(&runner.owner.lock().unwrap().shared);
        shared.request_stop();
        let mut native = 0;
        assert!(matches!(
            phase.spawn(TaskKind::DnsChild, std::future::pending()),
            Err(DnsFailure::Ledger(LedgerError::Closing))
        ));
        assert!(matches!(
            phase.begin_io(),
            Err(DnsFailure::Ledger(LedgerError::Closing))
        ));
        assert!(matches!(
            phase.poll_io(|_| {
                native += 1;
                Poll::Ready(Ok(()))
            }),
            Poll::Ready(Err(_))
        ));
        assert_eq!(native, 0);
        assert_eq!(
            runner.owner.lock().unwrap().used_tasks(TaskKind::DnsChild),
            0
        );
        assert_eq!(runner.owner.lock().unwrap().fault(), None);
        drop(command);
        parent.shutdown().await;
        assert_eq!(runner.owner.lock().unwrap().used_jobs(), 0);
    }

    #[tokio::test]
    async fn dns_brand_entry_winner_registers_one_child_after_stop() {
        let mut parent = ParentDrain::new(1).unwrap();
        let (client, sender) = parent.register(limits(), 32).unwrap();
        let (runner, command, key, _lease) = admit(&parent, client, &sender).await;
        let phase = DnsPhase::for_job(
            &runner,
            &key,
            Instant::now() + Duration::from_secs(1),
            endpoint(),
        )
        .unwrap();
        phase
            .spawn(TaskKind::DnsRoot, std::future::pending())
            .unwrap();
        let shared = Arc::clone(&runner.owner.lock().unwrap().shared);
        phase
            .spawn_with_after_entry(TaskKind::DnsChild, std::future::pending(), || {
                shared.request_stop()
            })
            .unwrap();
        assert_eq!(
            runner.owner.lock().unwrap().used_tasks(TaskKind::DnsChild),
            1
        );
        assert!(matches!(
            phase.spawn(TaskKind::DnsChild, std::future::pending()),
            Err(DnsFailure::Ledger(LedgerError::Closing))
        ));
        assert_eq!(
            runner.owner.lock().unwrap().used_tasks(TaskKind::DnsChild),
            1
        );
        assert_eq!(runner.owner.lock().unwrap().fault(), None);
        drop(command);
        parent.shutdown().await;
        assert_eq!(runner.owner.lock().unwrap().used_jobs(), 0);
    }

    #[tokio::test]
    async fn dns_brand_cancelled_root_keeps_ticket_until_receipt_and_join() {
        let mut parent = ParentDrain::new(1).unwrap();
        let (client, sender) = parent.register(limits(), 32).unwrap();
        let (runner, command, key, _lease) = admit(&parent, client, &sender).await;
        let phase = DnsPhase::for_job(
            &runner,
            &key,
            Instant::now() + Duration::from_secs(1),
            endpoint(),
        )
        .unwrap();
        phase
            .spawn(TaskKind::DnsRoot, std::future::pending())
            .unwrap();
        let ticket = phase.begin_io().unwrap();
        runner.cancel(&key).unwrap();
        assert_eq!(runner.owner.lock().unwrap().used_jobs(), 1);
        assert_eq!(phase.0.book.lock().unwrap().io_live, 1);
        drop(ticket);
        assert_eq!(phase.0.book.lock().unwrap().io_closed, 1);
        assert_eq!(
            phase.finish().await,
            Err(DnsFailure::Ledger(LedgerError::Closing))
        );
        assert_eq!(
            runner.owner.lock().unwrap().used_tasks(TaskKind::DnsRoot),
            0
        );
        assert_eq!(runner.owner.lock().unwrap().fault(), None);
        drop(command);
        parent.shutdown().await;
        assert_eq!(runner.owner.lock().unwrap().used_jobs(), 0);
    }

    #[tokio::test]
    async fn dns_brand_joined_root_refuses_start_but_same_key_receipt_drains() {
        let mut parent = ParentDrain::new(1).unwrap();
        let (client, sender) = parent.register(limits(), 32).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        let (_lease, authorized) = grant(end);
        sender.submit(authorized, vec![1], end).await.unwrap();
        let runner = parent.runner_owner(client).unwrap();
        let mut command = runner.take_command().unwrap().unwrap();
        let key = command.key().clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        assert!(runner
            .spawn_root(&mut command, async move {
                let _ = rx.await;
            })
            .is_ok());
        let phase = DnsPhase::for_job(&runner, &key, end, endpoint()).unwrap();
        phase
            .spawn(TaskKind::DnsRoot, std::future::pending())
            .unwrap();
        let ticket = phase.begin_io().unwrap();
        tx.send(()).unwrap();
        let owner = Arc::clone(&runner.owner);
        tokio::time::timeout(
            Duration::from_secs(2),
            std::future::poll_fn(|cx| {
                let mut state = owner.lock().unwrap();
                let _ = state.poll_parent(cx);
                let index = state
                    .key_index(&key)
                    .expect("popped command retains record");
                if state.jobs[index]
                    .tasks
                    .iter()
                    .any(|task| task.kind == TaskKind::JobRoot && task.handle.is_some())
                {
                    Poll::Pending
                } else {
                    Poll::Ready(())
                }
            }),
        )
        .await
        .unwrap();
        assert!(matches!(
            DnsPhase::for_job(&runner, &key, end, endpoint()),
            Err(DnsFailure::Ledger(LedgerError::PhaseClosed))
        ));
        let mut native = 0;
        assert!(matches!(
            phase.poll_io(|_| {
                native += 1;
                Poll::Ready(Ok(()))
            }),
            Poll::Ready(Err(_))
        ));
        assert_eq!(native, 0);
        assert_eq!(phase.0.book.lock().unwrap().io_live, 1);
        drop(ticket);
        assert_eq!(phase.0.book.lock().unwrap().io_closed, 1);
        assert_eq!(phase.close(), Ok(()));
        assert_eq!(phase.finish().await, Ok(()));
        assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::DnsRoot), 0);
        assert_eq!(owner.lock().unwrap().fault(), None);
        drop(command);
        parent.shutdown().await;
        assert_eq!(owner.lock().unwrap().used_jobs(), 0);
    }

    #[tokio::test]
    async fn dns_brand_deadline_after_entry_skips_native_primitive() {
        let mut parent = ParentDrain::new(1).unwrap();
        let (client, sender) = parent.register(limits(), 32).unwrap();
        let (runner, command, key, _lease) = admit(&parent, client, &sender).await;
        // The original resolve end remains unchanged while the admitted entry waits.
        // This one-second test bound is well inside the ten-second job lease.
        let original_end = Instant::now() + Duration::from_secs(1);
        let phase = DnsPhase::for_job(&runner, &key, original_end, endpoint()).unwrap();
        assert_eq!(phase.0.end, original_end);
        phase
            .spawn(TaskKind::DnsRoot, std::future::pending())
            .unwrap();
        struct Rejected {
            owner: Arc<Mutex<Owner>>,
            phase: DnsPhase,
            outside: Arc<AtomicBool>,
            polled: Arc<AtomicBool>,
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
                self.outside.store(
                    self.owner.try_lock().is_ok() && self.phase.0.book.try_lock().is_ok(),
                    Ordering::Release,
                );
            }
        }
        let outside = Arc::new(AtomicBool::new(false));
        let polled = Arc::new(AtomicBool::new(false));
        let job_original_end = runner
            .owner
            .lock()
            .unwrap()
            .jobs
            .iter()
            .find(|job| job.id == key.id())
            .unwrap()
            .end;
        let shared = Arc::clone(&runner.owner.lock().unwrap().shared);
        let mut entered = false;
        let result = phase.spawn_with_after_entry(
            TaskKind::DnsChild,
            Rejected {
                owner: Arc::clone(&runner.owner),
                phase: phase.clone(),
                outside: Arc::clone(&outside),
                polled: Arc::clone(&polled),
            },
            || {
                entered = shared.stop_admit.load(Ordering::Acquire) & ENQUEUE_ACCEPTED != 0;
                let remaining = original_end.saturating_duration_since(Instant::now());
                assert!(remaining <= Duration::from_secs(1));
                std::thread::sleep(remaining + Duration::from_millis(5));
                assert!(Instant::now() >= original_end);
            },
        );
        assert!(entered, "CAS winner observed before original end passes");
        assert_eq!(phase.0.end, original_end);
        assert_eq!(
            runner
                .owner
                .lock()
                .unwrap()
                .jobs
                .iter()
                .find(|job| job.id == key.id())
                .unwrap()
                .end,
            job_original_end
        );
        assert_eq!(result, Err(DnsFailure::Ledger(LedgerError::Expired)));
        assert!(
            outside.load(Ordering::Acquire),
            "rejected Future Drop outside both gates"
        );
        assert!(!polled.load(Ordering::Acquire));
        assert_eq!(
            runner.owner.lock().unwrap().used_tasks(TaskKind::DnsChild),
            0
        );
        assert_eq!(runner.owner.lock().unwrap().fault(), None);
        drop(command);
        parent.shutdown().await;
        assert_eq!(runner.owner.lock().unwrap().used_jobs(), 0);
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn dns_brand_root_finishes_during_book_wait_skips_io() {
        let mut parent = ParentDrain::new(1).unwrap();
        let (client, sender) = parent.register(limits(), 32).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        let (_lease, authorized) = grant(end);
        sender.submit(authorized, vec![1], end).await.unwrap();
        let runner = parent.runner_owner(client).unwrap();
        let mut command = runner.take_command().unwrap().unwrap();
        let key = command.key().clone();
        let (root_release, root_wait) = tokio::sync::oneshot::channel::<()>();
        assert!(runner
            .spawn_root(&mut command, async move {
                let _ = root_wait.await;
            })
            .is_ok());
        let phase = DnsPhase::for_job(&runner, &key, end, endpoint()).unwrap();
        phase
            .spawn(TaskKind::DnsRoot, std::future::pending())
            .unwrap();
        let signal = {
            let state = runner.owner.lock().unwrap();
            let index = state.key_index(&key).unwrap();
            Arc::clone(
                state.jobs[index]
                    .tasks
                    .iter()
                    .find(|task| task.kind == TaskKind::JobRoot)
                    .unwrap()
                    .root_signal
                    .as_ref()
                    .unwrap(),
            )
        };
        let (held_tx, held_rx) = std::sync::mpsc::sync_channel::<()>(1);
        let (release_book, release_rx) = std::sync::mpsc::sync_channel::<()>(1);
        let held_phase = phase.clone();
        let holder = std::thread::spawn(move || {
            let book = held_phase.0.book.lock().unwrap();
            let _ = held_tx.send(());
            let _ = release_rx.recv_timeout(Duration::from_secs(5));
            drop(book);
        });
        let book_held = held_rx.recv_timeout(Duration::from_secs(2)).is_ok();
        let finished = Arc::new(AtomicBool::new(false));
        let finished_in_attempt = Arc::clone(&finished);
        let attempted_phase = phase.clone();
        let attempt = std::thread::spawn(move || {
            let result = attempted_phase.begin_io();
            let refused = matches!(&result, Err(DnsFailure::Ledger(LedgerError::PhaseClosed)));
            drop(result);
            finished_in_attempt.store(true, Ordering::Release);
            refused
        });
        let owner = Arc::clone(&runner.owner);
        let blocked_at_book = if book_held {
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if owner.try_lock().is_err() && !finished.load(Ordering::Acquire) {
                        tokio::time::sleep(Duration::from_millis(1)).await;
                        if owner.try_lock().is_err() && !finished.load(Ordering::Acquire) {
                            break true;
                        }
                    }
                    if finished.load(Ordering::Acquire) {
                        break false;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap_or(false)
        } else {
            false
        };
        let root_released = root_release.send(()).is_ok();
        let root_completed = tokio::time::timeout(Duration::from_secs(2), async {
            while signal.state.load(Ordering::Acquire) == ROOT_RUNNING {
                tokio::task::yield_now().await;
            }
        })
        .await
        .is_ok()
            && signal.state.load(Ordering::Acquire) == ROOT_COMPLETED;
        let _ = release_book.send(());
        let holder_joined = holder.join().is_ok();
        let refused = attempt.join().unwrap_or(false);
        let io_started = phase.0.book.lock().unwrap().io_started;
        let fault = runner.owner.lock().unwrap().fault();
        drop(command);
        parent.shutdown().await;
        let used_jobs = runner.owner.lock().unwrap().used_jobs();
        assert!(
            book_held && blocked_at_book && root_released && root_completed,
            "book_held={book_held} blocked_at_book={blocked_at_book} root_released={root_released} root_completed={root_completed} root_state={}",
            signal.state.load(Ordering::Acquire)
        );
        assert!(holder_joined);
        assert!(refused, "terminal JobRoot must refuse new native I/O");
        assert_eq!(io_started, 0);
        assert_eq!(fault, None);
        assert_eq!(used_jobs, 0);
    }
}

// Append to runtime/src/http_client/core_tests.rs on #126 HEAD 37c47e1.
// Test-only, not executed yet. No real IoTicket: its blocking Drop would hang
// the vulnerable implementation. Probe observes all gates with try_lock only.
#[test]
fn sf06_closed_runtime_dns_drop_is_outside_owner_book_and_entry() {
    struct Probe {
        phase: DnsPhase,
        shared: Arc<Shared>,
        polls: Arc<AtomicUsize>,
        drops: Arc<AtomicUsize>,
        held: Arc<AtomicUsize>,
    }
    impl Future for Probe {
        type Output = ();
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            self.polls.fetch_add(1, Ordering::SeqCst);
            Poll::Pending
        }
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            let owner = &self.phase.0.runner.owner;
            let owner_held = matches!(owner.try_lock(), Err(std::sync::TryLockError::WouldBlock));
            let book_held = matches!(
                self.phase.0.book.try_lock(),
                Err(std::sync::TryLockError::WouldBlock)
            );
            let entry_held =
                usize::from(self.shared.stop_admit.load(Ordering::Acquire) & ENQUEUE_ACCEPTED != 0)
                    * 4;
            self.held.store(
                usize::from(owner_held) + usize::from(book_held) * 2 + entry_held,
                Ordering::SeqCst,
            );
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }
    let live = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (owner, sender, id, phase) = live.block_on(async {
        let (owner, sender, id) = fixture();
        let phase = phase(&sender, id);
        (owner, sender, id, phase)
    });
    let dead_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let dead_handle = dead_runtime.handle().clone();
    drop(dead_runtime);
    let polls = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let held = Arc::new(AtomicUsize::new(0));
    let answer = {
        let _entered = dead_handle.enter();
        phase.spawn(
            TaskKind::DnsChild,
            Probe {
                phase: phase.clone(),
                shared: Arc::clone(&sender.shared),
                polls: Arc::clone(&polls),
                drops: Arc::clone(&drops),
                held: Arc::clone(&held),
            },
        )
    };
    let before = owner.lock().unwrap().used_tasks(TaskKind::DnsChild);
    live.block_on(cleanup(&owner));
    let after = owner.lock().unwrap().used_tasks(TaskKind::DnsChild);
    let jobs = owner.lock().unwrap().used_jobs();
    drop(sender);
    assert!(answer.is_ok());
    assert_eq!(polls.load(Ordering::SeqCst), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        before, 1,
        "closed runtime returned a retained actual handle"
    );
    assert_eq!(after, 0);
    assert_eq!(
        jobs, 0,
        "cleanup polled all actual handles, including live roots"
    );
    assert_eq!(
        held.load(Ordering::SeqCst),
        0,
        "F Drop ran under a lifecycle gate"
    );
    assert_eq!(owner.lock().unwrap().fixture_task_records(id), 0);
}
