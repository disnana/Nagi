//! Finite real SQLite/socket one-use delivery; generated cookie bytes stay in RAM.
use super::super::{self as session, Store};
use crate::{
    auth::{AuthScope, VerifiedIdentity},
    http_server::*,
    sqlite, Error,
};
use axum::http::Method;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

fn with_test_response_limits(options: Options) -> Options {
    let limits = response_limits(
        4096,
        65_536,
        256,
        4096,
        32,
        131_072,
        256,
        16_777_216 + 32_768,
        1024,
        285_212_672,
        4096,
        32,
    )
    .unwrap();
    with_response_limits(options, limits).unwrap()
}

fn response_text(request: &Request, status: Status, body: &str) -> Result<Response, Error> {
    text(response_context(request), status, body)
}

fn response_empty(request: &Request, status: Status) -> Result<Response, Error> {
    empty(response_context(request), status)
}

fn fixture_response(status: Status, body: &str) -> Response {
    let request = crate::http_server::bounded_response_request_fixture(body.len());
    text(response_context(&request), status, body).expect("bounded test response fits fixture")
}

struct Server {
    address: std::net::SocketAddr,
    stopped: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<Result<(), Error>>,
}
impl Server {
    async fn new<S, E>(app: App<S, E>, options: Options) -> Self
    where
        S: Send + Sync + 'static,
        E: 'static,
    {
        // Existing lifecycle fixtures explicitly accept their localhost Host;
        // public serve/serve_listener do not supply an authority default.
        let options = authority(
            with_test_response_limits(options),
            "https://localhost",
            vec!["localhost".to_owned()],
            1,
            128,
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stopped, shutdown) = oneshot::channel();
        let task = tokio::spawn(serve_listener(listener, app, options, async {
            let _ = shutdown.await;
        }));
        Self {
            address,
            stopped: Some(stopped),
            task,
        }
    }
    async fn connect(&self) -> BufReader<TcpStream> {
        BufReader::new(TcpStream::connect(self.address).await.unwrap())
    }
    async fn stop(mut self) {
        self.stopped.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), &mut self.task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(stopped) = self.stopped.take() {
            let _ = stopped.send(());
        }
        self.task.abort();
    }
}

struct Received {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}
impl Received {
    fn all(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .collect()
    }
}
async fn send(socket: &mut BufReader<TcpStream>, data: &[u8]) {
    socket.get_mut().write_all(data).await.unwrap();
}
async fn response(socket: &mut BufReader<TcpStream>, head: bool) -> Received {
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut line = String::new();
        socket.read_line(&mut line).await.unwrap();
        let status: u16 = line
            .split_whitespace()
            .nth(1)
            .unwrap_or_else(|| panic!("missing response status: {line:?}"))
            .parse()
            .unwrap();
        let mut headers = Vec::new();
        let mut length = 0;
        let mut chunked = false;
        loop {
            line.clear();
            assert_ne!(socket.read_line(&mut line).await.unwrap(), 0);
            if line == "\r\n" {
                break;
            }
            let (name, value) = line.split_once(':').unwrap();
            let value = value.trim().to_owned();
            if name.eq_ignore_ascii_case("content-length") {
                length = value.parse().unwrap();
            }
            if name.eq_ignore_ascii_case("transfer-encoding") {
                chunked = value == "chunked";
            }
            headers.push((name.to_owned(), value));
        }
        let mut body = Vec::new();
        if !head && !matches!(status, 204 | 205 | 304) {
            if chunked {
                loop {
                    line.clear();
                    socket.read_line(&mut line).await.unwrap();
                    let length = usize::from_str_radix(line.trim(), 16).unwrap();
                    if length == 0 {
                        socket.read_line(&mut line).await.unwrap();
                        break;
                    }
                    let start = body.len();
                    body.resize(start + length, 0);
                    socket.read_exact(&mut body[start..]).await.unwrap();
                    let mut end = [0; 2];
                    socket.read_exact(&mut end).await.unwrap();
                    assert_eq!(&end, b"\r\n");
                }
            } else {
                body.resize(length, 0);
                socket.read_exact(&mut body).await.unwrap();
            }
        }
        Received {
            status,
            headers,
            body,
        }
    })
    .await
    .unwrap()
}

static NEXT_DB: AtomicU64 = AtomicU64::new(1);
struct Fixture {
    directory: std::path::PathBuf,
    pool: sqlite::Pool,
    store: Store,
}
impl Fixture {
    async fn new() -> Self {
        let directory = loop {
            let n = NEXT_DB.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("owned-wire-{}-{n}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => break path,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("owned directory: {e}"),
            }
        };
        let pool = sqlite::open(
            directory.join("owned.sqlite").to_str().unwrap(),
            sqlite::options(2, 4, 1000, 1000).unwrap(),
        )
        .await
        .unwrap();
        let store = session::open(
            &pool,
            session::options(4, 5, 72, 1, 10000, 100000, 5000, 2).unwrap(),
            session::cookie_options("__Host-owned", session::SameSite::Strict).unwrap(),
        )
        .await
        .unwrap();
        Self {
            directory,
            pool,
            store,
        }
    }
    async fn count(&self) -> i64 {
        let tx = sqlite::begin(&self.pool, sqlite::BeginMode::Immediate)
            .await
            .unwrap();
        let row = sqlite::query::<session::Count>(
            &tx,
            sqlite::literal("SELECT count(*) AS n FROM __nagi_session_rows"),
            sqlite::parameters(),
        )
        .await
        .unwrap()
        .unwrap();
        sqlite::rollback(tx).await.unwrap();
        row.0
    }
    async fn stop(self) {
        drop(self.store);
        sqlite::close(&self.pool, 2000).await.unwrap();
        std::fs::remove_dir_all(self.directory).unwrap();
    }
}
async fn verifier(_: Request, _: Arc<Store>) -> Result<VerifiedIdentity, crate::auth::Failure> {
    VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(60))
}
async fn issue_handler(
    request: Request,
    store: Arc<Store>,
    scope: AuthScope,
) -> Result<Response, Error> {
    let intent = session::issue(&store, scope).await.unwrap();
    let response = response_text(&request, Status::OK, "owned")?;
    Ok(session::apply(response, intent).unwrap())
}

// Test-only observation of the four ordinary issue paths that have returned
// unexpected 504s on CI. Each request owns its observer; no global slot, extra
// query, await, deadline change, or credential/response bytes enter the record.
// These timestamps perturb scheduling and locate a boundary, not a root cause.
#[derive(Clone, Copy)]
enum IssueCase {
    Applied,
    NotModified,
    PrivateCache,
    Head,
}
struct IssueObservation {
    case: IssueCase,
    started: Instant,
    verifier_enter_us: AtomicU64,
    verifier_return_us: AtomicU64,
    handler_enter_us: AtomicU64,
    issue_return_us: AtomicU64,
    response_ready_us: AtomicU64,
    handler_drop_us: AtomicU64,
    issue_ok: AtomicBool,
}
impl IssueObservation {
    fn new(case: IssueCase) -> Arc<Self> {
        Arc::new(Self {
            case,
            started: Instant::now(),
            verifier_enter_us: AtomicU64::new(u64::MAX),
            verifier_return_us: AtomicU64::new(u64::MAX),
            handler_enter_us: AtomicU64::new(u64::MAX),
            issue_return_us: AtomicU64::new(u64::MAX),
            response_ready_us: AtomicU64::new(u64::MAX),
            handler_drop_us: AtomicU64::new(u64::MAX),
            issue_ok: AtomicBool::new(false),
        })
    }
    fn mark(&self, event: &AtomicU64) {
        // MAX means unseen; clamp finite elapsed values below that sentinel.
        let elapsed = self
            .started
            .elapsed()
            .as_micros()
            .min(u128::from(u64::MAX - 1));
        event.store(elapsed as u64, Ordering::SeqCst);
    }
}
impl std::fmt::Display for IssueObservation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let case = match self.case {
            IssueCase::Applied => "applied",
            IssueCase::NotModified => "not_modified",
            IssueCase::PrivateCache => "private_cache",
            IssueCase::Head => "head",
        };
        write!(
            f,
            "issue_observer case={case} test_only=true unseen_us={}",
            u64::MAX
        )?;
        for (label, value) in [
            ("verifier_enter_us", &self.verifier_enter_us),
            ("verifier_return_us", &self.verifier_return_us),
            ("handler_enter_us", &self.handler_enter_us),
            ("issue_return_us", &self.issue_return_us),
            ("response_ready_us", &self.response_ready_us),
            ("handler_drop_us", &self.handler_drop_us),
        ] {
            write!(f, " {label}={}", value.load(Ordering::SeqCst))?;
        }
        write!(f, " issue_ok={}", self.issue_ok.load(Ordering::SeqCst))
    }
}
struct IssueHandlerDrop(Arc<IssueObservation>);
impl Drop for IssueHandlerDrop {
    fn drop(&mut self) {
        self.0.mark(&self.0.handler_drop_us);
    }
}
fn observed_issue_app(
    f: &Fixture,
    method: Method,
    observation: &Arc<IssueObservation>,
) -> App<Store, Error> {
    let verified = Arc::clone(observation);
    let handled = Arc::clone(observation);
    route(
        app_default(session::clone_store(&f.store)),
        method,
        "/owned",
        authenticated_policy(move |request, store| {
            let observation = Arc::clone(&verified);
            async move {
                observation.mark(&observation.verifier_enter_us);
                let identity = verifier(request, store).await;
                observation.mark(&observation.verifier_return_us);
                identity
            }
        }),
        move |request, store, scope| {
            let observation = Arc::clone(&handled);
            async move {
                let _drop_observer = IssueHandlerDrop(Arc::clone(&observation));
                observation.mark(&observation.handler_enter_us);
                let intent = session::issue(&store, scope).await;
                observation.issue_ok.store(intent.is_ok(), Ordering::SeqCst);
                observation.mark(&observation.issue_return_us);
                let intent = intent.unwrap();
                let response = match observation.case {
                    IssueCase::NotModified => {
                        response_empty(&request, crate::http_server::status(304).unwrap())
                    }
                    _ => response_text(&request, Status::OK, "owned"),
                }?;
                let response = session::apply(response, intent).unwrap();
                let response = if matches!(observation.case, IssueCase::PrivateCache) {
                    crate::http_server::conflicting_cache_for_test(response)
                } else {
                    response
                };
                observation.mark(&observation.response_ready_us);
                Ok(response)
            }
        },
    )
    .unwrap()
}
async fn issue_server(f: &Fixture) -> Server {
    let app = route(
        app_default(session::clone_store(&f.store)),
        Method::POST,
        "/owned",
        authenticated_policy(verifier),
        issue_handler,
    )
    .unwrap();
    Server::new(app, options(64, 1000, 1000, 1000).unwrap()).await
}
async fn request(server: &Server, extra: &str) -> Received {
    let mut socket = server.connect().await;
    let raw =
        format!("POST /owned HTTP/1.1\r\nHost: localhost\r\n{extra}Content-Length: 0\r\n\r\n");
    send(&mut socket, raw.as_bytes()).await;
    response(&mut socket, false).await
}
fn assert_cookie_policy(reply: &Received) {
    assert_eq!(reply.all("set-cookie").len(), 1);
    let value = reply.all("set-cookie")[0];
    // Boolean policy checks only: never Debug or print actual secret value.
    let parsed = cookie::Cookie::parse(value).unwrap();
    assert!(parsed.name() == "__Host-owned");
    assert!(parsed.secure() == Some(true) && parsed.http_only() == Some(true));
    assert!(parsed.path() == Some("/") && parsed.domain().is_none());
    assert!(parsed.same_site() == Some(cookie::SameSite::Strict));
    assert!(parsed.max_age().is_none() && parsed.expires().is_none());
}
#[tokio::test]
async fn applied_real_issue_commit_has_exactly_one_wire_cookie() {
    let f = Fixture::new().await;
    let observation = IssueObservation::new(IssueCase::Applied);
    let app = observed_issue_app(&f, Method::POST, &observation);
    let server = Server::new(app, options(64, 1000, 1000, 1000).unwrap()).await;
    let reply = request(&server, "Authorization: Bearer owned\r\n").await;
    assert_eq!(reply.status, 200, "{observation}");
    assert_eq!(f.count().await, 1); // real SQL visibility before the cookie assertion
    assert_eq!(reply.all("cache-control"), vec!["no-store"]);
    assert_cookie_policy(&reply);
    server.stop().await;
    f.stop().await;
}

fn cookie_pair(reply: &Received) -> String {
    assert_eq!(reply.all("set-cookie").len(), 1);
    reply.all("set-cookie")[0]
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}
// These direct calls retain real public mutations and the real finalizer,
// while guarded Session HTTP mutation success remains pending B1.
async fn bound_scope(store: &Store, cookie: &str) -> (crate::auth::LeaseOwner, AuthScope) {
    let mut headers = hyper::header::HeaderMap::new();
    headers.insert(
        hyper::header::COOKIE,
        hyper::header::HeaderValue::from_str(cookie).unwrap(),
    );
    let identity = store.lookup_http_identity(&headers).await.unwrap();
    let owner = crate::auth::LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap();
    let scope = AuthScope::bind(identity, Arc::clone(&owner.lease)).unwrap();
    (owner, scope)
}
fn direct_delivery(response: Response, owner: &crate::auth::LeaseOwner) -> Received {
    let (status, headers) = crate::http_server::session_delivery_headers_for_test(response, owner);
    Received {
        status: status.as_u16(),
        headers: headers
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_str().unwrap().to_owned()))
            .collect(),
        body: Vec::new(),
    }
}
async fn lookup_rejects(store: &Store, cookie: &str) {
    let mut headers = hyper::header::HeaderMap::new();
    headers.insert(
        hyper::header::COOKIE,
        hyper::header::HeaderValue::from_str(cookie).unwrap(),
    );
    let result = store.lookup_http_identity(&headers).await;
    assert!(
        matches!(result,Err(error) if error.kind()==crate::auth::FailureKind::InvalidCredential)
    );
}
#[tokio::test]
async fn real_direct_rotate_logout_finalize_once_and_old_lookup_never_reactivates() {
    let f = Fixture::new().await;
    let server = issue_server(&f).await;
    let first = request(&server, "Authorization: Bearer owned\r\n").await;
    assert_cookie_policy(&first);
    assert_eq!(first.all("vary"), vec!["Authorization, Cookie"]);
    let old_cookie = cookie_pair(&first);
    server.stop().await;
    let (owner, scope) = bound_scope(&f.store, &old_cookie).await;
    let intent = session::rotate(&f.store, scope).await.unwrap();
    let response = session::apply(fixture_response(Status::OK, "owned"), intent).unwrap();
    let rotated = direct_delivery(response, &owner);
    assert_eq!(rotated.status, 200);
    assert_cookie_policy(&rotated);
    assert_eq!(rotated.all("vary"), vec!["Authorization, Cookie"]);
    let new_cookie = cookie_pair(&rotated);
    assert!(old_cookie != new_cookie);
    assert_eq!(f.count().await, 1);
    // Destructive take is the actual owner's finalizer, not a header clone
    // standing in for a second delivery. Already drained material cannot recur.
    let empty = direct_delivery(fixture_response(Status::OK, "owned"), &owner);
    assert!(empty.all("set-cookie").is_empty());
    drop(owner);
    lookup_rejects(&f.store, &old_cookie).await;
    let (owner, scope) = bound_scope(&f.store, &new_cookie).await;
    let intent = session::logout(&f.store, scope).await.unwrap();
    let logged_out = direct_delivery(
        session::apply(fixture_response(Status::OK, "owned"), intent).unwrap(),
        &owner,
    );
    assert_eq!(logged_out.status, 200);
    assert_eq!(logged_out.all("set-cookie").len(), 1);
    let deletion = cookie::Cookie::parse(logged_out.all("set-cookie")[0]).unwrap();
    assert!(
        deletion.value().is_empty() && deletion.max_age() == Some(cookie::time::Duration::ZERO)
    );
    assert!(deletion
        .expires_datetime()
        .is_some_and(|date| date <= cookie::time::OffsetDateTime::UNIX_EPOCH));
    assert!(
        deletion.secure() == Some(true)
            && deletion.http_only() == Some(true)
            && deletion.path() == Some("/")
            && deletion.domain().is_none()
    );
    assert_eq!(f.count().await, 0);
    let empty = direct_delivery(fixture_response(Status::OK, "owned"), &owner);
    assert!(empty.all("set-cookie").is_empty());
    drop(owner);
    lookup_rejects(&f.store, &new_cookie).await;
    f.stop().await;
}
#[tokio::test]
async fn real_commit_survives_dropped_intent_mapper_and_panic_without_cookie() {
    for (mode, status) in [(0, 200), (1, 400), (2, 500)] {
        let f = Fixture::new().await;
        let app = route(
            app_default(session::clone_store(&f.store)),
            Method::POST,
            "/owned",
            authenticated_policy(verifier),
            move |request, store, scope| async move {
                let intent = session::issue(&store, scope).await.unwrap();
                if mode == 0 {
                    drop(intent);
                    return response_text(&request, Status::OK, "owned");
                }
                let owned =
                    session::apply(response_text(&request, Status::OK, "owned")?, intent).unwrap();
                match mode {
                    1 => {
                        drop(owned);
                        Err(Error::invalid("owned mapper replacement"))
                    }
                    2 => {
                        drop(owned);
                        panic!("owned applied handler panic")
                    }
                    _ => unreachable!("normal completion fixture mode"),
                }
            },
        )
        .unwrap();
        // These are ordinary completed handlers, not the timeout fixture.
        // Preserve the public default handler deadline; test timeout separately
        // with commit observation and controlled Tokio time below.
        let server = Server::new(app, Options::default()).await;
        let reply = request(&server, "Authorization: Bearer owned\r\n").await;
        assert_eq!(reply.status, status);
        assert!(reply.all("set-cookie").is_empty());
        assert_eq!(reply.all("cache-control"), vec!["no-store"]);
        assert_eq!(f.count().await, 1);
        server.stop().await;
        f.stop().await;
    }
}

#[tokio::test]
async fn real_commit_then_controlled_handler_timeout_drops_future_without_cookie() {
    use std::sync::{atomic::AtomicUsize, Mutex};
    let f = Fixture::new().await;
    let (committed, received) = oneshot::channel();
    let committed = Arc::new(Mutex::new(Some(committed)));
    let future_dropped = Arc::new(AtomicUsize::new(0));
    let retained_lease = Arc::new(Mutex::new(None));
    struct ObservedDrop(Arc<AtomicUsize>);
    impl Drop for ObservedDrop {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let dropped = Arc::clone(&future_dropped);
    let observed = Arc::clone(&retained_lease);
    let app = route(
        app_default(session::clone_store(&f.store)),
        Method::POST,
        "/owned",
        authenticated_policy(verifier),
        move |request, store, scope| {
            let committed = Arc::clone(&committed);
            let dropped = Arc::clone(&dropped);
            let observed = Arc::clone(&observed);
            async move {
                *observed.lock().unwrap() = Some(Arc::downgrade(&scope.lease));
                let intent = session::issue(&store, scope).await.unwrap();
                let owned =
                    session::apply(response_text(&request, Status::OK, "owned")?, intent).unwrap();
                let _owned_future_drop = ObservedDrop(dropped);
                // Public issue returned only after real reply and full finish;
                // no bool or synthetic SQL result is used as commit proof.
                committed.lock().unwrap().take().unwrap().send(()).unwrap();
                std::future::pending::<()>().await;
                Ok(owned)
            }
        },
    )
    .unwrap();
    // Keep the intentional timer exactly 100ms. Freeze only Tokio's clock,
    // not the LeaseOwner/Session std::time::Instant authority clock.
    let server = Server::new(app, options(64, 1000, 100, 1000).unwrap()).await;
    tokio::time::pause();
    struct Resume;
    impl Drop for Resume {
        fn drop(&mut self) {
            tokio::time::resume();
        }
    }
    let resume = Resume;
    let tick = tokio::time::Instant::now();
    let wall = Instant::now();
    let mut committed = Box::pin(received);
    let mut reply = Box::pin(request(&server, "Authorization: Bearer owned\r\n"));
    // Direct yield_now keeps this coordinator runnable, preventing paused
    // Tokio's idle auto-advance while actual socket/SQLite workers are waiting.
    // This finite wall guard fails explicitly if native work stalls; no retry.
    loop {
        if let std::task::Poll::Ready(signal) = futures_util::poll!(committed.as_mut()) {
            signal.unwrap();
            break;
        }
        assert!(
            futures_util::poll!(reply.as_mut()).is_pending(),
            "response preceded real commit"
        );
        assert!(
            wall.elapsed() < Duration::from_secs(2),
            "owned commit wall guard elapsed"
        );
        assert!(
            tokio::time::Instant::now() == tick,
            "clock advanced before real commit"
        );
        tokio::task::yield_now().await;
    }
    assert!(future_dropped.load(Ordering::SeqCst) == 0);
    assert!(tokio::time::Instant::now() == tick);
    // The handler deadline remains 100ms. Tokio 1.53.1 rounds timer
    // deadlines up to the next 1ms driver tick (runtime/time/source.rs).
    // Check a point strictly before it, then cross that rounding quantum.
    tokio::time::advance(Duration::from_millis(99)).await;
    assert!(futures_util::poll!(reply.as_mut()).is_pending());
    assert!(future_dropped.load(Ordering::SeqCst) == 0);
    assert!(tokio::time::Instant::now() == tick + Duration::from_millis(99));
    tokio::time::advance(Duration::from_millis(2)).await;
    let response = loop {
        if let std::task::Poll::Ready(response) = futures_util::poll!(reply.as_mut()) {
            break response;
        }
        assert!(
            wall.elapsed() < Duration::from_secs(2),
            "owned response wall guard elapsed"
        );
        assert!(tokio::time::Instant::now() == tick + Duration::from_millis(101));
        tokio::task::yield_now().await;
    };
    drop(reply);
    drop(resume);
    assert_eq!(response.status, 504);
    assert!(response.all("set-cookie").is_empty());
    assert_eq!(response.all("cache-control"), vec!["no-store"]);
    assert!(future_dropped.load(Ordering::SeqCst) == 1);
    if let Some(lease) = retained_lease.lock().unwrap().as_ref().unwrap().upgrade() {
        assert!(lease.validate().is_err());
    }
    assert_eq!(f.count().await, 1);
    server.stop().await;
    f.stop().await;
}

#[tokio::test]
async fn expiry_after_real_commit_retires_applied_material_without_cookie() {
    let f = Fixture::new().await;
    let app = route(
        app_default(session::clone_store(&f.store)),
        Method::POST,
        "/owned",
        authenticated_policy(verifier),
        |request, store, scope| async move {
            let lease = Arc::clone(&scope.lease);
            let intent = session::issue(&store, scope).await.unwrap();
            let response =
                session::apply(response_text(&request, Status::OK, "owned")?, intent).unwrap();
            // Finite private clock boundary after real commit, no timing race/sleep.
            lease.gate.lock().unwrap().expires_at = Instant::now();
            Ok(response)
        },
    )
    .unwrap();
    let server = Server::new(app, options(64, 1000, 1000, 1000).unwrap()).await;
    let reply = request(&server, "Authorization: Bearer owned\r\n").await;
    assert_eq!(reply.status, 500);
    assert!(reply.all("set-cookie").is_empty());
    assert_eq!(reply.all("cache-control"), vec!["no-store"]);
    assert_eq!(f.count().await, 1);
    server.stop().await;
    f.stop().await;
}
#[tokio::test]
async fn applied_304_and_private_cache_conflict_are_fixed_no_cookie_500() {
    for conflict in [false, true] {
        let f = Fixture::new().await;
        let observation = IssueObservation::new(if conflict {
            IssueCase::PrivateCache
        } else {
            IssueCase::NotModified
        });
        let app = observed_issue_app(&f, Method::POST, &observation);
        let server = Server::new(app, options(64, 1000, 1000, 1000).unwrap()).await;
        let reply = request(&server, "Authorization: Bearer owned\r\n").await;
        assert_eq!(reply.status, 500, "{observation}");
        assert!(reply.all("set-cookie").is_empty());
        assert_eq!(reply.all("cache-control"), vec!["no-store"]);
        assert_eq!(f.count().await, 1);
        server.stop().await;
        f.stop().await;
    }
}
#[tokio::test]
async fn head_applied_response_transfers_cookie_once_without_body() {
    let f = Fixture::new().await;
    let observation = IssueObservation::new(IssueCase::Head);
    let app = observed_issue_app(&f, Method::GET, &observation);
    let server = Server::new(app, options(64, 1000, 1000, 1000).unwrap()).await;
    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"HEAD /owned HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer owned\r\n\r\n",
    )
    .await;
    let reply = response(&mut socket, true).await;
    assert_eq!(reply.status, 200, "{observation}");
    assert!(reply.body.is_empty());
    assert_cookie_policy(&reply);
    assert_eq!(reply.all("vary"), vec!["Authorization, Cookie"]);
    assert_eq!(f.count().await, 1);
    server.stop().await;
    f.stop().await;
}
#[tokio::test]
async fn stored_nonsecret_response_seal_is_refused_on_foreign_public_request() {
    struct State {
        store: Store,
        saved: std::sync::Mutex<Option<Response>>,
    }
    let f = Fixture::new().await;
    let app = route(
        app_default(State {
            store: session::clone_store(&f.store),
            saved: std::sync::Mutex::new(None),
        }),
        Method::POST,
        "/owned",
        authenticated_policy(|_, _| async {
            VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(60))
        }),
        |request, state, scope| async move {
            let intent = session::issue(&state.store, scope).await.unwrap();
            let response =
                session::apply(response_text(&request, Status::OK, "owned")?, intent).unwrap();
            *state.saved.lock().unwrap() = Some(response);
            response_text(&request, Status::OK, "stored nonsecret seal")
        },
    )
    .unwrap();
    let app = route(
        app,
        Method::GET,
        "/owned",
        public_policy(),
        |_, state, _| async move { Ok(state.saved.lock().unwrap().take().unwrap()) },
    )
    .unwrap();
    let server = Server::new(app, options(64, 1000, 1000, 1000).unwrap()).await;
    let first = request(&server, "Authorization: Bearer owned\r\n").await;
    assert_eq!(first.status, 200);
    assert!(first.all("set-cookie").is_empty());
    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"GET /owned HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    let foreign = response(&mut socket, false).await;
    assert_eq!(foreign.status, 500);
    assert!(foreign.all("set-cookie").is_empty());
    assert_eq!(foreign.all("cache-control"), vec!["no-store"]);
    assert_eq!(f.count().await, 1);
    server.stop().await;
    f.stop().await;
}

// Business Err stays a replacement even when its payload is an owned Response.
#[tokio::test]
async fn mapper_identity_of_same_request_seal_never_delivers_cookie() {
    let f = Fixture::new().await;
    let app = route_mapped(
        app_default(session::clone_store(&f.store)),
        Method::POST,
        "/owned",
        authenticated_policy(verifier),
        |request, store, scope| async move {
            let intent = session::issue(&store, scope).await.unwrap();
            let applied = session::apply(
                response_text(&request, Status::BAD_REQUEST, "owned mapped")
                    .expect("bounded test response fits fixture"),
                intent,
            )
            .unwrap();
            Err::<Response, Response>(applied)
        },
        |_, owned| Ok(owned),
    )
    .unwrap();
    let server = Server::new(app, options(64, 1000, 1000, 1000).unwrap()).await;
    let reply = request(&server, "Authorization: Bearer owned\r\n").await;
    assert_eq!(reply.status, 400);
    assert_eq!(reply.body, b"owned mapped");
    assert_eq!(f.count().await, 1); // real commit is not rolled back by replacement
    assert!(reply.all("set-cookie").is_empty());
    assert_eq!(reply.all("cache-control"), vec!["no-store"]);
    server.stop().await;
    f.stop().await;
}
// Mapper can select an owned saved seal from an ordinary error payload. It
// still cannot confer HandlerOk origin or revive retired material.
#[tokio::test]
async fn mapper_selecting_saved_same_request_seal_never_delivers_cookie() {
    let f = Fixture::new().await;
    let app = route_mapped(
        app_default(session::clone_store(&f.store)),
        Method::POST,
        "/owned",
        authenticated_policy(verifier),
        |request, store, scope| async move {
            let intent = session::issue(&store, scope).await.unwrap();
            let saved = session::apply(
                response_text(&request, Status::BAD_REQUEST, "owned replacement")
                    .expect("bounded test response fits fixture"),
                intent,
            )
            .unwrap();
            Err::<Response, (Response, Response)>((
                response_text(&request, Status::OK, "unused")
                    .expect("bounded test response fits fixture"),
                saved,
            ))
        },
        |_, (_, saved)| Ok(saved),
    )
    .unwrap();
    let server = Server::new(app, options(64, 1000, 1000, 1000).unwrap()).await;
    let reply = request(&server, "Authorization: Bearer owned\r\n").await;
    assert_eq!(reply.status, 400);
    assert_eq!(reply.body, b"owned replacement");
    assert_eq!(f.count().await, 1);
    assert!(reply.all("set-cookie").is_empty());
    assert_eq!(reply.all("cache-control"), vec!["no-store"]);
    server.stop().await;
    f.stop().await;
}

#[tokio::test]
async fn mapper_foreign_public_saved_seal_remains_no_store_without_cookie() {
    struct State {
        store: Store,
        saved: std::sync::Mutex<Option<Response>>,
    }
    let f = Fixture::new().await;
    let app = route(
        app_default(State {
            store: session::clone_store(&f.store),
            saved: std::sync::Mutex::new(None),
        }),
        Method::POST,
        "/owned",
        authenticated_policy(|_, _| async {
            VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(60))
        }),
        |request, state, scope| async move {
            let intent = session::issue(&state.store, scope).await.unwrap();
            let sealed = session::apply(
                response_text(&request, Status::BAD_REQUEST, "saved mapper body")?,
                intent,
            )
            .unwrap();
            *state.saved.lock().unwrap() = Some(sealed);
            response_text(&request, Status::OK, "stored nonsecret seal")
        },
    )
    .unwrap();
    let app = route_mapped(
        app,
        Method::GET,
        "/owned",
        public_policy(),
        |_, state, _| async move {
            Err::<Response, Response>(state.saved.lock().unwrap().take().unwrap())
        },
        |_, saved| Ok(saved),
    )
    .unwrap();
    let server = Server::new(app, options(64, 1000, 1000, 1000).unwrap()).await;
    let first = request(&server, "Authorization: Bearer owned\r\n").await;
    assert_eq!(first.status, 200);
    assert!(first.all("set-cookie").is_empty());
    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"GET /owned HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    let mapped = response(&mut socket, false).await;
    assert_eq!(f.count().await, 1);
    assert_eq!(mapped.status, 400);
    assert_eq!(mapped.body, b"saved mapper body");
    assert!(mapped.all("set-cookie").is_empty());
    assert_eq!(mapped.all("cache-control"), vec!["no-store"]);
    server.stop().await;
    f.stop().await;
}
