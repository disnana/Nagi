//! Real Session factory binding; finite authorization barriers, no token output.
use super::*;
use crate::{auth::session, sqlite};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use tokio::sync::Notify;

// Keep these in the existing SF03 deadline CI filter on every target.
#[path = "csrf_body_deadline_tests.rs"]
mod body_selection_tests;
#[path = "csrf_pool_readiness_tests.rs"]
mod pool_readiness_tests;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    dir: std::path::PathBuf,
    pool: sqlite::Pool,
    store: session::Store,
    probe: Arc<session::deadline_probe::Probe>,
    headers: RequestHeaders,
}
impl Fixture {
    async fn new() -> Self {
        Self::profile(1000, 10000, 10).await
    }
    // Only late-result setup uses the normal Session profile. The stage below
    // still expires the same actual lease in 200ms, after native setup is done.
    async fn normal() -> Self {
        Self::profile(10000, 100000, 60).await
    }
    async fn profile(idle_ms: i64, absolute_ms: i64, credential_s: u64) -> Self {
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
        let mut store = session::open(
            &pool,
            session::options(4, 5, 72, 1, idle_ms, absolute_ms, 500, 2).unwrap(),
            session::cookie_options("__Host-auth", session::SameSite::Strict).unwrap(),
        )
        .await
        .unwrap();
        let probe = session::deadline_probe::install(&mut store);
        let owner = LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap();
        let scope = AuthScope::bind(
            VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(credential_s))
                .unwrap(),
            Arc::clone(&owner.lease),
        )
        .unwrap();
        let intent = session::issue(&store, scope).await.unwrap();
        let request = response_public::tests::request_fixture(1);
        let response = empty(response_context(&request), Status::OK).unwrap();
        let response = session::apply(response, intent).unwrap();
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
            probe,
            headers: cookie.into(),
        }
    }
    async fn original_row(&self) -> DeadlineRow {
        let tx = sqlite::begin(&self.pool, sqlite::BeginMode::Deferred)
            .await
            .unwrap();
        let mut rows = sqlite::all::<DeadlineRow>(
            &tx,
            sqlite::literal("SELECT idle_ms,absolute_ms FROM __nagi_session_rows"),
            sqlite::parameters(),
        )
        .await
        .unwrap();
        sqlite::commit(tx).await.unwrap();
        assert_eq!(rows.len(), 1, "projection_requires_one_original_row");
        rows.pop().unwrap()
    }
    fn request(&self) -> Request {
        Request {
            method: Method::POST,
            uri: "/guarded".parse().unwrap(),
            headers: self.headers.share(),
            body: Bytes::new(),
        }
    }
    async fn stop(self) {
        sqlite::close(&self.pool, 2000).await.unwrap();
        std::fs::remove_dir_all(self.dir).unwrap();
    }
}
struct DeadlineRow {
    idle_ms: i64,
    absolute_ms: i64,
}
impl crate::FromRow for DeadlineRow {
    fn columns() -> &'static [&'static str] {
        &["idle_ms", "absolute_ms"]
    }
    fn read(r: &crate::rusqlite::Row<'_>, i: &[usize]) -> crate::rusqlite::Result<Self> {
        Ok(Self {
            idle_ms: r.get(i[0])?,
            absolute_ms: r.get(i[1])?,
        })
    }
}
struct Projection {
    row: DeadlineRow,
    probe: Arc<session::deadline_probe::Probe>,
    lease: Arc<Lease>,
    until: Instant,
    late: bool,
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
    bound_count: AtomicUsize,
    stage_started: AtomicUsize,
    stage_expired: AtomicUsize,
    dropped: AtomicUsize,
    projection: Option<Projection>,
    completion: Completion,
}
fn state(completion: Completion) -> Arc<BarrierState> {
    projected_state(completion, None)
}
fn projected_state(completion: Completion, projection: Option<Projection>) -> Arc<BarrierState> {
    Arc::new(BarrierState {
        bound: Notify::new(),
        release: Notify::new(),
        completed: AtomicUsize::new(0),
        bound_count: AtomicUsize::new(0),
        stage_started: AtomicUsize::new(0),
        stage_expired: AtomicUsize::new(0),
        dropped: AtomicUsize::new(0),
        projection,
        completion,
    })
}
struct AuthorizerDrop(Arc<BarrierState>);
impl Drop for AuthorizerDrop {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
    }
}
fn factory(store: &session::Store) -> Arc<SessionPolicyFactory<BarrierState, Grant<()>>> {
    let policy = session_authorized_policy(
        session::clone_store(store),
        |scope, _, state: Arc<BarrierState>| async move {
            // A real Session factory binds before invoking this callback.
            let grant = Grant::from_authorized(scope, 7)?;
            let _drop = AuthorizerDrop(Arc::clone(&state));
            if let Some(p) = &state.projection {
                let c = p.probe.take();
                assert!(c.floor_ms <= c.ceil_ms, "projection_sample_order");
                assert!(
                    p.row.idle_ms < p.row.absolute_ms,
                    "projection_idle_before_absolute"
                );
                let remaining = p
                    .row
                    .idle_ms
                    .min(p.row.absolute_ms)
                    .checked_sub(c.ceil_ms)
                    .filter(|n| *n > 0)
                    .expect("projection_original_row_live");
                let expected = c
                    .monotonic
                    .checked_add(Duration::from_millis(u64::try_from(remaining).unwrap()))
                    .expect("projection_checked_deadline");
                assert!(expected < p.until, "projection_original_before_request");
                assert!(
                    grant.belongs_to(&p.lease),
                    "projection_same_real_grant_lease"
                );
                assert!(
                    csrf::live_deadline(&p.lease).unwrap() == expected,
                    "projection_deadline_equality"
                );
            }
            assert_eq!(state.bound_count.fetch_add(1, Ordering::SeqCst), 0);
            state.bound.notify_one();
            state.release.notified().await;
            if let Some(p) = state.projection.as_ref().filter(|p| p.late) {
                // This continuation is polled only after await_preparation's
                // live precheck. The same poll creates and exhausts the
                // shorter bound, so only its post-poll check can win.
                let original = csrf::live_deadline(&p.lease).unwrap();
                let stage = Instant::now()
                    .checked_add(Duration::from_millis(200))
                    .unwrap();
                assert!(stage < original, "late_stage_before_original");
                csrf::constrain(&p.lease, stage).unwrap();
                assert!(
                    csrf::live_deadline(&p.lease).unwrap() == stage,
                    "late_stage_same_gate"
                );
                assert_eq!(state.stage_started.fetch_add(1, Ordering::SeqCst), 0);
                while let Some(remaining) = stage.checked_duration_since(Instant::now()) {
                    std::thread::sleep(remaining);
                }
                assert!(p.lease.validate().is_err(), "late_stage_actual_expired");
                assert_eq!(state.stage_expired.fetch_add(1, Ordering::SeqCst), 0);
            }
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
struct PreparationWake(Notify);
impl std::task::Wake for PreparationWake {
    fn wake(self: Arc<Self>) {
        self.0.notify_one();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.notify_one();
    }
}
#[tokio::test]
async fn guarded_preparation_pending_uses_actual_session_deadline() {
    // Keep the original 1000/10000 Session and 10s credential profile. This
    // asserts its actual projection/timer, not load-independent 1s admission.
    let f = Fixture::new().await;
    let until = Instant::now() + Duration::from_secs(10);
    let owner = LeaseOwner::new(until).unwrap();
    let row = f.original_row().await;
    f.probe.arm();
    let state = projected_state(
        Completion::Grant,
        Some(Projection {
            row,
            probe: Arc::clone(&f.probe),
            lease: Arc::clone(&owner.lease),
            until,
            late: false,
        }),
    );
    {
        let future = factory(&f.store)(
            f.request(),
            Arc::clone(&state),
            Arc::clone(&owner.lease),
            PreparationProfile::GuardedSession,
        );
        let mut wait = Box::pin(await_preparation(
            &owner,
            until,
            AssertUnwindSafe(future).catch_unwind(),
        ));
        let wake = Arc::new(PreparationWake(Notify::new()));
        let waker = std::task::Waker::from(Arc::clone(&wake));
        // Native completion is also driven only by request-local notifications.
        // The timeout/watchdog never receives or polls `wait`.
        loop {
            let polled = wait.as_mut().poll(&mut Context::from_waker(&waker));
            assert!(polled.is_pending(), "pending_completed_before_bound");
            if state.bound_count.load(Ordering::SeqCst) == 1 {
                break;
            }
            assert!(
                tokio::time::timeout_at(tokio::time::Instant::from_std(until), wake.0.notified())
                    .await
                    .is_ok(),
                "pending_native_notification_watchdog"
            );
        }
        let actual = csrf::live_deadline(&owner.lease).unwrap();
        assert!(actual < until);
        // Consume any notification already issued by the completed lookup.
        // The authorizer's sole remaining source is its unreleased Notify.
        while wake.0.notified().now_or_never().is_some() {}
        loop {
            let notification = tokio::time::timeout_at(
                tokio::time::Instant::from_std(actual + Duration::from_millis(300)),
                wake.0.notified(),
            )
            .await;
            assert!(notification.is_ok(), "pending_actual_timer_wake_watchdog");
            // Observe real time after a wake and before this explicit poll.
            let expired = Instant::now() >= actual;
            let polled = wait.as_mut().poll(&mut Context::from_waker(&waker));
            if expired {
                assert!(
                    matches!(polled, Poll::Ready(Err(Status::GATEWAY_TIMEOUT))),
                    "pending_actual_timer_outer_504"
                );
                break;
            }
            assert!(polled.is_pending(), "pending_before_actual_deadline");
        }
    }
    assert_eq!(state.completed.load(Ordering::SeqCst), 0);
    assert_eq!(
        state.dropped.load(Ordering::SeqCst),
        1,
        "pending_future_dropped_once"
    );
    assert!(owner.lease.validate().is_err());
    f.stop().await;
}
// Retain the original aggregate name/three strict late branches for existing CI
// filters. Separate exact cases below let compiled mutants reach every branch.
#[tokio::test]
async fn guarded_preparation_expiry_precedes_late_grant_denial_and_panic() {
    for completion in [Completion::Grant, Completion::Denied, Completion::Panic] {
        preparation_result_case(completion, false).await;
        preparation_result_case(completion, true).await;
    }
}
#[tokio::test]
async fn guarded_preparation_same_poll_late_grant_is_504() {
    preparation_result_case(Completion::Grant, true).await;
}
#[tokio::test]
async fn guarded_preparation_same_poll_late_denied_is_504() {
    preparation_result_case(Completion::Denied, true).await;
}
#[tokio::test]
async fn guarded_preparation_same_poll_late_panic_is_504() {
    preparation_result_case(Completion::Panic, true).await;
}
async fn preparation_result_case(completion: Completion, late: bool) {
    let f = Fixture::normal().await;
    let until = Instant::now() + Duration::from_secs(20);
    let owner = LeaseOwner::new(until).unwrap();
    let row = f.original_row().await;
    f.probe.arm();
    let state = projected_state(
        completion,
        Some(Projection {
            row,
            probe: Arc::clone(&f.probe),
            lease: Arc::clone(&owner.lease),
            until,
            late,
        }),
    );
    // Own this one future in a lexical scope: even a Pending timeout result is
    // dropped before DB close/join. No replacement owner, factory or proof.
    {
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
            result = &mut wait => match result {
                Err(status) => panic!(
                    "preparation completed before barrier: outer_status={}", status.value()
                ),
                Ok(Err(_)) => panic!("preparation completed before barrier: preparation_panicked"),
                Ok(Ok(Err(error))) => panic!(
                    "preparation completed before barrier: kind={:?}", error.kind()
                ),
                Ok(Ok(Ok(_))) => panic!("preparation completed before barrier: preparation_ready"),
            },
        }
        state.release.notify_one();
        let result = wait.await;
        if late {
            let outer_504 = matches!(result, Err(Status::GATEWAY_TIMEOUT));
            match completion {
                Completion::Grant => assert!(outer_504, "late_grant_outer_504"),
                Completion::Denied => assert!(outer_504, "late_denied_outer_504"),
                Completion::Panic => assert!(outer_504, "late_panic_outer_504"),
            }
            assert_eq!(
                state.stage_started.load(Ordering::SeqCst),
                1,
                "late_stage_started_once"
            );
            assert_eq!(
                state.stage_expired.load(Ordering::SeqCst),
                1,
                "late_stage_expired_once"
            );
            assert!(owner.lease.validate().is_err());
        } else {
            match completion {
                Completion::Grant => assert!(matches!(result, Ok(Ok(Ok(_)))), "live_real_grant"),
                Completion::Denied => assert!(
                    matches!(result, Ok(Ok(Err(e))) if e.kind() == FailureKind::Denied),
                    "live_real_denied"
                ),
                Completion::Panic => assert!(matches!(result, Ok(Err(_))), "live_real_panic"),
            }
            assert!(owner.lease.validate().is_ok(), "live_control_gate");
            assert_eq!(state.stage_started.load(Ordering::SeqCst), 0);
            assert_eq!(state.stage_expired.load(Ordering::SeqCst), 0);
        }
    }
    assert_eq!(
        state.completed.load(Ordering::SeqCst),
        1,
        "authorizer_completed_once"
    );
    assert_eq!(
        state.dropped.load(Ordering::SeqCst),
        1,
        "authorizer_dropped_once"
    );
    f.stop().await;
}
#[tokio::test]
async fn guarded_preparation_initially_expired_session_still_challenges_401() {
    let response_request = response_public::tests::request_fixture(64);
    let response_context = response_context(&response_request);
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
            let response = security_response(response_context, error, PolicySource::Session, false);
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
    let response_request = response_public::tests::request_fixture(64);
    let context = response_context(&response_request);
    let producer = Arc::clone(&context.producer);
    let fallback = Arc::clone(&context.fallback);
    drop(response_request);
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
        let producer = Arc::clone(&producer);
        let fallback = Arc::clone(&fallback);
        let connection = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let ledger =
                response_send::SendLedger::new(Arc::clone(&producer), Arc::clone(&fallback));
            let service_ledger = Arc::clone(&ledger);
            let service = hyper::service::service_fn(
                move |request: hyper::Request<hyper::body::Incoming>| {
                    let observed = Arc::clone(&observed);
                    let ledger = Arc::clone(&service_ledger);
                    let context = ResponseContext {
                        profile: response_public::ResponseProfile::default(),
                        producer: Arc::clone(&producer),
                        fallback: Arc::clone(&fallback),
                    };
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
                        Ok::<_, std::convert::Infallible>(
                            vary::finish(managed(
                                transport(&context, status, false, true),
                                false,
                                true,
                            ))
                            .transfer_native(&ledger, false)
                            .unwrap(),
                        )
                    }
                },
            );
            let permit = Arc::new(Semaphore::new(1)).try_acquire_owned().unwrap();
            let native = hyper::server::conn::http1::Builder::new()
                .serve_connection(hyper_util::rt::TokioIo::new(socket), service);
            response_send::NativeDriver::new(native, ledger, permit)
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
        assert!(headers.contains("vary: authorization, cookie, origin, x-csrf-token"));
        assert!(!headers.contains("set-cookie:") && !headers.contains("www-authenticate:"));
        tokio::time::timeout(Duration::from_secs(2), connection)
            .await
            .unwrap()
            .unwrap();
    }
}
