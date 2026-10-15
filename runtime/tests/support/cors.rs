#![allow(dead_code)]
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
pub(crate) const UI: &str = "https://app.example.invalid";
#[derive(Default)]
pub(crate) struct State {
    pub(crate) calls: AtomicUsize,
    pub(crate) verifier: AtomicUsize,
    pub(crate) mapper: AtomicUsize,
    pub(crate) entered: Notify,
    pub(crate) release: Notify,
    pub(crate) cancelled: AtomicUsize,
    pub(crate) cancel_seen: Notify,
}
pub(crate) fn cors(mode: http::CorsCredentials) -> http::CorsPolicy {
    cors_origins(mode, UI, "https://localhost")
}
fn cors_origins(
    mode: http::CorsCredentials,
    ui_origin: &str,
    app_origin: &str,
) -> http::CorsPolicy {
    http::cors_policy(
        vec![ui_origin.into(), app_origin.into()],
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
pub(crate) fn mapped(_: Error) -> http::Response {
    http::text(http::Status::INTERNAL_SERVER_ERROR, "mapped")
}
pub(crate) struct Server {
    pub(crate) address: std::net::SocketAddr,
    pub(crate) state: Arc<State>,
    pub(crate) stop: Option<oneshot::Sender<()>>,
    pub(crate) task: tokio::task::JoinHandle<Result<(), Error>>,
}
impl Server {
    pub(crate) async fn new(capacity: i64, handler_ms: i64) -> Self {
        Self::browser(capacity, handler_ms, "https://localhost", UI).await
    }
    pub(crate) async fn browser(
        capacity: i64,
        handler_ms: i64,
        app_origin: &str,
        ui_origin: &str,
    ) -> Self {
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
            cors_origins(http::CorsCredentials::ALLOW, ui_origin, app_origin),
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
            cors_origins(http::CorsCredentials::ALLOW, ui_origin, app_origin),
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
            cors_origins(http::CorsCredentials::OMIT, ui_origin, app_origin),
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
            cors_origins(http::CorsCredentials::ALLOW, ui_origin, app_origin),
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
            cors_origins(http::CorsCredentials::ALLOW, ui_origin, app_origin),
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
            cors_origins(http::CorsCredentials::ALLOW, ui_origin, app_origin),
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
            cors_origins(http::CorsCredentials::ALLOW, ui_origin, app_origin),
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
            cors_origins(http::CorsCredentials::ALLOW, ui_origin, app_origin),
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
            cors_origins(http::CorsCredentials::ALLOW, ui_origin, app_origin),
        )
        .unwrap();
        let options = http::authority(
            http::capacity(
                http::options(32, 1000, handler_ms, 1000).unwrap(),
                8,
                capacity,
            )
            .unwrap(),
            app_origin,
            vec![app_origin.strip_prefix("https://").unwrap().into()],
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
    pub(crate) async fn stop(mut self) {
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
pub(crate) struct Reply {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}
impl Reply {
    pub(crate) fn values(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
            .collect()
    }
    pub(crate) fn grant(&self) {
        assert_eq!(self.values("access-control-allow-origin"), vec![UI]);
        assert_eq!(
            self.values("access-control-allow-credentials"),
            vec!["true"]
        );
        assert_eq!(self.values("content-security-policy").len(), 1);
        assert_eq!(self.values("x-content-type-options"), vec!["nosniff"]);
    }
    pub(crate) fn no_grant(&self) {
        assert!(self
            .headers
            .iter()
            .all(|(k, _)| !k.to_ascii_lowercase().starts_with("access-control-")));
    }
}
pub(crate) async fn read(stream: TcpStream) -> Reply {
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
pub(crate) async fn start(
    s: &Server,
    method: &str,
    path: &str,
    extra: &str,
    body: &[u8],
) -> TcpStream {
    let mut stream = TcpStream::connect(s.address).await.unwrap();
    let head=format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n{extra}\r\n",body.len());
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();
    stream
}
pub(crate) async fn request(
    s: &Server,
    method: &str,
    path: &str,
    extra: &str,
    body: &[u8],
) -> Reply {
    read(start(s, method, path, extra, body).await).await
}
pub(crate) fn preflight(method: &str, extra: &str) -> String {
    format!("Origin: {UI}\r\nAccess-Control-Request-Method: {method}\r\n{extra}")
}
