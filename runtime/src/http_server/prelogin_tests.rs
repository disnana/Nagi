//! Prospective real SQLite/socket bootstrap oracles; none executed locally.
//! Raw response material is RAM-only; assertions never format cookies/tokens.
use super::*;
use crate::{
    auth::{prelogin, session},
    sqlite,
};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    dir: std::path::PathBuf,
    auth: sqlite::Pool,
    pool: Arc<prelogin::Pool>,
}
impl Fixture {
    async fn new(ttl: i64) -> Self {
        let dir = loop {
            let path = std::env::temp_dir().join(format!(
                "sf03-wire-owned-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => break path,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => panic!("owned fixture unavailable"),
            }
        };
        let auth = sqlite::open(
            dir.join("owned.sqlite").to_str().unwrap(),
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
        let host = authority(
            default_options(),
            "https://localhost",
            vec!["localhost".to_owned()],
            1,
            128,
        )
        .unwrap();
        let pool = prelogin::open_for_http(
            &store,
            sqlite::options(1, 2, 1000, 1000).unwrap(),
            prelogin::options(2, 3, 144, 1, ttl, 2, 8, 10000, 4, 16).unwrap(),
            prelogin::cookie_options("__Host-prelogin", session::SameSite::Strict).unwrap(),
            &host,
        )
        .await
        .unwrap();
        Self {
            dir,
            auth,
            pool: Arc::new(pool),
        }
    }
    async fn count(&self) -> i64 {
        self.pool.row_count_for_test().await
    }
    async fn stop(self) {
        prelogin::close_native(&self.pool, 2000).await.unwrap();
        sqlite::close(&self.auth, 2000).await.unwrap();
        std::fs::remove_dir_all(self.dir).unwrap();
    }
}
struct Server {
    address: std::net::SocketAddr,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<Result<(), Error>>,
}
impl Server {
    async fn new(pool: Arc<prelogin::Pool>) -> Self {
        Self::with_options(pool, default_options()).await
    }
    async fn with_options(pool: Arc<prelogin::Pool>, config: Options) -> Self {
        let app = register_prelogin_bootstrap(app_default(()), "/bootstrap", pool).unwrap();
        let config = authority(
            config,
            "https://localhost",
            vec!["localhost".to_owned()],
            1,
            128,
        )
        .unwrap();
        let config = super::tests::with_response_fixture(config);
        let (app, config) = super::tests::with_rate_fixture(app, config);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, until) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(serve_listener(listener, app, config, async {
            let _ = until.await;
        }));
        Self {
            address,
            stop: Some(stop),
            task,
        }
    }
    async fn request(&self, method: &str, extra: &str) -> Reply {
        let mut stream = tokio::net::TcpStream::connect(self.address).await.unwrap();
        let input = format!(
            "{method} /bootstrap HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n{extra}\r\n"
        );
        stream.write_all(input.as_bytes()).await.unwrap();
        let mut bytes = Vec::new();
        tokio::time::timeout(
            Duration::from_secs(3),
            stream.take(16384).read_to_end(&mut bytes),
        )
        .await
        .unwrap()
        .unwrap();
        Reply::parse(bytes)
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
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.task.abort();
    }
}
struct Reply {
    code: u16,
    headers: HeaderMap,
    body: Vec<u8>,
}
impl Reply {
    fn parse(bytes: Vec<u8>) -> Self {
        let at = bytes
            .windows(4)
            .position(|v| v == b"\r\n\r\n")
            .expect("missing bounded header terminator");
        let text = std::str::from_utf8(&bytes[..at]).unwrap();
        let mut lines = text.split("\r\n");
        let code = lines
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let mut headers = HeaderMap::new();
        for line in lines {
            let (name, value) = line.split_once(':').unwrap();
            headers.append(
                HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value.trim()).unwrap(),
            );
        }
        Self {
            code,
            headers,
            body: bytes[at + 4..].to_vec(),
        }
    }
    fn no_material(&self) {
        assert!(!self.headers.contains_key(names::SET_COOKIE));
        assert!(self.body.is_empty() || !self.body.windows(10).any(|v| v == b"csrf_token"));
        assert!(self
            .headers
            .get(names::CACHE_CONTROL)
            .is_some_and(|v| v == "no-store"));
    }
}
#[tokio::test]
async fn explicit_head_has_zero_entropy_business_sql_cookie_and_token() {
    let f = Fixture::new(1000).await;
    let server = Server::new(Arc::clone(&f.pool)).await;
    let head = server.request("HEAD", "").await;
    assert!(head.code == 200);
    head.no_material();
    assert!(head.body.is_empty());
    assert!(head
        .headers
        .get(names::VARY)
        .is_some_and(|v| v == "Authorization, Cookie, Origin"));
    assert!(head
        .headers
        .get(names::CONTENT_LENGTH)
        .is_some_and(|v| v == "60"));
    assert!(
        f.count().await == 0
            && f.pool.entropy_calls_for_test() == 0
            && f.pool.business_calls_for_test() == 0
    );
    server.stop().await;
    f.stop().await;
}
#[tokio::test]
async fn get_commits_one_row_and_only_fixed_json_host_only_cookie_reaches_wire() {
    let f = Fixture::new(1000).await;
    let server = Server::new(Arc::clone(&f.pool)).await;
    let reply = server.request("GET", "").await;
    assert!(reply.code == 200);
    assert!(f.count().await == 1);
    assert!(reply
        .headers
        .get(names::VARY)
        .is_some_and(|v| v == "Authorization, Cookie, Origin"));
    assert!(reply
        .headers
        .get(names::CACHE_CONTROL)
        .is_some_and(|v| v == "no-store"));
    assert!(reply
        .headers
        .get(names::CONTENT_TYPE)
        .is_some_and(|v| v == "application/json"));
    let mut cookies = reply.headers.get_all(names::SET_COOKIE).iter();
    let cookie = cookie::Cookie::parse(cookies.next().unwrap().to_str().unwrap()).unwrap();
    assert!(cookies.next().is_none());
    assert!(
        cookie.name() == "__Host-prelogin"
            && cookie.secure() == Some(true)
            && cookie.http_only() == Some(true)
            && cookie.path() == Some("/")
            && cookie.domain().is_none()
    );
    let json: serde_json::Value = serde_json::from_slice(&reply.body).unwrap();
    assert!(json.as_object().is_some_and(|v| v.len() == 1));
    let token = json
        .get("csrf_token")
        .and_then(serde_json::Value::as_str)
        .unwrap();
    assert!(token.len() == 43 && cookie.value().len() == 43);
    assert!(f.pool.entropy_calls_for_test() == 1 && f.pool.business_calls_for_test() == 1);
    server.stop().await;
    f.stop().await;
}
#[tokio::test]
async fn origin_credential_and_body_rejections_never_issue_state() {
    let f = Fixture::new(1000).await;
    let server = Server::new(Arc::clone(&f.pool)).await;
    for (extra, status) in [
        ("Origin: https://other.invalid\r\n", 403),
        ("Authorization: Bearer ordinary\r\n", 400),
        ("Content-Length: 1\r\n", 400),
    ] {
        let reply = server.request("GET", extra).await;
        assert!(reply.code == status);
        reply.no_material();
    }
    assert!(
        f.count().await == 0
            && f.pool.entropy_calls_for_test() == 0
            && f.pool.business_calls_for_test() == 0
    );
    server.stop().await;
    f.stop().await;
}
#[tokio::test]
async fn wire_take_refuses_expired_foreign_reused_and_replacement_seals() {
    let response_request = response_public::tests::request_fixture(64);
    let response_context = response_context(&response_request);
    let f = Fixture::new(1000).await;
    let first = prelogin::HttpOwner::new(
        LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap(),
    );
    let weak = first.downgrade();
    f.pool.bootstrap_http(&first, None).await.unwrap();
    let seal = first.seal().unwrap();
    tokio::time::sleep(Duration::from_millis(1000)).await;
    let denied = finalize_prelogin(
        response_context,
        PreloginCompletion::Ready(seal),
        &first,
        false,
        &f.pool,
    );
    assert!(
        denied.status() == StatusCode::GATEWAY_TIMEOUT
            && !denied.headers().contains_key(names::SET_COOKIE)
    );
    // Apply the same outer Vary projection as the actual dispatcher.
    let denied = vary::finish(denied);
    no_terminal_material(&denied, StatusCode::GATEWAY_TIMEOUT, &first);
    drop(denied);
    drop(first);
    assert!(!weak.has_live_owner());
    assert!(f.count().await == 1);
    // Fresh normal lifetime for one-use/foreign/replacement paths.
    prelogin::close_native(&f.pool, 2000).await.unwrap();
    sqlite::close(&f.auth, 2000).await.unwrap();
    std::fs::remove_dir_all(&f.dir).unwrap();
    let f = Fixture::new(1000).await;
    let owner = prelogin::HttpOwner::new(
        LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap(),
    );
    f.pool.bootstrap_http(&owner, None).await.unwrap();
    let seal = owner.seal().unwrap();
    let foreign = prelogin::HttpOwner::new(
        LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap(),
    );
    let denied = finalize_prelogin(
        response_context,
        PreloginCompletion::Ready(seal),
        &foreign,
        false,
        &f.pool,
    );
    assert!(denied.status() == StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!denied.headers().contains_key(names::SET_COOKIE));
    drop(denied);
    // A second seal cannot resurrect the first material after terminal discard.
    owner.retire();
    assert!(owner.seal().is_err());
    let owner = prelogin::HttpOwner::new(
        LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap(),
    );
    f.pool.bootstrap_http(&owner, None).await.unwrap();
    let seal = owner.seal().unwrap();
    let denied = finalize_prelogin(
        response_context,
        PreloginCompletion::Replacement(Some(seal), empty(response_context, Status::OK).unwrap()),
        &owner,
        false,
        &f.pool,
    );
    assert!(!denied.headers().contains_key(names::SET_COOKIE));
    assert!(owner.seal().is_err());
    drop(denied);
    f.stop().await;
    // Destructive success and impossible cache/304 replacements are separate
    // terminal cases; bounded new fixtures preserve the two-live-row budget.
    let f = Fixture::new(1000).await;
    let owner = prelogin::HttpOwner::new(
        LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap(),
    );
    f.pool.bootstrap_http(&owner, None).await.unwrap();
    let seal = owner.seal().unwrap();
    let accepted = finalize_prelogin(
        response_context,
        PreloginCompletion::Ready(seal),
        &owner,
        false,
        &f.pool,
    );
    assert!(accepted.status() == StatusCode::OK);
    assert!(accepted.headers().contains_key(names::SET_COOKIE));
    assert!(owner.seal().is_err());
    f.stop().await;
    for (case, response) in [
        (
            "not_modified",
            empty(response_context, Status::NOT_MODIFIED).unwrap(),
        ),
        (
            "conflicting_cache",
            conflicting_cache_for_test(empty(response_context, Status::OK).unwrap()),
        ),
    ] {
        let f = Fixture::new(1000).await;
        let owner = prelogin::HttpOwner::new(
            LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap(),
        );
        f.pool.bootstrap_http(&owner, None).await.unwrap();
        let seal = owner.seal().unwrap();
        let denied = finalize_prelogin(
            response_context,
            PreloginCompletion::Replacement(Some(seal), response),
            &owner,
            false,
            &f.pool,
        );
        assert!(
            denied.status() == StatusCode::INTERNAL_SERVER_ERROR,
            "prelogin_replacement_case={case} status={}",
            denied.status().as_u16()
        );
        let denied = vary::finish(denied);
        no_terminal_material(&denied, StatusCode::INTERNAL_SERVER_ERROR, &owner);
        assert!(!denied.headers().contains_key(names::SET_COOKIE));
        assert!(owner.seal().is_err());
        assert!(f.count().await == 1);
        f.stop().await;
    }
}

#[tokio::test]
async fn owned_writer_wait_timeout_retires_pending_without_rollback_claim() {
    let f = Fixture::new(1000).await;
    let lock = sqlite::begin(&f.auth, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let server =
        Server::with_options(Arc::clone(&f.pool), options(64, 1000, 20, 2000).unwrap()).await;
    let reply = server.request("GET", "").await;
    assert!(reply.code == 504);
    reply.no_material();
    sqlite::rollback(lock).await.unwrap();
    server.stop().await;
    assert!(f.count().await == 0 && f.pool.business_calls_for_test() == 0);
    // HTTP future cancellation is separate from actual SQLite close/nativejoin.
    f.stop().await;
}

#[tokio::test]
async fn prior_cookie_replacement_and_duplicate_pair_rejection_use_actual_rows() {
    let f = Fixture::new(1000).await;
    let server = Server::new(Arc::clone(&f.pool)).await;
    let first = server.request("GET", "").await;
    assert!(first.code == 200);
    let cookie = cookie::Cookie::parse(
        first
            .headers
            .get(names::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap(),
    )
    .unwrap();
    let pair = format!("{}={}", cookie.name(), cookie.value()); // owned native RAMonly.
    let duplicate = server
        .request("GET", &format!("Cookie: {pair}\r\nCookie: {pair}\r\n"))
        .await;
    assert!(duplicate.code == 400);
    duplicate.no_material();
    assert!(f.count().await == 1 && f.pool.entropy_calls_for_test() == 1);
    let replaced = server.request("GET", &format!("Cookie: {pair}\r\n")).await;
    assert!(
        replaced.code == 200
            && f.count().await == 1
            && f.pool.entropy_calls_for_test() == 2
            && f.pool.business_calls_for_test() == 3
    );
    server.stop().await;
    f.stop().await;
}

// IR01 classification oracles written before revision2 production edits.
// Intended runtime assertion failures, never compiled RED in this environment.
fn no_terminal_material(response: &HttpResponse, status: StatusCode, owner: &prelogin::HttpOwner) {
    assert!(response.status() == status);
    assert!(!response.headers().contains_key(names::SET_COOKIE));
    assert!(!response
        .body()
        .bytes
        .windows(10)
        .any(|v| v == b"csrf_token"));
    assert!(response
        .headers()
        .get(names::CACHE_CONTROL)
        .is_some_and(|v| v == "no-store"));
    assert!(response
        .headers()
        .get(names::VARY)
        .is_some_and(|v| v == "Authorization, Cookie, Origin"));
    assert!(owner.seal().is_err());
    assert!(!owner.has_wire_material_for_test());
}

#[tokio::test]
async fn earlier_security_deadline_rejects_finite_non_yielding_ready_as_504() {
    let response_request = response_public::tests::request_fixture(64);
    let response_context = response_context(&response_request);
    let f = Fixture::new(1000).await;
    let owner = prelogin::HttpOwner::new(
        LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap(),
    );
    f.pool.bootstrap_http(&owner, None).await.unwrap();
    let seal = owner.seal().unwrap();
    let admitted = Instant::now();
    let security_until = admitted + Duration::from_millis(10);
    let handler_until = admitted + Duration::from_secs(1);
    owner.constrain_until(security_until).unwrap();
    let response = prelogin_http::complete_bootstrap(
        response_context,
        &f.pool,
        &owner,
        handler_until.min(security_until),
        async {
            // Finite, owned non-yielding poll; no external traffic or load.
            std::thread::sleep(Duration::from_millis(20));
            Ok(seal)
        },
    )
    .await;
    // Apply the same outer Vary projection as the actual dispatcher.
    let response = vary::finish(response);
    no_terminal_material(&response, StatusCode::GATEWAY_TIMEOUT, &owner);
    assert!(f.count().await == 1);
    // Commit already happened; terminal status does not imply rollback.
    f.stop().await;
}

#[tokio::test]
async fn real_security_gate_expiring_after_seal_is_504_and_clears_material() {
    let response_request = response_public::tests::request_fixture(64);
    let response_context = response_context(&response_request);
    let f = Fixture::new(1000).await;
    let owner = prelogin::HttpOwner::new(
        LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap(),
    );
    f.pool.bootstrap_http(&owner, None).await.unwrap();
    let seal = owner.seal().unwrap();
    owner
        .constrain_until(Instant::now() + Duration::from_millis(10))
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    let response = finalize_prelogin(
        response_context,
        PreloginCompletion::Ready(seal),
        &owner,
        false,
        &f.pool,
    );
    // Apply the same outer Vary projection as the actual dispatcher.
    let response = vary::finish(response);
    no_terminal_material(&response, StatusCode::GATEWAY_TIMEOUT, &owner);
    assert!(f.count().await == 1);
    f.stop().await;
}

#[tokio::test]
async fn controlled_wire_clock_backstep_is_503_before_and_after_serialization() {
    let response_request = response_public::tests::request_fixture(64);
    let response_context = response_context(&response_request);
    for after_serialization in [false, true] {
        let mut f = Fixture::new(1000).await;
        let owner = prelogin::HttpOwner::new(
            LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap(),
        );
        f.pool.bootstrap_http(&owner, None).await.unwrap();
        let seal = owner.seal().unwrap();
        Arc::get_mut(&mut f.pool)
            .unwrap()
            .wire_clock_for_test(&owner, |wall, _expiry| {
                if after_serialization {
                    vec![wall, wall - 1]
                } else {
                    vec![wall - 1]
                }
            });
        let response = finalize_prelogin(
            response_context,
            PreloginCompletion::Ready(seal),
            &owner,
            false,
            &f.pool,
        );
        // Apply the same outer Vary projection as the actual dispatcher.
        let response = vary::finish(response);
        no_terminal_material(&response, StatusCode::SERVICE_UNAVAILABLE, &owner);
        assert!(f.count().await == 1);
        f.stop().await;
    }
}

#[tokio::test]
async fn controlled_wire_absolute_expiry_is_504_before_and_after_serialization() {
    let response_request = response_public::tests::request_fixture(64);
    let response_context = response_context(&response_request);
    for after_serialization in [false, true] {
        let mut f = Fixture::new(1000).await;
        let owner = prelogin::HttpOwner::new(
            LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap(),
        );
        f.pool.bootstrap_http(&owner, None).await.unwrap();
        let seal = owner.seal().unwrap();
        Arc::get_mut(&mut f.pool)
            .unwrap()
            .wire_clock_for_test(&owner, |wall, expiry| {
                if after_serialization {
                    vec![wall, expiry]
                } else {
                    vec![expiry]
                }
            });
        let response = finalize_prelogin(
            response_context,
            PreloginCompletion::Ready(seal),
            &owner,
            false,
            &f.pool,
        );
        // Apply the same outer Vary projection as the actual dispatcher.
        let response = vary::finish(response);
        no_terminal_material(&response, StatusCode::GATEWAY_TIMEOUT, &owner);
        assert!(f.count().await == 1);
        f.stop().await;
    }
}
