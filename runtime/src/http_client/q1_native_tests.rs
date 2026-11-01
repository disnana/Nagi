use super::super::super::q1_dial::{
    AttemptOrder, OperationBrand, PreferredFamily, SuccessfulLookup,
};
use super::*;
use std::{
    io::{Error, ErrorKind},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, TcpListener as StdTcpListener},
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

// Use both standard loopback interfaces; 127.0.0.2 is not assigned on macOS.
fn mixed_loopback_plan(port: u16) -> DialPlan {
    let now = Instant::now();
    let end = now + Duration::from_secs(5);
    let policy = super::super::super::InternalPolicy::checked(
        identity(),
        port,
        2,
        vec![
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
        ],
    )
    .unwrap();
    let checked = SuccessfulLookup::from_provider_results(
        OperationBrand::from_admitted_operation(NonZeroU64::new(1).unwrap()),
        Ok(vec![Ipv4Addr::LOCALHOST]),
        Ok(vec![Ipv6Addr::LOCALHOST]),
        vec![30],
        Some(end),
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
        2,
        Duration::from_secs(3),
        now,
    )
    .unwrap()
}

const LOOPBACK_PORT_PAIR_ATTEMPTS: usize = 32;

struct RefusedV4Guard {
    socket: Option<TcpSocket>,
    drop_count: Option<Arc<AtomicUsize>>,
}

impl RefusedV4Guard {
    fn new(socket: TcpSocket, drop_count: Option<Arc<AtomicUsize>>) -> Self {
        Self {
            socket: Some(socket),
            drop_count,
        }
    }

    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.as_ref().expect("live IPv4 guard").local_addr()
    }
}

impl Drop for RefusedV4Guard {
    fn drop(&mut self) {
        // Close the owned socket before publishing the test-only observation.
        drop(self.socket.take());
        if let Some(drop_count) = &self.drop_count {
            drop_count.fetch_add(1, Ordering::SeqCst);
        }
    }
}

fn reserve_refused_v4_and_live_v6_pair_with<B, C>(
    mut bind_v6: B,
    mut after_collision: C,
    drop_count: Option<Arc<AtomicUsize>>,
) -> std::io::Result<(RefusedV4Guard, TcpListener, SocketAddr)>
where
    B: FnMut(u16) -> std::io::Result<StdTcpListener>,
    C: FnMut(SocketAddr) -> std::io::Result<()>,
{
    for _ in 0..LOOPBACK_PORT_PAIR_ATTEMPTS {
        let first_socket = TcpSocket::new_v4()?;
        first_socket.bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
        let first_addr = first_socket.local_addr()?;
        let first_guard = RefusedV4Guard::new(first_socket, drop_count.clone());
        match bind_v6(first_addr.port()) {
            Ok(listener) => {
                listener.set_nonblocking(true)?;
                let listener = TcpListener::from_std(listener)?;
                let second_addr = listener.local_addr()?;
                return Ok((first_guard, listener, second_addr));
            }
            Err(error) if error.kind() == ErrorKind::AddrInUse => {
                // The rejected-address guard must not survive a failed pair
                // attempt. The hook gives the regression test a deterministic
                // point to observe that the owned socket was closed first.
                drop(first_guard);
                after_collision(first_addr)?;
            }
            Err(error) => return Err(error),
        }
    }
    Err(Error::new(
        ErrorKind::AddrInUse,
        "could not reserve an IPv4 refusal guard and IPv6 listener port pair",
    ))
}

fn reserve_refused_v4_and_live_v6_pair(
) -> std::io::Result<(RefusedV4Guard, TcpListener, SocketAddr)> {
    reserve_refused_v4_and_live_v6_pair_with(
        |port| StdTcpListener::bind((Ipv6Addr::LOCALHOST, port)),
        |_| Ok(()),
        None,
    )
}

#[test]
fn q1_native_refused_first_address_connects_only_checked_second_peer() {
    runtime().block_on(async {
        let (_first_guard, listener, addr) = reserve_refused_v4_and_live_v6_pair().unwrap();
        // A bound non-listening owned socket keeps this exact first address
        // reserved while the native connect receives refusal. Both family
        // endpoints are held before the connection plan begins.
        let control = DialControl::new();
        let quota = SocketQuota::checked(1).unwrap();
        let (socket, _) = connect_plan(mixed_loopback_plan(addr.port()), &control, &quota)
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
fn q1_native_port_pair_retry_releases_guard_and_is_bounded() {
    use std::cell::Cell;

    runtime().block_on(async {
        let bind_calls = Cell::new(0usize);
        let collision_port = Cell::new(None);
        let cleanup_checks = Cell::new(0usize);
        let drop_count = Arc::new(AtomicUsize::new(0));
        let (first_guard, listener, second_addr) = reserve_refused_v4_and_live_v6_pair_with(
            |port| {
                let call = bind_calls.get();
                bind_calls.set(call + 1);
                if call == 0 {
                    collision_port.set(Some(port));
                    return Err(Error::from(ErrorKind::AddrInUse));
                }
                StdTcpListener::bind((Ipv6Addr::LOCALHOST, port))
            },
            |failed_addr| {
                let cleanup = cleanup_checks.get() + 1;
                if cleanup == 1 {
                    assert_eq!(collision_port.get(), Some(failed_addr.port()));
                }
                cleanup_checks.set(cleanup);
                assert_eq!(drop_count.load(Ordering::SeqCst), cleanup);
                Ok(())
            },
            Some(Arc::clone(&drop_count)),
        )
        .unwrap();
        assert!((2..=LOOPBACK_PORT_PAIR_ATTEMPTS).contains(&bind_calls.get()));
        assert_eq!(cleanup_checks.get() + 1, bind_calls.get());
        assert_eq!(drop_count.load(Ordering::SeqCst), cleanup_checks.get());
        assert_eq!(first_guard.local_addr().unwrap().port(), second_addr.port());
        assert_eq!(listener.local_addr().unwrap(), second_addr);
        drop(listener);
        drop(first_guard);
        assert_eq!(drop_count.load(Ordering::SeqCst), cleanup_checks.get() + 1);

        let bind_calls = Cell::new(0usize);
        let cleanup_checks = Cell::new(0usize);
        let drop_count = Arc::new(AtomicUsize::new(0));
        let result = reserve_refused_v4_and_live_v6_pair_with(
            |_| {
                bind_calls.set(bind_calls.get() + 1);
                Err(Error::from(ErrorKind::AddrInUse))
            },
            |_| {
                let cleanup = cleanup_checks.get() + 1;
                cleanup_checks.set(cleanup);
                assert_eq!(drop_count.load(Ordering::SeqCst), cleanup);
                Ok(())
            },
            Some(Arc::clone(&drop_count)),
        );
        assert!(matches!(result, Err(error) if error.kind() == ErrorKind::AddrInUse));
        assert_eq!(bind_calls.get(), LOOPBACK_PORT_PAIR_ATTEMPTS);
        assert_eq!(cleanup_checks.get(), LOOPBACK_PORT_PAIR_ATTEMPTS);
        assert_eq!(
            drop_count.load(Ordering::SeqCst),
            LOOPBACK_PORT_PAIR_ATTEMPTS
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
