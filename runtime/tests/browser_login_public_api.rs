//! External normal-library BrowserLogin tests: owned SQLite and loopback only.
//! Raw cookie/token values stay in RAM; assertions never format material.
use nagi_runtime::{
    auth::{self, prelogin, session, AuthScope, VerifiedIdentity},
    http_server as http, sqlite, Error, FromRow,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        loop {
            let p = std::env::temp_dir().join(format!(
                "sf03-login-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&p) {
                Ok(()) => return Self(p),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => panic!("owned directory unavailable"),
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
    fn read(
        r: &nagi_runtime::rusqlite::Row<'_>,
        i: &[usize],
    ) -> nagi_runtime::rusqlite::Result<Self> {
        Ok(Self(r.get(i[0])?))
    }
}
async fn number(pool: &sqlite::Pool, q: &'static str) -> i64 {
    let tx = sqlite::begin(pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let n = sqlite::query::<Number>(&tx, sqlite::literal(q), sqlite::parameters())
        .await
        .unwrap()
        .unwrap()
        .0;
    sqlite::rollback(tx).await.unwrap();
    n
}
#[derive(Default)]
struct Counters {
    verifier: AtomicUsize,
    handler: AtomicUsize,
    mode: AtomicUsize,
    entered: tokio::sync::Notify,
    cancelled: AtomicUsize,
    sanitized: AtomicBool,
    claim_committed: AtomicBool,
    issue_committed: AtomicBool,
}
struct CancelSeen(Arc<Counters>);
impl Drop for CancelSeen {
    fn drop(&mut self) {
        self.0.cancelled.fetch_add(1, Ordering::SeqCst);
    }
}
struct State {
    store: session::Store,
    auth: sqlite::Pool,
    stats: Arc<Counters>,
    original: Instant,
}
async fn named_trusted_bearer(
    request: http::Request,
    state: Arc<State>,
) -> Result<VerifiedIdentity, auth::Failure> {
    state.stats.verifier.fetch_add(1, Ordering::SeqCst);
    state.stats.sanitized.store(
        http::header(&request, "cookie").unwrap().is_none()
            && http::header(&request, "x-csrf-token").unwrap().is_none()
            && request.body().is_empty(),
        Ordering::SeqCst,
    );
    // Acquire auth's independent manager after claim: no PreLogin Tx writer lock
    // crosses this trusted await, and the consumed row is already absent.
    state.stats.claim_committed.store(
        number(
            &state.auth,
            "SELECT count(*) AS n FROM __nagi_prelogin_ready",
        )
        .await
            == 0,
        Ordering::SeqCst,
    );
    match state.stats.mode.load(Ordering::SeqCst) {
        1 => return Err(auth::invalid_credential()),
        2 => panic!("owned verifier panic"),
        3 => tokio::time::sleep(Duration::from_millis(400)).await,
        4 => std::thread::sleep(Duration::from_millis(400)),
        5 => return Err(auth::expired()),
        6 => {
            let _seen = CancelSeen(Arc::clone(&state.stats));
            state.stats.entered.notify_one();
            std::future::pending::<()>().await;
        }
        _ => {}
    }
    VerifiedIdentity::from_verified(7, state.original)
}
async fn login_handler(
    request: http::Request,
    state: Arc<State>,
    scope: AuthScope,
) -> Result<http::Response, Error> {
    state.stats.handler.fetch_add(1, Ordering::SeqCst);
    assert!(
        http::header(&request, "cookie").unwrap().is_none()
            && http::header(&request, "x-csrf-token").unwrap().is_none()
    );
    let response = http::bytes(http::Status::OK, request.body());
    let intent = session::issue(&state.store, scope).await.map_err(|error| {
        if session::kind(&error) == auth::FailureKind::Unavailable {
            Error {
                kind: nagi_runtime::ErrorKind::Busy,
                message: "session unavailable".to_owned(),
            }
        } else {
            Error::internal("session issue refused")
        }
    })?;
    state.stats.issue_committed.store(true, Ordering::SeqCst);
    let response = session::apply(response, intent).unwrap();
    match state.stats.mode.load(Ordering::SeqCst) {
        10 => Err(Error::internal("owned handler Err")),
        11 => panic!("owned handler panic"),
        12 => {
            tokio::time::sleep(Duration::from_millis(400)).await;
            Ok(response)
        }
        _ => Ok(response),
    }
}
fn checked(body: i64, security: i64) -> http::Options {
    checked_phases(body, security, 1000, 1000)
}
fn checked_phases(body: i64, security: i64, body_ms: i64, shutdown_ms: i64) -> http::Options {
    http::authority(
        http::security_timeout(
            http::options(body, body_ms, 1000, shutdown_ms).unwrap(),
            security,
        )
        .unwrap(),
        "https://localhost",
        vec!["localhost".to_owned()],
        1,
        128,
    )
    .unwrap()
}
struct Fixture {
    _dir: Directory,
    auth: sqlite::Pool,
    store: session::Store,
    pool: prelogin::PreLoginPool,
    stats: Arc<Counters>,
}
impl Fixture {
    async fn new(ttl: i64, quota: i64) -> Self {
        let dir = Directory::new();
        let auth = sqlite::open(
            dir.0.join("owned.sqlite").to_str().unwrap(),
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
        let pool = prelogin::open(
            &store,
            sqlite::options(1, 2, 1000, 1000).unwrap(),
            prelogin::options(2, 3, quota, 1, ttl, 2, 16, 10000, 4, 16).unwrap(),
            prelogin::cookie_options("__Host-prelogin", session::SameSite::Strict).unwrap(),
            &checked(256, 1000),
        )
        .await
        .unwrap();
        Self {
            _dir: dir,
            auth,
            store,
            pool,
            stats: Arc::new(Counters::default()),
        }
    }
    fn app(&self) -> http::App<State, Error> {
        let state = State {
            store: session::clone_store(&self.store),
            auth: sqlite::clone_pool(&self.auth),
            stats: Arc::clone(&self.stats),
            original: Instant::now() + Duration::from_secs(60),
        };
        let app = http::prelogin_bootstrap(
            http::app_default(state),
            "/bootstrap",
            prelogin::clone_pool(&self.pool),
        )
        .unwrap();
        http::route(
            app,
            http::Method::POST,
            "/login",
            http::browser_login_policy(prelogin::clone_pool(&self.pool), named_trusted_bearer),
            login_handler,
        )
        .unwrap()
    }
    async fn rows(&self) -> i64 {
        number(
            &self.auth,
            "SELECT count(*) AS n FROM __nagi_prelogin_ready",
        )
        .await
    }
    async fn stop(self) {
        prelogin::close(&self.pool, 2000).await.unwrap();
        sqlite::close(&self.auth, 2000).await.unwrap();
    }
}
struct Server {
    address: std::net::SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<Result<(), Error>>,
}
impl Server {
    async fn new(f: &Fixture, body: i64, security: i64) -> Self {
        Self::with_options(f, checked(body, security)).await
    }
    async fn with_options(f: &Fixture, options: http::Options) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, until) = oneshot::channel();
        let task = tokio::spawn(http::serve_listener(listener, f.app(), options, async {
            let _ = until.await;
        }));
        Self {
            address,
            stop: Some(stop),
            task,
        }
    }
    async fn stop(mut self) {
        self.stop.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), &mut self.task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
        self.task.abort();
    }
}
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}
impl Reply {
    fn values(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
            .collect()
    }
    fn guarded(&self) {
        assert!(self.values("cache-control") == vec!["no-store"]);
        assert!(self.values("vary") == vec!["Cookie, Authorization, Origin"]);
    }
    fn material(&self) -> (String, String) {
        assert!(self.status == 200);
        let c = self.values("set-cookie");
        assert!(c.len() == 1);
        let cookie = c[0].split(';').next().unwrap().to_owned();
        let json: nagi_runtime::serde_json::Value =
            nagi_runtime::serde_json::from_slice(&self.body).unwrap();
        let token = json
            .get("csrf_token")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_owned();
        (cookie, token)
    }
    fn deletion(&self) -> bool {
        self.values("set-cookie").iter().any(|v| {
            v.starts_with("__Host-prelogin=;")
                && v.contains("Max-Age=0")
                && v.contains("Secure")
                && v.contains("HttpOnly")
                && v.contains("Path=/")
        })
    }
    fn auth_cookie(&self) -> bool {
        self.values("set-cookie")
            .iter()
            .any(|v| v.starts_with("__Host-auth="))
    }
}
async fn request(
    address: std::net::SocketAddr,
    method: &str,
    path: &str,
    headers: &str,
    body: &[u8],
) -> Reply {
    tokio::time::timeout(Duration::from_secs(3),async { let mut s=TcpStream::connect(address).await.unwrap(); let h=format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n{headers}\r\n",body.len()); s.write_all(h.as_bytes()).await.unwrap(); s.write_all(body).await.unwrap(); let mut raw=Vec::new(); s.take(8193).read_to_end(&mut raw).await.unwrap(); assert!(raw.len()<=8192); parse_reply(raw) }).await.unwrap()
}
fn parse_reply(raw: Vec<u8>) -> Reply {
    let split = raw.windows(4).position(|v| v == b"\r\n\r\n").unwrap();
    let head = std::str::from_utf8(&raw[..split]).unwrap();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .map(|v| {
            let (n, v) = v.split_once(':').unwrap();
            (n.to_owned(), v.trim().to_owned())
        })
        .collect();
    Reply {
        status,
        headers,
        body: raw[split + 4..].to_vec(),
    }
}
async fn bootstrap(s: &Server) -> (String, String) {
    request(s.address, "GET", "/bootstrap", "", b"")
        .await
        .material()
}
fn login_headers(cookie: &str, token: &str) -> String {
    format!("Origin: https://localhost\r\nAuthorization: Bearer owned\r\nCookie: {cookie}\r\nX-CSRF-Token: {token}\r\nContent-Type: application/json\r\n")
}

#[tokio::test]
async fn public_login_claims_once_then_named_verifier_issue_and_appended_cookies() {
    let f = Fixture::new(1000, 144).await;
    let s = Server::new(&f, 256, 1000).await;
    let (c, t) = bootstrap(&s).await;
    let h = login_headers(&c, &t);
    let (a, b) = tokio::join!(
        request(s.address, "POST", "/login", &h, b"{}"),
        request(s.address, "POST", "/login", &h, b"{}")
    );
    assert!((a.status == 200 && b.status == 403) || (a.status == 403 && b.status == 200));
    let win = if a.status == 200 { &a } else { &b };
    win.guarded();
    assert!(
        win.auth_cookie()
            && win.deletion()
            && win.values("set-cookie").len() == 2
            && win.body == b"{}"
    );
    assert!(
        f.rows().await == 0
            && f.stats.verifier.load(Ordering::SeqCst) == 1
            && f.stats.handler.load(Ordering::SeqCst) == 1
            && f.stats.sanitized.load(Ordering::SeqCst)
            && f.stats.claim_committed.load(Ordering::SeqCst)
    );
    let remaining=number(&f.auth,"SELECT (SELECT absolute_ms FROM __nagi_session_rows LIMIT 1)-last_wall AS n FROM __nagi_session_meta").await;
    assert!(remaining > 30000 && remaining <= 60000);
    s.stop().await;
    f.stop().await;
}
#[tokio::test]
async fn input_and_strict_json_rejections_do_not_consume_or_call_verifier() {
    let f = Fixture::new(10000, 144).await;
    let s = Server::new(&f, 256, 1000).await;
    let (c, t) = bootstrap(&s).await;
    let h = login_headers(&c, &t);
    let cases = vec![
        (
            h.replace("Origin: https://localhost\r\n", ""),
            b"{}".to_vec(),
            403,
        ),
        (h.replace("https://localhost", "null"), b"{}".to_vec(), 403),
        (
            h.replace("https://localhost", "https://other.invalid"),
            b"{}".to_vec(),
            403,
        ),
        (
            format!("Origin: https://localhost\r\n{h}"),
            b"{}".to_vec(),
            400,
        ),
        (
            h.replace("Authorization: Bearer owned\r\n", ""),
            b"{}".to_vec(),
            401,
        ),
        (
            h.replace("Bearer owned", "Basic owned"),
            b"{}".to_vec(),
            401,
        ),
        (
            format!("Authorization: Bearer owned\r\n{h}"),
            b"{}".to_vec(),
            400,
        ),
        (
            h.replace(&format!("Cookie: {c}\r\n"), ""),
            b"{}".to_vec(),
            403,
        ),
        (format!("Cookie: {c}\r\n{h}"), b"{}".to_vec(), 400),
        (
            format!("Cookie: __Host-auth=owned\r\n{h}"),
            b"{}".to_vec(),
            400,
        ),
        (
            h.replace(&format!("X-CSRF-Token: {t}\r\n"), ""),
            b"{}".to_vec(),
            403,
        ),
        (h.replace(&t, &"A".repeat(43)), b"{}".to_vec(), 403),
        (format!("X-CSRF-Token: {t}\r\n{h}"), b"{}".to_vec(), 400),
        (
            h.replace("application/json", "text/plain"),
            b"{}".to_vec(),
            400,
        ),
        (h.clone(), vec![255], 400),
        (h.clone(), br#"{"a":1,"a":2}"#.to_vec(), 400),
        (h.clone(), br#"{"a":{"b":1,"\u0062":2}}"#.to_vec(), 400),
        (h.clone(), b"[[[[[0]]]]]".to_vec(), 400),
        (
            h.clone(),
            b"[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]".to_vec(),
            400,
        ),
        (h.clone(), b"{} {}".to_vec(), 400),
    ];
    for (headers, body, status) in cases {
        let r = request(s.address, "POST", "/login", &headers, &body).await;
        assert!(r.status == status && !r.auth_cookie() && !r.deletion());
        r.guarded();
        assert!(f.rows().await == 1 && f.stats.verifier.load(Ordering::SeqCst) == 0);
    }
    let good = request(
        s.address,
        "POST",
        "/login",
        &h,
        br#"{"n":1.25,"a":[true,null]}"#,
    )
    .await;
    assert_eq!(good.status, 200, "input_rejection_preserves_login_claim");
    s.stop().await;
    f.stop().await;
}
// A socket timeout may precede the callback/issue stage. Prove the exact
// externally observed state instead of assuming that native SQL fit the budget.
// The mandatory broad native stage inventory separately exercises the completed
// claim/verifier/issue faults; an early socket result cannot satisfy that guard.
async fn assert_browser_deadline_state(f: &Fixture, r: &Reply) -> bool {
    assert!(r.status == 504 && !r.auth_cookie());
    r.guarded();
    let prelogin_rows = f.rows().await;
    let session_rows = number(&f.auth, "SELECT count(*) AS n FROM __nagi_session_rows").await;
    let verifier_calls = f.stats.verifier.load(Ordering::SeqCst);
    let handler_calls = f.stats.handler.load(Ordering::SeqCst);
    assert!((0..=1).contains(&prelogin_rows) && (0..=1).contains(&session_rows));
    assert!(verifier_calls <= 1 && handler_calls <= verifier_calls);
    if prelogin_rows == 1 {
        // A cancelled claim may retain its original row. No trusted callback
        // or Session issue can have run without the one-use claim completing.
        assert!(verifier_calls == 0 && handler_calls == 0 && session_rows == 0);
        assert!(!r.deletion());
    }
    if verifier_calls == 1 {
        assert!(prelogin_rows == 0 && r.deletion());
    }
    if r.deletion() || f.stats.claim_committed.load(Ordering::SeqCst) {
        assert!(prelogin_rows == 0);
    }
    if handler_calls == 0 {
        assert!(session_rows == 0);
    }
    if session_rows == 1 {
        assert!(handler_calls == 1 && verifier_calls == 1 && prelogin_rows == 0);
        assert!(r.deletion());
    }
    if f.stats.issue_committed.load(Ordering::SeqCst) {
        assert!(session_rows == 1 && handler_calls == 1);
    }
    prelogin_rows == 0
}

async fn assert_consumed_browser_replay(f: &Fixture, s: &Server, headers: &str) {
    let verifier_calls = f.stats.verifier.load(Ordering::SeqCst);
    let handler_calls = f.stats.handler.load(Ordering::SeqCst);
    let session_rows = number(&f.auth, "SELECT count(*) AS n FROM __nagi_session_rows").await;
    let retry = request(s.address, "POST", "/login", headers, b"{}").await;
    // A missing-row restoration may itself hit the unchanged security budget.
    // The live403/expired504 native oracles are both mandatory in broad CI.
    assert!(retry.status == 403 || retry.status == 504);
    assert!(!retry.auth_cookie() && !retry.deletion());
    retry.guarded();
    assert!(f.rows().await == 0);
    assert!(f.stats.verifier.load(Ordering::SeqCst) == verifier_calls);
    assert!(f.stats.handler.load(Ordering::SeqCst) == handler_calls);
    assert!(number(&f.auth, "SELECT count(*) AS n FROM __nagi_session_rows").await == session_rows);
}

#[tokio::test]
async fn committed_claim_stays_consumed_on_verifier_error_panic_and_deadline() {
    for (mode, status, security) in [
        (1, 401, 1000),
        (2, 500, 1000),
        (3, 504, 200),
        (4, 504, 200),
        (5, 401, 1000),
    ] {
        let f = Fixture::new(1000, 144).await;
        f.stats.mode.store(mode, Ordering::SeqCst);
        // Keep bootstrap under the normal profile; only the target login uses
        // the deliberately short security budget. Join setup before starting it.
        let setup = Server::new(&f, 256, 1000).await;
        let (c, t) = bootstrap(&setup).await;
        let s = if security == 1000 {
            setup
        } else {
            setup.stop().await;
            Server::new(&f, 256, security).await
        };
        let h = login_headers(&c, &t);
        let r = request(s.address, "POST", "/login", &h, b"{}").await;
        if r.status == 504 {
            let row_absent = assert_browser_deadline_state(&f, &r).await;
            assert!(f.stats.handler.load(Ordering::SeqCst) == 0);
            if row_absent {
                assert_consumed_browser_replay(&f, &s, &h).await;
            }
            s.stop().await;
            f.stop().await;
            continue;
        }
        assert_eq!(r.status, status, "committed_claim_status mode={mode}");
        assert!(!r.auth_cookie() && r.deletion());
        r.guarded();
        if status == 401 {
            assert!(r.values("www-authenticate") == vec!["Bearer"]);
        }
        assert!(
            f.rows().await == 0
                && f.stats.verifier.load(Ordering::SeqCst) == 1
                && f.stats.handler.load(Ordering::SeqCst) == 0
        );
        let retry = request(s.address, "POST", "/login", &h, b"{}").await;
        if retry.status == 504 {
            assert!(!retry.auth_cookie() && !retry.deletion());
            retry.guarded();
            assert!(f.rows().await == 0);
            assert!(f.stats.verifier.load(Ordering::SeqCst) == 1);
            assert!(f.stats.handler.load(Ordering::SeqCst) == 0);
            assert!(number(&f.auth, "SELECT count(*) AS n FROM __nagi_session_rows").await == 0);
            s.stop().await;
            f.stop().await;
            continue;
        }
        assert!(retry.status == 403 && !retry.auth_cookie() && !retry.deletion());
        s.stop().await;
        f.stop().await;
    }
}
#[tokio::test]
async fn issue_commit_then_handler_err_panic_or_timeout_never_delivers_auth_cookie() {
    for (mode, status, security) in [(10, 500, 1000), (11, 500, 1000), (12, 504, 200)] {
        let f = Fixture::new(1000, 144).await;
        f.stats.mode.store(mode, Ordering::SeqCst);
        // Keep bootstrap under the normal profile; only the target login uses
        // the deliberately short security budget. Join setup before starting it.
        let setup = Server::new(&f, 256, 1000).await;
        let (c, t) = bootstrap(&setup).await;
        let s = if security == 1000 {
            setup
        } else {
            setup.stop().await;
            Server::new(&f, 256, security).await
        };
        let h = login_headers(&c, &t);
        let r = request(s.address, "POST", "/login", &h, b"{}").await;
        if r.status == 504 {
            let row_absent = assert_browser_deadline_state(&f, &r).await;
            if row_absent {
                assert_consumed_browser_replay(&f, &s, &h).await;
            }
            s.stop().await;
            f.stop().await;
            continue;
        }
        assert_eq!(r.status, status, "issue_commit_status mode={mode}");
        assert!(!r.auth_cookie() && r.deletion());
        r.guarded();
        assert!(
            f.rows().await == 0
                && number(&f.auth, "SELECT count(*) AS n FROM __nagi_session_rows").await == 1
        );
        s.stop().await;
        f.stop().await;
    }
}
#[tokio::test]
async fn wire_body_cap_is_independent_of_db_quota_and_checked_before_claim() {
    for quota in [144, 720] {
        let f = Fixture::new(1000, quota).await;
        let s = Server::new(&f, 192, 1000).await;
        let (c, t) = bootstrap(&s).await;
        let h = login_headers(&c, &t);
        let cap = [
            b"\"".as_slice(),
            vec![b'x'; 190].as_slice(),
            b"\"".as_slice(),
        ]
        .concat();
        let over = [
            b"\"".as_slice(),
            vec![b'x'; 191].as_slice(),
            b"\"".as_slice(),
        ]
        .concat();
        let r = request(s.address, "POST", "/login", &h, &over).await;
        assert!(
            r.status == 413 && f.rows().await == 1 && f.stats.verifier.load(Ordering::SeqCst) == 0
        );
        r.guarded();
        let r = request(s.address, "POST", "/login", &h, &cap).await;
        assert!(r.status == 200 && r.body == cap && r.auth_cookie() && r.deletion());
        s.stop().await;
        f.stop().await;
    }
}
#[tokio::test]
async fn browser_profile_refuses_non_post_and_wrong_startup_origin() {
    let f = Fixture::new(1000, 144).await;
    for method in [
        http::Method::GET,
        http::Method::HEAD,
        http::Method::PUT,
        http::Method::DELETE,
    ] {
        let app = http::app_default(State {
            store: session::clone_store(&f.store),
            auth: sqlite::clone_pool(&f.auth),
            stats: Arc::clone(&f.stats),
            original: Instant::now() + Duration::from_secs(60),
        });
        assert!(http::route(
            app,
            method,
            "/login",
            http::browser_login_policy(prelogin::clone_pool(&f.pool), named_trusted_bearer),
            login_handler
        )
        .is_err());
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let app = http::route(
        http::app_default(State {
            store: session::clone_store(&f.store),
            auth: sqlite::clone_pool(&f.auth),
            stats: Arc::clone(&f.stats),
            original: Instant::now() + Duration::from_secs(60),
        }),
        http::Method::POST,
        "/login",
        http::browser_login_policy(prelogin::clone_pool(&f.pool), named_trusted_bearer),
        login_handler,
    )
    .unwrap();
    let other = http::authority(
        http::options(256, 1000, 1000, 1000).unwrap(),
        "https://other.invalid",
        vec!["localhost".to_owned()],
        1,
        128,
    )
    .unwrap();
    assert!(http::serve_listener(listener, app, other, async {})
        .await
        .is_err());
    assert!(f.rows().await == 0 && f.stats.verifier.load(Ordering::SeqCst) == 0);
    f.stop().await;
}

#[tokio::test]
async fn body_own_deadline_408_security_earlier_or_equal_504_before_claim() {
    for (body_ms, security, status) in [(80, 240, 408), (240, 80, 504), (100, 100, 504)] {
        let f = Fixture::new(10000, 144).await;
        // Bootstrap setup has its normal finite budget; only the incomplete
        // login upload under test uses these deliberately short deadlines.
        let setup = Server::new(&f, 256, 1000).await;
        let (c, t) = bootstrap(&setup).await;
        setup.stop().await;
        let s = Server::with_options(&f, checked_phases(256, security, body_ms, 1000)).await;
        let mut stream = TcpStream::connect(s.address).await.unwrap();
        let headers=format!("POST /login HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 2\r\n{}\r\n",login_headers(&c,&t));
        stream.write_all(headers.as_bytes()).await.unwrap();
        stream.write_all(b"{").await.unwrap();
        let mut raw = Vec::new();
        tokio::time::timeout(
            Duration::from_secs(3),
            stream.take(8193).read_to_end(&mut raw),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(raw.len() <= 8192);
        let r = parse_reply(raw);
        assert!(r.status == status && !r.deletion() && !r.auth_cookie());
        r.guarded();
        assert!(f.rows().await == 1 && f.stats.verifier.load(Ordering::SeqCst) == 0);
        s.stop().await;
        f.stop().await;
    }
}
#[tokio::test]
async fn shutdown_cancels_actual_request_and_committed_token_cannot_return() {
    let f = Fixture::new(10000, 144).await;
    f.stats.mode.store(6, Ordering::SeqCst);
    let s = Server::with_options(&f, checked_phases(256, 1000, 1000, 20)).await;
    let (c, t) = bootstrap(&s).await;
    let mut stream = TcpStream::connect(s.address).await.unwrap();
    let headers=format!("POST /login HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 2\r\n{}\r\n{{}}",login_headers(&c,&t));
    stream.write_all(headers.as_bytes()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), f.stats.entered.notified())
        .await
        .unwrap();
    assert!(f.rows().await == 0);
    s.stop().await;
    drop(stream);
    assert!(
        f.stats.cancelled.load(Ordering::SeqCst) == 1
            && f.stats.handler.load(Ordering::SeqCst) == 0
    );
    f.stats.mode.store(0, Ordering::SeqCst);
    let next = Server::new(&f, 256, 1000).await;
    let retry = request(
        next.address,
        "POST",
        "/login",
        &login_headers(&c, &t),
        b"{}",
    )
    .await;
    assert!(
        retry.status == 403 && !retry.auth_cookie() && f.stats.verifier.load(Ordering::SeqCst) == 1
    );
    next.stop().await;
    f.stop().await;
}
#[tokio::test]
async fn closed_native_pool_refuses_before_verifier_and_session_capacity_never_redelivers() {
    let f = Fixture::new(10000, 144).await;
    let s = Server::new(&f, 256, 1000).await;
    for _ in 0..4 {
        let (c, t) = bootstrap(&s).await;
        let r = request(s.address, "POST", "/login", &login_headers(&c, &t), b"{}").await;
        assert!(r.status == 200);
    }
    let (c, t) = bootstrap(&s).await;
    let r = request(s.address, "POST", "/login", &login_headers(&c, &t), b"{}").await;
    assert!(r.status == 503 && r.deletion() && !r.auth_cookie());
    assert!(
        f.rows().await == 0
            && number(&f.auth, "SELECT count(*) AS n FROM __nagi_session_rows").await == 4
    );
    let (c, t) = bootstrap(&s).await;
    prelogin::close(&f.pool, 2000).await.unwrap();
    let before = f.stats.verifier.load(Ordering::SeqCst);
    let r = request(s.address, "POST", "/login", &login_headers(&c, &t), b"{}").await;
    assert!(
        r.status == 503
            && !r.deletion()
            && !r.auth_cookie()
            && f.stats.verifier.load(Ordering::SeqCst) == before
    );
    assert!(f.rows().await == 1);
    s.stop().await;
    f.stop().await;
}

#[tokio::test]
async fn json_value_nodes_and_container_depth_accept_exact_bounds() {
    let f = Fixture::new(10000, 144).await;
    let s = Server::new(&f, 256, 1000).await;
    for (over, exact) in [
        (b"[[[[[0]]]]]".as_slice(), b"[[[[0]]]]".as_slice()),
        (
            b"[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]".as_slice(),
            b"[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]".as_slice(),
        ),
    ] {
        let (c, t) = bootstrap(&s).await;
        let h = login_headers(&c, &t);
        let before = f.stats.verifier.load(Ordering::SeqCst);
        let r = request(s.address, "POST", "/login", &h, over).await;
        assert!(
            r.status == 400
                && f.rows().await == 1
                && f.stats.verifier.load(Ordering::SeqCst) == before
        );
        let r = request(s.address, "POST", "/login", &h, exact).await;
        assert!(r.status == 200 && r.body == exact && f.rows().await == 0);
    }
    s.stop().await;
    f.stop().await;
}
