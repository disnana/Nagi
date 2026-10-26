//! Actual Browser Handler polling: setup never pre-polls Session Issue.
use super::*;
use crate::{
    auth::{self, session, session_deadline_observer as session_diag},
    sqlite,
};
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Mutex,
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "nagi-handler-diag-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => panic!("owned diagnostic directory unavailable"),
            }
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[derive(Clone, Copy)]
enum Mode {
    Timer,
    ReleasedStartup,
    LateReady,
}
struct State {
    store: session::Store,
    auth: sqlite::Pool,
    mode: Mode,
    until: Mutex<Option<Instant>>,
    observer: Mutex<Option<session_diag::Observer>>,
    verifier: AtomicUsize,
    handler: AtomicUsize,
    issue_ok: AtomicUsize,
}
async fn verifier(_: Request, state: Arc<State>) -> Result<VerifiedIdentity, auth::Failure> {
    state.verifier.fetch_add(1, Ordering::SeqCst);
    VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(60))
}
async fn handler(request: Request, state: Arc<State>, scope: AuthScope) -> Result<Response, Error> {
    state.handler.fetch_add(1, Ordering::SeqCst);
    *state.observer.lock().unwrap() = session_diag::current();
    let _held = if matches!(state.mode, Mode::Timer | Mode::ReleasedStartup) {
        Some(sqlite::hold_http_checkout_for_test(&state.auth).await)
    } else {
        None
    };
    let intent = session::issue(&state.store, scope)
        .await
        .map_err(|_| Error::internal("owned issue refused"))?;
    state.issue_ok.fetch_add(1, Ordering::SeqCst);
    let response = session::apply(
        text(response_context(&request), Status::OK, "owned")?,
        intent,
    )
    .map_err(|_| Error::internal("owned apply refused"))?;
    if matches!(state.mode, Mode::LateReady) {
        let until = state.until.lock().unwrap().unwrap();
        // Finite non-yielding Ready after the SAME selected bound, no renewal.
        std::thread::sleep(
            until.saturating_duration_since(Instant::now()) + Duration::from_millis(2),
        );
    }
    Ok(response)
}
fn checked() -> Options {
    authority(
        security_timeout(options(256, 1000, 1000, 1000).unwrap(), 1000).unwrap(),
        "https://localhost",
        vec!["localhost".to_owned()],
        1,
        128,
    )
    .unwrap()
}
async fn check(mode: Mode) {
    let directory = Directory::new();
    let (auth, startup) = sqlite::http_startup_test_pool(directory.0.join("owned.sqlite"));
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
        prelogin::options(2, 3, 144, 1, 10000, 2, 16, 10000, 4, 16).unwrap(),
        prelogin::cookie_options("__Host-prelogin", session::SameSite::Strict).unwrap(),
        &checked(),
    )
    .await
    .unwrap();
    let app = route(
        app_default(State {
            store,
            auth: sqlite::clone_pool(&auth),
            mode,
            until: Mutex::new(None),
            observer: Mutex::new(None),
            verifier: AtomicUsize::new(0),
            handler: AtomicUsize::new(0),
            issue_ok: AtomicUsize::new(0),
        }),
        Method::POST,
        "/login",
        browser_login_policy(prelogin::clone_pool(&prelogin), verifier),
        handler,
    )
    .unwrap();
    let state = Arc::clone(&app.state);
    let mut routes = app.routes;
    let route = routes.get_mut("/login").unwrap().pop().unwrap();
    let RouteKind::Browser { pool, prepare } = route.kind else {
        panic!("Browser route required");
    };
    let (owner, context) = pool.claim_stage_for_test().await.unwrap();
    assert!(owner.was_consumed());
    assert_eq!(startup.created(), 1, "setup must not consume worker2 gate");
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
    let request = Request {
        method: Method::POST,
        uri: "/login".parse().unwrap(),
        headers: response_public::tests::headers_fixture(headers, 256),
        body: Bytes::from_static(b"{}"),
    };
    let response_context = response_context(&request).share();
    let options = checked();
    let admitted = Instant::now();
    let until = admitted + Duration::from_millis(1000);
    *state.until.lock().unwrap() = Some(until);
    let mut operation = Box::pin(dispatch_owned::<_, hyper::body::Incoming>(
        DispatchBorrowed {
            response_context: &response_context,
            pool: &pool,
            prepare: &prepare,
            options: &options,
            owner: &owner,
        },
        Arc::clone(&state),
        Input::Claimed { request, context },
        admitted,
    ));
    if matches!(mode, Mode::Timer | Mode::ReleasedStartup) {
        tokio::select! {
            biased;
            _ = startup.wait() => {},
            _ = &mut operation => panic!("handler completed before actual worker2 gate"),
        }
        assert!(
            Instant::now() < until,
            "native gate must be reached within original budget"
        );
        if matches!(mode, Mode::ReleasedStartup) {
            startup.release();
        }
    }
    let wire = tokio::time::timeout(Duration::from_secs(3), &mut operation)
        .await
        .unwrap();
    drop(operation);
    let wire = vary::finish(wire);
    let expected = if matches!(mode, Mode::ReleasedStartup) {
        200
    } else {
        504
    };
    assert_eq!(
        wire.status().as_u16(),
        expected,
        "HTTP/security assertions precede diagnostic assertions"
    );
    let cookies = wire
        .headers()
        .get_all(names::SET_COOKIE)
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(cookies.len(), if expected == 200 { 2 } else { 1 });
    assert_eq!(
        cookies
            .iter()
            .filter(|c| c.as_bytes().starts_with(b"__Host-prelogin="))
            .count(),
        1
    );
    assert_eq!(
        cookies
            .iter()
            .filter(|c| c.as_bytes().starts_with(b"__Host-auth="))
            .count(),
        usize::from(expected == 200)
    );
    assert!(owner.was_consumed());
    assert_eq!(state.verifier.load(Ordering::SeqCst), 1);
    assert_eq!(state.handler.load(Ordering::SeqCst), 1);
    assert_eq!(
        state.issue_ok.load(Ordering::SeqCst),
        usize::from(!matches!(mode, Mode::Timer))
    );
    startup.release();
    prelogin::close(&prelogin, 2000).await.unwrap();
    sqlite::close(&auth, 2000).await.unwrap();
    assert!(startup.all_joined());
    let observer = state
        .observer
        .lock()
        .unwrap()
        .take()
        .expect("ScopeMissing: actual Browser Handler has no Session observer");
    let snapshot = observer.take_browser_terminal_for_test();
    if expected == 200 {
        assert!(snapshot.is_none());
        return;
    }
    let snapshot = snapshot
        .expect("actual terminal envelope missing")
        .to_string();
    assert!(snapshot.len() < 1024);
    let lines = snapshot.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    fn fields(line: &str) -> std::collections::HashMap<&str, u64> {
        line.split_whitespace()
            .skip(1)
            .map(|field| {
                let (key, value) = field.split_once('=').unwrap();
                assert!(value.bytes().all(|c| c.is_ascii_digit()));
                (key, value.parse().unwrap())
            })
            .collect()
    }
    assert!(lines[0].starts_with("browser_handler_deadline_diag "));
    assert!(lines[1].starts_with("browser_handler_native_call_diag "));
    let header = fields(lines[0]);
    let native = fields(lines[1]);
    let request_id = diag::Observer::new(&owner.lease_owner().lease)
        .snapshot()
        .request;
    assert_eq!(header["request"], request_id);
    assert_eq!(native["request"], request_id);
    assert_eq!(header["phase"], 15);
    assert_eq!(native["origin"], 4);
    assert_eq!(header["budget_ms"], 1000);
    assert_eq!(
        header["terminal"],
        if matches!(mode, Mode::Timer) { 8 } else { 9 }
    );
    assert_eq!(header["terminal"], native["terminal"]);
    if matches!(mode, Mode::Timer) {
        assert_eq!(header["db"], 1); // checkout waits for the actual ready gate.
        assert_eq!(header["begins"], 0);
        assert_eq!(native["diagnostic_valid"], 0);
        assert_eq!(native["call_enter_us"], 60_000_001);
        assert_eq!(native["call_return_us"], 60_000_001);
    } else {
        assert_eq!(native["diagnostic_valid"], 1);
        assert_eq!(native["native_call_state"], 4);
        assert_eq!(native["commit_result"], 1);
        assert_eq!(native["begins"], 1);
        assert_eq!(native["commits"], 1);
    }
    assert!(
        observer.take_browser_terminal_for_test().is_none(),
        "single terminal capture only"
    );
}
#[tokio::test]
async fn original_handler_timer_preserves_pending_issue_witness() {
    check(Mode::Timer).await;
}
#[tokio::test]
async fn original_handler_late_ready_preserves_committed_issue_witness() {
    check(Mode::LateReady).await;
}
#[tokio::test]
async fn released_native_startup_preserves_success_without_terminal_record() {
    check(Mode::ReleasedStartup).await;
}
