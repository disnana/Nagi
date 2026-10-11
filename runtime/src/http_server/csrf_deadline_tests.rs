//! Real Session factory binding; finite authorization barriers, no token output.
use super::*;
use crate::{auth::session, sqlite};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use tokio::sync::Notify;

// Keep these in the existing SF03 deadline CI filter on every target.
#[path = "csrf_body_deadline_tests.rs"]
mod body_selection_tests;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    dir: std::path::PathBuf,
    pool: sqlite::Pool,
    store: session::Store,
    headers: HeaderMap,
}
impl Fixture {
    async fn new() -> Self {
        let dir = loop {
            let p = std::env::temp_dir().join(format!(
                "b1-deadline-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&p) {
                Ok(()) => break p,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => panic!("owned directory unavailable"),
            }
        };
        let pool = sqlite::open(
            dir.join("owned.sqlite").to_str().unwrap(),
            sqlite::options(1, 2, 1000, 1000).unwrap(),
        )
        .await
        .unwrap();
        let store = session::open(
            &pool,
            session::options(4, 5, 72, 1, 1000, 10000, 500, 2).unwrap(),
            session::cookie_options("__Host-auth", session::SameSite::Strict).unwrap(),
        )
        .await
        .unwrap();
        let owner = LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap();
        let scope = AuthScope::bind(
            VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(10)).unwrap(),
            Arc::clone(&owner.lease),
        )
        .unwrap();
        let intent = session::issue(&store, scope).await.unwrap();
        let response = session::apply(empty(Status::OK), intent).unwrap();
        let (status, headers) = session_delivery_headers_for_test(response, &owner);
        assert_eq!(status.as_u16(), 200);
        let mut cookie = HeaderMap::new();
        cookie.insert(
            names::COOKIE,
            HeaderValue::from_str(
                headers
                    .get(names::SET_COOKIE)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .split(';')
                    .next()
                    .unwrap(),
            )
            .unwrap(),
        );
        Self {
            dir,
            pool,
            store,
            headers: cookie,
        }
    }
    fn request(&self) -> Request {
        Request {
            method: Method::POST,
            uri: "/guarded".parse().unwrap(),
            headers: self.headers.clone(),
            body: Bytes::new(),
        }
    }
    async fn stop(self) {
        sqlite::close(&self.pool, 2000).await.unwrap();
        std::fs::remove_dir_all(self.dir).unwrap();
    }
}
#[derive(Clone, Copy)]
enum Completion {
    Grant,
    Denied,
    Panic,
}
struct BarrierState {
    bound: Notify,
    release: Notify,
    completed: AtomicUsize,
    completion: Completion,
}
fn state(completion: Completion) -> Arc<BarrierState> {
    Arc::new(BarrierState {
        bound: Notify::new(),
        release: Notify::new(),
        completed: AtomicUsize::new(0),
        completion,
    })
}
fn factory(store: &session::Store) -> Arc<SessionPolicyFactory<BarrierState, Grant<()>>> {
    let policy = session_authorized_policy(
        session::clone_store(store),
        |scope, _, state: Arc<BarrierState>| async move {
            // A real Session factory binds before invoking this callback.
            let grant = Grant::from_authorized(scope, 7)?;
            state.bound.notify_one();
            state.release.notified().await;
            state.completed.fetch_add(1, Ordering::SeqCst);
            match state.completion {
                Completion::Grant => Ok(grant),
                Completion::Denied => Err(Failure::denied()),
                Completion::Panic => panic!("bounded authorizer panic"),
            }
        },
    );
    match policy.kind {
        PolicyKind::Session { factory, .. } => factory,
        _ => panic!("expected fixed Session factory"),
    }
}
#[tokio::test]
async fn guarded_preparation_pending_uses_actual_session_deadline() {
    let f = Fixture::new().await;
    let state = state(Completion::Grant);
    let until = Instant::now() + Duration::from_secs(10);
    let owner = LeaseOwner::new(until).unwrap();
    let future = factory(&f.store)(
        f.request(),
        Arc::clone(&state),
        Arc::clone(&owner.lease),
        PreparationProfile::GuardedSession,
    );
    let wait = await_preparation(&owner, until, AssertUnwindSafe(future).catch_unwind());
    tokio::pin!(wait);
    tokio::select! {
        _ = state.bound.notified() => {},
        _ = &mut wait => panic!("preparation completed before barrier"),
    }
    let actual = csrf::live_deadline(&owner.lease).unwrap();
    assert!(actual < until);
    let result = tokio::time::timeout_at(
        tokio::time::Instant::from_std(actual + Duration::from_millis(300)),
        &mut wait,
    )
    .await;
    assert!(
        matches!(result, Ok(Err(Status::GATEWAY_TIMEOUT))),
        "pending preparation must expire at its actual bound lease without barrier release"
    );
    assert_eq!(state.completed.load(Ordering::SeqCst), 0);
    assert!(owner.lease.validate().is_err());
    f.stop().await;
}
#[tokio::test]
async fn guarded_preparation_expiry_precedes_late_grant_denial_and_panic() {
    for completion in [Completion::Grant, Completion::Denied, Completion::Panic] {
        let f = Fixture::new().await;
        let state = state(completion);
        let until = Instant::now() + Duration::from_secs(10);
        let owner = LeaseOwner::new(until).unwrap();
        let future = factory(&f.store)(
            f.request(),
            Arc::clone(&state),
            Arc::clone(&owner.lease),
            PreparationProfile::GuardedSession,
        );
        let wait = await_preparation(&owner, until, AssertUnwindSafe(future).catch_unwind());
        tokio::pin!(wait);
        tokio::select! {
            _ = state.bound.notified() => {},
            _ = &mut wait => panic!("preparation completed before barrier"),
        }
        let actual = csrf::live_deadline(&owner.lease).unwrap();
        // Stop polling preparation until the real gate expires, then make its
        // completion ready in the same poll. No scheduler timing race is needed.
        tokio::time::sleep_until(tokio::time::Instant::from_std(actual)).await;
        assert!(owner.lease.validate().is_err());
        state.release.notify_one();
        assert!(
            matches!(wait.await, Err(Status::GATEWAY_TIMEOUT)),
            "common gate expiry must precede every late completion branch"
        );
        assert!(state.completed.load(Ordering::SeqCst) <= 1);
        f.stop().await;
    }
}
#[tokio::test]
async fn guarded_preparation_initially_expired_session_still_challenges_401() {
    let f = Fixture::new().await;
    let tx = sqlite::begin(&f.pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    sqlite::exec(
        &tx,
        sqlite::literal("UPDATE __nagi_session_rows SET idle_ms=0"),
        sqlite::parameters(),
    )
    .await
    .unwrap();
    sqlite::commit(tx).await.unwrap();
    let state = state(Completion::Grant);
    let until = Instant::now() + Duration::from_secs(10);
    let owner = LeaseOwner::new(until).unwrap();
    let future = factory(&f.store)(
        f.request(),
        Arc::clone(&state),
        Arc::clone(&owner.lease),
        PreparationProfile::GuardedSession,
    );
    match await_preparation(&owner, until, AssertUnwindSafe(future).catch_unwind()).await {
        Ok(Ok(Err(error))) => {
            let response = security_response(error, PolicySource::Session).into_http(false);
            assert_eq!(response.status().as_u16(), 401);
            assert!(response.headers().contains_key(names::WWW_AUTHENTICATE));
            assert!(!response.headers().contains_key(names::SET_COOKIE));
        }
        _ => panic!("initial expired credential must remain an authentication refusal"),
    }
    assert!(owner.lease.validate().is_ok());
    assert_eq!(state.completed.load(Ordering::SeqCst), 0);
    f.stop().await;
}

// Exercise the same production body wait with real Hyper Incoming and socket
// framing. Session/SQLite preparation has its own public wire oracle, so these
// short body windows do not assume a bound on database or scheduler throughput.
#[tokio::test]
async fn guarded_body_wire_preserves_absolute_deadline_and_strict_precedence() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for (body_ms, security_ms, spent_ms, expected) in [
        (80, 240, 0, 408),
        (240, 80, 0, 504),
        (100, 100, 0, 504),
        (80, 240, 200, 504),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&calls);
        let connection = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let service = hyper::service::service_fn(
                move |request: hyper::Request<hyper::body::Incoming>| {
                    let observed = Arc::clone(&observed);
                    async move {
                        observed.fetch_add(1, Ordering::SeqCst);
                        assert_eq!(request.headers().get(names::CONTENT_LENGTH).unwrap(), "5");
                        let body_started = Instant::now();
                        // Synthetic spent preparation isolates the absolute-bound
                        // contract, without sleeping or replacing either clock.
                        let admitted = body_started - Duration::from_millis(spent_ms);
                        let common = admitted + Duration::from_millis(security_ms);
                        let status = match receive_guarded_body(
                            request.into_body(),
                            256,
                            common,
                            body_started,
                            Duration::from_millis(body_ms),
                        )
                        .await
                        {
                            Ok(_) => panic!("incomplete Content-Length body must not complete"),
                            Err(status) => status,
                        };
                        Ok::<_, std::convert::Infallible>(managed(
                            transport(status, false, true),
                            false,
                            true,
                        ))
                    }
                },
            );
            hyper::server::conn::http1::Builder::new()
                .serve_connection(hyper_util::rt::TokioIo::new(socket), service)
                .await
                .unwrap();
        });
        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        socket.write_all(b"POST /pending HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 5\r\n\r\nov").await.unwrap();
        let mut raw = Vec::new();
        tokio::time::timeout(
            Duration::from_secs(2),
            (&mut socket).take(8193).read_to_end(&mut raw),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(raw.len() <= 8192);
        let wire = std::str::from_utf8(&raw).unwrap();
        let status: u16 = wire.split_whitespace().nth(1).unwrap().parse().unwrap();
        assert_eq!(status, expected);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let headers = wire.split("\r\n\r\n").next().unwrap().to_ascii_lowercase();
        assert!(headers.contains("cache-control: no-store"));
        assert!(headers.contains("vary: cookie, authorization, origin, x-csrf-token"));
        assert!(!headers.contains("set-cookie:") && !headers.contains("www-authenticate:"));
        tokio::time::timeout(Duration::from_secs(2), connection)
            .await
            .unwrap()
            .unwrap();
    }
}
