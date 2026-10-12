use super::*;
use std::{
    future::pending,
    net::{IpAddr, Ipv4Addr},
    task::{Context, Wake, Waker},
    time::Duration,
};
use tokio::{io::AsyncReadExt, net::TcpListener};

use super::super::{check_literal, InternalPolicy};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

fn identity() -> ServiceIdentity {
    ServiceIdentity {
        service: 7,
        purpose: 8,
        generation: 9,
        trust: 10,
    }
}

fn snapshot(port: u16, until: Instant, end: Instant) -> CheckedSnapshot {
    // A test-only reviewed exact exception permits an owned loopback listener.
    // This is not a production bypass or a network policy default.
    let policy =
        InternalPolicy::checked(identity(), port, 1, vec![IpAddr::V4(Ipv4Addr::LOCALHOST)])
            .unwrap();
    check_literal(
        &policy,
        identity(),
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        until,
        end,
        Instant::now(),
    )
    .unwrap()
}

async fn listener() -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
}

async fn eof(mut stream: TcpStream) {
    let mut one = [0_u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut one))
        .await
        .expect("owned peer did not observe closure")
        .unwrap();
    assert_eq!(read, 0);
}

#[test]
fn numeric_peer_is_exact_and_owner_retains_quota_until_socket_drop() {
    runtime().block_on(async {
        let (listener, addr) = listener().await;
        let control = DialControl::new();
        let quota = SocketQuota::checked(1).unwrap();
        let end = Instant::now() + Duration::from_secs(3);
        let socket = connect(snapshot(addr.port(), end, end), end, &control, &quota)
            .await
            .unwrap();
        let (peer, _) = listener.accept().await.unwrap();
        assert_eq!(socket.selected, addr);
        assert_eq!(socket.identity, identity());
        assert_eq!(
            socket.owned.stream.as_ref().unwrap().peer_addr().unwrap(),
            addr
        );
        assert_eq!(quota.used(), 1);
        assert_eq!(socket.ready_for_tls(&control), Ok(()));
        drop(socket);
        assert_eq!(quota.used(), 0);
        eof(peer).await;
    });
}

#[test]
fn wrong_numeric_peer_is_rejected_and_closed_before_quota_release() {
    runtime().block_on(async {
        let (_expected_listener, expected) = listener().await;
        let (other_listener, other) = listener().await;
        let control = DialControl::new();
        let quota = SocketQuota::checked(1).unwrap();
        let end = Instant::now() + Duration::from_secs(3);
        let selected = snapshot(expected.port(), end, end)
            .begin_dial(Instant::now())
            .unwrap();
        let wrong = TcpStream::connect(other).await.unwrap();
        let (peer, _) = other_listener.accept().await.unwrap();
        let result = verify_stream(
            selected,
            OwnedSocket {
                stream: Some(wrong),
                ticket: Some(quota.reserve().unwrap()),
            },
            end,
            &control,
        );
        assert!(matches!(result, Err(SocketError::PeerMismatch)));
        assert_eq!(quota.used(), 0);
        eof(peer).await;
    });
}

#[test]
fn closed_or_faulted_gate_blocks_first_native_poll() {
    runtime().block_on(async {
        let (_listener, addr) = listener().await;
        let quota = SocketQuota::checked(1).unwrap();
        for (fault, expected) in [(false, SocketError::Closing), (true, SocketError::Faulted)] {
            let control = DialControl::new();
            if fault {
                control.fault()
            } else {
                control.close()
            }
            let end = Instant::now() + Duration::from_secs(3);
            let calls = std::cell::Cell::new(0);
            let result = connect_inner(
                snapshot(addr.port(), end, end),
                end,
                &control,
                &quota,
                |_| {
                    calls.set(calls.get() + 1);
                    async { panic!("native future must not be polled") }
                },
            )
            .await;
            assert!(matches!(result, Err(error) if error == expected));
            assert_eq!(calls.get(), 1); // constructed lazily; no socket was opened
            assert_eq!(quota.used(), 0);
        }
    });
}

#[test]
fn expired_snapshot_never_enters_native_connect() {
    runtime().block_on(async {
        let (_listener, addr) = listener().await;
        let quota = SocketQuota::checked(1).unwrap();
        let control = DialControl::new();
        let end = Instant::now() + Duration::from_secs(3);
        let checked = snapshot(addr.port(), Instant::now() + Duration::from_millis(20), end);
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(matches!(
            connect(checked, end, &control, &quota).await,
            Err(SocketError::Address(AddressError::Expired))
        ));
        assert_eq!(quota.used(), 0);
    });
}

#[test]
fn close_and_deadline_drop_pending_dial_before_releasing_quota() {
    runtime().block_on(async {
        let (_listener, addr) = listener().await;
        let quota = SocketQuota::checked(1).unwrap();
        let control = DialControl::new();
        let end = Instant::now() + Duration::from_secs(3);
        let dial = connect_inner(
            snapshot(addr.port(), end, end),
            end,
            &control,
            &quota,
            |_| pending::<std::io::Result<TcpStream>>(),
        );
        tokio::pin!(dial);
        struct NoWake;
        impl Wake for NoWake {
            fn wake(self: Arc<Self>) {}
        }
        let waker = Waker::from(Arc::new(NoWake));
        assert!(matches!(
            dial.as_mut().poll(&mut Context::from_waker(&waker)),
            Poll::Pending
        ));
        assert_eq!(quota.used(), 1);
        control.close();
        assert!(matches!(dial.await, Err(SocketError::Closing)));
        assert_eq!(quota.used(), 0);

        let control = DialControl::new();
        let soon = Instant::now() + Duration::from_millis(25);
        let dial = connect_inner(
            snapshot(addr.port(), end, end),
            soon,
            &control,
            &quota,
            |_| pending::<std::io::Result<TcpStream>>(),
        );
        tokio::pin!(dial);
        assert!(matches!(
            dial.as_mut().poll(&mut Context::from_waker(&waker)),
            Poll::Pending
        ));
        assert_eq!(quota.used(), 1);
        assert!(matches!(dial.await, Err(SocketError::Deadline)));
        assert_eq!(quota.used(), 0);
    });
}

#[test]
fn dropping_pending_connect_future_releases_its_slot() {
    runtime().block_on(async {
        let (_listener, addr) = listener().await;
        let quota = SocketQuota::checked(1).unwrap();
        let control = DialControl::new();
        let end = Instant::now() + Duration::from_secs(3);
        let mut dial = Box::pin(connect_inner(
            snapshot(addr.port(), end, end),
            end,
            &control,
            &quota,
            |_| pending::<std::io::Result<TcpStream>>(),
        ));
        struct NoWake;
        impl Wake for NoWake {
            fn wake(self: Arc<Self>) {}
        }
        let waker = Waker::from(Arc::new(NoWake));
        assert!(matches!(
            dial.as_mut().poll(&mut Context::from_waker(&waker)),
            Poll::Pending
        ));
        assert_eq!(quota.used(), 1);
        drop(dial);
        assert_eq!(quota.used(), 0);
    });
}

#[test]
fn full_capacity_cannot_open_second_socket_until_first_owner_drops() {
    runtime().block_on(async {
        let (first_listener, first) = listener().await;
        let (second_listener, second) = listener().await;
        let control = DialControl::new();
        let quota = SocketQuota::checked(1).unwrap();
        let end = Instant::now() + Duration::from_secs(3);
        let first_socket = connect(snapshot(first.port(), end, end), end, &control, &quota)
            .await
            .unwrap();
        let (first_peer, _) = first_listener.accept().await.unwrap();
        assert!(matches!(
            connect(snapshot(second.port(), end, end), end, &control, &quota).await,
            Err(SocketError::Capacity)
        ));
        assert_eq!(quota.used(), 1);
        drop(first_socket);
        eof(first_peer).await;
        let second_socket = connect(snapshot(second.port(), end, end), end, &control, &quota)
            .await
            .unwrap();
        let (second_peer, _) = second_listener.accept().await.unwrap();
        assert_eq!(second_socket.selected, second);
        drop(second_socket);
        eof(second_peer).await;
        assert_eq!(quota.used(), 0);
    });
}

#[test]
fn close_after_connect_prevents_handoff_and_drop_closes_socket() {
    runtime().block_on(async {
        let (listener, addr) = listener().await;
        let control = DialControl::new();
        let quota = SocketQuota::checked(1).unwrap();
        let end = Instant::now() + Duration::from_secs(3);
        let socket = connect(snapshot(addr.port(), end, end), end, &control, &quota)
            .await
            .unwrap();
        let (peer, _) = listener.accept().await.unwrap();
        control.close();
        assert_eq!(socket.ready_for_tls(&control), Err(SocketError::Closing));
        assert_eq!(quota.used(), 1);
        drop(socket);
        eof(peer).await;
        assert_eq!(quota.used(), 0);
    });
}

#[test]
fn whole_deadline_after_connect_prevents_handoff_without_early_quota_release() {
    runtime().block_on(async {
        let (listener, addr) = listener().await;
        let control = DialControl::new();
        let quota = SocketQuota::checked(1).unwrap();
        let end = Instant::now() + Duration::from_millis(40);
        let socket = connect(snapshot(addr.port(), end, end), end, &control, &quota)
            .await
            .unwrap();
        let (peer, _) = listener.accept().await.unwrap();
        tokio::time::sleep_until(end.into()).await;
        assert_eq!(socket.ready_for_tls(&control), Err(SocketError::Deadline));
        assert_eq!(quota.used(), 1);
        drop(socket);
        eof(peer).await;
        assert_eq!(quota.used(), 0);
    });
}

#[test]
fn invalid_socket_quota_is_rejected_before_work() {
    assert!(matches!(
        SocketQuota::checked(0),
        Err(SocketError::InvalidLimit)
    ));
    assert!(matches!(
        SocketQuota::checked(MAX_NATIVE_SOCKETS + 1),
        Err(SocketError::InvalidLimit)
    ));
}

#[test]
fn ttl_expiring_before_first_native_poll_never_opens_socket() {
    // This barrier is reached after begin_dial() and quota reservation, but
    // before Tokio's real numeric connect future receives its first poll.
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        runtime().block_on(async {
            let quota = SocketQuota::checked(1).unwrap();
            let control = DialControl::new();
            let valid_until = Instant::now() + Duration::from_millis(500);
            let operation_end = Instant::now() + Duration::from_secs(3);
            let checked = snapshot(port, valid_until, operation_end);
            let result = connect_inner(checked, operation_end, &control, &quota, |selected| {
                entered_tx.send(valid_until).unwrap();
                release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
                TcpStream::connect(selected)
            })
            .await
            .map(|_| ());
            (result, quota.used())
        })
    });
    let valid_until = entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    std::thread::sleep(valid_until.saturating_duration_since(Instant::now()));
    assert!(Instant::now() >= valid_until);
    release_tx.send(()).unwrap();
    let (result, used) = worker.join().unwrap();
    let accepted = listener.accept();
    assert!(
        matches!(result, Err(SocketError::Address(AddressError::Expired))),
        "TTL must be rechecked at native poll, actual result: {result:?}"
    );
    assert!(
        matches!(accepted, Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock),
        "post-TTL native connect reached the owned listener"
    );
    assert_eq!(used, 0);
}

#[test]
fn ttl_expiring_after_native_start_does_not_invalidate_owned_connection() {
    runtime().block_on(async {
        let (listener, addr) = listener().await;
        let control = DialControl::new();
        let quota = SocketQuota::checked(1).unwrap();
        let valid_until = Instant::now() + Duration::from_secs(1);
        let operation_end = Instant::now() + Duration::from_secs(3);
        let socket = connect(
            snapshot(addr.port(), valid_until, operation_end),
            operation_end,
            &control,
            &quota,
        )
        .await
        .unwrap();
        let (peer, _) = listener.accept().await.unwrap();
        tokio::time::sleep_until(valid_until.into()).await;
        assert!(Instant::now() >= valid_until);
        assert_eq!(socket.ready_for_tls(&control), Ok(()));
        assert_eq!(quota.used(), 1);
        drop(socket);
        eof(peer).await;
        assert_eq!(quota.used(), 0);
    });
}
