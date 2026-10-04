use super::*;

async fn healthy(_: Request, _: Arc<()>) -> Result<Response, Error> {
    Ok(text(Status::OK, "still serving"))
}

async fn assert_failed_request_isolated(app: App<(), Error>, head: bool) {
    let app = route(app, Method::GET, "/healthy", healthy).unwrap();
    let server = Server::new(app, capacity(default_options(), 4, 1).unwrap()).await;
    let mut socket = server.connect().await;
    let method = if head { "HEAD" } else { "GET" };
    send(
        &mut socket,
        format!("{method} /panic HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes(),
    )
    .await;
    let failed = response(&mut socket, head).await;
    assert_eq!(failed.status, 500);
    assert_eq!(failed.all("connection"), ["close"]);
    if head {
        assert!(failed.body.is_empty());
    } else {
        assert_eq!(failed.body, b"Internal Server Error");
    }
    assert!(closed(&mut socket).await.is_empty());

    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"GET /healthy HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    let healthy = response(&mut socket, false).await;
    assert_eq!(healthy.status, 200);
    assert_eq!(healthy.body, b"still serving");
    // A healthy request retains the ordinary keep-alive behavior after a
    // failed request on a separate connection released its request permit.
    send(
        &mut socket,
        b"GET /healthy HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    assert_eq!(response(&mut socket, false).await.status, 200);
    drop(socket);
    server.stop().await;
}

#[tokio::test]
async fn http_handler_out_of_bounds_returns_500_and_server_keeps_serving() {
    let app = route(app_default(()), Method::GET, "/panic", |_, _| async {
        let values = [1_i64];
        let index = std::hint::black_box(4_usize);
        let _ = values[index];
        Ok(text(Status::OK, "unreachable"))
    })
    .unwrap();
    assert_failed_request_isolated(app, false).await;
}

#[tokio::test]
async fn http_handler_division_by_zero_returns_500_and_server_keeps_serving() {
    let app = route(app_default(()), Method::GET, "/panic", |_, _| async {
        let zero = std::hint::black_box(0_i64);
        let _ = 10_i64 / zero;
        Ok(text(Status::OK, "unreachable"))
    })
    .unwrap();
    assert_failed_request_isolated(app, false).await;
}

#[tokio::test]
async fn http_handler_panic_after_await_returns_500_without_exposing_payload() {
    async fn handler(_: Request, _: Arc<()>) -> Result<Response, Error> {
        tokio::task::yield_now().await;
        panic!("private handler crash: password=not-for-clients");
    }
    let app = route(app_default(()), Method::GET, "/panic", handler).unwrap();
    assert_failed_request_isolated(app, false).await;
}

#[tokio::test]
async fn http_handler_constructor_panic_returns_500() {
    let app = route(
        app_default(()),
        Method::GET,
        "/panic",
        |_, _| -> std::future::Ready<Result<Response, Error>> {
            panic!("private native future-construction failure")
        },
    )
    .unwrap();
    assert_failed_request_isolated(app, false).await;
}

fn failing_mapper(_: Error) -> Response {
    panic!("private error-mapper crash")
}

#[tokio::test]
async fn http_error_mapper_panic_returns_500() {
    let app = route_mapped(
        app_default(()),
        Method::GET,
        "/panic",
        |_, _| async { Err(Error::invalid("expected business error")) },
        failing_mapper,
    )
    .unwrap();
    assert_failed_request_isolated(app, false).await;
}

#[tokio::test]
async fn http_handler_panic_head_response_has_no_body() {
    let app = route(
        app_default(()),
        Method::GET,
        "/panic",
        |_, _| -> std::future::Ready<Result<Response, Error>> {
            panic!("private HEAD handler crash")
        },
    )
    .unwrap();
    assert_failed_request_isolated(app, true).await;
}
