//! HTTP rate/Session ordering on an owned SQLite file and real native socket.
//! Fixed closed assertions only: no raw credential/response diagnostics.
use super::*;
use crate::{
    auth::{self, session, AuthScope, Grant, LeaseOwner},
    http_server as http, sqlite, Error,
};
use session::{Clock, ClockSample, Foundation};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Fixture {
    directory: PathBuf,
    pool: sqlite::Pool,
    store: Store,
    wall: Arc<AtomicI64>,
}
impl Fixture {
    async fn new() -> Self {
        let directory = loop {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("nagi-sf07-http-rate-{}-{id}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => break path,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => panic!("owned rate fixture directory unavailable"),
            }
        };
        let pool = sqlite::open(
            directory.join("owned.sqlite").to_str().unwrap(),
            sqlite::options(2, 4, 1000, 1000).unwrap(),
        )
        .await
        .unwrap();
        let wall = Arc::new(AtomicI64::new(1000));
        let clock_wall = Arc::clone(&wall);
        let clock: Clock = Arc::new(move || {
            let now = Instant::now();
            let wall = clock_wall.load(Ordering::SeqCst);
            Ok(ClockSample {
                monotonic: now,
                monotonic_after: now,
                floor_ms: wall,
                ceil_ms: wall,
            })
        });
        let foundation = Foundation::initialize(
            &pool,
            session::options(4, 8, 72, 1, 10000, 100000, 5000, 2).unwrap(),
            "__Host-rate",
            1,
            [31; 32],
            clock,
        )
        .await
        .unwrap();
        Self {
            directory,
            pool,
            store: Store {
                foundation: Arc::new(foundation),
            },
            wall,
        }
    }
    async fn seed(&self, subject: i64, byte: u8) -> String {
        let id = session::secrets::SessionId::test_from_entropy([byte; 32]);
        self.store
            .foundation
            .insert(
                subject,
                id.digest().into_bytes(),
                Instant::now() + Duration::from_secs(90),
            )
            .await
            .unwrap();
        let wire = session::secrets::CookiePolicy::new("__Host-rate", cookie::SameSite::Strict)
            .unwrap_or_else(|_| panic!("fixed Cookie policy invalid"))
            .issue(id)
            .unwrap_or_else(|_| panic!("fixed Cookie serialization failed"))
            .into_header();
        wire.to_str()
            .unwrap_or_else(|_| panic!("fixed Cookie encoding invalid"))
            .split(';')
            .next()
            .unwrap()
            .to_owned()
    }
    async fn rows(&self) -> Vec<(i64, i64, i64, i64)> {
        let tx = sqlite::begin(&self.pool, sqlite::BeginMode::Immediate)
            .await
            .unwrap();
        let rows: Vec<session::SessionRow> = sqlite::all(
            &tx,
            sqlite::literal("SELECT lineage,subject,digest,generation,idle_ms,absolute_ms FROM __nagi_session_rows ORDER BY lineage"),
            sqlite::parameters(),
        )
        .await
        .unwrap();
        sqlite::rollback(tx).await.unwrap();
        rows.into_iter()
            .map(|row| (row.subject, row.generation, row.idle_ms, row.absolute_ms))
            .collect()
    }
    async fn stop(self) {
        drop(self.store);
        sqlite::close(&self.pool, 2000).await.unwrap();
        std::fs::remove_dir_all(self.directory).unwrap();
    }
}
struct Native {
    address: std::net::SocketAddr,
    stop: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<Result<(), Error>>,
}
impl Native {
    async fn start<S: Send + Sync + 'static>(app: http::App<S, Error>, rules: i64) -> Self {
        // Explicit new-fixture inputs. These do not alter any existing deadline,
        // native parser capacity, runtime default or application quota.
        let options = http::capacity(http::options(256, 1000, 1000, 1000).unwrap(), 4, 4).unwrap();
        let options = http::with_response_limits(
            options,
            http::response_limits(
                256,
                65536,
                256,
                4096,
                32,
                131072,
                4,
                8 * 256 + 16 * 1024 * 1024,
                4,
                4 * 262144 + 16 * 1024 * 1024,
                4096,
                32,
            )
            .unwrap(),
        )
        .unwrap();
        let options = http::with_rate_limits(
            options,
            http::rate_limits(rules, rules, rules, 4 * 1024 * 1024, 10000).unwrap(),
        )
        .unwrap();
        let options = http::authority(
            options,
            "https://localhost",
            vec!["localhost".into()],
            1,
            128,
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(http::serve_listener(listener, app, options, async {
            let _ = stopped.await;
        }));
        Self {
            address,
            stop,
            task,
        }
    }
    async fn request(&self, path: &str, credential: &str) -> (u16, usize, bool) {
        self.exchange("GET", path, credential).await
    }
    async fn exchange(&self, method: &str, path: &str, credential: &str) -> (u16, usize, bool) {
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\n{credential}Content-Length: 0\r\nConnection: close\r\n\r\n"
        );
        self.raw(&request).await
    }
    async fn raw(&self, request: &str) -> (u16, usize, bool) {
        let mut socket = TcpStream::connect(self.address).await.unwrap();
        socket.write_all(request.as_bytes()).await.unwrap();
        let mut raw = Vec::new();
        tokio::time::timeout(
            Duration::from_secs(3),
            (&mut socket).take(8193).read_to_end(&mut raw),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(raw.len() <= 8192);
        let text = std::str::from_utf8(&raw)
            .unwrap_or_else(|_| panic!("fixed native response encoding invalid"));
        let status = text
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse().ok())
            .expect("fixed HTTP status missing");
        let cookies = text
            .lines()
            .filter(|line| line.to_ascii_lowercase().starts_with("set-cookie:"))
            .count();
        let retry = text
            .lines()
            .any(|line| line.to_ascii_lowercase().starts_with("retry-after:"));
        (status, cookies, retry)
    }
    async fn stop(self) {
        self.stop.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), self.task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}
fn policy() -> http::RatePolicy {
    http::rate_subject(http::rate_policy(64, 60000).unwrap(), 1, 60000, 1, 60000).unwrap()
}

#[tokio::test]
async fn http_subject_denial_and_new_subject_capacity_do_not_refresh_session_idle() {
    let fixture = Fixture::new().await;
    let first = fixture.seed(7, 7).await;
    let second = fixture.seed(8, 8).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let handler_calls = Arc::clone(&calls);
    let app = http::route(
        http::app_default(()),
        hyper::Method::GET,
        "/owned",
        http::session_authenticated_policy(session::clone_store(&fixture.store)),
        move |request: http::Request, _, _: AuthScope| {
            let calls = Arc::clone(&handler_calls);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                http::empty(http::response_context(&request), http::Status::OK)
            }
        },
    )
    .unwrap();
    let app = http::rate_route(app, hyper::Method::GET, "/owned", policy()).unwrap();
    let native = Native::start(app, 2).await;
    fixture.wall.store(7000, Ordering::SeqCst);
    assert_eq!(
        native
            .request("/owned", &format!("Cookie: {first}\r\n"))
            .await,
        (200, 0, false)
    );
    let admitted = fixture.rows().await;
    assert_eq!(admitted[0].2, 17000);
    assert_eq!(admitted[1].2, 11000);
    assert_eq!(
        native
            .request("/owned", &format!("Cookie: {second}\r\n"))
            .await,
        (503, 0, false)
    );
    assert_eq!(fixture.rows().await, admitted);
    fixture.wall.store(13000, Ordering::SeqCst);
    assert_eq!(
        native
            .request("/owned", &format!("Cookie: {first}\r\n"))
            .await,
        (429, 0, true)
    );
    assert_eq!(
        fixture.rows().await.into_iter().find(|row| row.0 == 7),
        Some(admitted[0])
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    native.stop().await;
    fixture.stop().await;
}

struct Early;
struct EarlyState {
    store: Store,
    authorizers: Arc<AtomicUsize>,
    handlers: Arc<AtomicUsize>,
}
async fn early_authorizer(
    scope: AuthScope,
    state: Arc<EarlyState>,
    operation: u8,
) -> Result<Grant<Early>, auth::Failure> {
    state.authorizers.fetch_add(1, Ordering::SeqCst);
    let lease = Arc::clone(&scope.lease);
    assert_eq!(
        session::delivery::test_slot_status(&lease),
        Some((true, false, false, false))
    );
    let result = match operation {
        0 => session::issue(&state.store, scope).await,
        1 => session::rotate(&state.store, scope).await,
        2 => session::logout(&state.store, scope).await,
        _ => panic!("fixed operation invalid"),
    };
    assert_eq!(
        session::kind(&result.err().expect("early mutation must fail")),
        auth::FailureKind::Denied
    );
    assert_eq!(
        session::delivery::test_slot_status(&lease),
        Some((true, false, false, false))
    );
    Err(auth::Failure::denied())
}
async fn early_handler(
    request: http::Request,
    state: Arc<EarlyState>,
    _: Grant<Early>,
) -> Result<http::Response, Error> {
    state.handlers.fetch_add(1, Ordering::SeqCst);
    http::empty(http::response_context(&request), http::Status::OK)
}
#[tokio::test]
async fn http_authorizers_cannot_stage_issue_rotate_or_logout_before_body_subject_admission() {
    let fixture = Fixture::new().await;
    let cookie = fixture.seed(7, 7).await;
    let before = fixture.rows().await;
    let authorizers = Arc::new(AtomicUsize::new(0));
    let handlers = Arc::new(AtomicUsize::new(0));
    let app = http::app_default(EarlyState {
        store: session::clone_store(&fixture.store),
        authorizers: Arc::clone(&authorizers),
        handlers: Arc::clone(&handlers),
    });
    let app = http::route(
        app,
        hyper::Method::GET,
        "/issue",
        http::authorized_policy(
            |_, _| async {
                auth::VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(30))
            },
            |scope, _, state| early_authorizer(scope, state, 0),
        ),
        early_handler,
    )
    .unwrap();
    let app = http::route(
        app,
        hyper::Method::GET,
        "/rotate",
        http::session_authorized_policy(session::clone_store(&fixture.store), |scope, _, state| {
            early_authorizer(scope, state, 1)
        }),
        early_handler,
    )
    .unwrap();
    let app = http::route(
        app,
        hyper::Method::GET,
        "/logout",
        http::session_authorized_policy(session::clone_store(&fixture.store), |scope, _, state| {
            early_authorizer(scope, state, 2)
        }),
        early_handler,
    )
    .unwrap();
    let app = http::rate_route(app, hyper::Method::GET, "/issue", policy()).unwrap();
    let app = http::rate_route(app, hyper::Method::GET, "/rotate", policy()).unwrap();
    let app = http::rate_route(app, hyper::Method::GET, "/logout", policy()).unwrap();
    let native = Native::start(app, 6).await;
    assert_eq!(
        native
            .request("/issue", "Authorization: Bearer fixed-test\r\n")
            .await,
        (403, 0, false)
    );
    assert_eq!(fixture.rows().await, before);
    for path in ["/rotate", "/logout"] {
        assert_eq!(
            native.request(path, &format!("Cookie: {cookie}\r\n")).await,
            (403, 0, false)
        );
        assert_eq!(fixture.rows().await, before);
    }
    assert_eq!(authorizers.load(Ordering::SeqCst), 3);
    assert_eq!(handlers.load(Ordering::SeqCst), 0);
    native.stop().await;
    fixture.stop().await;
}

#[tokio::test]
async fn delayed_deferred_touch_cannot_admit_a_retired_http_lease() {
    let fixture = Fixture::new().await;
    let cookie = fixture.seed(7, 7).await;
    fixture.wall.store(7000, Ordering::SeqCst);
    let mut headers = HeaderMap::new();
    headers.insert(
        COOKIE,
        hyper::header::HeaderValue::from_str(&cookie).unwrap(),
    );
    let identity = fixture.store.lookup_http_identity(&headers).await.unwrap();
    let owner = LeaseOwner::new_http(Instant::now() + Duration::from_secs(10)).unwrap();
    let _scope = AuthScope::bind(identity, Arc::clone(&owner.lease)).unwrap();
    let lease = Arc::clone(&owner.lease);
    let before = fixture.rows().await;
    let held = sqlite::begin(&fixture.pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let attempt = lease.finish_http_auth();
    tokio::pin!(attempt);
    assert!(futures_util::poll!(attempt.as_mut()).is_pending());
    drop(owner);
    sqlite::rollback(held).await.unwrap();
    assert_eq!(
        attempt.await.err().unwrap().kind(),
        auth::FailureKind::Expired
    );
    let gate = lease.gate.lock().unwrap();
    assert!(!gate.active);
    assert_eq!(gate.http_admitted, Some(false));
    drop(gate);
    assert_eq!(fixture.rows().await, before);
    fixture.stop().await;
}

#[tokio::test]
async fn existing_non_http_admission_and_admitted_http_session_delivery_remain_live() {
    let fixture = Fixture::new().await;
    let owner = LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap();
    let scope = AuthScope::bind(
        auth::VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(30)).unwrap(),
        Arc::clone(&owner.lease),
    )
    .unwrap();
    let intent = session::issue(&fixture.store, scope).await.unwrap();
    assert_eq!(fixture.rows().await.len(), 1);
    drop(intent);
    let rebound = AuthScope::bind(
        auth::VerifiedIdentity::from_verified(9, Instant::now() + Duration::from_secs(30)).unwrap(),
        Arc::clone(&owner.lease),
    )
    .unwrap();
    assert_eq!(rebound.subject(), 9);
    drop(rebound);
    drop(owner);
    let owner = LeaseOwner::new_http(Instant::now() + Duration::from_secs(10)).unwrap();
    let scope = AuthScope::bind(
        auth::VerifiedIdentity::from_verified(8, Instant::now() + Duration::from_secs(30)).unwrap(),
        Arc::clone(&owner.lease),
    )
    .unwrap();
    owner.lease.finish_http_auth().await.unwrap();
    assert_eq!(
        AuthScope::bind(
            auth::VerifiedIdentity::from_verified(9, Instant::now() + Duration::from_secs(30))
                .unwrap(),
            Arc::clone(&owner.lease),
        )
        .err()
        .expect("HTTP subject cannot be rebound")
        .kind(),
        auth::FailureKind::InvalidRequest
    );
    assert_eq!(scope.subject(), 8);
    let intent = session::issue(&fixture.store, scope).await.unwrap();
    let rows = fixture.rows().await;
    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0].0, rows[1].0), (7, 8));
    drop(intent);
    drop(owner);
    fixture.stop().await;
}

#[tokio::test]
async fn admitted_http_subject_can_issue_once_and_limited_subject_cannot_issue_again() {
    struct State {
        store: Store,
        calls: Arc<AtomicUsize>,
    }
    let fixture = Fixture::new().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let app = http::route(
        http::app_default(State {
            store: session::clone_store(&fixture.store),
            calls: Arc::clone(&calls),
        }),
        hyper::Method::GET,
        "/issue",
        http::authenticated_policy(|_, _| async {
            auth::VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(30))
        }),
        |request: http::Request, state: Arc<State>, scope: AuthScope| async move {
            state.calls.fetch_add(1, Ordering::SeqCst);
            let intent = session::issue(&state.store, scope)
                .await
                .map_err(|_| Error::internal("fixed Session issue failed"))?;
            let response = http::empty(http::response_context(&request), http::Status::OK)?;
            session::apply(response, intent)
                .map_err(|_| Error::internal("fixed Session application failed"))
        },
    )
    .unwrap();
    let app = http::rate_route(app, hyper::Method::GET, "/issue", policy()).unwrap();
    let native = Native::start(app, 2).await;
    assert_eq!(
        native
            .request("/issue", "Authorization: Bearer fixed-test\r\n")
            .await,
        (200, 1, false)
    );
    let admitted = fixture.rows().await;
    assert_eq!(admitted.len(), 1);
    assert_eq!(
        native
            .request("/issue", "Authorization: Bearer fixed-test\r\n")
            .await,
        (429, 0, true)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.rows().await, admitted);
    native.stop().await;
    fixture.stop().await;
}

#[tokio::test]
async fn cors_preflight_uses_finite_entry_without_charging_either_rate_bucket() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler_calls = Arc::clone(&calls);
    let verifiers = Arc::new(AtomicUsize::new(0));
    let verifier_calls = Arc::clone(&verifiers);
    let app = http::route(
        http::app_default(()),
        hyper::Method::GET,
        "/public",
        http::authenticated_policy(move |_, _| {
            let verifiers = Arc::clone(&verifier_calls);
            async move {
                verifiers.fetch_add(1, Ordering::SeqCst);
                auth::VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(30))
            }
        }),
        move |request: http::Request, _, _: AuthScope| {
            let calls = Arc::clone(&handler_calls);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                http::empty(http::response_context(&request), http::Status::OK)
            }
        },
    )
    .unwrap();
    let cors = http::cors_policy(
        vec!["https://client.example".into()],
        Vec::new(),
        http::CorsCredentials::Omit,
        0,
        1,
        128,
        1,
        128,
    )
    .unwrap();
    let app = http::cors_route(app, hyper::Method::GET, "/public", cors).unwrap();
    let app = http::rate_route(
        app,
        hyper::Method::GET,
        "/public",
        http::rate_subject(http::rate_policy(1, 60000).unwrap(), 1, 60000, 1, 60000).unwrap(),
    )
    .unwrap();
    let native = Native::start(app, 2).await;
    assert_eq!(
        native
            .exchange(
                "OPTIONS",
                "/public",
                "Origin: https://client.example\r\nAccess-Control-Request-Method: GET\r\n"
            )
            .await,
        (204, 0, false)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(verifiers.load(Ordering::SeqCst), 0);
    let bearer = "Authorization: Bearer fixed-test\r\n";
    assert_eq!(native.request("/public", bearer).await, (200, 0, false));
    assert_eq!(native.request("/public", bearer).await, (429, 0, true));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(verifiers.load(Ordering::SeqCst), 1);
    native.stop().await;
}

#[tokio::test]
async fn body_refusal_after_auth_does_not_charge_subject_or_admit_session_work() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler_calls = Arc::clone(&calls);
    let verifiers = Arc::new(AtomicUsize::new(0));
    let verifier_calls = Arc::clone(&verifiers);
    let app = http::route(
        http::app_default(()),
        hyper::Method::POST,
        "/body",
        http::authenticated_policy(move |_, _| {
            let verifiers = Arc::clone(&verifier_calls);
            async move {
                verifiers.fetch_add(1, Ordering::SeqCst);
                auth::VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(30))
            }
        }),
        move |request: http::Request, _, _: AuthScope| {
            let calls = Arc::clone(&handler_calls);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                http::empty(http::response_context(&request), http::Status::OK)
            }
        },
    )
    .unwrap();
    let app = http::rate_route(app, hyper::Method::POST, "/body", policy()).unwrap();
    let native = Native::start(app, 2).await;
    let raw = format!("POST /body HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer fixed-test\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n101\r\n{}\r\n0\r\n\r\n", "x".repeat(257));
    assert_eq!(native.raw(&raw).await, (413, 0, false));
    assert_eq!(verifiers.load(Ordering::SeqCst), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        native
            .exchange("POST", "/body", "Authorization: Bearer fixed-test\r\n")
            .await,
        (200, 0, false)
    );
    assert_eq!(
        native
            .exchange("POST", "/body", "Authorization: Bearer fixed-test\r\n")
            .await,
        (429, 0, true)
    );
    assert_eq!(verifiers.load(Ordering::SeqCst), 3);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    native.stop().await;
}
