use super::super::super::q1_dial::{
    AttemptOrder, OperationBrand, PreferredFamily, SuccessfulLookup,
};
use super::*;
use std::{
    io::{Error, ErrorKind},
    net::{IpAddr, Ipv4Addr},
    num::NonZeroU64,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpSocket},
};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn identity() -> ServiceIdentity {
    ServiceIdentity {
        service: 7,
        purpose: 8,
        generation: 9,
        trust: 10,
    }
}

fn plan(port: u16, attempts: usize, ttl: u32, budget: Duration) -> DialPlan {
    let now = Instant::now();
    let end = now + Duration::from_secs(5);
    let addresses = vec![Ipv4Addr::LOCALHOST, Ipv4Addr::new(127, 0, 0, 2)];
    let policy = super::super::super::InternalPolicy::checked(
        identity(),
        port,
        2,
        addresses.iter().copied().map(IpAddr::V4).collect(),
    )
    .unwrap();
    let checked = SuccessfulLookup::from_provider_results(
        OperationBrand::from_admitted_operation(NonZeroU64::new(1).unwrap()),
        Ok(addresses),
        Ok(vec![]),
        vec![ttl],
        (ttl > 0).then_some(end),
        end,
        end,
    )
    .unwrap()
    .check(&policy, identity(), now)
    .unwrap();
    DialPlan::checked(
        checked,
        AttemptOrder::Interleave {
            preferred: PreferredFamily::V4,
        },
        attempts,
        budget,
        now,
    )
    .unwrap()
}

#[test]
fn q1_native_refused_first_address_connects_only_checked_second_peer() {
    runtime().block_on(async {
        let listener = TcpListener::bind((Ipv4Addr::new(127, 0, 0, 2), 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let first_guard = TcpSocket::new_v4().unwrap();
        first_guard
            .bind(SocketAddr::from((Ipv4Addr::LOCALHOST, addr.port())))
            .unwrap();
        // A bound non-listening owned socket keeps this exact first address
        // reserved while the native connect receives refusal. No port race.
        let control = DialControl::new();
        let quota = SocketQuota::checked(1).unwrap();
        let (socket, _) = connect_plan(
            plan(addr.port(), 2, 30, Duration::from_secs(3)),
            &control,
            &quota,
        )
        .await
        .unwrap();
        let (mut peer, _) = listener.accept().await.unwrap();
        assert_eq!(socket.selected, addr);
        assert_eq!(socket.identity, identity());
        assert_eq!(
            socket.owned.stream.as_ref().unwrap().peer_addr().unwrap(),
            addr
        );
        assert_eq!(quota.used(), 1);
        drop(socket);
        assert_eq!(quota.used(), 0);
        let mut byte = [0];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), peer.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
    });
}

#[test]
fn q1_native_attempt_cap_prevents_second_connector_and_releases_first_slot() {
    runtime().block_on(async {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let quota = SocketQuota::checked(1).unwrap();
        let result = connect_plan_inner(
            plan(443, 1, 30, Duration::from_secs(2)),
            &DialControl::new(),
            &quota,
            move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
                async { Err(Error::from(ErrorKind::ConnectionRefused)) }
            },
        )
        .await;
        assert!(matches!(
            result,
            Err(FailoverError::Plan(PlanStop::Exhausted))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(quota.used(), 0);
    });
}

#[test]
fn q1_native_terminal_connect_failure_does_not_try_another_candidate() {
    runtime().block_on(async {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let quota = SocketQuota::checked(1).unwrap();
        let result = connect_plan_inner(
            plan(443, 2, 30, Duration::from_secs(2)),
            &DialControl::new(),
            &quota,
            move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
                async { Err(Error::from(ErrorKind::PermissionDenied)) }
            },
        )
        .await;
        assert!(matches!(
            result,
            Err(FailoverError::Plan(PlanStop::FatalConnect))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(quota.used(), 0);
    });
}

#[test]
fn q1_native_ttl_zero_is_usable_for_one_owned_operation_without_cache() {
    runtime().block_on(async {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let quota = SocketQuota::checked(1).unwrap();
        let (socket, _) = connect_plan(
            plan(addr.port(), 2, 0, Duration::from_secs(2)),
            &DialControl::new(),
            &quota,
        )
        .await
        .unwrap();
        let (_peer, _) = listener.accept().await.unwrap();
        assert_eq!(socket.selected, addr);
        drop(socket);
        assert_eq!(quota.used(), 0);
    });
}

#[test]
fn q1_native_expired_original_connect_end_does_not_reset_budget() {
    runtime().block_on(async {
        let dial = plan(443, 2, 30, Duration::from_millis(1));
        tokio::time::sleep(Duration::from_millis(5)).await;
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let quota = SocketQuota::checked(1).unwrap();
        let result = connect_plan_inner(dial, &DialControl::new(), &quota, move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
            async { Err(Error::from(ErrorKind::ConnectionRefused)) }
        })
        .await;
        assert!(matches!(
            result,
            Err(FailoverError::Plan(PlanStop::ConnectExpired))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(quota.used(), 0);
    });
}

#[test]
fn q1_native_closed_plan_starts_no_connector() {
    runtime().block_on(async {
        let control = DialControl::new();
        control.close();
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let quota = SocketQuota::checked(1).unwrap();
        let result = connect_plan_inner(
            plan(443, 2, 30, Duration::from_secs(2)),
            &control,
            &quota,
            move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
                async { Err(Error::from(ErrorKind::ConnectionRefused)) }
            },
        )
        .await;
        assert!(matches!(
            result,
            Err(FailoverError::Plan(PlanStop::Closing))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(quota.used(), 0);
    });
}

#[test]
fn q1_native_cancelled_caller_is_joined_before_reusing_socket_quota() {
    runtime().block_on(async {
        let control = Arc::new(DialControl::new());
        let quota = Arc::new(SocketQuota::checked(1).unwrap());
        let entered = Arc::new(tokio::sync::Notify::new());
        let child_quota = Arc::clone(&quota);
        let child_control = Arc::clone(&control);
        let child_entered = Arc::clone(&entered);
        let dial = plan(443, 2, 30, Duration::from_secs(3));
        let task = tokio::spawn(async move {
            connect_plan_inner(dial, &child_control, &child_quota, move |_| {
                let entered = Arc::clone(&child_entered);
                std::future::poll_fn(move |_| {
                    entered.notify_one();
                    Poll::Pending::<std::io::Result<TcpStream>>
                })
            })
            .await
            .map(|_| ())
        });
        tokio::time::timeout(Duration::from_secs(2), entered.notified())
            .await
            .unwrap();
        assert_eq!(quota.used(), 1);
        task.abort();
        let stopped = task
            .await
            .expect_err("caller cancellation must be observed by actual join");
        assert!(stopped.is_cancelled());
        assert_eq!(quota.used(), 0);
        // The same finite quota admits a normal native request after actual join.
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (socket, mut sealed) = connect_plan(
            plan(addr.port(), 2, 30, Duration::from_secs(2)),
            &control,
            &quota,
        )
        .await
        .unwrap();
        let (_peer, _) = listener.accept().await.unwrap();
        assert_eq!(
            sealed
                .next_attempt(Instant::now(), &AttemptControl::default())
                .unwrap_err(),
            PlanStop::Finished
        );
        drop(socket);
        assert_eq!(quota.used(), 0);
    });
}

#[test]
fn q1_native_close_during_pending_connect_does_not_attempt_second_address() {
    runtime().block_on(async {
        let control = Arc::new(DialControl::new());
        let quota = Arc::new(SocketQuota::checked(1).unwrap());
        let entered = Arc::new(tokio::sync::Notify::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let child_control = Arc::clone(&control);
        let child_quota = Arc::clone(&quota);
        let child_entered = Arc::clone(&entered);
        let child_calls = Arc::clone(&calls);
        let dial = plan(443, 2, 30, Duration::from_secs(3));
        let task = tokio::spawn(async move {
            connect_plan_inner(dial, &child_control, &child_quota, move |_| {
                child_calls.fetch_add(1, Ordering::SeqCst);
                let entered = Arc::clone(&child_entered);
                std::future::poll_fn(move |_| {
                    entered.notify_one();
                    Poll::Pending::<std::io::Result<TcpStream>>
                })
            })
            .await
            .map(|_| ())
        });
        tokio::time::timeout(Duration::from_secs(2), entered.notified())
            .await
            .unwrap();
        control.close();
        let result = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            result,
            Err(FailoverError::Socket(SocketError::Closing))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(quota.used(), 0);
    });
}
