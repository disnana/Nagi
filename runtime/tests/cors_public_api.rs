//! Owned finite loopback wire tests. Browser/TLS read enforcement remains a separate gate.
use nagi_runtime::{auth, http_server as http, Error};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{oneshot, Notify},
};
const UI: &str = "https://app.example.invalid";
#[derive(Default)]
struct State {
    calls: AtomicUsize,
    verifier: AtomicUsize,
    mapper: AtomicUsize,
    entered: Notify,
    release: Notify,
    cancelled: AtomicUsize,
    cancel_seen: Notify,
}
fn cors(mode: http::CorsCredentials) -> http::CorsPolicy {
    http::cors_policy(
        vec![UI.into(), "https://localhost".into()],
        vec!["content-type".into(), "x-csrf-token".into()],
        mode,
        0,
        2,
        128,
        2,
        64,
    )
    .unwrap()
}
fn mapped(_: Error) -> http::Response {
    http::text(http::Status::INTERNAL_SERVER_ERROR, "mapped")
}
struct Server {
    address: std::net::SocketAddr,
    state: Arc<State>,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<Result<(), Error>>,
}
impl Server {
    async fn new(capacity: i64, handler_ms: i64) -> Self {
        let state = Arc::new(State::default());
        let mut app = http::route(
            http::app(Arc::clone(&state), mapped),
            http::Method::GET,
            "/items/{id}",
            http::public_policy(),
            |_, s: Arc<Arc<State>>, _| async move {
                s.calls.fetch_add(1, Ordering::SeqCst);
                Ok(http::text(http::Status::OK, "owned"))
            },
        )
        .unwrap();
        app = http::cors_route(
            app,
            http::Method::GET,
            "/items/{id}",
            cors(http::CorsCredentials::ALLOW),
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::HEAD,
            "/explicit",
            http::public_policy(),
            |_, _, _| async { Ok(http::text(http::Status::OK, "explicit")) },
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::GET,
            "/explicit",
            http::public_policy(),
            |_, _, _| async { Ok(http::text(http::Status::OK, "fallback")) },
        )
        .unwrap();
        app = http::cors_route(
            app,
            http::Method::GET,
            "/explicit",
            cors(http::CorsCredentials::ALLOW),
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::GET,
            "/omit",
            http::public_policy(),
            |_, _, _| async { Ok(http::empty(http::Status::OK)) },
        )
        .unwrap();
        app = http::cors_route(
            app,
            http::Method::GET,
            "/omit",
            cors(http::CorsCredentials::OMIT),
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::POST,
            "/write",
            http::public_policy(),
            |_, s: Arc<Arc<State>>, _| async move {
                s.calls.fetch_add(1, Ordering::SeqCst);
                Ok(http::empty(http::Status::OK))
            },
        )
        .unwrap();
        app = http::cors_route(
            app,
            http::Method::POST,
            "/write",
            cors(http::CorsCredentials::ALLOW),
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::OPTIONS,
            "/write",
            http::public_policy(),
            |_, s: Arc<Arc<State>>, _| async move {
                s.calls.fetch_add(1, Ordering::SeqCst);
                Ok(http::empty(http::Status::OK))
            },
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::GET,
            "/auth",
            http::authenticated_policy(|_, s: Arc<Arc<State>>| async move {
                s.verifier.fetch_add(1, Ordering::SeqCst);
                Err::<auth::VerifiedIdentity, _>(auth::Failure::invalid_credential())
            }),
            |_, _, _| async { Ok(http::empty(http::Status::OK)) },
        )
        .unwrap();
        app = http::cors_route(
            app,
            http::Method::GET,
            "/auth",
            cors(http::CorsCredentials::ALLOW),
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::OPTIONS,
            "/auth",
            http::authenticated_policy(|_, s: Arc<Arc<State>>| async move {
                s.verifier.fetch_add(1, Ordering::SeqCst);
                Err::<auth::VerifiedIdentity, _>(auth::Failure::invalid_credential())
            }),
            |_, _, _| async { Ok(http::empty(http::Status::OK)) },
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::GET,
            "/panic",
            http::public_policy(),
            |_, _, _| async {
                panic!("owned panic");
                #[allow(unreachable_code)]
                Ok::<_, Error>(http::empty(http::Status::OK))
            },
        )
        .unwrap();
        app = http::cors_route(
            app,
            http::Method::GET,
            "/panic",
            cors(http::CorsCredentials::ALLOW),
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::GET,
            "/err",
            http::public_policy(),
            |_, s: Arc<Arc<State>>, _| async move {
                s.mapper.fetch_add(1, Ordering::SeqCst);
                Err::<http::Response, _>(Error::internal("owned replacement"))
            },
        )
        .unwrap();
        app = http::cors_route(
            app,
            http::Method::GET,
            "/err",
            cors(http::CorsCredentials::ALLOW),
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::GET,
            "/304",
            http::public_policy(),
            |_, _, _| async { Ok(http::empty(http::Status::NOT_MODIFIED)) },
        )
        .unwrap();
        app = http::cors_route(
            app,
            http::Method::GET,
            "/304",
            cors(http::CorsCredentials::ALLOW),
        )
        .unwrap();
        app = http::route(
            app,
            http::Method::GET,
            "/pending",
            http::public_policy(),
            |_, s: Arc<Arc<State>>, _| async move {
                struct Cancel(Arc<State>);
                impl Drop for Cancel {
                    fn drop(&mut self) {
                        self.0.cancelled.fetch_add(1, Ordering::SeqCst);
                        self.0.cancel_seen.notify_one();
                    }
                }
                let _cancel = Cancel(Arc::clone(s.as_ref()));
                s.entered.notify_one();
                s.release.notified().await;
                Ok(http::empty(http::Status::OK))
            },
        )
        .unwrap();
        app = http::cors_route(
            app,
            http::Method::GET,
            "/pending",
            cors(http::CorsCredentials::ALLOW),
        )
        .unwrap();
        let options = http::authority(
            http::capacity(
                http::options(32, 1000, handler_ms, 1000).unwrap(),
                8,
                capacity,
            )
            .unwrap(),
            "https://localhost",
            vec!["localhost".into()],
            1,
            128,
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
        tokio::time::timeout(Duration::from_secs(5), &mut self.task)
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
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}
impl Reply {
    fn values(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
            .collect()
    }
    fn grant(&self) {
        assert_eq!(self.values("access-control-allow-origin"), vec![UI]);
        assert_eq!(
            self.values("access-control-allow-credentials"),
            vec!["true"]
        );
        assert_eq!(self.values("content-security-policy").len(), 1);
        assert_eq!(self.values("x-content-type-options"), vec!["nosniff"]);
    }
    fn no_grant(&self) {
        assert!(self
            .headers
            .iter()
            .all(|(k, _)| !k.to_ascii_lowercase().starts_with("access-control-")));
    }
}
async fn read(stream: TcpStream) -> Reply {
    let mut stream = stream.take(8193);
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert!(bytes.len() < 8192);
    let split = bytes.windows(4).position(|b| b == b"\r\n\r\n").unwrap();
    let mut lines = std::str::from_utf8(&bytes[..split]).unwrap().split("\r\n");
    let status = lines
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .map(|line| {
            let (k, v) = line.split_once(':').unwrap();
            (k.to_owned(), v.trim().to_owned())
        })
        .collect();
    Reply {
        status,
        headers,
        body: bytes[split + 4..].to_vec(),
    }
}
async fn start(s: &Server, method: &str, path: &str, extra: &str, body: &[u8]) -> TcpStream {
    let mut stream = TcpStream::connect(s.address).await.unwrap();
    let head=format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n{extra}\r\n",body.len());
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();
    stream
}
async fn request(s: &Server, method: &str, path: &str, extra: &str, body: &[u8]) -> Reply {
    read(start(s, method, path, extra, body).await).await
}
fn preflight(method: &str, extra: &str) -> String {
    format!("Origin: {UI}\r\nAccess-Control-Request-Method: {method}\r\n{extra}")
}
#[tokio::test]
async fn actual_allow_deny_omit_and_head_record_selection_preserve_execution() {
    let s = Server::new(2, 1000).await;
    let origin = format!("Origin: {UI}\r\n");
    let ok = request(&s, "GET", "/items/7", &origin, b"").await;
    assert_eq!(ok.status, 200);
    ok.grant();
    assert_eq!(ok.body, b"owned");
    assert_eq!(ok.values("vary"), vec!["Origin"]);
    for denied in [
        "null",
        "https://other.invalid",
        "http://app.example.invalid",
    ] {
        let r = request(&s, "GET", "/items/7", &format!("Origin: {denied}\r\n"), b"").await;
        assert_eq!(r.status, 200);
        r.no_grant();
        assert_eq!(r.values("vary"), vec!["Origin"]);
    }
    let duplicate = request(&s, "GET", "/items/7", &format!("{origin}{origin}"), b"").await;
    assert_eq!(duplicate.status, 200);
    duplicate.no_grant();
    let head = request(&s, "HEAD", "/items/7", &origin, b"").await;
    head.grant();
    assert!(head.body.is_empty());
    assert_eq!(head.values("content-length"), vec!["5"]);
    let explicit = request(&s, "HEAD", "/explicit", &origin, b"").await;
    assert_eq!(explicit.status, 200);
    explicit.no_grant();
    let omit = request(&s, "GET", "/omit", &origin, b"").await;
    assert_eq!(omit.values("access-control-allow-origin"), vec![UI]);
    assert!(omit.values("access-control-allow-credentials").is_empty());
    assert_eq!(s.state.calls.load(Ordering::SeqCst), 6);
    s.stop().await;
}
#[tokio::test]
async fn successful_preflight_is_metadata_only_and_ordinary_OPTIONS_remains_distinct() {
    let s = Server::new(2, 1000).await;
    let r = request(
        &s,
        "OPTIONS",
        "/write",
        &preflight(
            "POST",
            "Access-Control-Request-Headers: X-CSRF-Token, Content-Type\r\n",
        ),
        b"",
    )
    .await;
    assert_eq!(r.status, 204);
    assert!(r.body.is_empty());
    r.grant();
    assert_eq!(r.values("access-control-allow-methods"), vec!["POST"]);
    assert_eq!(
        r.values("access-control-allow-headers"),
        vec!["content-type, x-csrf-token"]
    );
    assert_eq!(r.values("access-control-max-age"), vec!["0"]);
    assert_eq!(r.values("cache-control"), vec!["no-store"]);
    assert_eq!(
        r.values("vary"),
        vec!["Access-Control-Request-Headers, Access-Control-Request-Method, Origin"]
    );
    let auth = request(
        &s,
        "OPTIONS",
        "/auth",
        &preflight("GET", "Cookie: ignored\r\nAuthorization: ignored\r\n"),
        b"",
    )
    .await;
    assert_eq!(auth.status, 204);
    assert_eq!(s.state.verifier.load(Ordering::SeqCst), 0);
    assert_eq!(s.state.calls.load(Ordering::SeqCst), 0);
    assert!(auth.values("set-cookie").is_empty());
    let ordinary = request(&s, "OPTIONS", "/write", &format!("Origin: {UI}\r\n"), b"").await;
    assert_eq!(ordinary.status, 200);
    ordinary.no_grant();
    assert_eq!(s.state.calls.load(Ordering::SeqCst), 1);
    s.stop().await;
}
#[tokio::test]
async fn preflight_grammar_denials_and_lookup_status_are_exact() {
    let s = Server::new(2, 1000).await;
    for (extra, status) in [
        ("Access-Control-Request-Headers: x-unknown\r\n", 403),
        (
            "Access-Control-Request-Headers: content-type, Content-Type\r\n",
            400,
        ),
        ("Access-Control-Request-Headers: *\r\n", 400),
        ("Access-Control-Request-Headers: \r\n", 400),
        ("Access-Control-Request-Headers: a,,b\r\n", 400),
    ] {
        let r = request(&s, "OPTIONS", "/write", &preflight("POST", extra), b"").await;
        assert_eq!(r.status, status);
        r.grant();
        assert!(r.values("access-control-allow-methods").is_empty());
        assert!(r.values("access-control-max-age").is_empty());
    }
    for (extra, status) in [
        ("Access-Control-Request-Method: POST\r\n", 400),
        (
            "Origin: null\r\nAccess-Control-Request-Method: POST\r\n",
            403,
        ),
        (
            "Origin: https://other.invalid\r\nAccess-Control-Request-Method: POST\r\n",
            403,
        ),
        (
            "Origin: https://app.example.invalid/\r\nAccess-Control-Request-Method: POST\r\n",
            400,
        ),
        ("Access-Control-Request-Headers: content-type\r\n", 400),
    ] {
        let r = request(&s, "OPTIONS", "/write", extra, b"").await;
        assert_eq!(r.status, status);
        r.no_grant();
    }
    let unknown = request(&s, "OPTIONS", "/missing", &preflight("GET", ""), b"").await;
    assert_eq!(unknown.status, 404);
    unknown.no_grant();
    let method = request(&s, "OPTIONS", "/write", &preflight("DELETE", ""), b"").await;
    assert_eq!(method.status, 405);
    method.no_grant();
    assert_eq!(method.values("allow"), vec!["OPTIONS, POST"]);
    let denied = request(&s, "OPTIONS", "/explicit", &preflight("HEAD", ""), b"").await;
    assert_eq!(denied.status, 403);
    denied.no_grant();
    assert_eq!(s.state.calls.load(Ordering::SeqCst), 0);
    s.stop().await;
}
#[tokio::test]
async fn selected_early_auth_handler_replacements_and_public304_keep_CORS() {
    let s = Server::new(2, 1000).await;
    let origin = format!("Origin: {UI}\r\n");
    let auth = request(
        &s,
        "GET",
        "/auth",
        &format!("{origin}Authorization: Bearer owned\r\n"),
        b"",
    )
    .await;
    assert_eq!(auth.status, 401);
    auth.grant();
    assert_eq!(auth.values("vary"), vec!["Authorization, Cookie, Origin"]);
    assert_eq!(auth.values("cache-control"), vec!["no-store"]);
    for path in ["/panic", "/err"] {
        let r = request(&s, "GET", path, &origin, b"").await;
        assert_eq!(r.status, 500);
        r.grant();
        if path == "/err" {
            assert_eq!(r.body, b"mapped");
        }
    }
    let large = request(&s, "POST", "/write", &origin, &[0; 33]).await;
    assert_eq!(large.status, 413);
    large.grant();
    let dto = request(&s, "GET", "/304", &origin, b"").await;
    assert_eq!(dto.status, 304);
    dto.grant();
    assert!(dto.body.is_empty());
    s.stop().await;
}
#[tokio::test]
async fn preflight_body_and_admission_errors_do_not_create_proof_or_run_handler() {
    let s = Server::new(1, 1000).await;
    let held = start(&s, "GET", "/pending", &format!("Origin: {UI}\r\n"), b"").await;
    tokio::time::timeout(Duration::from_secs(5), s.state.entered.notified())
        .await
        .unwrap();
    let busy = request(&s, "OPTIONS", "/write", &preflight("POST", ""), b"").await;
    assert_eq!(busy.status, 503);
    busy.grant();
    assert_eq!(
        busy.values("vary"),
        vec!["Access-Control-Request-Headers, Access-Control-Request-Method, Origin"]
    );
    s.state.release.notify_one();
    assert_eq!(read(held).await.status, 200);
    let large = request(&s, "OPTIONS", "/write", &preflight("POST", ""), &[0; 33]).await;
    assert_eq!(large.status, 413);
    large.grant();
    let nonempty = request(&s, "OPTIONS", "/write", &preflight("POST", ""), b"x").await;
    assert_eq!(nonempty.status, 400);
    nonempty.grant();
    assert_eq!(s.state.calls.load(Ordering::SeqCst), 0);
    s.stop().await;
}
#[tokio::test]
async fn pending_timeout_and_cancel_release_capacity_without_delay_masking() {
    let s = Server::new(1, 80).await;
    let held = start(&s, "GET", "/pending", &format!("Origin: {UI}\r\n"), b"").await;
    tokio::time::timeout(Duration::from_secs(5), s.state.entered.notified())
        .await
        .unwrap();
    let timeout = read(held).await;
    assert_eq!(timeout.status, 504);
    timeout.grant();
    assert_eq!(s.state.cancelled.load(Ordering::SeqCst), 1);
    let next = request(&s, "GET", "/items/7", &format!("Origin: {UI}\r\n"), b"").await;
    assert_eq!(next.status, 200);
    tokio::time::timeout(Duration::from_secs(5), s.state.cancel_seen.notified())
        .await
        .unwrap();
    let held = start(&s, "GET", "/pending", "", b"").await;
    tokio::time::timeout(Duration::from_secs(5), s.state.entered.notified())
        .await
        .unwrap();
    drop(held);
    tokio::time::timeout(Duration::from_secs(5), s.state.cancel_seen.notified())
        .await
        .unwrap();
    assert_eq!(s.state.cancelled.load(Ordering::SeqCst), 2);
    let next = request(&s, "GET", "/items/7", "", b"").await;
    assert_eq!(next.status, 200);
    // Shutdown joins the owned connection tasks; no synthetic response was asserted for cancel.
    s.stop().await;
}
#[tokio::test]
async fn authority_and_route_less_errors_never_borrow_another_records_grant() {
    let s = Server::new(2, 1000).await;
    let origin = format!("Origin: {UI}\r\n");
    for (method, path, status) in [("GET", "/missing", 404), ("DELETE", "/items/7", 405)] {
        let r = request(&s, method, path, &origin, b"").await;
        assert_eq!(r.status, status);
        r.no_grant();
    }
    for extra in [
        "Host: other.invalid\r\n",
        "Forwarded: host=localhost\r\n",
        "X-Forwarded-Host: localhost\r\n",
    ] {
        let r = request(&s, "GET", "/items/7", &format!("{origin}{extra}"), b"").await;
        assert_eq!(r.status, 400);
        r.no_grant();
    }
    let metadata = request(
        &s,
        "OPTIONS",
        "/auth",
        &format!("{origin}Host: localhost\r\nAccess-Control-Request-Method: GET\r\n"),
        b"",
    )
    .await;
    assert_eq!(metadata.status, 400);
    metadata.no_grant();
    assert_eq!(
        metadata.values("vary"),
        vec!["Access-Control-Request-Headers, Access-Control-Request-Method, Origin"]
    );
    assert_eq!(s.state.verifier.load(Ordering::SeqCst), 0);
    assert_eq!(s.state.calls.load(Ordering::SeqCst), 0);
    s.stop().await;
}
