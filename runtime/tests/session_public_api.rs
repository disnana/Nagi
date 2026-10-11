//! External normal-library Session API oracle; real I/O, controlled handler time.
//! Real cookies stay in process memory and never appear in assertions/artifacts.
use futures_util::FutureExt;
use nagi_runtime::{
    auth::{session, VerifiedIdentity},
    http_server as http, sqlite, Error, FromRow,
};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
static NEXT: AtomicU64 = AtomicU64::new(1);
struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        loop {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("session-public-{}-{n}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("owned directory: {e}"),
            }
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn configuration() -> session::Options {
    session::options(4, 5, 72, 1, 10000, 100000, 5000, 2).unwrap()
}
fn cookie_policy() -> session::CookieOptions {
    session::cookie_options("__Host-owned", session::SameSite::Strict).unwrap()
}
struct Count(i64);
impl FromRow for Count {
    fn columns() -> &'static [&'static str] {
        &["n"]
    }
    fn read(
        row: &nagi_runtime::rusqlite::Row<'_>,
        columns: &[usize],
    ) -> nagi_runtime::rusqlite::Result<Self> {
        Ok(Self(row.get(columns[0])?))
    }
}
async fn count(pool: &sqlite::Pool, query: &'static str) -> i64 {
    let tx = sqlite::begin(pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let n = sqlite::query::<Count>(&tx, sqlite::literal(query), sqlite::parameters())
        .await
        .unwrap()
        .unwrap()
        .0;
    sqlite::rollback(tx).await.unwrap();
    n
}
#[test]
fn published_checked_configuration_exports_match_contract() {
    fn shared<T: Send + Sync>() {}
    shared::<session::Options>();
    shared::<session::CookieOptions>();
    shared::<session::Store>();
    assert!(session::options(0, 5, 72, 1, 10000, 100000, 5000, 2).is_err());
    assert!(session::cookie_options("ordinary", session::SameSite::Strict).is_err());
    // None stays represented but fail-closed before the SF03 guard exists.
    assert!(session::cookie_options("__Host-owned", session::SameSite::None).is_err());
}
#[tokio::test]
async fn published_store_reopens_and_observes_original_pool_close() {
    let dir = Directory::new();
    let pool = sqlite::open(
        dir.0.join("owned.sqlite").to_str().unwrap(),
        sqlite::options(2, 4, 1000, 1000).unwrap(),
    )
    .await
    .unwrap();
    let first = session::open(&pool, configuration(), cookie_policy())
        .await
        .unwrap();
    let second = session::open(&pool, configuration(), cookie_policy())
        .await
        .unwrap();
    let explicit = session::clone_store(&first);
    let state = format!("{explicit:?}");
    assert!(!state.is_empty());
    drop(first);
    drop(second);
    drop(explicit);
    sqlite::close(&pool, 2000).await.unwrap();
    let failure = session::open(&pool, configuration(), cookie_policy())
        .await
        .err()
        .unwrap();
    assert!(session::kind(&failure) == nagi_runtime::auth::FailureKind::Unavailable);
    assert!(!session::message(&failure).is_empty());
    assert!(session::outcome(&failure) == sqlite::Outcome::NotApplicable);
}
#[tokio::test]
async fn published_store_rejects_memory_before_reserved_ddl() {
    let pool = sqlite::open(":memory:", sqlite::options(1, 4, 1000, 1000).unwrap())
        .await
        .unwrap();
    let failure = session::open(&pool, configuration(), cookie_policy())
        .await
        .err()
        .unwrap();
    assert!(session::kind(&failure) == nagi_runtime::auth::FailureKind::InvalidRequest);
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) AS n FROM sqlite_master WHERE name LIKE '__nagi_session_%'"
        )
        .await,
        0
    );
    sqlite::close(&pool, 2000).await.unwrap();
}
struct Received {
    status: u16,
    headers: Vec<(String, String)>,
}
impl Received {
    fn values(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .collect()
    }
    fn cookie(&self) -> String {
        let values = self.values("set-cookie");
        assert_eq!(values.len(), 1);
        values[0].split(';').next().unwrap().to_owned()
    }
}
// The successful public-API sequence is not a one-second storage benchmark.
// Keep production HTTP/SQLite/authority budgets intact. Only Tokio's test clock
// is held fixed; std::time authority clocks and actual socket/DB work remain real.
// A runnable coordinator prevents paused-time idle auto-advance. The independent
// wall guard bounds a stalled test without retrying or extending runtime limits.
async fn without_handler_clock_advance<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::pause();
    struct Resume;
    impl Drop for Resume {
        fn drop(&mut self) {
            tokio::time::resume();
        }
    }
    let _resume = Resume;
    let tick = tokio::time::Instant::now();
    let wall = Instant::now();
    let mut future = std::pin::pin!(future);
    loop {
        assert!(
            wall.elapsed() < Duration::from_secs(3),
            "public Session I/O wall guard elapsed"
        );
        let result = futures_util::poll!(future.as_mut());
        assert!(
            tokio::time::Instant::now() == tick,
            "public Session clock advanced during real I/O"
        );
        if let std::task::Poll::Ready(value) = result {
            return value;
        }
        tokio::task::yield_now().await;
    }
}
async fn request(address: std::net::SocketAddr, path: &str, credential: &str) -> Received {
    without_handler_clock_advance(async {
        let mut socket = TcpStream::connect(address).await.unwrap();
        let owned = format!("POST {path} HTTP/1.1\r\nHost: localhost\r\n{credential}Content-Length: 0\r\nConnection: close\r\n\r\n");
        socket.write_all(owned.as_bytes()).await.unwrap();
        let mut raw = Vec::new();
        socket.take(4097).read_to_end(&mut raw).await.unwrap();
        assert!(raw.len() <= 4096);
        let text = std::str::from_utf8(&raw).unwrap();
        let head = text.split("\r\n\r\n").next().unwrap();
        let mut lines = head.split("\r\n");
        let status = lines.next().unwrap().split_whitespace().nth(1).unwrap().parse().unwrap();
        let headers = lines.map(|line| { let (k, v) = line.split_once(':').unwrap(); (k.to_owned(), v.trim().to_owned()) }).collect();
        Received { status, headers }
    }).await
}
async fn verifier(
    _: http::Request,
    _: Arc<session::Store>,
) -> Result<VerifiedIdentity, nagi_runtime::auth::Failure> {
    // Existing explicitly trusted verifier fixture; not a public Session factory.
    VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(60))
}
#[tokio::test]
async fn published_issue_rotate_logout_transfer_cookie_once_after_real_commit() {
    let dir = Directory::new();
    let pool = sqlite::open(
        dir.0.join("owned.sqlite").to_str().unwrap(),
        sqlite::options(2, 4, 1000, 1000).unwrap(),
    )
    .await
    .unwrap();
    let store = session::open(&pool, configuration(), cookie_policy())
        .await
        .unwrap();
    let app = http::route(
        http::app_default(session::clone_store(&store)),
        http::Method::POST,
        "/issue",
        http::authenticated_policy(verifier),
        |_, store, scope| async move {
            let intent = session::issue(&store, scope).await.unwrap();
            Ok::<_, Error>(session::apply(http::empty(http::Status::OK), intent).unwrap())
        },
    )
    .unwrap();
    let app = http::route(
        app,
        http::Method::POST,
        "/rotate",
        http::session_authenticated_policy(session::clone_store(&store)),
        |_, store, scope| async move {
            let intent = session::rotate(&store, scope).await.unwrap();
            Ok::<_, Error>(session::apply(http::empty(http::Status::OK), intent).unwrap())
        },
    )
    .unwrap();
    let app = http::route(
        app,
        http::Method::POST,
        "/logout",
        http::session_authenticated_policy(session::clone_store(&store)),
        |_, store, scope| async move {
            let intent = session::logout(&store, scope).await.unwrap();
            Ok::<_, Error>(session::apply(http::empty(http::Status::OK), intent).unwrap())
        },
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    // HTTP-only owned wire test; Secure is serialization, not a browser/TLS proof.
    let options = http::authority(
        http::options(64, 1000, 1000, 1000).unwrap(),
        "https://localhost",
        vec!["localhost".to_owned()],
        1,
        128,
    )
    .unwrap();
    let (stop, shutdown) = oneshot::channel();
    let mut worker = tokio::spawn(http::serve_listener(listener, app, options, async {
        let _ = shutdown.await;
    }));
    let outcome = std::panic::AssertUnwindSafe(async {
        let issued = request(address, "/issue", "Authorization: Bearer owned\r\n").await;
        assert_eq!(issued.status, 200);
        assert_eq!(issued.values("cache-control"), vec!["no-store"]);
        assert_eq!(issued.values("vary"), vec!["Cookie, Authorization"]);
        assert_eq!(
            count(&pool, "SELECT count(*) AS n FROM __nagi_session_rows").await,
            1
        );
        let old = issued.cookie();
        let rotated = request(address, "/rotate", &format!("Cookie: {old}\r\n")).await;
        assert_eq!(rotated.status, 200);
        assert_eq!(rotated.values("vary"), vec!["Cookie"]);
        let new = rotated.cookie();
        assert!(old != new);
        let stale = request(address, "/rotate", &format!("Cookie: {old}\r\n")).await;
        assert_eq!(stale.status, 401);
        assert!(stale.values("set-cookie").is_empty());
        let logged_out = request(address, "/logout", &format!("Cookie: {new}\r\n")).await;
        assert_eq!(logged_out.status, 200);
        assert_eq!(logged_out.values("set-cookie").len(), 1);
        let deletion = cookie::Cookie::parse(logged_out.values("set-cookie")[0]).unwrap();
        assert!(deletion.value().is_empty());
        assert!(deletion.max_age() == Some(cookie::time::Duration::ZERO));
        assert_eq!(
            count(&pool, "SELECT count(*) AS n FROM __nagi_session_rows").await,
            0
        );
        let absent = request(address, "/logout", &format!("Cookie: {new}\r\n")).await;
        assert_eq!(absent.status, 401);
        assert!(absent.values("set-cookie").is_empty());
    })
    .catch_unwind()
    .await;
    // Always attempt bounded shutdown and actual pool close before reporting a
    // failed assertion. Directory Drop alone is not worker/connection completion.
    let _ = stop.send(());
    let stopped = tokio::time::timeout(Duration::from_secs(3), &mut worker).await;
    if stopped.is_err() {
        worker.abort();
        let _ = worker.await;
    }
    drop(store);
    let closed = sqlite::close(&pool, 2000).await;
    if let Err(payload) = outcome {
        std::panic::resume_unwind(payload);
    }
    stopped.unwrap().unwrap().unwrap();
    closed.unwrap();
}
