//! Real native stage witnesses for the same owned BrowserLogin dispatcher.
//! SQL setup precedes the explicit stage budget. This is not evidence that
//! native claim/issue always completes inside an original 200ms admission.
use super::*;
use crate::{
    auth::{self, prelogin, session},
    sqlite, FromRow,
};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "nagi-browser-stage-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed),
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => panic!("owned stage directory unavailable"),
            }
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Number(i64);
impl FromRow for Number {
    fn columns() -> &'static [&'static str] {
        &["n"]
    }
    fn read(row: &crate::rusqlite::Row<'_>, indexes: &[usize]) -> crate::rusqlite::Result<Self> {
        Ok(Self(row.get(indexes[0])?))
    }
}
async fn count(pool: &sqlite::Pool, sql: &'static str) -> i64 {
    let tx = sqlite::begin(pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let value = sqlite::query::<Number>(&tx, sqlite::literal(sql), sqlite::parameters())
        .await
        .unwrap()
        .unwrap()
        .0;
    sqlite::rollback(tx).await.unwrap();
    value
}
fn checked(security: i64) -> Options {
    authority(
        security_timeout(options(256, 1000, 1000, 1000).unwrap(), security).unwrap(),
        "https://localhost",
        vec!["localhost".to_owned()],
        1,
        128,
    )
    .unwrap()
}
struct Fixture {
    _directory: Directory,
    auth: sqlite::Pool,
    store: session::Store,
    prelogin: prelogin::PreLoginPool,
}
impl Fixture {
    async fn new() -> Self {
        let directory = Directory::new();
        let auth = sqlite::open(
            directory.0.join("owned.sqlite").to_str().unwrap(),
            sqlite::options(2, 4, 1000, 1000).unwrap(),
        )
        .await
        .unwrap();
        let store = session::open(
            &auth,
            session::options(4, 5, 72, 1, 10000, 100000, 5000, 2).unwrap(),
            session::cookie_options("__Host-auth", session::SameSite::Strict).unwrap(),
        )
        .await
        .unwrap();
        let prelogin = prelogin::open(
            &store,
            sqlite::options(1, 2, 1000, 1000).unwrap(),
            // Finite stage-setup TTL, not the external socket fixture's TTL.
            prelogin::options(2, 3, 144, 1, 60000, 2, 8, 10000, 4, 16).unwrap(),
            prelogin::cookie_options("__Host-prelogin", session::SameSite::Strict).unwrap(),
            &checked(1000),
        )
        .await
        .unwrap();
        Self {
            _directory: directory,
            auth,
            store,
            prelogin,
        }
    }
    async fn stop(self) {
        prelogin::close(&self.prelogin, 2000).await.unwrap();
        sqlite::close(&self.auth, 2000).await.unwrap();
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Plain,
    PrepareErr,
    PreparePanic,
    PreparePending,
    PrepareLate,
    Issue,
    HandlerErr,
    HandlerPanic,
    HandlerPending,
}
#[derive(Default)]
struct Stats {
    verifier: AtomicUsize,
    handler: AtomicUsize,
    verifier_drops: AtomicUsize,
    handler_drops: AtomicUsize,
    sanitized: AtomicBool,
    issue_ack: AtomicBool,
    fault_entered: AtomicUsize,
    release_issue: tokio::sync::Notify,
}
struct State {
    mode: Mode,
    store: session::Store,
    stats: Arc<Stats>,
}
struct PendingDrop<'a>(&'a AtomicUsize);
impl Drop for PendingDrop<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
async fn verifier(request: Request, state: Arc<State>) -> Result<VerifiedIdentity, auth::Failure> {
    state.stats.verifier.fetch_add(1, Ordering::SeqCst);
    state.stats.sanitized.store(
        !request.headers.contains_key(names::COOKIE)
            && !request.headers.contains_key("x-csrf-token")
            && request.body.is_empty(),
        Ordering::SeqCst,
    );
    match state.mode {
        Mode::PrepareErr => {
            state.stats.fault_entered.store(1, Ordering::SeqCst);
            return Err(auth::invalid_credential());
        }
        Mode::PreparePanic => {
            state.stats.fault_entered.store(2, Ordering::SeqCst);
            panic!("owned stage verifier panic");
        }
        Mode::PreparePending => {
            let _drop = PendingDrop(&state.stats.verifier_drops);
            std::future::pending::<()>().await;
        }
        Mode::PrepareLate => std::thread::sleep(Duration::from_millis(400)),
        _ => {}
    }
    VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(60))
}
async fn handler(request: Request, state: Arc<State>, scope: AuthScope) -> Result<Response, Error> {
    state.stats.handler.fetch_add(1, Ordering::SeqCst);
    assert!(!request.headers.contains_key(names::COOKIE));
    assert!(!request.headers.contains_key("x-csrf-token"));
    if state.mode == Mode::Plain {
        return Ok(text(Status::OK, "owned"));
    }
    let intent = session::issue(&state.store, scope)
        .await
        .map_err(|_| Error::internal("owned stage issue refused"))?;
    let response = session::apply(text(Status::OK, "owned"), intent).unwrap();
    state.stats.issue_ack.store(true, Ordering::SeqCst);
    // This suspension is reached only after the real Session issue/apply
    // returned. The original prepared_handler future retains the real response.
    state.stats.release_issue.notified().await;
    match state.mode {
        Mode::HandlerErr => {
            state.stats.fault_entered.store(3, Ordering::SeqCst);
            Err(Error::internal("owned stage handler Err"))
        }
        Mode::HandlerPanic => {
            state.stats.fault_entered.store(4, Ordering::SeqCst);
            panic!("owned stage handler panic");
        }
        Mode::HandlerPending => {
            let _drop = PendingDrop(&state.stats.handler_drops);
            std::future::pending::<()>().await;
            Ok(response)
        }
        Mode::Issue => Ok(response),
        _ => panic!("unexpected stage handler admission"),
    }
}
fn actual_route(f: &Fixture, mode: Mode) -> (Arc<Pool>, BrowserPrepare<State>, Arc<State>) {
    let app = route(
        app_default(State {
            mode,
            store: session::clone_store(&f.store),
            stats: Arc::new(Stats::default()),
        }),
        Method::POST,
        "/login",
        browser_login_policy(prelogin::clone_pool(&f.prelogin), verifier),
        handler,
    )
    .unwrap();
    let state = Arc::clone(&app.state);
    let mut routes = app.routes;
    let route = routes.get_mut("/login").unwrap().pop().unwrap();
    assert!(route.source == PolicySource::BrowserLogin);
    match route.kind {
        RouteKind::Browser { pool, prepare } => (pool, prepare, state),
        _ => panic!("actual BrowserLogin route missing"),
    }
}
fn request() -> Request {
    let mut headers = HeaderMap::new();
    headers.insert(
        names::AUTHORIZATION,
        HeaderValue::from_static("Bearer owned"),
    );
    headers.insert(
        names::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(names::ORIGIN, HeaderValue::from_static("https://localhost"));
    Request {
        method: Method::POST,
        uri: "/login".parse().unwrap(),
        headers,
        body: Bytes::from_static(b"{}"),
    }
}
// No Clone or arbitrary authority constructor. All fields stay request-local.
struct StageWitness {
    owner: HttpOwner,
    input: Input<'static, hyper::body::Incoming>,
}
impl StageWitness {
    async fn claimed(pool: &Pool) -> Self {
        let (owner, context) = pool.claim_stage_for_test().await.unwrap();
        assert!(owner.was_consumed());
        assert!(Arc::ptr_eq(&context.lease(), &owner.lease_owner().lease));
        Self {
            owner,
            input: Input::Claimed {
                request: request(),
                context,
            },
        }
    }
    async fn issued(self, prepare: &BrowserPrepare<State>, state: Arc<State>) -> Self {
        let (request, context) = match self.input {
            Input::Claimed { request, context } => (request, context),
            _ => panic!("stage witness was already consumed"),
        };
        // This is the actual route factory: real ClaimContext::verify,
        // AuthScope::bind and prepared_handler, not an alternate dispatcher.
        let prepared = prepare(request.security_snapshot(), Arc::clone(&state), context)
            .await
            .unwrap();
        let mut future = prepared(request, Arc::clone(&state));
        let wall = Instant::now();
        loop {
            let poll = futures_util::poll!(future.as_mut());
            if state.stats.issue_ack.load(Ordering::SeqCst) {
                assert!(poll.is_pending(), "response escaped actual issue stage");
                break;
            }
            assert!(
                poll.is_pending(),
                "real issue failed before stage acknowledgement"
            );
            assert!(
                wall.elapsed() < Duration::from_secs(2),
                "native stage setup wall guard"
            );
            tokio::task::yield_now().await;
        }
        assert!(state.stats.verifier.load(Ordering::SeqCst) == 1);
        assert!(state.stats.handler.load(Ordering::SeqCst) == 1);
        state.stats.release_issue.notify_one();
        Self {
            owner: self.owner,
            input: Input::Handler(future),
        }
    }
}
struct ResumeClock;
impl Drop for ResumeClock {
    fn drop(&mut self) {
        tokio::time::resume();
    }
}

async fn check(mode: Mode, security: i64, expected: u16, issued: bool) {
    let f = Fixture::new().await;
    let (pool, prepare, state) = actual_route(&f, mode);
    let mut witness = StageWitness::claimed(&pool).await;
    assert!(pool.row_count_for_test().await == 0);
    if issued {
        witness = witness.issued(&prepare, Arc::clone(&state)).await;
        assert!(count(&f.auth, "SELECT count(*) AS n FROM __nagi_session_rows").await == 1);
    }
    let lease = Arc::clone(&witness.owner.lease_owner().lease);
    let original_until = witness.owner.request_until().unwrap();
    let options = checked(security);
    let pending = matches!(mode, Mode::PreparePending | Mode::HandlerPending);
    let resume = if pending {
        tokio::time::pause();
        Some(ResumeClock)
    } else {
        None
    };
    let admitted = Instant::now();
    let stage_until = admitted + Duration::from_millis(security as u64);
    // The same original actual owner only narrows; it is never replaced or
    // revived. This stage budget starts after the deliberately untimed SQL setup.
    assert!(stage_until < original_until);
    let mut operation = Box::pin(
        AssertUnwindSafe(async {
            // Real owner remains inside the dispatched future, while the SAME
            // production core borrows it; cancellation drops it before SQL reads.
            let owner = witness.owner;
            dispatch_owned(
                &pool,
                &prepare,
                Arc::clone(&state),
                witness.input,
                &options,
                &owner,
                admitted,
            )
            .await
        })
        .catch_unwind(),
    );
    let mut pending_progress = true;
    let mut completed = None;
    let mut escaped_panic = false;
    if pending {
        let first = futures_util::poll!(operation.as_mut());
        let first_pending = first.is_pending();
        let reached = state.stats.verifier.load(Ordering::SeqCst) == 1
            && state.stats.handler.load(Ordering::SeqCst) == usize::from(issued)
            && state.stats.verifier_drops.load(Ordering::SeqCst) == 0
            && state.stats.handler_drops.load(Ordering::SeqCst) == 0;
        if let std::task::Poll::Ready(result) = first {
            match result {
                Ok(wire) => completed = Some(wire),
                Err(_) => escaped_panic = true,
            }
        }
        let mut before_pending = false;
        let mut drops_before = false;
        if first_pending {
            let until = tokio::time::Instant::from_std(stage_until);
            let before = until - Duration::from_millis(1);
            tokio::time::advance(before.duration_since(tokio::time::Instant::now())).await;
            let before = futures_util::poll!(operation.as_mut());
            before_pending = before.is_pending();
            drops_before = state.stats.verifier_drops.load(Ordering::SeqCst) == 0
                && state.stats.handler_drops.load(Ordering::SeqCst) == 0;
            if let std::task::Poll::Ready(result) = before {
                match result {
                    Ok(wire) => completed = Some(wire),
                    Err(_) => escaped_panic = true,
                }
            }
            if before_pending {
                tokio::time::advance(Duration::from_millis(2)).await;
                // A broken timeout must become a bounded native assertion,
                // not hang in an unbounded await or an external harness kill.
                for _ in 0..64 {
                    if let std::task::Poll::Ready(result) = futures_util::poll!(operation.as_mut())
                    {
                        match result {
                            Ok(wire) => completed = Some(wire),
                            Err(_) => escaped_panic = true,
                        }
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            }
        }
        pending_progress = first_pending && reached && before_pending && drops_before;
    } else {
        match operation.as_mut().await {
            Ok(wire) => completed = Some(wire),
            Err(_) => escaped_panic = true,
        }
    }
    // Even a timeout mutant that stayed Pending is cancelled locally before
    // clock resume, real database observation, and actual native close/join.
    drop(operation);
    drop(resume);
    let timer_completed = completed.is_some();
    let mut status = None;
    let mut cookie_count = 0;
    let mut successful_body = false;
    let mut auth_cookies = 0;
    let mut prelogin_cookies = 0;
    let mut managed = false;
    let mut challenge = false;
    if let Some(wire) = completed {
        status = Some(wire.status().as_u16());
        let cookies = wire
            .headers()
            .get_all(names::SET_COOKIE)
            .iter()
            .collect::<Vec<_>>();
        cookie_count = cookies.len();
        successful_body = wire.body().bytes.as_ref() == b"owned";
        auth_cookies = cookies
            .iter()
            .filter(|value| value.as_bytes().starts_with(b"__Host-auth="))
            .count();
        prelogin_cookies = cookies
            .iter()
            .filter(|value| {
                value.as_bytes().starts_with(b"__Host-prelogin=")
                    && value
                        .to_str()
                        .is_ok_and(|value| value.contains("Max-Age=0"))
            })
            .count();
        managed = wire
            .headers()
            .get(names::CACHE_CONTROL)
            .is_some_and(|value| value == "no-store")
            && wire
                .headers()
                .get(names::VARY)
                .is_some_and(|value| value == "Cookie, Authorization, Origin");
        challenge = wire
            .headers()
            .get(names::WWW_AUTHENTICATE)
            .is_some_and(|value| value == "Bearer");
    }
    let prelogin_rows = pool.row_count_for_test().await;
    let session_rows = count(&f.auth, "SELECT count(*) AS n FROM __nagi_session_rows").await;
    let retired = lease.validate().is_err();
    let fault_entered = state.stats.fault_entered.load(Ordering::SeqCst);
    let expected_fault = match mode {
        Mode::PrepareErr => 1,
        Mode::PreparePanic => 2,
        Mode::HandlerErr => 3,
        Mode::HandlerPanic => 4,
        _ => 0,
    };
    let verifier_calls = state.stats.verifier.load(Ordering::SeqCst);
    let handler_calls = state.stats.handler.load(Ordering::SeqCst);
    let sanitized = state.stats.sanitized.load(Ordering::SeqCst);
    let verifier_drops = state.stats.verifier_drops.load(Ordering::SeqCst);
    let handler_drops = state.stats.handler_drops.load(Ordering::SeqCst);
    drop(state);
    drop(prepare);
    drop(pool);
    f.stop().await; // Both native managers actually close/join before assertions.
    assert!(!escaped_panic, "browser_stage_oracle phase=uncaught_panic");
    assert!(
        timer_completed,
        "browser_stage_oracle phase=timer_completion"
    );
    assert!(
        pending_progress,
        "browser_stage_oracle phase=pending_before_deadline"
    );
    assert!(
        status == Some(expected),
        "browser_stage_oracle phase=response_status"
    );
    assert!(managed, "browser_stage_oracle phase=managed_headers");
    assert!(retired, "browser_stage_oracle phase=owner_retired");
    assert!(sanitized, "browser_stage_oracle phase=sanitized_request");
    assert!(verifier_calls == 1);
    let expected_handler_calls = usize::from(issued || mode == Mode::Plain);
    assert!(handler_calls == expected_handler_calls);
    assert!(prelogin_rows == 0 && session_rows == i64::from(issued));
    assert!(
        prelogin_cookies == 1,
        "browser_stage_oracle phase=consumed_deletion"
    );
    assert!(
        auth_cookies == usize::from(mode == Mode::Issue),
        "browser_stage_oracle phase=auth_cookie"
    );
    assert!(cookie_count == 1 + usize::from(mode == Mode::Issue));
    assert!(expected != 200 || successful_body);
    assert!(challenge == (expected == 401));
    assert!(verifier_drops == usize::from(mode == Mode::PreparePending));
    assert!(handler_drops == usize::from(mode == Mode::HandlerPending));
    // A generic replacement 500 must not substitute for actually polling the
    // post-issue handler Err/panic arm (whose call count was already one).
    assert_eq!(
        fault_entered, expected_fault,
        "browser_stage_oracle phase=business_fault_entered"
    );
}

#[tokio::test]
async fn live_claimed_200_runs_verifier_and_handler_and_returns_200() {
    check(Mode::Plain, 200, 200, false).await;
}
#[tokio::test]
async fn consumed_prepare_err_1000_returns_401_without_cookie() {
    check(Mode::PrepareErr, 1000, 401, false).await;
}
#[tokio::test]
async fn consumed_prepare_panic_1000_returns_500_without_cookie() {
    check(Mode::PreparePanic, 1000, 500, false).await;
}
#[tokio::test]
async fn consumed_prepare_pending_200_returns_504_and_drops_once() {
    check(Mode::PreparePending, 200, 504, false).await;
}
#[tokio::test]
async fn consumed_prepare_non_yielding_200_late_ready_returns_504() {
    check(Mode::PrepareLate, 200, 504, false).await;
}
#[tokio::test]
async fn committed_issue_live_200_delivers_auth_cookie_once() {
    check(Mode::Issue, 200, 200, true).await;
}
#[tokio::test]
async fn committed_issue_handler_err_1000_returns_500_without_cookie() {
    check(Mode::HandlerErr, 1000, 500, true).await;
}
#[tokio::test]
async fn committed_issue_handler_panic_1000_returns_500_without_cookie() {
    check(Mode::HandlerPanic, 1000, 500, true).await;
}
#[tokio::test]
async fn committed_issue_handler_pending_200_returns_504_and_drops_once() {
    check(Mode::HandlerPending, 200, 504, true).await;
}
