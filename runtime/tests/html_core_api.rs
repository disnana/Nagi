use nagi_runtime::html::{
    self, HtmlAttributes, HtmlDocument, HtmlFragment, HtmlPolicy, HtmlTag, NavigationUrl,
};
use nagi_runtime::http_server::{self, Status};
use nagi_runtime::Error;
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

fn immutable_owned<T: Send + Sync + 'static>() {}

fn with_test_rate_limits(options: http_server::Options) -> http_server::Options {
    // Finite test-only route catalogue; no test route uses a subject policy.
    // The API requires a positive subject capacity even when it stays unused.
    let limits = http_server::rate_limits(64, 64, 1, 67_108_864, 65_536).unwrap();
    http_server::with_rate_limits(options, limits).unwrap()
}
fn attach_test_rate<S, E>(
    app: http_server::App<S, E>,
    method: http_server::Method,
    path: &str,
) -> http_server::App<S, E> {
    let policy = http_server::rate_policy(1000, 1000).unwrap();
    http_server::rate_route(app, method, path, policy).unwrap()
}

fn document() -> HtmlDocument {
    let policy = html::policy("https://example.invalid").unwrap();
    let link = html::navigation(&policy, "/items/7?view=details").unwrap();
    let attrs = html::href(html::attributes(&policy), link).unwrap();
    let child = html::text(&policy, "通常の item").unwrap();
    let fragment = html::element(&policy, HtmlTag::A, attrs, child).unwrap();
    let copied = html::copy_fragment(&policy, &fragment).unwrap();
    let body = html::join(&policy, fragment, copied).unwrap();
    html::document(&policy, "Item list", body).unwrap()
}

async fn response_handler(
    request: http_server::Request,
    _: Arc<()>,
    _: (),
) -> Result<http_server::Response, Error> {
    let context = http_server::response_context(&request);
    let response = http_server::html_response(context, Status::CREATED, document())?;
    assert_eq!(response.status(), Status::CREATED);
    assert!(response.body().starts_with(b"<!doctype html>"));
    assert!(http_server::append_header_text(response, "Content-Encoding", "gzip").is_err());
    http_server::html_response(context, Status::CREATED, document())
}

#[tokio::test]
async fn opaque_api_owns_data_and_is_transferable() {
    immutable_owned::<HtmlPolicy>();
    immutable_owned::<HtmlAttributes>();
    immutable_owned::<NavigationUrl>();
    immutable_owned::<HtmlFragment>();
    immutable_owned::<HtmlDocument>();
    immutable_owned::<HtmlTag>();
    let app = http_server::route(
        http_server::app_default(()),
        http_server::Method::GET,
        "/",
        http_server::public_policy(),
        response_handler,
    )
    .unwrap();
    let app = attach_test_rate(app, http_server::Method::GET, "/");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let response_limits = http_server::response_limits(
        16_777_216,
        128,
        4,
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
    let options = with_test_rate_limits(
        http_server::with_response_limits(http_server::default_options(), response_limits).unwrap(),
    );
    let options = http_server::authority(
        options,
        "https://localhost",
        vec!["localhost".to_owned()],
        1,
        128,
    )
    .unwrap();
    let (stop, shutdown) = oneshot::channel();
    let task = tokio::spawn(http_server::serve_listener(listener, app, options, async {
        let _ = shutdown.await;
    }));
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    socket
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut raw = Vec::new();
    tokio::time::timeout(
        Duration::from_secs(3),
        socket.take(65537).read_to_end(&mut raw),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(raw.len() <= 65536);
    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap();
    let head = std::str::from_utf8(&raw[..split]).unwrap();
    assert!(head.starts_with("HTTP/1.1 201"));
    assert!(raw[split + 4..].starts_with(b"<!doctype html>"));
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
