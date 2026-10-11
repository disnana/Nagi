//! Bounded owned Session-CSRF runtime oracles; wire material stays in RAM.
use nagi_runtime::{
    auth::{self, csrf, session, AuthScope, VerifiedIdentity},
    http_server as http, sqlite, Error, FromRow,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
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
                "sf03-b1-{}-{}",
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
async fn number(pool: &sqlite::Pool, query: &'static str) -> i64 {
    let tx = sqlite::begin(pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let n = sqlite::query::<Number>(&tx, sqlite::literal(query), sqlite::parameters())
        .await
        .unwrap()
        .unwrap()
        .0;
    sqlite::rollback(tx).await.unwrap();
    n
}
fn checked() -> http::Options {
    http::authority(
        http::security_timeout(http::options(256, 1000, 1000, 1000).unwrap(), 1000).unwrap(),
        "https://localhost",
        vec!["localhost".to_owned()],
        1,
        128,
    )
    .unwrap()
}
struct Observations {
    calls: AtomicUsize,
    sanitized: AtomicUsize,
    authorized: tokio::sync::Notify,
    authorizer_entered: AtomicUsize,
    authorizer_sanitized: AtomicUsize,
    parallel: tokio::sync::Barrier,
    deadline_bound: tokio::sync::Notify,
    deadline_release: tokio::sync::Notify,
    deadline_cancelled: AtomicUsize,
}
struct State {
    store: session::Store,
    observations: Arc<Observations>,
}
impl std::ops::Deref for State {
    type Target = Observations;
    fn deref(&self) -> &Observations {
        &self.observations
    }
}
async fn verifier(_: http::Request, _: Arc<State>) -> Result<VerifiedIdentity, auth::Failure> {
    VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(60))
}
async fn issue(
    _: http::Request,
    state: Arc<State>,
    scope: AuthScope,
) -> Result<http::Response, Error> {
    let intent = session::issue(&state.store, scope)
        .await
        .map_err(|_| Error::internal("issue refused"))?;
    session::apply(http::text(http::Status::OK, "issued"), intent)
        .map_err(|_| Error::internal("apply refused"))
}
async fn write(
    request: http::Request,
    state: Arc<State>,
    _: AuthScope,
) -> Result<http::Response, Error> {
    state.calls.fetch_add(1, Ordering::SeqCst);
    if http::header(&request, "cookie").unwrap().is_none()
        && http::header(&request, "x-csrf-token").unwrap().is_none()
    {
        state.sanitized.fetch_add(1, Ordering::SeqCst);
    }
    Ok(http::bytes(http::Status::OK, request.body()))
}
async fn rotate(
    _: http::Request,
    state: Arc<State>,
    scope: AuthScope,
) -> Result<http::Response, Error> {
    let intent = session::rotate(&state.store, scope)
        .await
        .map_err(|_| Error::internal("rotate refused"))?;
    session::apply(http::text(http::Status::OK, "rotated"), intent)
        .map_err(|_| Error::internal("apply refused"))
}
async fn logout(
    _: http::Request,
    state: Arc<State>,
    scope: AuthScope,
) -> Result<http::Response, Error> {
    let intent = session::logout(&state.store, scope)
        .await
        .map_err(|_| Error::internal("logout refused"))?;
    session::apply(http::text(http::Status::OK, "logged out"), intent)
        .map_err(|_| Error::internal("apply refused"))
}
async fn authorize(
    scope: AuthScope,
    request: http::Request,
    state: Arc<State>,
) -> Result<auth::Grant<()>, auth::Failure> {
    state.authorizer_entered.fetch_add(1, Ordering::SeqCst);
    let grant = auth::Grant::from_authorized(scope, 7)?;
    if http::header(&request, "cookie").unwrap().is_none()
        && http::header(&request, "x-csrf-token").unwrap().is_none()
    {
        state.authorizer_sanitized.fetch_add(1, Ordering::SeqCst);
    }
    state.authorized.notify_one();
    Ok(grant)
}
struct Fixture {
    _dir: Directory,
    pool: sqlite::Pool,
    store: session::Store,
    csrf: csrf::CsrfStore,
}
impl Fixture {
    async fn new(count: i64, bytes: i64) -> Self {
        let dir = Directory::new();
        let pool = sqlite::open(
            dir.0.join("owned.sqlite").to_str().unwrap(),
            sqlite::options(2, 4, 1000, 1000).unwrap(),
        )
        .await
        .unwrap();
        let store = session::open(
            &pool,
            session::options(4, 5, 72, 1, 10000, 100000, 5000, 2).unwrap(),
            session::cookie_options("__Host-auth", session::SameSite::Strict).unwrap(),
        )
        .await
        .unwrap();
        let csrf = csrf::open(
            &store,
            csrf::options(count, bytes, 1, 10000, 2).unwrap(),
            &checked(),
        )
        .await
        .unwrap();
        Self {
            _dir: dir,
            pool,
            store,
            csrf,
        }
    }
    async fn stop(self) {
        sqlite::close(&self.pool, 2000).await.unwrap();
    }
}
struct Server {
    address: std::net::SocketAddr,
    state: Arc<Observations>,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<Result<(), Error>>,
}
impl Server {
    async fn new(f: &Fixture) -> Self {
        Self::new_with_options(f, checked()).await
    }
    async fn new_with_options(f: &Fixture, options: http::Options) -> Self {
        let state = Arc::new(Observations {
            calls: AtomicUsize::new(0),
            sanitized: AtomicUsize::new(0),
            authorized: tokio::sync::Notify::new(),
            authorizer_entered: AtomicUsize::new(0),
            authorizer_sanitized: AtomicUsize::new(0),
            parallel: tokio::sync::Barrier::new(2),
            deadline_bound: tokio::sync::Notify::new(),
            deadline_release: tokio::sync::Notify::new(),
            deadline_cancelled: AtomicUsize::new(0),
        });
        let mut app = http::route(
            http::app_default(State {
                store: session::clone_store(&f.store),
                observations: Arc::clone(&state),
            }),
            http::Method::POST,
            "/issue",
            http::authenticated_policy(|r, s: Arc<State>| async move {
                verifier(r, Arc::clone(&s)).await
            }),
            |r, s: Arc<State>, a| async move { issue(r, Arc::clone(&s), a).await },
        )
        .unwrap();
        app = http::session_csrf_tokens(app, "/csrf", csrf::clone_store(&f.csrf)).unwrap();
        let policy = http::session_csrf_policy(
            http::session_authenticated_policy(session::clone_store(&f.store)),
            csrf::clone_store(&f.csrf),
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::POST,
            "/write",
            policy,
            |r, s: Arc<State>, a| async move { write(r, Arc::clone(&s), a).await },
        )
        .unwrap();
        let policy = http::session_csrf_policy(
            http::session_authenticated_policy(session::clone_store(&f.store)),
            csrf::clone_store(&f.csrf),
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::POST,
            "/rotate",
            policy,
            |r, s: Arc<State>, a| async move { rotate(r, Arc::clone(&s), a).await },
        )
        .unwrap();
        let policy = http::session_csrf_policy(
            http::session_authenticated_policy(session::clone_store(&f.store)),
            csrf::clone_store(&f.csrf),
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::POST,
            "/logout",
            policy,
            |r, s: Arc<State>, a| async move { logout(r, Arc::clone(&s), a).await },
        )
        .unwrap();
        let policy = http::session_csrf_policy(
            http::session_authorized_policy(session::clone_store(&f.store), authorize),
            csrf::clone_store(&f.csrf),
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::POST,
            "/pending",
            policy,
            |r, s: Arc<State>, _: auth::Grant<()>| async move {
                s.calls.fetch_add(1, Ordering::SeqCst);
                Ok(http::bytes(http::Status::OK, r.body()))
            },
        )
        .unwrap();
        let policy = http::session_csrf_policy(
            http::session_authenticated_policy(session::clone_store(&f.store)),
            csrf::clone_store(&f.csrf),
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::POST,
            "/parallel",
            policy,
            |_, s: Arc<State>, _: AuthScope| async move {
                s.calls.fetch_add(1, Ordering::SeqCst);
                s.parallel.wait().await;
                Ok(http::empty(http::Status::OK))
            },
        )
        .unwrap();
        let policy = http::session_csrf_policy(
            http::session_authenticated_policy(session::clone_store(&f.store)),
            csrf::clone_store(&f.csrf),
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::POST,
            "/failed-rotate",
            policy,
            |r, s: Arc<State>, a: AuthScope| async move {
                let _sealed = rotate(r, Arc::clone(&s), a).await?;
                Err::<http::Response, _>(Error::internal("bounded failure"))
            },
        )
        .unwrap();
        let policy = http::session_csrf_policy(
            http::session_authorized_policy(
                session::clone_store(&f.store),
                |scope, _, s: Arc<State>| async move {
                    struct Pending<'a>(&'a AtomicUsize);
                    impl Drop for Pending<'_> {
                        fn drop(&mut self) {
                            self.0.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    let grant = auth::Grant::from_authorized(scope, 7)?;
                    let _pending = Pending(&s.deadline_cancelled);
                    s.deadline_bound.notify_one();
                    s.deadline_release.notified().await;
                    Ok(grant)
                },
            ),
            csrf::clone_store(&f.csrf),
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::POST,
            "/deadline",
            policy,
            |_, s: Arc<State>, _: auth::Grant<()>| async move {
                s.calls.fetch_add(1, Ordering::SeqCst);
                Ok(http::empty(http::Status::OK))
            },
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, shutdown) = oneshot::channel();
        let task = tokio::spawn(http::serve_listener(listener, app, options, async {
            let _ = shutdown.await;
        }));
        Self {
            address,
            state,
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
    fn values(&self, key: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
            .collect()
    }
    fn cookie(&self) -> String {
        let c = self.values("set-cookie");
        assert!(c.len() == 1);
        c[0].split(';').next().unwrap().to_owned()
    }
    fn token(&self) -> String {
        assert!(self.status == 200 && self.body.len() == 60);
        assert!(self.values("set-cookie").is_empty());
        let v: nagi_runtime::serde_json::Value =
            nagi_runtime::serde_json::from_slice(&self.body).unwrap();
        let t = v["csrf_token"].as_str().unwrap().to_owned();
        assert!(t.len() == 43);
        t
    }
}
async fn request(s: &Server, method: &str, path: &str, headers: &str, body: &[u8]) -> Reply {
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut stream = TcpStream::connect(s.address).await.unwrap();
        let h = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n{headers}\r\n", body.len());
        stream.write_all(h.as_bytes()).await.unwrap(); stream.write_all(body).await.unwrap(); let mut raw = Vec::new(); stream.take(8193).read_to_end(&mut raw).await.unwrap(); assert!(raw.len() <= 8192);
        let split = raw.windows(4).position(|v| v == b"\r\n\r\n").unwrap(); let mut lines = std::str::from_utf8(&raw[..split]).unwrap().split("\r\n"); let status = lines.next().unwrap().split_whitespace().nth(1).unwrap().parse().unwrap(); let headers = lines.map(|s| { let (k,v) = s.split_once(':').unwrap(); (k.to_owned(), v.trim().to_owned()) }).collect();
        Reply { status, headers, body: raw[split+4..].to_vec() }
    }).await.unwrap()
}
fn guarded(cookie: &str, token: &str) -> String {
    format!("Cookie: {cookie}\r\nOrigin: https://localhost\r\nX-CSRF-Token: {token}\r\n")
}

#[tokio::test]
async fn checked_profiles_reject_unsealed_or_incompatible_policy_sources() {
    for (count, bytes, cleanup, ttl, collision) in [
        (0, 176, 1, 10000, 2),
        (2, 87, 1, 10000, 2),
        (2, 176, 3, 10000, 2),
        (2, 176, 1, 0, 2),
        (i64::MAX, i64::MAX, 1, 10000, 2),
    ] {
        assert!(csrf::options(count, bytes, cleanup, ttl, collision).is_err());
    }
    let f = Fixture::new(2, 176).await;
    assert!(csrf::open(
        &f.store,
        csrf::options(2, 176, 1, 10000, 2).unwrap(),
        &http::default_options()
    )
    .await
    .is_err());
    assert!(
        http::session_csrf_policy(http::public_policy::<State>(), csrf::clone_store(&f.csrf))
            .is_err()
    );
    assert!(http::session_csrf_policy(
        http::authenticated_policy(verifier),
        csrf::clone_store(&f.csrf)
    )
    .is_err());
    let guarded = http::session_csrf_policy(
        http::session_authenticated_policy::<State>(session::clone_store(&f.store)),
        csrf::clone_store(&f.csrf),
    )
    .unwrap();
    assert!(http::session_csrf_policy(guarded, csrf::clone_store(&f.csrf)).is_err());
    let other = Fixture::new(2, 176).await;
    assert!(http::session_csrf_policy(
        http::session_authenticated_policy::<State>(session::clone_store(&other.store)),
        csrf::clone_store(&f.csrf)
    )
    .is_err());
    assert!(http::route(
        http::app_default(()),
        http::Method::POST,
        "/unguarded",
        http::session_authenticated_policy(session::clone_store(&f.store)),
        |_, _, _| async { Ok(http::empty(http::Status::OK)) }
    )
    .is_err());
    other.stop().await;
    f.stop().await;
}

#[tokio::test]
async fn tokens_are_authenticated_reusable_and_preserve_bounded_ledger() {
    let f = Fixture::new(2, 176).await;
    let s = Server::new(&f).await;
    let absent = request(&s, "GET", "/csrf", "", b"").await;
    assert!(absent.status == 401 && absent.body.len() != 60);
    let cookie = request(&s, "POST", "/issue", "Authorization: Bearer owned\r\n", b"")
        .await
        .cookie();
    let headers = format!("Cookie: {cookie}\r\n");
    let head = request(&s, "HEAD", "/csrf", &headers, b"").await;
    assert!(
        head.status == 200 && head.body.is_empty() && head.values("content-length") == vec!["60"]
    );
    assert!(number(&f.pool, "SELECT count(*) AS n FROM __nagi_csrf_tokens").await == 0);
    let one = request(&s, "GET", "/csrf", &headers, b"").await;
    let a = one.token();
    let two = request(&s, "GET", "/csrf", &headers, b"").await;
    let b = two.token();
    assert!(a != b);
    assert!(
        one.values("cache-control") == vec!["no-store"]
            && one.values("vary") == vec!["Cookie, Authorization, Origin"]
    );
    let expiry = number(
        &f.pool,
        "SELECT sum(expiry_ms) AS n FROM __nagi_csrf_tokens",
    )
    .await;
    for token in [&a, &a, &b, &a] {
        let r = request(&s, "POST", "/write", &guarded(&cookie, token), b"owned").await;
        assert!(
            r.status == 200
                && r.body == b"owned"
                && r.values("vary") == vec!["Cookie, Authorization, Origin, X-CSRF-Token"]
        );
    }
    assert!(
        s.state.calls.load(Ordering::SeqCst) == 4 && s.state.sanitized.load(Ordering::SeqCst) == 4
    );
    assert!(
        number(&f.pool, "SELECT count(*) AS n FROM __nagi_csrf_tokens").await == 2
            && number(
                &f.pool,
                "SELECT sum(expiry_ms) AS n FROM __nagi_csrf_tokens"
            )
            .await
                == expiry
    );
    let full = request(&s, "GET", "/csrf", &headers, b"").await;
    assert!(full.status == 503 && full.body.len() != 60);
    assert!(
        request(&s, "POST", "/write", &guarded(&cookie, &a), b"owned")
            .await
            .status
            == 200
    );
    s.stop().await;
    f.stop().await;
}

#[tokio::test]
async fn guarded_rotation_and_logout_invalidate_old_generation_tokens() {
    let f = Fixture::new(4, 352).await;
    let s = Server::new(&f).await;
    let old = request(&s, "POST", "/issue", "Authorization: Bearer owned\r\n", b"")
        .await
        .cookie();
    let token = request(&s, "GET", "/csrf", &format!("Cookie: {old}\r\n"), b"")
        .await
        .token();
    let rotated = request(&s, "POST", "/rotate", &guarded(&old, &token), b"").await;
    assert!(rotated.status == 200);
    let new = rotated.cookie();
    assert!(old != new);
    assert!(
        request(&s, "POST", "/write", &guarded(&new, &token), b"")
            .await
            .status
            == 403
    );
    let fresh = request(&s, "GET", "/csrf", &format!("Cookie: {new}\r\n"), b"")
        .await
        .token();
    let removed = request(&s, "POST", "/logout", &guarded(&new, &fresh), b"").await;
    assert!(removed.status == 200 && removed.cookie().ends_with('='));
    assert!(
        request(&s, "POST", "/write", &guarded(&new, &fresh), b"")
            .await
            .status
            == 401
    );
    assert!(
        s.state.calls.load(Ordering::SeqCst) == 0
            && number(&f.pool, "SELECT count(*) AS n FROM __nagi_session_rows").await == 0
            && number(&f.pool, "SELECT count(*) AS n FROM __nagi_csrf_tokens").await == 2
    );
    s.stop().await;
    f.stop().await;
}

#[tokio::test]
async fn body_checkpoint_rejects_generation_changed_after_actual_authorization() {
    let f = Fixture::new(4, 352).await;
    let s = Server::new(&f).await;
    let cookie = request(&s, "POST", "/issue", "Authorization: Bearer owned\r\n", b"")
        .await
        .cookie();
    let token = request(&s, "GET", "/csrf", &format!("Cookie: {cookie}\r\n"), b"")
        .await
        .token();
    let mut stream = TcpStream::connect(s.address).await.unwrap();
    let h = format!(
        "POST /pending HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 5\r\n{}\r\nov",
        guarded(&cookie, &token)
    );
    stream.write_all(h.as_bytes()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), s.state.authorized.notified())
        .await
        .unwrap();
    assert!(
        s.state.calls.load(Ordering::SeqCst) == 0
            && s.state.authorizer_sanitized.load(Ordering::SeqCst) == 1
    );
    let rotated = request(&s, "POST", "/rotate", &guarded(&cookie, &token), b"").await;
    assert!(rotated.status == 200);
    stream.write_all(b"ned").await.unwrap();
    let mut raw = Vec::new();
    tokio::time::timeout(
        Duration::from_secs(3),
        stream.take(8193).read_to_end(&mut raw),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(raw.len() <= 8192);
    let status: u16 = std::str::from_utf8(&raw)
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    assert!(status == 403 && s.state.calls.load(Ordering::SeqCst) == 0);
    s.stop().await;
    f.stop().await;
}

#[tokio::test]
async fn finite_origin_and_header_refusals_preserve_existing_token_and_capacity() {
    let f = Fixture::new(2, 176).await;
    let s = Server::new(&f).await;
    let cookie = request(&s, "POST", "/issue", "Authorization: Bearer owned\r\n", b"")
        .await
        .cookie();
    let token = request(&s, "GET", "/csrf", &format!("Cookie: {cookie}\r\n"), b"")
        .await
        .token();
    let cases = [
        (
            format!("Cookie: {cookie}\r\nX-CSRF-Token: {token}\r\n"),
            403,
        ),
        (
            format!("Cookie: {cookie}\r\nOrigin: null\r\nX-CSRF-Token: {token}\r\n"),
            403,
        ),
        (
            format!(
                "Cookie: {cookie}\r\nOrigin: https://other.invalid\r\nX-CSRF-Token: {token}\r\n"
            ),
            403,
        ),
        (
            format!("{}Origin: https://localhost\r\n", guarded(&cookie, &token)),
            403,
        ),
        (
            format!("Cookie: {cookie}\r\nOrigin: https://localhost\r\n"),
            403,
        ),
        (guarded(&cookie, "ordinary-invalid"), 403),
        (
            format!("{}X-CSRF-Token: {token}\r\n", guarded(&cookie, &token)),
            400,
        ),
        (
            format!(
                "{}Authorization: Bearer owned\r\n",
                guarded(&cookie, &token)
            ),
            400,
        ),
        (
            format!("{}Host: other.invalid\r\n", guarded(&cookie, &token)),
            400,
        ),
    ];
    for (h, status) in cases {
        let r = request(&s, "POST", "/write", &h, b"owned").await;
        assert!(
            r.status == status
                && r.values("set-cookie").is_empty()
                && r.values("vary") == vec!["Cookie, Authorization, Origin, X-CSRF-Token"]
        );
    }
    let large = request(
        &s,
        "POST",
        "/write",
        &guarded(&cookie, &token),
        &[b'o'; 257],
    )
    .await;
    assert!(
        large.status == 413
            && large.values("set-cookie").is_empty()
            && large.values("vary") == vec!["Cookie, Authorization, Origin, X-CSRF-Token"]
    );
    assert!(s.state.calls.load(Ordering::SeqCst) == 0);
    assert!(
        request(
            &s,
            "GET",
            "/csrf",
            &format!("Cookie: {cookie}\r\nOrigin: https://other.invalid\r\n"),
            b""
        )
        .await
        .status
            == 403
    );
    assert!(request(&s, "HEAD", "/csrf", "", b"").await.status == 401);
    assert!(
        request(&s, "POST", "/write", &guarded(&cookie, &token), b"owned")
            .await
            .status
            == 200
    );
    s.stop().await;
    f.stop().await;
}
#[tokio::test]
async fn one_reusable_token_admits_two_distinct_actual_requests() {
    let f = Fixture::new(2, 176).await;
    let s = Server::new(&f).await;
    let cookie = request(&s, "POST", "/issue", "Authorization: Bearer owned\r\n", b"")
        .await
        .cookie();
    let token = request(&s, "GET", "/csrf", &format!("Cookie: {cookie}\r\n"), b"")
        .await
        .token();
    let headers = guarded(&cookie, &token);
    let (one, two) = tokio::join!(
        request(&s, "POST", "/parallel", &headers, b""),
        request(&s, "POST", "/parallel", &headers, b"")
    );
    assert!(
        one.status == 200
            && two.status == 200
            && s.state.calls.load(Ordering::SeqCst) == 2
            && number(&f.pool, "SELECT count(*) AS n FROM __nagi_csrf_tokens").await == 1
    );
    s.stop().await;
    f.stop().await;
}
#[tokio::test]
async fn guarded_partial_body_preparation_uses_original_security_deadline() {
    // Post-preparation body precedence is exercised by the production body wait
    // with real Hyper Incoming in the private deadline wire fixture. Here a real
    // SQLite transaction keeps Session preparation pending, independently of
    // Windows preparation throughput. Security includes that preparation wait.
    for (body_ms, security_ms) in [(80, 240), (240, 80), (100, 100)] {
        let f = Fixture::new(2, 176).await;
        let bootstrap = Server::new(&f).await;
        let cookie = request(
            &bootstrap,
            "POST",
            "/issue",
            "Authorization: Bearer owned\r\n",
            b"",
        )
        .await
        .cookie();
        let token = request(
            &bootstrap,
            "GET",
            "/csrf",
            &format!("Cookie: {cookie}\r\n"),
            b"",
        )
        .await
        .token();
        bootstrap.stop().await;
        let options = http::authority(
            http::security_timeout(
                http::options(256, body_ms, 1000, 1000).unwrap(),
                security_ms,
            )
            .unwrap(),
            "https://localhost",
            vec!["localhost".to_owned()],
            1,
            128,
        )
        .unwrap();
        let s = Server::new_with_options(&f, options).await;
        let held = sqlite::begin(&f.pool, sqlite::BeginMode::Immediate)
            .await
            .unwrap();
        let mut stream = TcpStream::connect(s.address).await.unwrap();
        let h = format!(
            "POST /pending HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 5\r\n{}\r\nov",
            guarded(&cookie, &token)
        );
        stream.write_all(h.as_bytes()).await.unwrap();
        let mut raw = Vec::new();
        tokio::time::timeout(
            Duration::from_secs(2),
            stream.take(8193).read_to_end(&mut raw),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(raw.len() <= 8192);
        let status_seen: u16 = std::str::from_utf8(&raw)
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(status_seen, 504);
        assert_eq!(s.state.calls.load(Ordering::SeqCst), 0);
        assert_eq!(s.state.authorizer_entered.load(Ordering::SeqCst), 0);
        assert_eq!(s.state.authorizer_sanitized.load(Ordering::SeqCst), 0);
        let headers = std::str::from_utf8(&raw)
            .unwrap()
            .split("\r\n\r\n")
            .next()
            .unwrap()
            .to_ascii_lowercase();
        assert!(headers.contains("cache-control: no-store"));
        assert!(headers.contains("vary: cookie, authorization, origin, x-csrf-token"));
        assert!(!headers.contains("set-cookie:") && !headers.contains("www-authenticate:"));
        sqlite::rollback(held).await.unwrap();
        assert!(number(&f.pool, "SELECT count(*) AS n FROM __nagi_csrf_tokens").await == 1);
        s.stop().await;
        f.stop().await;
    }
}
#[tokio::test]
async fn failed_guarded_handler_never_finalizes_committed_rotation_cookie() {
    let f = Fixture::new(2, 176).await;
    let s = Server::new(&f).await;
    let cookie = request(&s, "POST", "/issue", "Authorization: Bearer owned\r\n", b"")
        .await
        .cookie();
    let token = request(&s, "GET", "/csrf", &format!("Cookie: {cookie}\r\n"), b"")
        .await
        .token();
    let failed = request(&s, "POST", "/failed-rotate", &guarded(&cookie, &token), b"").await;
    assert!(failed.status == 500 && failed.values("set-cookie").is_empty());
    assert!(number(&f.pool, "SELECT generation AS n FROM __nagi_session_rows").await == 1);
    assert!(
        request(&s, "POST", "/write", &guarded(&cookie, &token), b"")
            .await
            .status
            == 401
    );
    s.stop().await;
    f.stop().await;
}

#[tokio::test]
async fn mismatched_startup_origin_refuses_token_and_guarded_routes_before_bind() {
    let f = Fixture::new(2, 176).await;
    let occupied = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = i64::from(occupied.local_addr().unwrap().port());
    for token_route in [false, true] {
        let app = if token_route {
            http::session_csrf_tokens(http::app_default(()), "/csrf", csrf::clone_store(&f.csrf))
                .unwrap()
        } else {
            let policy = http::session_csrf_policy(
                http::session_authenticated_policy(session::clone_store(&f.store)),
                csrf::clone_store(&f.csrf),
            )
            .unwrap();
            http::route(
                http::app_default(()),
                http::Method::POST,
                "/guarded",
                policy,
                |_, _, _: AuthScope| async { Ok(http::empty(http::Status::OK)) },
            )
            .unwrap()
        };
        let wrong = http::authority(
            http::options(256, 1000, 1000, 1000).unwrap(),
            "https://other.invalid",
            vec!["localhost".to_owned()],
            1,
            128,
        )
        .unwrap();
        let e = tokio::time::timeout(Duration::from_secs(2), http::serve(app, port, wrong))
            .await
            .unwrap()
            .expect_err("mismatched startup bound server");
        assert!(matches!(e.kind, nagi_runtime::ErrorKind::Invalid));
    }
    drop(occupied);
    f.stop().await;
}

#[tokio::test]
async fn pending_guarded_authorizer_expires_at_bound_session_and_recovers_permit() {
    let f = Fixture::new(3, 264).await;
    let options = http::capacity(http::security_timeout(checked(), 2000).unwrap(), 4, 1).unwrap();
    let s = Server::new_with_options(&f, options).await;
    let cookie = request(&s, "POST", "/issue", "Authorization: Bearer owned\r\n", &[])
        .await
        .cookie();
    let token = request(&s, "GET", "/csrf", &format!("Cookie: {cookie}\r\n"), &[])
        .await
        .token();
    let expiry = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
        + 500;
    let tx = sqlite::begin(&f.pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    assert_eq!(
        sqlite::exec(
            &tx,
            sqlite::literal("UPDATE __nagi_session_rows SET idle_ms=?,absolute_ms=?"),
            sqlite::bind_i64(sqlite::bind_i64(sqlite::parameters(), expiry), expiry)
        )
        .await
        .unwrap(),
        1
    );
    sqlite::commit(tx).await.unwrap();
    let header = guarded(&cookie, &token);
    let reply = {
        let pending = request(&s, "POST", "/deadline", &header, &[]);
        tokio::pin!(pending);
        tokio::select! {
            _ = s.state.deadline_bound.notified() => {},
            _ = &mut pending => panic!("authorizer did not reach actual binding barrier"),
        }
        // This watchdog is earlier than fixed security2000 and later than the
        // real credential500. The barrier remains closed throughout the response.
        tokio::time::timeout(Duration::from_millis(1000), &mut pending)
            .await
            .unwrap()
    };
    assert_eq!(reply.status, 504);
    assert!(reply.values("set-cookie").is_empty());
    assert_eq!(
        reply.values("vary"),
        ["Cookie, Authorization, Origin, X-CSRF-Token"]
    );
    assert_eq!(s.state.deadline_cancelled.load(Ordering::SeqCst), 1);
    assert_eq!(s.state.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        number(&f.pool, "SELECT count(*) AS n FROM __nagi_csrf_tokens").await,
        1
    );
    let expired = request(&s, "POST", "/deadline", &header, &[]).await;
    assert_eq!(expired.status, 401);
    assert!(expired.values("set-cookie").is_empty());
    assert_eq!(s.state.deadline_cancelled.load(Ordering::SeqCst), 1);
    // Capacity is one: a complete fresh guarded exchange proves actual recovery.
    let fresh_cookie = request(&s, "POST", "/issue", "Authorization: Bearer owned\r\n", &[])
        .await
        .cookie();
    let fresh_token = request(
        &s,
        "GET",
        "/csrf",
        &format!("Cookie: {fresh_cookie}\r\n"),
        &[],
    )
    .await
    .token();
    assert_eq!(
        request(
            &s,
            "POST",
            "/write",
            &guarded(&fresh_cookie, &fresh_token),
            b"bounded"
        )
        .await
        .status,
        200
    );
    assert_eq!(s.state.calls.load(Ordering::SeqCst), 1);
    s.stop().await;
    f.stop().await;
}
