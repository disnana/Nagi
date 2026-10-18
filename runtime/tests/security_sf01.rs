use nagi_runtime::{
    auth::{AuthScope, Failure, Grant, VerifiedIdentity},
    http_server as http, Error,
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[derive(Default, Clone)]
struct State {
    observed: Arc<Mutex<Option<AuthScope>>>,
}
async fn verify(_: http::Request, _: Arc<State>) -> Result<VerifiedIdentity, Failure> {
    VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(1))
}
async fn handler(
    request: http::Request,
    state: Arc<State>,
    scope: AuthScope,
) -> Result<http::Response, Error> {
    *state.observed.lock().unwrap() = Some(scope);
    http::text(http::response_context(&request), http::Status::OK, "ready")
}
fn with_test_rate_limits(options: http::Options) -> http::Options {
    // Finite test-only route catalogue; no test route uses a subject policy.
    // The API requires a positive subject capacity even when it stays unused.
    let limits = http::rate_limits(64, 64, 1, 67_108_864, 65_536).unwrap();
    http::with_rate_limits(options, limits).unwrap()
}
fn attach_test_rate<S, E>(
    app: http::App<S, E>,
    method: http::Method,
    path: &str,
) -> http::App<S, E> {
    let policy = http::rate_policy(1000, 1000).unwrap();
    http::rate_route(app, method, path, policy).unwrap()
}
#[tokio::test]
async fn request_end_invalidates_retained_rust_proofs() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::oneshot,
    };
    let state = State::default();
    let app = http::app_default(state.clone());
    let app = http::route(
        app,
        http::Method::GET,
        "/",
        http::authenticated_policy(verify),
        handler,
    )
    .unwrap();
    let app = attach_test_rate(app, http::Method::GET, "/");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let response_limits = http::response_limits(
        16_777_216,
        512,
        16,
        4096,
        32,
        128,
        256,
        150_994_944,
        1024,
        285_212_672,
        4096,
        32,
    )
    .unwrap();
    let (stop, done) = oneshot::channel();
    let server = tokio::spawn(http::serve_listener(
        listener,
        app,
        http::authority(
            with_test_rate_limits(
                http::with_response_limits(http::default_options(), response_limits).unwrap(),
            ),
            "https://localhost",
            vec!["localhost".to_owned()],
            1,
            128,
        )
        .unwrap(),
        async {
            let _ = done.await;
        },
    ));
    let mut socket = TcpStream::connect(address).await.unwrap();
    socket.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer fixture\r\nConnection: close\r\n\r\n").await.unwrap();
    let mut out = String::new();
    tokio::time::timeout(Duration::from_secs(2), socket.read_to_string(&mut out))
        .await
        .unwrap()
        .unwrap();
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    let scope = state.observed.lock().unwrap().take().unwrap();
    assert!(Grant::<()>::from_authorized(scope, 9).is_err());
    stop.send(()).unwrap();
    server.await.unwrap().unwrap();
}
