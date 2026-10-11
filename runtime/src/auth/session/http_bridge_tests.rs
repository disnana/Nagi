use crate::http_server::*;
use crate::{auth::VerifiedIdentity, Error};
use axum::http::Method;
use std::time::{Duration, Instant};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
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
            options,
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

async fn bearer_server() -> Server {
    let app = route(
        app_default(()),
        Method::POST,
        "/owned",
        authenticated_policy(|_, _| async {
            VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(60))
        }),
        |_, _, _| async { Ok(text(Status::OK, "owned")) },
    )
    .unwrap();
    Server::new(app, options(4, 1000, 1000, 1000).unwrap()).await
}
#[tokio::test]
async fn bearer_success_is_no_store() {
    let server = bearer_server().await;
    let mut socket = server.connect().await;
    send(&mut socket,b"POST /owned HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer owned\r\nContent-Length: 0\r\n\r\n").await;
    let reply = response(&mut socket, false).await;
    assert_eq!(reply.status, 200);
    assert_eq!(reply.all("cache-control"), vec!["no-store"]);
    server.stop().await;
}
#[tokio::test]
async fn bearer_body_rejection_is_no_store() {
    let server = bearer_server().await;
    let mut socket = server.connect().await;
    send(&mut socket,b"POST /owned HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer owned\r\nContent-Length: 5\r\n\r\nsmall").await;
    let reply = response(&mut socket, false).await;
    assert_eq!(reply.status, 413);
    assert_eq!(reply.all("cache-control"), vec!["no-store"]);
    server.stop().await;
}

use super::super::{self as session, secrets, Store};
use crate::{
    auth::{AuthScope, Grant, LeaseOwner},
    sqlite,
};
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
};
static NEXT_HTTP_DB: AtomicU64 = AtomicU64::new(1);
struct CookieFixture {
    directory: std::path::PathBuf,
    pool: sqlite::Pool,
    store: Store,
    cookie: String,
}
impl CookieFixture {
    async fn new() -> Self {
        let directory = loop {
            let id = NEXT_HTTP_DB.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("owned-http-session-{}-{id}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => break path,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("owned fixture: {error}"),
            }
        };
        let pool = sqlite::open(
            directory.join("owned.sqlite").to_str().unwrap(),
            sqlite::options(2, 4, 1000, 1000).unwrap(),
        )
        .await
        .unwrap();
        let config = session::options(2, 3, 72, 1, 10000, 100000, 5000, 2).unwrap();
        let cookie_config =
            session::cookie_options("__Host-owned", session::SameSite::Strict).unwrap();
        let store = session::open(&pool, config, cookie_config).await.unwrap();
        // Trusted private fixture uses actual production entropy/digest/insert.
        // The raw ID exists only in RAM request material, never a public getter.
        let id = secrets::SessionId::generate().unwrap_or_else(|_| panic!("entropy unavailable"));
        let digest = id.digest().into_bytes();
        store
            .foundation
            .insert(7, digest, Instant::now() + Duration::from_secs(30))
            .await
            .unwrap();
        let wire = secrets::CookiePolicy::new("__Host-owned", cookie::SameSite::Strict)
            .unwrap_or_else(|_| panic!("policy invalid"))
            .issue(id)
            .unwrap_or_else(|_| panic!("cookie unavailable"))
            .into_header();
        let cookie = wire.to_str().unwrap().split(';').next().unwrap().to_owned();
        Self {
            directory,
            pool,
            store,
            cookie,
        }
    }
    async fn stop(self) {
        drop(self.store);
        sqlite::close(&self.pool, 2000).await.unwrap();
        std::fs::remove_dir_all(self.directory).unwrap();
    }
}
async fn cookie_request(server: &Server, method: &str, extra: &str, body: &str) -> Received {
    let mut socket = server.connect().await;
    let raw = format!(
        "{method} /owned HTTP/1.1\r\nHost: localhost\r\n{extra}Content-Length: {}\r\n\r\n{body}",
        body.len()
    );
    send(&mut socket, raw.as_bytes()).await;
    response(&mut socket, method == "HEAD").await
}
fn assert_private(reply: &Received, expected: u16) {
    assert_eq!(reply.status, expected);
    assert_eq!(reply.all("cache-control"), vec!["no-store"]);
    assert_eq!(reply.all("vary"), vec!["Cookie"]);
    assert!(reply.all("set-cookie").is_empty());
}
async fn session_server(f: &CookieFixture, calls: Arc<AtomicUsize>) -> Server {
    let app = route(
        app_default(()),
        Method::POST,
        "/owned",
        session_authenticated_policy(session::clone_store(&f.store)),
        move |_, _, scope| {
            calls.fetch_add(1, Ordering::SeqCst);
            async move {
                assert_eq!(scope.subject(), 7);
                Ok(text(Status::OK, "owned"))
            }
        },
    )
    .unwrap();
    Server::new(app, options(4, 1000, 1000, 1000).unwrap()).await
}
#[tokio::test]
async fn session_real_lookup_binds_request_and_classifies_all_credentials() {
    let fixture = CookieFixture::new().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let server = session_server(&fixture, Arc::clone(&calls)).await;
    let valid = format!("Cookie: {}\r\n", fixture.cookie);
    assert_private(&cookie_request(&server, "POST", &valid, "").await, 200);
    let missing = cookie_request(&server, "POST", "", "").await;
    assert_private(&missing, 401);
    assert_eq!(missing.all("www-authenticate"), vec!["Session"]);
    assert_private(
        &cookie_request(&server, "POST", "Cookie: __Host-owned=\r\n", "").await,
        401,
    );
    assert_private(
        &cookie_request(&server, "POST", "Cookie: __Host-owned=ordinary\r\n", "").await,
        401,
    );
    assert_private(
        &cookie_request(&server, "POST", &format!("{valid}{valid}"), "").await,
        400,
    );
    assert_private(
        &cookie_request(
            &server,
            "POST",
            &format!("{valid}Authorization: Bearer owned\r\n"),
            "",
        )
        .await,
        400,
    );
    assert_private(
        &cookie_request(
            &server,
            "POST",
            &format!("{valid}Cookie: malformed\r\n"),
            "",
        )
        .await,
        400,
    );
    assert_private(&cookie_request(&server, "POST", &valid, "small").await, 413);
    assert_private(&cookie_request(&server, "POST", &valid, "").await, 200);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    server.stop().await;
    fixture.stop().await;
}
#[tokio::test]
async fn session_authorizer_receives_real_metadata_and_foreign_grant_is_refused() {
    struct Owned;
    let fixture = CookieFixture::new().await;
    let app = route(
        app_default(()),
        Method::POST,
        "/owned",
        session_authorized_policy(session::clone_store(&fixture.store), |scope, _, _| async {
            assert_eq!(scope.subject(), 7);
            assert!(matches!(
                &scope.credential.source,
                crate::auth::CredentialSource::Session(_)
            ));
            assert!(
                scope.credential.original_expires_at
                    > scope.lease.checked_gate().unwrap().expires_at
            );
            Grant::<Owned>::from_authorized(scope, 9)
        }),
        |_, _, grant| async move {
            grant.validate().unwrap();
            Ok(text(Status::OK, "owned"))
        },
    )
    .unwrap();
    let server = Server::new(app, options(4, 1000, 1000, 1000).unwrap()).await;
    let valid = format!("Cookie: {}\r\n", fixture.cookie);
    assert_private(&cookie_request(&server, "POST", &valid, "").await, 200);
    server.stop().await;
    let foreign_owner = Arc::new(LeaseOwner::new(Instant::now() + Duration::from_secs(5)).unwrap());
    let retained = Arc::clone(&foreign_owner);
    let app = route(
        app_default(()),
        Method::POST,
        "/owned",
        session_authorized_policy(session::clone_store(&fixture.store), move |_, _, _| {
            let owner = Arc::clone(&retained);
            async move {
                let identity =
                    VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(30))
                        .unwrap();
                Grant::<Owned>::from_authorized(
                    AuthScope::bind(identity, Arc::clone(&owner.lease)).unwrap(),
                    9,
                )
            }
        }),
        |_, _, _| async {
            panic!("foreign authority reached handler");
            #[allow(unreachable_code)]
            Ok(text(Status::OK, "owned"))
        },
    )
    .unwrap();
    let server = Server::new(app, options(4, 1000, 1000, 1000).unwrap()).await;
    assert_private(&cookie_request(&server, "POST", &valid, "").await, 400);
    server.stop().await;
    drop(foreign_owner);
    fixture.stop().await;
}
#[tokio::test]
async fn session_head_mapper_panic_timeout_and_304_use_managed_finalizer() {
    let fixture = CookieFixture::new().await;
    let valid = format!("Cookie: {}\r\n", fixture.cookie);
    let app = route(
        app_default(()),
        Method::GET,
        "/owned",
        session_authenticated_policy(session::clone_store(&fixture.store)),
        |_, _, _| async { Ok(text(Status::OK, "owned")) },
    )
    .unwrap();
    let server = Server::new(app, options(4, 1000, 1000, 1000).unwrap()).await;
    let head = cookie_request(&server, "HEAD", &valid, "").await;
    assert_private(&head, 200);
    assert!(head.body.is_empty());
    server.stop().await;
    for (mode, status) in [(0, 400), (1, 500), (2, 504), (3, 500)] {
        let app = route(
            app_default(()),
            Method::POST,
            "/owned",
            session_authenticated_policy(session::clone_store(&fixture.store)),
            move |_, _, _| async move {
                match mode {
                    0 => Err(Error::invalid("owned business error")),
                    1 => panic!("owned handler panic"),
                    2 => std::future::pending::<Result<Response, Error>>().await,
                    _ => Ok(empty(crate::http_server::status(304).unwrap())),
                }
            },
        )
        .unwrap();
        let server = Server::new(app, options(4, 1000, 20, 1000).unwrap()).await;
        assert_private(&cookie_request(&server, "POST", &valid, "").await, status);
        server.stop().await;
    }
    fixture.stop().await;
}
#[tokio::test]
async fn session_bad_host_precedes_closed_pool_lookup_and_cookie_bounds_are_finite() {
    let fixture = CookieFixture::new().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let server = session_server(&fixture, Arc::clone(&calls)).await;
    let valid = format!("Cookie: {}\r\n", fixture.cookie);
    // Real Pool shutdown is an intentional finite negative, not normal lifecycle.
    sqlite::close(&fixture.pool, 2000).await.unwrap();
    let mut socket = server.connect().await;
    let raw =
        format!("POST /owned HTTP/1.1\r\nHost: other.invalid\r\n{valid}Content-Length: 0\r\n\r\n");
    send(&mut socket, raw.as_bytes()).await;
    let reply = response(&mut socket, false).await;
    assert_eq!(reply.status, 400);
    assert_eq!(reply.all("cache-control"), vec!["no-store"]);
    assert!(reply.all("set-cookie").is_empty());
    assert_private(&cookie_request(&server, "POST", &valid, "").await, 503);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.stop().await;
    fixture.stop().await;
    let fixture = CookieFixture::new().await;
    let mut headers = hyper::HeaderMap::new();
    for _ in 0..65 {
        headers.append(
            hyper::header::COOKIE,
            hyper::header::HeaderValue::from_static("owned=value"),
        );
    }
    let failure = fixture
        .store
        .lookup_http_identity(&headers)
        .await
        .err()
        .unwrap();
    assert_eq!(failure.kind(), crate::auth::FailureKind::InvalidRequest);
    fixture.stop().await;
}
