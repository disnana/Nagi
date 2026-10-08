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
    _: http::Request,
    state: Arc<State>,
    scope: AuthScope,
) -> Result<http::Response, Error> {
    *state.observed.lock().unwrap() = Some(scope);
    Ok(http::text(http::Status::OK, "ready"))
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
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, done) = oneshot::channel();
    let server = tokio::spawn(http::serve_listener(
        listener,
        app,
        http::default_options(),
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
