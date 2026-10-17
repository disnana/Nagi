//! External ordinary-library API tests. Secret wire values stay in RAM;
//! assertions compare status/count/booleans without formatting material.
use nagi_runtime::{
    auth::{prelogin, session, FailureKind},
    http_server as http, sqlite, Error, FromRow,
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
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
            let path = std::env::temp_dir().join(format!(
                "sf03-public-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
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
struct Fixture {
    auth: sqlite::Pool,
    store: session::Store,
    _dir: Directory,
}
impl Fixture {
    async fn new() -> Self {
        let dir = Directory::new();
        let auth = sqlite::open(
            dir.0.join("owned.sqlite").to_str().unwrap(),
            sqlite::options(1, 4, 1000, 1000).unwrap(),
        )
        .await
        .unwrap();
        let store = session::open(
            &auth,
            session::options(2, 3, 72, 1, 10000, 100000, 5000, 2).unwrap(),
            session::cookie_options("__Host-auth", session::SameSite::Strict).unwrap(),
        )
        .await
        .unwrap();
        Self {
            auth,
            store,
            _dir: dir,
        }
    }
    async fn open(
        &self,
        host: &http::Options,
    ) -> Result<prelogin::PreLoginPool, prelogin::Failure> {
        prelogin::open(
            &self.store,
            sqlite::options(1, 2, 1000, 1000).unwrap(),
            configuration(),
            cookie(),
            host,
        )
        .await
    }
    async fn stop(self) {
        sqlite::close(&self.auth, 2000).await.unwrap();
    }
}
fn configuration() -> prelogin::Options {
    prelogin::options(2, 3, 144, 1, 10000, 2, 8, 10000, 4, 16).unwrap()
}
fn cookie() -> prelogin::CookieOptions {
    prelogin::cookie_options("__Host-prelogin", session::SameSite::Strict).unwrap()
}
fn checked(origin: &str) -> http::Options {
    http::authority(
        http::options(64, 1000, 1000, 1000).unwrap(),
        origin,
        vec!["localhost".to_owned()],
        1,
        128,
    )
    .unwrap()
}
struct Number(i64);
impl FromRow for Number {
    fn columns() -> &'static [&'static str] {
        &["n"]
    }
    fn read(
        row: &nagi_runtime::rusqlite::Row<'_>,
        i: &[usize],
    ) -> nagi_runtime::rusqlite::Result<Self> {
        Ok(Self(row.get(i[0])?))
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
struct Snapshot {
    config: String,
    incarnation: Vec<u8>,
    last_wall: i64,
    issued: i64,
    expiry: i64,
}
impl FromRow for Snapshot {
    fn columns() -> &'static [&'static str] {
        &["config", "incarnation", "last_wall", "issued", "expiry"]
    }
    fn read(
        row: &nagi_runtime::rusqlite::Row<'_>,
        i: &[usize],
    ) -> nagi_runtime::rusqlite::Result<Self> {
        Ok(Self {
            config: row.get(i[0])?,
            incarnation: row.get(i[1])?,
            last_wall: row.get(i[2])?,
            issued: row.get(i[3])?,
            expiry: row.get(i[4])?,
        })
    }
}
async fn snapshot(pool: &sqlite::Pool) -> Snapshot {
    let tx = sqlite::begin(pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let s = sqlite::query(&tx, sqlite::literal("SELECT config,incarnation,last_wall,issued,(SELECT expiry_ms FROM __nagi_prelogin_ready LIMIT 1) AS expiry FROM __nagi_prelogin_meta WHERE id=1"), sqlite::parameters()).await.unwrap().unwrap();
    sqlite::rollback(tx).await.unwrap();
    s
}
struct Reply {
    status: u16,
    cookie: bool,
    token: bool,
    body_empty: bool,
    no_store: bool,
}
async fn request(address: std::net::SocketAddr, method: &str) -> Reply {
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut socket = TcpStream::connect(address).await.unwrap();
        let input =
            format!("{method} /bootstrap HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
        socket.write_all(input.as_bytes()).await.unwrap();
        let mut raw = Vec::new();
        socket.take(4097).read_to_end(&mut raw).await.unwrap();
        assert!(raw.len() <= 4096);
        let split = raw.windows(4).position(|v| v == b"\r\n\r\n").unwrap();
        let header = std::str::from_utf8(&raw[..split]).unwrap();
        let mut lines = header.split("\r\n");
        let status = lines
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let mut cookie = false;
        let mut no_store = false;
        for line in lines {
            let (name, value) = line.split_once(':').unwrap();
            cookie |= name.eq_ignore_ascii_case("set-cookie");
            no_store |= name.eq_ignore_ascii_case("cache-control") && value.trim() == "no-store";
        }
        let body = &raw[split + 4..];
        let token = if method == "GET" && status == 200 {
            let value: nagi_runtime::serde_json::Value =
                nagi_runtime::serde_json::from_slice(body).unwrap();
            value.as_object().is_some_and(|v| {
                v.len() == 1
                    && v.get("csrf_token")
                        .and_then(|v| v.as_str())
                        .is_some_and(|v| v.len() == 43)
            })
        } else {
            body.windows(10).any(|v| v == b"csrf_token")
        };
        Reply {
            status,
            cookie,
            token,
            body_empty: body.is_empty(),
            no_store,
        }
    })
    .await
    .unwrap()
}
async fn serve(
    pool: prelogin::PreLoginPool,
    options: http::Options,
) -> (
    std::net::SocketAddr,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<Result<(), Error>>,
) {
    let app = http::prelogin_bootstrap(http::app_default(()), "/bootstrap", pool).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, until) = oneshot::channel();
    let task = tokio::spawn(http::serve_listener(listener, app, options, async {
        let _ = until.await;
    }));
    (address, stop, task)
}
async fn stop_server(stop: oneshot::Sender<()>, task: tokio::task::JoinHandle<Result<(), Error>>) {
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[test]
fn external_checked_types_and_failure_contract() {
    fn shared<T: Send + Sync>() {}
    shared::<prelogin::Options>();
    shared::<prelogin::CookieOptions>();
    shared::<prelogin::PreLoginPool>();
    let failure = prelogin::options(0, 3, 144, 1, 10000, 2, 8, 10000, 4, 16)
        .err()
        .unwrap();
    assert!(prelogin::kind(&failure) == FailureKind::InvalidRequest);
    assert!(prelogin::outcome(&failure) == sqlite::Outcome::NotApplicable);
    assert!(!prelogin::message(&failure).is_empty());
    assert!(!format!("{failure:?}").is_empty());
    assert!(!format!("{:?}", configuration()).is_empty());
    assert!(!format!("{:?}", cookie()).is_empty());
    assert!(prelogin::cookie_options("__Host-prelogin", session::SameSite::None).is_err());
}
#[tokio::test]
async fn unsealed_authority_refuses_before_reserved_ddl_and_auth_stays_open() {
    let f = Fixture::new().await;
    let failure = f
        .open(&http::options(64, 1000, 1000, 1000).unwrap())
        .await
        .err()
        .unwrap();
    assert!(prelogin::kind(&failure) == FailureKind::InvalidRequest);
    assert!(prelogin::outcome(&failure) == sqlite::Outcome::NotApplicable);
    assert!(
        number(
            &f.auth,
            "SELECT count(*) AS n FROM sqlite_schema WHERE name GLOB '__nagi_prelogin_*'"
        )
        .await
            == 0
    );
    // The original manager can still obtain its single native connection.
    assert!(number(&f.auth, "SELECT count(*) AS n FROM __nagi_session_meta").await == 1);
    let pool = f.open(&checked("https://localhost")).await.unwrap();
    prelogin::close(&pool, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn borrowed_checked_options_move_to_serve_get_head_and_native_close_aliases() {
    let f = Fixture::new().await;
    let options = checked("https://localhost:443");
    let pool = f.open(&options).await.unwrap();
    let alias = prelogin::clone_pool(&pool);
    assert!(!format!("{alias:?}").is_empty());
    // Borrow ends at open; the same owned, non-Clone options move to serving.
    let (address, stop, task) = serve(prelogin::clone_pool(&pool), options).await;
    let head = request(address, "HEAD").await;
    assert!(head.status == 200 && !head.cookie && !head.token && head.body_empty && head.no_store);
    assert!(number(&f.auth, "SELECT count(*) AS n FROM __nagi_prelogin_ready").await == 0);
    let get = request(address, "GET").await;
    assert!(get.status == 200 && get.cookie && get.token && get.no_store);
    assert!(number(&f.auth, "SELECT count(*) AS n FROM __nagi_prelogin_ready").await == 1);
    stop_server(stop, task).await;
    prelogin::close(&pool, 2000).await.unwrap();
    drop(pool);
    // Shared alias is stopped after actual native close/join; no fresh delivery.
    let (address, stop, task) =
        serve(prelogin::clone_pool(&alias), checked("https://localhost")).await;
    let closed = request(address, "GET").await;
    assert!(closed.status == 503 && !closed.cookie && !closed.token && closed.no_store);
    stop_server(stop, task).await;
    prelogin::close(&alias, 2000).await.unwrap();
    assert!(number(&f.auth, "SELECT count(*) AS n FROM __nagi_session_meta").await == 1);
    f.stop().await;
}
#[tokio::test]
async fn mismatched_serve_rejects_before_bind_or_accept() {
    let f = Fixture::new().await;
    let pool = f.open(&checked("https://localhost")).await.unwrap();
    for origin in ["https://other.invalid", "https://localhost:444"] {
        // An occupied port distinguishes startup rejection from a bind error.
        let occupied = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = occupied.local_addr().unwrap().port();
        let app = http::prelogin_bootstrap(
            http::app_default(()),
            "/bootstrap",
            prelogin::clone_pool(&pool),
        )
        .unwrap();
        let error = http::serve(app, i64::from(port), checked(origin))
            .await
            .err()
            .unwrap();
        assert!(matches!(error.kind, nagi_runtime::ErrorKind::Invalid));
        assert!(error.message == "pre-login origin does not match checked startup authority");
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mut pending = TcpStream::connect(address).await.unwrap();
        pending
            .write_all(b"GET /bootstrap HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let app = http::prelogin_bootstrap(
            http::app_default(()),
            "/bootstrap",
            prelogin::clone_pool(&pool),
        )
        .unwrap();
        let error = http::serve_listener(listener, app, checked(origin), async {})
            .await
            .err()
            .unwrap();
        assert!(matches!(error.kind, nagi_runtime::ErrorKind::Invalid));
        assert!(error.message == "pre-login origin does not match checked startup authority");
        let mut raw = Vec::new();
        let _ = tokio::time::timeout(
            Duration::from_secs(1),
            pending.take(128).read_to_end(&mut raw),
        )
        .await
        .unwrap();
        assert!(raw.is_empty());
    }
    assert!(number(&f.auth, "SELECT count(*) AS n FROM __nagi_prelogin_ready").await == 0);
    prelogin::close(&pool, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn reopen_mismatch_preserves_meta_ready_and_same_origin_original_expiry() {
    let f = Fixture::new().await;
    let pool = f.open(&checked("https://localhost")).await.unwrap();
    let (address, stop, task) =
        serve(prelogin::clone_pool(&pool), checked("https://localhost")).await;
    assert!(request(address, "GET").await.status == 200);
    stop_server(stop, task).await;
    prelogin::close(&pool, 2000).await.unwrap();
    let before = snapshot(&f.auth).await;
    for origin in ["https://other.invalid", "https://localhost:444"] {
        let failure = f.open(&checked(origin)).await.err().unwrap();
        assert!(prelogin::kind(&failure) == FailureKind::Unavailable);
        let after = snapshot(&f.auth).await;
        assert!(
            before.config == after.config
                && before.incarnation == after.incarnation
                && before.last_wall == after.last_wall
                && before.issued == after.issued
                && before.expiry == after.expiry
        );
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
    let reopened = f.open(&checked("https://localhost:443")).await.unwrap();
    let after = snapshot(&f.auth).await;
    assert!(
        before.expiry == after.expiry
            && before.incarnation == after.incarnation
            && before.config == after.config
            && before.issued == after.issued
    );
    prelogin::close(&reopened, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn bootstrap_registration_is_closed_and_rejects_get_head_duplicates() {
    let f = Fixture::new().await;
    let pool = f.open(&checked("https://localhost")).await.unwrap();
    let app = http::prelogin_bootstrap(
        http::app_default(()),
        "/bootstrap",
        prelogin::clone_pool(&pool),
    )
    .unwrap();
    assert!(http::prelogin_bootstrap(app, "/bootstrap", prelogin::clone_pool(&pool)).is_err());
    assert!(http::prelogin_bootstrap(
        http::app_default(()),
        "/bad?query",
        prelogin::clone_pool(&pool)
    )
    .is_err());
    prelogin::close(&pool, 2000).await.unwrap();
    f.stop().await;
}

#[tokio::test]
async fn native_partial_ddl_failure_rolls_back_and_does_not_close_auth_manager() {
    let f = Fixture::new().await;
    // SQLite names are case-insensitive, while the exact reserved-schema GLOB
    // selects canonical lowercase names. This owned collision fails ROWS DDL
    // after META DDL, exercising the existing rollback/failed-fork close path.
    let tx = sqlite::begin(&f.auth, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    sqlite::exec(
        &tx,
        sqlite::literal("CREATE TABLE __NAGI_PRELOGIN_READY(n INTEGER)"),
        sqlite::parameters(),
    )
    .await
    .unwrap();
    sqlite::commit(tx).await.unwrap();
    let failure = f.open(&checked("https://localhost")).await.err().unwrap();
    assert!(prelogin::kind(&failure) == FailureKind::Unavailable);
    // Preserve the original exec failure's Active outcome; this accessor does
    // not rewrite it to infer a rollback guarantee from the later cleanup.
    assert!(prelogin::outcome(&failure) == sqlite::Outcome::Active);
    assert!(
        number(
            &f.auth,
            "SELECT count(*) AS n FROM sqlite_schema WHERE name='__nagi_prelogin_meta'"
        )
        .await
            == 0
    );
    assert!(number(&f.auth, "SELECT count(*) AS n FROM __nagi_session_meta").await == 1);
    let tx = sqlite::begin(&f.auth, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    sqlite::exec(
        &tx,
        sqlite::literal("DROP TABLE __NAGI_PRELOGIN_READY"),
        sqlite::parameters(),
    )
    .await
    .unwrap();
    sqlite::commit(tx).await.unwrap();
    let pool = f.open(&checked("https://localhost")).await.unwrap();
    prelogin::close(&pool, 2000).await.unwrap();
    f.stop().await;
}

#[tokio::test]
async fn configuration_mismatch_and_auth_cookie_collision_preserve_existing_metadata() {
    let f = Fixture::new().await;
    let options = checked("https://localhost");
    let pool = f.open(&options).await.unwrap();
    let (address, stop, task) =
        serve(prelogin::clone_pool(&pool), checked("https://localhost")).await;
    assert!(request(address, "GET").await.status == 200);
    stop_server(stop, task).await;
    prelogin::close(&pool, 2000).await.unwrap();
    let before = snapshot(&f.auth).await;
    let failure = prelogin::open(
        &f.store,
        sqlite::options(1, 2, 1000, 1000).unwrap(),
        prelogin::options(2, 3, 144, 1, 9000, 2, 8, 10000, 4, 16).unwrap(),
        cookie(),
        &options,
    )
    .await
    .err()
    .unwrap();
    assert!(prelogin::kind(&failure) == FailureKind::Unavailable);
    let failure = prelogin::open(
        &f.store,
        sqlite::options(1, 2, 1000, 1000).unwrap(),
        configuration(),
        prelogin::cookie_options("__Host-auth", session::SameSite::Strict).unwrap(),
        &options,
    )
    .await
    .err()
    .unwrap();
    assert!(prelogin::kind(&failure) == FailureKind::InvalidRequest);
    let after = snapshot(&f.auth).await;
    assert!(
        before.config == after.config
            && before.incarnation == after.incarnation
            && before.last_wall == after.last_wall
            && before.issued == after.issued
            && before.expiry == after.expiry
    );
    f.stop().await;
}
