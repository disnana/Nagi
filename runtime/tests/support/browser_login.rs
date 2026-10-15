#![allow(dead_code)]
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
pub(crate) struct Directory(pub(crate) PathBuf, bool);
impl Directory {
    pub(crate) fn new() -> Self {
        loop {
            let p = std::env::temp_dir().join(format!(
                "sf03-login-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&p) {
                Ok(()) => return Self(p, true),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => panic!("owned directory unavailable"),
            }
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.1 {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
pub(crate) struct Number(i64);
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
pub(crate) async fn number(pool: &sqlite::Pool, q: &'static str) -> i64 {
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
pub(crate) struct Counters {
    pub(crate) verifier: AtomicUsize,
    pub(crate) handler: AtomicUsize,
    pub(crate) mode: AtomicUsize,
    pub(crate) entered: tokio::sync::Notify,
    pub(crate) cancelled: AtomicUsize,
    pub(crate) sanitized: AtomicBool,
    pub(crate) claim_committed: AtomicBool,
}
pub(crate) struct CancelSeen(Arc<Counters>);
impl Drop for CancelSeen {
    fn drop(&mut self) {
        self.0.cancelled.fetch_add(1, Ordering::SeqCst);
    }
}
pub(crate) struct State {
    pub(crate) store: session::Store,
    pub(crate) auth: sqlite::Pool,
    pub(crate) stats: Arc<Counters>,
    pub(crate) original: Instant,
}
pub(crate) async fn named_trusted_bearer(
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
pub(crate) async fn login_handler(
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
pub(crate) fn checked(body: i64, security: i64) -> http::Options {
    checked_phases(body, security, 1000, 1000)
}
pub(crate) fn checked_phases(
    body: i64,
    security: i64,
    body_ms: i64,
    shutdown_ms: i64,
) -> http::Options {
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
pub(crate) struct Fixture {
    pub(crate) _dir: Directory,
    pub(crate) auth: sqlite::Pool,
    pub(crate) store: session::Store,
    pub(crate) pool: prelogin::PreLoginPool,
    pub(crate) stats: Arc<Counters>,
}
impl Fixture {
    pub(crate) async fn new(ttl: i64, quota: i64) -> Self {
        Self::open_in(Directory::new(), ttl, quota, &checked(256, 1000)).await
    }
    // Launcher owns this directory; no Drop cleanup without explicit close acknowledgements.
    pub(crate) async fn owned_browser(dir: PathBuf, options: &http::Options) -> Self {
        assert!(dir.is_dir());
        Self::open_in(Directory(dir, false), 120_000, 72, options).await
    }
    async fn open_in(dir: Directory, ttl: i64, quota: i64, options: &http::Options) -> Self {
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
            options,
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
    pub(crate) fn app(&self) -> http::App<State, Error> {
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
    pub(crate) async fn rows(&self) -> i64 {
        number(
            &self.auth,
            "SELECT count(*) AS n FROM __nagi_prelogin_ready",
        )
        .await
    }
    pub(crate) async fn stop(self) {
        prelogin::close(&self.pool, 2000).await.unwrap();
        sqlite::close(&self.auth, 2000).await.unwrap();
    }
}
pub(crate) struct Server {
    pub(crate) address: std::net::SocketAddr,
    pub(crate) stop: Option<oneshot::Sender<()>>,
    pub(crate) task: tokio::task::JoinHandle<Result<(), Error>>,
}
impl Server {
    pub(crate) async fn new(f: &Fixture, body: i64, security: i64) -> Self {
        Self::with_options(f, checked(body, security)).await
    }
    pub(crate) async fn with_options(f: &Fixture, options: http::Options) -> Self {
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
    pub(crate) async fn stop(mut self) {
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
pub(crate) struct Reply {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}
impl Reply {
    pub(crate) fn values(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
            .collect()
    }
    pub(crate) fn guarded(&self) {
        assert!(self.values("cache-control") == vec!["no-store"]);
        assert!(
            self.values("vary")
                == vec!["Authorization, Content-Type, Cookie, Origin, X-CSRF-Token"]
        );
    }
    pub(crate) fn material(&self) -> (String, String) {
        if self.status != 200 {
            let class = match self.status {
                400 => "request",
                401 | 403 => "refusal",
                500 => "internal",
                503 => "unavailable",
                504 => "deadline",
                _ => "other",
            };
            eprintln!(
                "browser_login_bootstrap_diag check=material status={} class={} set_cookie_count={} body_bytes={} auth_cookie={} deletion={}",
                self.status, class, self.values("set-cookie").len().min(65),
                self.body.len().min(8193), self.auth_cookie(), self.deletion(),
            );
        }
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
    pub(crate) fn deletion(&self) -> bool {
        self.values("set-cookie").iter().any(|v| {
            v.starts_with("__Host-prelogin=;")
                && v.contains("Max-Age=0")
                && v.contains("Secure")
                && v.contains("HttpOnly")
                && v.contains("Path=/")
        })
    }
    pub(crate) fn auth_cookie(&self) -> bool {
        self.values("set-cookie")
            .iter()
            .any(|v| v.starts_with("__Host-auth="))
    }
}
pub(crate) async fn request(
    address: std::net::SocketAddr,
    method: &str,
    path: &str,
    headers: &str,
    body: &[u8],
) -> Reply {
    tokio::time::timeout(Duration::from_secs(3),async { let mut s=TcpStream::connect(address).await.unwrap(); let h=format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n{headers}\r\n",body.len()); s.write_all(h.as_bytes()).await.unwrap(); s.write_all(body).await.unwrap(); let mut raw=Vec::new(); s.take(8193).read_to_end(&mut raw).await.unwrap(); assert!(raw.len()<=8192); parse_reply(raw) }).await.unwrap()
}
pub(crate) fn parse_reply(raw: Vec<u8>) -> Reply {
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
pub(crate) async fn bootstrap(s: &Server) -> (String, String) {
    request(s.address, "GET", "/bootstrap", "", b"")
        .await
        .material()
}
pub(crate) fn login_headers(cookie: &str, token: &str) -> String {
    format!("Origin: https://localhost\r\nAuthorization: Bearer owned\r\nCookie: {cookie}\r\nX-CSRF-Token: {token}\r\nContent-Type: application/json\r\n")
}
