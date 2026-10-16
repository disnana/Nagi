use super::super::response_budget::Limits;
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot, Semaphore},
};

const LIMIT: Duration = Duration::from_secs(2); // Test harness only.
fn producer(live: usize) -> Arc<Producer> {
    Arc::new(
        Producer::checked_wire(Limits {
            body_bytes: 32,
            user_header_bytes: 128,
            user_header_count: 4,
            managed_header_bytes: 512,
            managed_header_count: 12,
            serialization_writes: 64,
            live_responses: live,
            retained_bytes: 65536 * live,
        })
        .unwrap(),
    )
}
fn fallback() -> Arc<FallbackLedger> {
    Arc::new(
        FallbackLedger::checked(
            FallbackLimits {
                responses: 1,
                retained_bytes: 65536,
                managed_header_bytes: 512,
                managed_header_count: 12,
            },
            1,
        )
        .unwrap(),
    )
}
fn capacity(producer: &Producer) {
    assert_eq!(producer.text("blocked").err(), Some(Diagnostic::Capacity));
}
async fn sockets() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (server, _) = listener.accept().await.unwrap();
    (client, server)
}
async fn request(client: &mut TcpStream, bytes: &[u8]) {
    tokio::time::timeout(LIMIT, client.write_all(bytes))
        .await
        .unwrap()
        .unwrap();
}
async fn response(client: &mut TcpStream, head: bool) -> (u16, Vec<u8>) {
    tokio::time::timeout(LIMIT, async {
        let mut raw = Vec::new();
        while !raw.ends_with(b"\r\n\r\n") {
            assert!(raw.len() < 8192);
            let mut byte = [0u8; 1];
            client.read_exact(&mut byte).await.unwrap();
            raw.push(byte[0]);
        }
        let headers = std::str::from_utf8(&raw).unwrap();
        let status = headers
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let length: usize = headers
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .map_or(0, |(_, value)| value.trim().parse().unwrap());
        let mut body = vec![0; if head { 0 } else { length }];
        client.read_exact(&mut body).await.unwrap();
        (status, body)
    })
    .await
    .unwrap()
}
async fn eof(client: &mut TcpStream) {
    let mut byte = [0; 1];
    assert_eq!(
        tokio::time::timeout(LIMIT, client.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
}
async fn permit(pool: &Arc<Semaphore>) -> OwnedSemaphorePermit {
    Arc::clone(pool).acquire_owned().await.unwrap()
}

#[tokio::test]
async fn head_and_empty_keep_native_cache_charge_at_ready_until_driver_drop() {
    for head in [false, true] {
        let ordinary = producer(1);
        let made = Arc::clone(&ordinary);
        let pool = Arc::new(Semaphore::new(1));
        let (mut client, server) = sockets().await;
        let (mut driver, ledger) = connection(
            server,
            Arc::clone(&ordinary),
            fallback(),
            permit(&pool).await,
            move |_request| {
                let wire = made
                    .text(if head { "representation" } else { "" })
                    .unwrap()
                    .into_wire()
                    .unwrap();
                async move { Ok((StatusCode::OK, wire)) }
            },
        );
        let task = tokio::spawn(async move {
            let result = (&mut driver).await;
            (driver, result)
        });
        request(
            &mut client,
            if head {
                b"HEAD / HTTP/1.1\r\nHost: localhost\r\n\r\n"
            } else {
                b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n"
            },
        )
        .await;
        let (status, body) = response(&mut client, head).await;
        assert_eq!(status, 200);
        assert!(body.is_empty());
        capacity(&ordinary); // Successful flush and empty/HEAD body do not release.
        client.shutdown().await.unwrap();
        let (driver, result) = tokio::time::timeout(LIMIT, task).await.unwrap().unwrap();
        assert!(result.is_ok());
        assert_eq!(pool.available_permits(), 0);
        capacity(&ordinary);
        assert!(!ledger.state.lock().unwrap().closed);
        drop(driver);
        assert!(ledger.state.lock().unwrap().closed);
        assert_eq!(pool.available_permits(), 1);
        assert!(ordinary.text("released").is_ok());
    }
}

#[tokio::test]
async fn keep_alive_cache_moves_to_same_shared_request_owner_past_native_shutdown() {
    let ordinary = producer(1);
    let made = Arc::clone(&ordinary);
    let (view_tx, view_rx) = oneshot::channel();
    let view_tx = Mutex::new(Some(view_tx));
    let pool = Arc::new(Semaphore::new(1));
    let (mut client, server) = sockets().await;
    let (mut driver, ledger) = connection(
        server,
        Arc::clone(&ordinary),
        fallback(),
        permit(&pool).await,
        move |request| {
            let later = request.uri.path() == "/later";
            if later {
                assert_eq!(request.headers.value("x-read"), Some(b"value".as_slice()));
                assert!(view_tx
                    .lock()
                    .unwrap()
                    .take()
                    .unwrap()
                    .send(request.snapshot())
                    .is_ok());
                capacity(&made); // Prior response claim follows this actual native map.
            }
            let output = if later {
                Err(FallbackStatus::Capacity)
            } else {
                Ok((
                    StatusCode::OK,
                    made.text("first").unwrap().into_wire().unwrap(),
                ))
            };
            async move { output }
        },
    );
    let task = tokio::spawn(async move {
        let result = (&mut driver).await;
        (driver, result)
    });
    request(
        &mut client,
        b"GET /first HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    assert_eq!(response(&mut client, false).await, (200, b"first".to_vec()));
    request(
        &mut client,
        b"GET /later HTTP/1.1\r\nHost: localhost\r\nX-Read: value\r\n\r\n",
    )
    .await;
    assert_eq!(response(&mut client, false).await.0, 503);
    let view = view_rx.await.unwrap();
    let clone = Arc::clone(&view);
    assert!(Arc::ptr_eq(&view, &clone));
    assert!(view.capacity() >= 16);
    client.shutdown().await.unwrap();
    let (driver, result) = tokio::time::timeout(LIMIT, task).await.unwrap().unwrap();
    assert!(result.is_ok());
    drop(driver);
    assert!(ledger.state.lock().unwrap().closed);
    assert_eq!(pool.available_permits(), 1);
    capacity(&ordinary);
    drop(view);
    capacity(&ordinary);
    assert_eq!(clone.value("x-read"), Some(b"value".as_slice()));
    drop(clone);
    assert!(ordinary.text("released").is_ok());
}

#[tokio::test]
async fn pipelined_requests_keep_one_cache_slot_and_multiple_prior_map_owners() {
    let ordinary = producer(3);
    let made = Arc::clone(&ordinary);
    let (views, mut receive) = mpsc::channel(2);
    let pool = Arc::new(Semaphore::new(1));
    let (mut client, server) = sockets().await;
    let (mut driver, ledger) = connection(
        server,
        Arc::clone(&ordinary),
        fallback(),
        permit(&pool).await,
        move |request| {
            if request.uri.path() != "/one" {
                assert!(views.try_send(request.snapshot()).is_ok());
            }
            let output = made.text("x").unwrap().into_wire().unwrap();
            async move { Ok((StatusCode::OK, output)) }
        },
    );
    let task = tokio::spawn(async move {
        let result = (&mut driver).await;
        (driver, result)
    });
    request(&mut client, b"GET /one HTTP/1.1\r\nHost: localhost\r\n\r\nGET /two HTTP/1.1\r\nHost: localhost\r\n\r\nGET /three HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    for _ in 0..3 {
        assert_eq!(response(&mut client, false).await, (200, b"x".to_vec()));
    }
    let second = receive.recv().await.unwrap();
    let third = receive.recv().await.unwrap();
    assert!(ledger.state.lock().unwrap().cached.is_some());
    capacity(&ordinary);
    client.shutdown().await.unwrap();
    let (driver, result) = tokio::time::timeout(LIMIT, task).await.unwrap().unwrap();
    assert!(result.is_ok());
    capacity(&ordinary);
    drop(driver);
    let free = ordinary.text("one free").unwrap();
    capacity(&ordinary);
    drop(second);
    let another = ordinary.text("two free").unwrap();
    capacity(&ordinary);
    drop(third);
    assert!(ordinary.text("three free").is_ok());
    drop((free, another));
    assert_eq!(pool.available_permits(), 1);
}

#[tokio::test]
async fn fallback_is_independent_then_exhaustion_closes_real_transport_without_reallocation() {
    let ordinary = producer(1);
    let held = ordinary.text("held").unwrap();
    capacity(&ordinary);
    let fallback = fallback();
    let pool = Arc::new(Semaphore::new(1));
    let (view_tx, view_rx) = oneshot::channel();
    let view_tx = Mutex::new(Some(view_tx));
    let (mut client, server) = sockets().await;
    let (mut driver, _) = connection(
        server,
        Arc::clone(&ordinary),
        Arc::clone(&fallback),
        permit(&pool).await,
        move |request| {
            if request.uri.path() == "/last" {
                assert!(view_tx
                    .lock()
                    .unwrap()
                    .take()
                    .unwrap()
                    .send(request.snapshot())
                    .is_ok());
            }
            async { Err(FallbackStatus::Capacity) }
        },
    );
    let task = tokio::spawn(async move {
        let result = (&mut driver).await;
        (driver, result)
    });
    request(
        &mut client,
        b"GET /first HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    assert_eq!(response(&mut client, false).await.0, 503);
    let attempts = fallback.allocation_attempts();
    request(
        &mut client,
        b"GET /last HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    let view = view_rx.await.unwrap();
    eof(&mut client).await;
    let (driver, result) = tokio::time::timeout(LIMIT, task).await.unwrap().unwrap();
    assert!(result.is_err());
    drop(driver);
    assert_eq!(pool.available_permits(), 1);
    assert_eq!(fallback.allocation_attempts(), attempts);
    assert!(fallback.produce(FallbackStatus::Internal).is_close());
    drop(view);
    assert!(fallback
        .produce(FallbackStatus::Internal)
        .into_response()
        .is_some());
    capacity(&ordinary);
    drop(held);
    assert!(ordinary.text("released").is_ok());
}

#[tokio::test]
async fn foreign_wire_fails_closed_and_native_connection_actually_ends() {
    let ordinary = producer(1);
    let foreign = producer(1);
    let made = Arc::clone(&foreign);
    let pool = Arc::new(Semaphore::new(1));
    let (mut client, server) = sockets().await;
    let (mut driver, ledger) = connection(
        server,
        Arc::clone(&ordinary),
        fallback(),
        permit(&pool).await,
        move |_| {
            let wire = made.text("foreign").unwrap().into_wire().unwrap();
            async move { Ok((StatusCode::OK, wire)) }
        },
    );
    let task = tokio::spawn(async move {
        let result = (&mut driver).await;
        (driver, result)
    });
    request(&mut client, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    eof(&mut client).await;
    let (driver, result) = tokio::time::timeout(LIMIT, task).await.unwrap().unwrap();
    assert!(result.is_err());
    assert!(ledger.state.lock().unwrap().cached.is_none());
    drop(driver);
    assert_eq!(pool.available_permits(), 1);
    assert!(ordinary.text("own free").is_ok());
    assert!(foreign.text("foreign free").is_ok());
}

#[tokio::test]
async fn parse_failure_does_not_release_pending_claim_at_ready() {
    let ordinary = producer(1);
    let made = Arc::clone(&ordinary);
    let pool = Arc::new(Semaphore::new(1));
    let (mut client, server) = sockets().await;
    let (mut driver, ledger) = connection(
        server,
        Arc::clone(&ordinary),
        fallback(),
        permit(&pool).await,
        move |_| {
            let wire = made.text("").unwrap().into_wire().unwrap();
            async move { Ok((StatusCode::OK, wire)) }
        },
    );
    let task = tokio::spawn(async move {
        let result = (&mut driver).await;
        (driver, result)
    });
    request(&mut client, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    assert_eq!(response(&mut client, false).await.0, 200);
    capacity(&ordinary);
    request(&mut client, b"not-http\r\n\r\n").await;
    assert_eq!(response(&mut client, false).await.0, 400);
    eof(&mut client).await;
    let (driver, result) = tokio::time::timeout(LIMIT, task).await.unwrap().unwrap();
    assert!(result.is_err());
    capacity(&ordinary);
    assert_eq!(pool.available_permits(), 0);
    drop(driver);
    assert!(ledger.state.lock().unwrap().closed);
    assert!(ordinary.text("released").is_ok());
    assert_eq!(pool.available_permits(), 1);
}

#[tokio::test]
async fn abort_before_actual_join_and_handler_panic_release_only_on_native_destruction() {
    for panic_handler in [false, true] {
        let ordinary = producer(1);
        let made = Arc::clone(&ordinary);
        let pool = Arc::new(Semaphore::new(1));
        let (mut client, server) = sockets().await;
        let (driver, ledger) = connection(
            server,
            Arc::clone(&ordinary),
            fallback(),
            permit(&pool).await,
            move |request| {
                if request.uri.path() == "/panic" {
                    panic!("private native panic control");
                }
                let wire = made.text("").unwrap().into_wire().unwrap();
                async move { Ok((StatusCode::OK, wire)) }
            },
        );
        let task = tokio::spawn(driver);
        request(&mut client, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
        assert_eq!(response(&mut client, false).await.0, 200);
        capacity(&ordinary);
        if panic_handler {
            request(
                &mut client,
                b"GET /panic HTTP/1.1\r\nHost: localhost\r\n\r\n",
            )
            .await;
            assert!(tokio::time::timeout(LIMIT, task)
                .await
                .unwrap()
                .unwrap_err()
                .is_panic());
        } else {
            // current_thread test: abort only schedules cancellation. There is
            // no intervening await/poll that could have destroyed native yet.
            task.abort();
            capacity(&ordinary);
            assert_eq!(pool.available_permits(), 0);
            assert!(!ledger.state.lock().unwrap().closed);
            assert!(tokio::time::timeout(LIMIT, task)
                .await
                .unwrap()
                .unwrap_err()
                .is_cancelled());
        }
        assert!(ledger.state.lock().unwrap().closed);
        assert_eq!(pool.available_permits(), 1);
        assert!(ordinary.text("released").is_ok());
    }
}

struct ProbeIo {
    inner: TcpStream,
    ledger: Arc<Mutex<Option<std::sync::Weak<SendLedger>>>>,
    pool: Arc<Semaphore>,
    dropped_in_order: Arc<AtomicBool>,
}
impl AsyncRead for ProbeIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buffer)
    }
}
impl AsyncWrite for ProbeIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, bytes)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
impl Drop for ProbeIo {
    fn drop(&mut self) {
        let native_first = self
            .ledger
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .upgrade()
            .is_some_and(|ledger| !ledger.state.lock().unwrap().closed);
        self.dropped_in_order.store(
            native_first && self.pool.available_permits() == 0,
            Ordering::SeqCst,
        );
    }
}
#[tokio::test]
async fn unpolled_and_ready_native_io_drop_before_cache_guard_and_connection_permit() {
    for polled in [false, true] {
        let ordinary = producer(1);
        let made = Arc::clone(&ordinary);
        let pool = Arc::new(Semaphore::new(1));
        let (mut client, server) = sockets().await;
        let link = Arc::new(Mutex::new(None));
        let ordered = Arc::new(AtomicBool::new(false));
        let io = ProbeIo {
            inner: server,
            ledger: Arc::clone(&link),
            pool: Arc::clone(&pool),
            dropped_in_order: Arc::clone(&ordered),
        };
        let (mut driver, ledger) = connection(
            io,
            Arc::clone(&ordinary),
            fallback(),
            permit(&pool).await,
            move |_| {
                let wire = made.text("").unwrap().into_wire().unwrap();
                async move { Ok((StatusCode::OK, wire)) }
            },
        );
        *link.lock().unwrap() = Some(Arc::downgrade(&ledger));
        if polled {
            let task = tokio::spawn(async move {
                let result = (&mut driver).await;
                (driver, result)
            });
            request(&mut client, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
            assert_eq!(response(&mut client, false).await.0, 200);
            capacity(&ordinary);
            client.shutdown().await.unwrap();
            let (returned, result) = tokio::time::timeout(LIMIT, task).await.unwrap().unwrap();
            assert!(result.is_ok());
            assert!(ordered.load(Ordering::SeqCst));
            capacity(&ordinary);
            drop(returned);
        } else {
            drop(driver);
        }
        assert!(ordered.load(Ordering::SeqCst));
        assert!(ledger.state.lock().unwrap().closed);
        assert_eq!(pool.available_permits(), 1);
        assert!(ordinary.text("released").is_ok());
    }
}
