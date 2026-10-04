use super::*;
use axum::body::Body;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    sync::{oneshot, Notify},
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
async fn closed(socket: &mut BufReader<TcpStream>) -> Vec<u8> {
    let mut rest = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), socket.read_to_end(&mut rest))
        .await
        .unwrap()
        .unwrap();
    rest
}
fn basic_request() -> Request {
    Request {
        method: Method::GET,
        uri: "/users/7?q=yes".parse().unwrap(),
        headers: HeaderMap::new(),
        body: Bytes::from_static(b"body"),
    }
}

#[test]
fn status_is_compact_validated_and_has_modern_compatible_names() {
    assert_eq!(std::mem::size_of::<Status>(), 2);
    assert_eq!(Status::OK.value(), 200);
    assert_eq!(Status::CREATED.phrase(), "Created");
    assert!(Status::CREATED.is_success());
    assert!(Status::NOT_MODIFIED.is_redirection());
    assert!(Status::UNAUTHORIZED.is_client_error());
    assert!(Status::SERVICE_UNAVAILABLE.is_server_error());
    assert_eq!(Status::CONTENT_TOO_LARGE, Status::REQUEST_ENTITY_TOO_LARGE);
    assert_eq!(Status::UNPROCESSABLE_CONTENT, Status::UNPROCESSABLE_ENTITY);
    for value in [-1, 0, 99, 100, 101, 199, 600, 1000, i64::MAX] {
        assert!(status(value).is_err(), "{value}");
    }
    for value in [200, 299, 399, 499, 599] {
        assert_eq!(status(value).unwrap().value(), value);
    }
    assert_eq!(status(299).unwrap().phrase(), "");
}

#[test]
fn method_status_and_shared_state_inspection_allocate_nothing() {
    struct State {
        value: i64,
    }
    let state = Arc::new(State { value: 7 });
    let request = basic_request();
    let (_, count) = crate::metrics::measure(|| {
        assert_eq!(Status::UNAUTHORIZED.value(), 401);
        assert_eq!(Status::UNAUTHORIZED.phrase(), "Unauthorized");
        assert!(Status::UNAUTHORIZED.is_client_error());
        assert!(request.is_get());
        assert_eq!(method_name(&request.method), "GET");
        assert_eq!(request.path(), "/users/7");
        assert_eq!(Arc::clone(&state).value, 7);
        assert_eq!(status(299).unwrap().value(), 299);
    });
    assert_eq!(count.allocations, 0);
    assert_eq!(count.reallocations, 0);
}

#[test]
fn heterogeneous_route_erasure_adds_exactly_one_future_allocation() {
    let app = route(app_default("state".to_owned()), Method::GET, "/", hello).unwrap();
    let request = basic_request();
    let (future, count) =
        crate::metrics::measure(|| (app.routes["/"][0].handler)(request, Arc::clone(&app.state)));
    assert_eq!(count.allocations, 1);
    assert_eq!(count.reallocations, 0);
    assert!(count.allocated_bytes > 0);
    drop(future);
}

#[test]
fn native_method_and_request_fields_do_not_stringify_or_normalize_extension_tokens() {
    let mut request = basic_request();
    assert!(request.is_get());
    assert_eq!(request.path(), "/users/7");
    assert_eq!(request.query(), Some("q=yes"));
    assert_eq!(request.body(), b"body");
    request.method = method("CUSTOM").unwrap();
    assert_eq!(method_name(&request.method), "CUSTOM");
    assert!(!request.is_get());
    assert_ne!(method("get").unwrap(), Method::GET);
    for value in ["", "G ET", "GET\r\n", "あ"] {
        assert!(method(value).is_err());
    }
}

#[test]
fn header_access_preserves_duplicates_and_checks_single_value_and_utf8() {
    let mut request = basic_request();
    request
        .headers
        .append("authorization", HeaderValue::from_static("Bearer first"));
    assert_eq!(
        header_text(&request, "Authorization").unwrap(),
        Some("Bearer first")
    );
    request
        .headers
        .append("authorization", HeaderValue::from_static("Bearer second"));
    assert!(header(&request, "authorization").is_err());
    assert!(header_text(&request, "authorization").is_err());
    assert_eq!(
        headers(&request, "authorization").unwrap(),
        [b"Bearer first".as_slice(), b"Bearer second".as_slice()]
    );
    request
        .headers
        .insert("x-raw", HeaderValue::from_bytes(b"\xff").unwrap());
    assert_eq!(header(&request, "x-raw").unwrap(), Some(b"\xff".as_slice()));
    assert!(header_text(&request, "x-raw").is_err());
    assert_eq!(header_text(&request, "missing").unwrap(), None);
    assert!(header(&request, "invalid name").is_err());
}

#[test]
fn json_content_type_matches_tokens_and_validates_every_parameter() {
    let mut request = basic_request();
    assert!(!is_json_content_type(&request).unwrap());
    for value in [
        "application/json",
        "APPLICATION/JSON",
        " \tapplication/json\t ",
        "application/json; charset=utf-8",
        "Application/Json \t; Charset=\"UTF-8\"; profile=\"a;b=\\\"c\"",
        "application/json; charset=iso-8859-1",
        "application/json;;; ; charset=utf-8;",
        "application/json; empty=\"\"",
    ] {
        request.headers.insert(
            names::CONTENT_TYPE,
            HeaderValue::from_bytes(value.as_bytes()).unwrap(),
        );
        assert!(is_json_content_type(&request).unwrap(), "{value:?}");
    }
    for value in [
        "text/plain",
        "application/problem+json",
        "application/jsonp",
        "application/json-seq; charset=utf-8",
    ] {
        request.headers.insert(
            names::CONTENT_TYPE,
            HeaderValue::from_bytes(value.as_bytes()).unwrap(),
        );
        assert!(!is_json_content_type(&request).unwrap(), "{value:?}");
    }
    for value in [
        "",
        "application",
        "/json",
        "application/",
        "application /json",
        "application/ json",
        "application/json, text/plain",
        "application/json trailing",
        "application/json; charset",
        "application/json; charset=",
        "application/json; charset =utf-8",
        "application/json; charset= utf-8",
        "application/json; charset=\"unfinished",
        "application/json; charset=\"escaped\\",
        "application/json; charset=\"utf-8\"tail",
        "text/plain; bad=",
    ] {
        request.headers.insert(
            names::CONTENT_TYPE,
            HeaderValue::from_bytes(value.as_bytes()).unwrap(),
        );
        assert!(
            matches!(
                is_json_content_type(&request).unwrap_err().kind,
                ErrorKind::Invalid
            ),
            "{value:?}"
        );
    }
    request.headers.insert(
        names::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    request.headers.append(
        names::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    assert!(is_json_content_type(&request).is_err());
}

#[test]
fn media_type_parser_rejects_controls_but_accepts_quoted_http_octets() {
    for value in [
        b"application/json\r\n".as_slice(),
        b"application/json; p=\"\x00\"",
        b"application/json; p=\"\x7f\"",
        b"application/json; p=\"\\\x00\"",
        b"application/json; p=\"\\\x7f\"",
        b"application/json; p=\xff",
        b"application/\xff",
    ] {
        assert!(media_type(value).is_none(), "{value:?}");
    }
    for value in [
        b"application/json; p=\"\xff\"".as_slice(),
        b"application/json; p=\"\\\xff\"",
        b"application/json; p=\"\t\"",
    ] {
        assert!(media_type(value).is_some(), "{value:?}");
    }
}

#[test]
fn successful_json_content_type_checks_allocate_nothing_and_preserve_body() {
    let mut request = basic_request();
    request.headers.insert(
        names::CONTENT_TYPE,
        HeaderValue::from_static("Application/JSON; charset=\"UTF-8\"; profile=\"a;b\""),
    );
    let body_pointer = request.body().as_ptr();
    let (_, allocations) = crate::metrics::measure(|| {
        for _ in 0..512 {
            assert!(is_json_content_type(&request).unwrap());
            assert_eq!(request.body().as_ptr(), body_pointer);
        }
    });
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
    request
        .headers
        .insert(names::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
    let (_, allocations) = crate::metrics::measure(|| {
        for _ in 0..512 {
            assert!(!is_json_content_type(&request).unwrap());
        }
    });
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
    assert_eq!(request.body(), b"body");
}

#[test]
fn standard_custom_mixed_case_and_long_borrowed_header_lookups_allocate_nothing() {
    let mut request = basic_request();
    request
        .headers
        .insert("authorization", HeaderValue::from_static("Bearer ok"));
    request
        .headers
        .insert("x-custom", HeaderValue::from_static("custom"));
    let long = format!("X-{}", "a".repeat(600));
    request.headers.insert(
        HeaderName::from_bytes(long.as_bytes()).unwrap(),
        HeaderValue::from_static("long"),
    );
    let (_, allocations) = crate::metrics::measure(|| {
        for _ in 0..512 {
            assert_eq!(
                header_text(&request, "Authorization").unwrap(),
                Some("Bearer ok")
            );
            assert_eq!(header_text(&request, "X-CUSTOM").unwrap(), Some("custom"));
            assert_eq!(
                header(&request, "x-CuStOm").unwrap(),
                Some(b"custom".as_slice())
            );
            assert_eq!(header_text(&request, &long).unwrap(), Some("long"));
            assert_eq!(header_text(&request, "x-not-present").unwrap(), None);
        }
    });
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
}

#[test]
fn borrowed_lookup_validation_agrees_with_native_token_rules_and_length_limits() {
    let request = basic_request();
    for byte in 0u8..=127 {
        let name = char::from(byte).to_string();
        assert_eq!(
            header(&request, &name).is_ok(),
            HeaderName::from_bytes(name.as_bytes()).is_ok(),
            "{byte}"
        );
    }
    for name in ["", "x\r\n", "a:b", "あ", "x-name "] {
        assert!(header(&request, name).is_err());
        assert!(headers(&request, name).is_err());
    }
    assert!(header(&request, &"x".repeat(65535)).is_ok());
    assert!(header(&request, &"x".repeat(65536)).is_err());
}

#[test]
fn response_headers_reject_injection_and_user_framing_and_keep_cookie_lines() {
    for (name, value) in [
        ("bad name", b"v".as_slice()),
        ("valid", b"a\r\nb"),
        ("Content-Length", b"100"),
        ("TRANSFER-ENCODING", b"chunked"),
    ] {
        assert!(append_header(empty(Status::OK), name, value).is_err());
    }
    assert!(append_header_text(empty(Status::OK), "x-value", "a\r\nb").is_err());
    let result = append_header(
        append_header(text(Status::CREATED, "ok"), "set-cookie", b"a=1").unwrap(),
        "Set-Cookie",
        b"b=2",
    )
    .unwrap()
    .into_http(false);
    assert_eq!(
        result
            .headers()
            .get_all(names::SET_COOKIE)
            .iter()
            .map(HeaderValue::as_bytes)
            .collect::<Vec<_>>(),
        [b"a=1".as_slice(), b"b=2".as_slice()]
    );
    assert_eq!(result.status(), 201);
    assert!(!result.headers().contains_key(names::CONTENT_LENGTH));
}

#[test]
fn ordinary_response_framing_uses_the_native_body_length_without_allocating_a_header_value() {
    let (_, normal) = crate::metrics::measure(|| text(Status::OK, "hello").into_http(false));
    let (_, head) = crate::metrics::measure(|| text(Status::OK, "hello").into_http(true));
    let (_, framing) = crate::metrics::measure(|| HeaderValue::from(5usize));
    // The locked native builder allocates the decimal buffer and, because its
    // capacity exceeds the length, Bytes ownership metadata. Ordinary frames
    // need neither allocation; Hyper writes their exact length itself.
    assert_eq!(normal.allocations, 3);
    assert_eq!(framing.allocations, 2);
    assert_eq!(head.allocations, normal.allocations + framing.allocations);
    assert_eq!(
        head.allocated_bytes,
        normal.allocated_bytes + framing.allocated_bytes
    );
    assert_eq!(
        framing.allocated_bytes,
        (std::mem::size_of::<usize>() * 3 + if usize::BITS == 64 { 20 } else { 10 }) as u64
    );
    assert_eq!(normal.reallocations, 0);
    assert_eq!(head.reallocations, 0);
}

#[test]
fn json_encoding_is_fallible_and_default_internal_mapping_hides_details() {
    struct Fails;
    impl Serialize for Fails {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("private serializer failure"))
        }
    }
    let error = json(Status::OK, &Fails).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::Internal));
    let response = default_error(error);
    assert_eq!(response.status(), Status::INTERNAL_SERVER_ERROR);
    assert_eq!(response.body(), b"internal error");
    let response = json(Status::CREATED, &vec![1, 2]).unwrap();
    assert_eq!(response.status(), Status::CREATED);
    assert_eq!(response.body(), b"[1,2]");
}

#[test]
fn options_validate_capacity_deadlines_and_leave_state_out_of_debug() {
    assert!(options(0, 1, 1, 1).is_err());
    assert!(options(1, 0, 1, 1).is_err());
    assert!(options(1, 1, -1, 1).is_err());
    assert!(options(1, 1, 1, 0).is_err());
    assert!(capacity(default_options(), 0, 1).is_err());
    assert!(capacity(default_options(), 1, -1).is_err());
    assert!(header_timeout(default_options(), 0).is_err());
    assert!(send_timeout(default_options(), 0).is_err());
    assert!(header_limits(default_options(), 8191, 100).is_err());
    assert!(header_limits(default_options(), 8192, 0).is_err());
    assert!(header_limits(default_options(), 2 * 1024 * 1024, 100).is_err());
    assert!(header_limits(default_options(), 8192, 3000).is_err());
    assert!(!format!("{:?}", app_default("very secret state")).contains("secret"));
}

async fn hello(_: Request, state: Arc<String>) -> Result<Response, Error> {
    Ok(text(Status::OK, &state))
}
#[test]
fn route_validation_returns_errors_without_panicking_and_supports_templates() {
    for path in [
        "",
        "relative",
        "/bad?query",
        "/bad#fragment",
        "/bad path",
        "/bad/{",
        "/bad/{}}",
    ] {
        assert!(
            route(app_default(String::new()), Method::GET, path, hello).is_err(),
            "{path}"
        );
    }
    let app = route(
        app_default(String::new()),
        Method::GET,
        "/users/{id}",
        hello,
    )
    .unwrap();
    assert!(route(app, Method::GET, "/users/{name}", hello).is_err());
    let app = route(app_default(String::new()), Method::GET, "/", hello).unwrap();
    assert!(route(app, Method::GET, "/", hello).is_err());
}

#[tokio::test]
async fn body_collection_moves_one_chunk_and_combines_only_multiple_chunks() {
    let bytes = Bytes::from(vec![1, 2, 3]);
    let pointer = bytes.as_ptr();
    let result = receive_body(Body::from(bytes), 3).await.ok().unwrap();
    assert_eq!(result.as_ptr(), pointer);
    let stream = futures_util::stream::iter([
        Ok::<_, std::io::Error>(Bytes::from_static(b"a")),
        Ok(Bytes::from_static(b"bc")),
    ]);
    assert_eq!(
        receive_body(Body::from_stream(stream), 3)
            .await
            .ok()
            .unwrap(),
        b"abc".as_slice()
    );
    assert!(matches!(
        receive_body(Body::from("abcd"), 3).await,
        Err(BodyFailure::TooLarge)
    ));
}

#[tokio::test]
async fn buffered_native_response_body_is_unboxed_exact_and_moves_its_frame() {
    use hyper::body::Body as _;
    let bytes = Bytes::from(vec![1, 2, 3]);
    let pointer = bytes.as_ptr();
    let (mut body, allocations) = crate::metrics::measure(|| BufferedBody { bytes });
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
    assert_eq!(body.size_hint().exact(), Some(3));
    assert!(!body.is_end_stream());
    let frame = futures_util::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))
        .await
        .unwrap()
        .unwrap()
        .into_data()
        .unwrap();
    assert_eq!(frame.as_ptr(), pointer);
    assert_eq!(frame.as_ref(), [1, 2, 3]);
    assert!(body.is_end_stream());
    assert_eq!(body.size_hint().exact(), Some(0));
    assert!(
        futures_util::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn apps_are_db_free_independent_and_support_native_extended_methods_and_template_paths() {
    let app = route(
        app_default("first".to_owned()),
        Method::GET,
        "/users/{id}",
        hello,
    )
    .unwrap();
    let app = route(app, method("CUSTOM").unwrap(), "/users/{id}", hello).unwrap();
    let first = Server::new(app, default_options()).await;
    let second = Server::new(
        route(
            app_default("second".to_owned()),
            Method::GET,
            "/users/{id}",
            hello,
        )
        .unwrap(),
        default_options(),
    )
    .await;
    for (server, method, expected) in [
        (&first, "GET", b"first".as_slice()),
        (&first, "CUSTOM", b"first".as_slice()),
        (&second, "GET", b"second".as_slice()),
    ] {
        let mut socket = server.connect().await;
        send(
            &mut socket,
            format!("{method} /users/42 HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes(),
        )
        .await;
        let result = response(&mut socket, false).await;
        assert_eq!(result.status, 200);
        assert_eq!(result.body, expected);
    }
    first.stop().await;
    second.stop().await;
}

#[tokio::test]
async fn successful_connect_is_rejected_instead_of_establishing_an_unowned_tunnel() {
    let app = route(
        app_default("cannot become a tunnel".to_owned()),
        Method::CONNECT,
        "/",
        hello,
    )
    .unwrap();
    let server = Server::new(app, default_options()).await;
    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"CONNECT / HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    let result = response(&mut socket, false).await;
    assert_eq!(result.status, 501);
    assert_eq!(result.all("connection"), ["close"]);
    assert!(closed(&mut socket).await.is_empty());
    server.stop().await;
}

#[tokio::test]
async fn oversized_headers_and_header_counts_are_rejected_without_reusing_the_connection() {
    let app = route(app_default("ok".to_owned()), Method::GET, "/", hello).unwrap();
    let server = Server::new(app, header_limits(default_options(), 8192, 4).unwrap()).await;
    let mut socket = server.connect().await;
    let request = format!(
        "GET / HTTP/1.1\r\nHost: localhost\r\nX-Large: {}\r\n\r\n",
        "x".repeat(16 * 1024)
    );
    send(&mut socket, request.as_bytes()).await;
    let result = response(&mut socket, false).await;
    assert_eq!(result.status, 431);
    assert!(closed(&mut socket).await.is_empty());
    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"GET / HTTP/1.1\r\nHost: localhost\r\nX-A: a\r\nX-B: b\r\nX-C: c\r\nX-D: d\r\n\r\n",
    )
    .await;
    assert_eq!(response(&mut socket, false).await.status, 431);
    assert!(closed(&mut socket).await.is_empty());
    server.stop().await;
    let app = route(app_default("ok".to_owned()), Method::GET, "/", hello).unwrap();
    let server = Server::new(app, default_options()).await;
    let mut socket = server.connect().await;
    for (additional, expected) in [(99, 200), (100, 431)] {
        let mut request = String::from("GET / HTTP/1.1\r\nHost: localhost\r\n");
        for index in 0..additional {
            use std::fmt::Write as _;
            writeln!(&mut request, "X-{index}: value\r").unwrap();
        }
        request.push_str("\r\n");
        send(&mut socket, request.as_bytes()).await;
        assert_eq!(response(&mut socket, false).await.status, expected);
    }
    assert!(closed(&mut socket).await.is_empty());
    server.stop().await;
}

#[tokio::test]
async fn head_uses_get_representation_length_explicit_head_wins_and_allow_is_correct() {
    async fn head_only(_: Request, _: Arc<String>) -> Result<Response, Error> {
        Ok(text(Status::CREATED, "explicit"))
    }
    let app = route(app_default("hello".to_owned()), Method::GET, "/", hello).unwrap();
    let app = route(app, Method::GET, "/explicit", hello).unwrap();
    let app = route(app, Method::HEAD, "/explicit", head_only).unwrap();
    let server = Server::new(app, default_options()).await;
    let mut socket = server.connect().await;
    send(&mut socket, b"HEAD / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    let result = response(&mut socket, true).await;
    assert_eq!(result.status, 200);
    assert_eq!(result.all("content-length"), ["5"]);
    assert!(result.body.is_empty());
    send(&mut socket, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    let result = response(&mut socket, false).await;
    assert_eq!(result.status, 200);
    assert_eq!(result.all("content-length"), ["5"]);
    assert_eq!(result.body, b"hello");
    send(
        &mut socket,
        b"HEAD /explicit HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    let result = response(&mut socket, true).await;
    assert_eq!(result.status, 201);
    assert_eq!(result.all("content-length"), ["8"]);
    send(
        &mut socket,
        b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 100\r\n\r\n",
    )
    .await;
    let result = response(&mut socket, false).await;
    assert_eq!(result.status, 405);
    assert_eq!(result.all("allow"), ["GET, HEAD"]);
    assert_eq!(result.all("connection"), ["close"]);
    assert!(closed(&mut socket).await.is_empty());
    server.stop().await;
}

#[derive(Debug)]
enum Business {
    Auth,
    Conflict,
    Limited,
}
fn business(error: Business) -> Response {
    text(
        match error {
            Business::Auth => Status::UNAUTHORIZED,
            Business::Conflict => Status::CONFLICT,
            Business::Limited => Status::TOO_MANY_REQUESTS,
        },
        "default",
    )
}
fn override_business(_: Business) -> Response {
    text(Status::FORBIDDEN, "override")
}
#[tokio::test]
async fn async_handlers_use_app_mapping_route_override_and_leave_success_alone() {
    async fn auth(_: Request, _: Arc<()>) -> Result<Response, Business> {
        tokio::task::yield_now().await;
        Err(Business::Auth)
    }
    async fn conflict(_: Request, _: Arc<()>) -> Result<Response, Business> {
        Err(Business::Conflict)
    }
    async fn limited(_: Request, _: Arc<()>) -> Result<Response, Business> {
        Err(Business::Limited)
    }
    async fn success(_: Request, _: Arc<()>) -> Result<Response, Business> {
        Ok(text(Status::CREATED, "created"))
    }
    async fn independent(_: Request, _: Arc<()>) -> Result<Response, i64> {
        Err(42)
    }
    fn map_number(error: i64) -> Response {
        assert_eq!(error, 42);
        text(Status::CONFLICT, "independent")
    }
    let app = route(app((), business), Method::GET, "/auth", auth).unwrap();
    let app = route_mapped(app, Method::GET, "/override", auth, override_business).unwrap();
    let app = route(app, Method::GET, "/conflict", conflict).unwrap();
    let app = route(app, Method::GET, "/limited", limited).unwrap();
    let app = route_mapped(app, Method::GET, "/success", success, override_business).unwrap();
    let app = route_mapped(app, Method::GET, "/independent", independent, map_number).unwrap();
    let server = Server::new(app, default_options()).await;
    let mut socket = server.connect().await;
    for (path, status, expected) in [
        ("auth", 401, "default"),
        ("override", 403, "override"),
        ("conflict", 409, "default"),
        ("limited", 429, "default"),
        ("success", 201, "created"),
        ("independent", 409, "independent"),
    ] {
        send(
            &mut socket,
            format!("GET /{path} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes(),
        )
        .await;
        let result = response(&mut socket, false).await;
        assert_eq!(result.status, status);
        assert_eq!(result.body, expected.as_bytes());
    }
    server.stop().await;
}

#[tokio::test]
async fn forbidden_response_bodies_do_not_desynchronize_keepalive() {
    async fn suppress(request: Request, _: Arc<()>) -> Result<Response, Error> {
        let code: i64 = request.path().trim_start_matches('/').parse().unwrap();
        Ok(text(status(code).unwrap(), "must not be transmitted"))
    }
    let mut app = app_default(());
    for code in [204, 205, 304] {
        app = route(app, Method::GET, &format!("/{code}"), suppress).unwrap();
    }
    let server = Server::new(app, default_options()).await;
    let mut socket = server.connect().await;
    for code in [204, 205, 304, 204] {
        send(
            &mut socket,
            format!("GET /{code} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes(),
        )
        .await;
        let result = response(&mut socket, false).await;
        assert_eq!(result.status, code);
        assert!(result.body.is_empty());
        assert!(result.all("content-type").is_empty());
        assert!(result.all("transfer-encoding").is_empty());
        assert!(
            result.all("content-length").is_empty()
                || (code == 205 && result.all("content-length") == ["0"])
        );
    }
    server.stop().await;
}

#[tokio::test]
async fn sockets_preserve_raw_duplicate_headers_cookies_and_strict_authorization() {
    async fn inspect(request: Request, _: Arc<()>) -> Result<Response, Error> {
        if header_text(&request, "authorization")?.is_none() {
            return Ok(empty(Status::UNAUTHORIZED));
        }
        let raw = headers(&request, "x-many")?;
        let result = bytes(Status::OK, &raw.concat());
        append_header_text(
            append_header_text(result, "set-cookie", "a=1")?,
            "set-cookie",
            "b=2",
        )
    }
    let server = Server::new(
        route(app_default(()), Method::GET, "/", inspect).unwrap(),
        default_options(),
    )
    .await;
    let mut socket = server.connect().await;
    send(&mut socket, b"GET / HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer ok\r\nX-Many: one\r\nX-Many: two\r\n\r\n").await;
    let result = response(&mut socket, false).await;
    assert_eq!(result.body, b"onetwo");
    assert_eq!(result.all("set-cookie"), ["a=1", "b=2"]);
    send(
        &mut socket,
        b"GET / HTTP/1.1\r\nHost: localhost\r\nAuthorization: a\r\nAuthorization: b\r\n\r\n",
    )
    .await;
    assert_eq!(response(&mut socket, false).await.status, 400);
    send(
        &mut socket,
        b"GET / HTTP/1.1\r\nHost: localhost\r\nAuthorization: \xff\r\n\r\n",
    )
    .await;
    assert_eq!(response(&mut socket, false).await.status, 400);
    send(&mut socket, b"GET / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\nTrailer: Authorization\r\n\r\n1\r\na\r\n0\r\nAuthorization: Bearer trailer-must-not-authenticate\r\n\r\n").await;
    assert_eq!(response(&mut socket, false).await.status, 401);
    server.stop().await;
}

#[tokio::test]
async fn actual_chunk_sizes_and_absolute_body_deadlines_are_bounded() {
    async fn echo(request: Request, _: Arc<()>) -> Result<Response, Error> {
        Ok(bytes(Status::OK, request.body()))
    }
    let server = Server::new(
        route(app_default(()), Method::POST, "/", echo).unwrap(),
        options(4, 100, 1000, 1000).unwrap(),
    )
    .await;
    let mut socket = server.connect().await;
    send(&mut socket, b"POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n2\r\ncd\r\n0\r\n\r\n").await;
    let result = response(&mut socket, false).await;
    assert_eq!(result.status, 200);
    assert_eq!(result.body, b"abcd");
    send(&mut socket, b"POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n").await;
    assert_eq!(response(&mut socket, false).await.status, 413);
    assert!(closed(&mut socket).await.is_empty());
    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\n",
    )
    .await;
    assert_eq!(response(&mut socket, false).await.status, 413);
    assert!(closed(&mut socket).await.is_empty());
    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\n\r\na",
    )
    .await;
    tokio::time::sleep(Duration::from_millis(70)).await;
    send(&mut socket, b"b").await;
    let began = Instant::now();
    assert_eq!(response(&mut socket, false).await.status, 408);
    assert!(began.elapsed() < Duration::from_millis(250));
    assert!(closed(&mut socket).await.is_empty());
    server.stop().await;
}

struct Gate {
    started: Arc<Notify>,
    released: Arc<Notify>,
    cancelled: Arc<AtomicUsize>,
}
struct Dropped(Arc<AtomicUsize>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
async fn wait(_: Request, state: Arc<Gate>) -> Result<Response, Error> {
    let _dropped = Dropped(Arc::clone(&state.cancelled));
    state.started.notify_one();
    state.released.notified().await;
    Ok(text(Status::OK, "released"))
}
fn gate() -> (Gate, Arc<Notify>, Arc<Notify>, Arc<AtomicUsize>) {
    let started = Arc::new(Notify::new());
    let released = Arc::new(Notify::new());
    let cancelled = Arc::new(AtomicUsize::new(0));
    (
        Gate {
            started: Arc::clone(&started),
            released: Arc::clone(&released),
            cancelled: Arc::clone(&cancelled),
        },
        started,
        released,
        cancelled,
    )
}

#[tokio::test]
async fn request_capacity_rejects_unread_bodies_then_releases_admission() {
    let (state, started, released, cancelled) = gate();
    let app = route(app_default(state), Method::POST, "/", wait).unwrap();
    let server = Server::new(app, capacity(default_options(), 4, 1).unwrap()).await;
    let mut first = server.connect().await;
    send(&mut first, b"POST / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    tokio::time::timeout(Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    let mut second = server.connect().await;
    send(
        &mut second,
        b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 10\r\n\r\n",
    )
    .await;
    assert_eq!(response(&mut second, false).await.status, 503);
    assert!(closed(&mut second).await.is_empty());
    released.notify_one();
    assert_eq!(response(&mut first, false).await.status, 200);
    send(&mut first, b"POST / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    tokio::time::timeout(Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    released.notify_one();
    assert_eq!(response(&mut first, false).await.status, 200);
    assert_eq!(cancelled.load(Ordering::SeqCst), 2);
    server.stop().await;
}

#[tokio::test]
async fn handler_timeout_cancels_future_and_releases_request_capacity() {
    let (state, started, released, cancelled) = gate();
    let app = route(app_default(state), Method::GET, "/", wait).unwrap();
    let server = Server::new(
        app,
        capacity(options(1024, 1000, 100, 1000).unwrap(), 4, 1).unwrap(),
    )
    .await;
    let mut socket = server.connect().await;
    send(&mut socket, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    started.notified().await;
    assert_eq!(response(&mut socket, false).await.status, 504);
    assert_eq!(cancelled.load(Ordering::SeqCst), 1);
    send(&mut socket, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    tokio::time::timeout(Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    released.notify_one();
    assert_eq!(response(&mut socket, false).await.status, 200);
    server.stop().await;
}

#[tokio::test]
async fn connection_capacity_and_shutdown_deadline_leave_no_handler_tasks() {
    let (state, started, _, cancelled) = gate();
    let app = route(app_default(state), Method::GET, "/", wait).unwrap();
    let server = Server::new(
        app,
        capacity(options(1024, 1000, 2000, 80).unwrap(), 1, 1).unwrap(),
    )
    .await;
    let mut first = server.connect().await;
    send(&mut first, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    tokio::time::timeout(Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    let mut second = server.connect().await;
    assert!(closed(&mut second).await.is_empty());
    let began = Instant::now();
    server.stop().await;
    assert!(began.elapsed() < Duration::from_secs(1));
    assert_eq!(cancelled.load(Ordering::SeqCst), 1);
    assert!(closed(&mut first).await.is_empty());
}

#[tokio::test]
async fn graceful_shutdown_finishes_an_in_flight_handler_and_response() {
    let (state, started, released, dropped) = gate();
    let app = route(app_default(state), Method::GET, "/", wait).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopping) = oneshot::channel();
    let (observed, shutdown_observed) = oneshot::channel();
    let task = tokio::spawn(serve_listener(
        listener,
        app,
        options(1024, 1000, 1000, 1200).unwrap(),
        async {
            stopping.await.unwrap();
            observed.send(()).unwrap();
        },
    ));
    let mut server = Server {
        address,
        stopped: Some(stop),
        task,
    };
    let mut socket = server.connect().await;
    send(&mut socket, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    tokio::time::timeout(Duration::from_secs(2), started.notified())
        .await
        .unwrap();

    server.stopped.take().unwrap().send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), shutdown_observed)
        .await
        .unwrap()
        .unwrap();
    // The current-thread runtime observes this acknowledgement after the
    // server has stopped accepting and notified its connection tasks.
    assert!(TcpStream::connect(address).await.is_err());
    assert_eq!(dropped.load(Ordering::SeqCst), 0);
    released.notify_one();
    let result = response(&mut socket, false).await;
    assert_eq!(result.status, 200);
    assert_eq!(result.body, b"released");
    assert!(closed(&mut socket).await.is_empty());
    tokio::time::timeout(Duration::from_secs(2), &mut server.task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn header_idle_deadline_is_independent_of_send_deadline_after_flush() {
    let server = Server::new(
        route(app_default("ok".to_owned()), Method::GET, "/", hello).unwrap(),
        send_timeout(header_timeout(default_options(), 400).unwrap(), 60).unwrap(),
    )
    .await;
    let mut socket = server.connect().await;
    send(&mut socket, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    assert_eq!(response(&mut socket, false).await.status, 200);
    tokio::time::sleep(Duration::from_millis(100)).await;
    send(&mut socket, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    assert_eq!(response(&mut socket, false).await.status, 200);
    let rest = closed(&mut socket).await;
    assert!(rest.is_empty() || rest.starts_with(b"HTTP/1.1 408"));
    server.stop().await;
}

#[tokio::test]
async fn stalled_response_send_expires_and_releases_connection_permit() {
    async fn large(_: Request, _: Arc<String>) -> Result<Response, Error> {
        Ok(bytes(Status::OK, &vec![b'x'; 16 * 1024 * 1024]))
    }
    let app = route(app_default("ok".to_owned()), Method::GET, "/large", large).unwrap();
    let app = route(app, Method::GET, "/", hello).unwrap();
    let server = Server::new(
        app,
        send_timeout(capacity(default_options(), 1, 1).unwrap(), 80).unwrap(),
    )
    .await;
    let mut blocked = server.connect().await;
    send(
        &mut blocked,
        b"GET /large HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut next = server.connect().await;
    send(&mut next, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    assert_eq!(response(&mut next, false).await.status, 200);
    drop(blocked);
    server.stop().await;
}

#[tokio::test]
async fn graceful_shutdown_preserves_stalled_response_send_deadline() {
    const BODY_LENGTH: usize = 16 * 1024 * 1024;
    async fn large(_: Request, _: Arc<()>) -> Result<Response, Error> {
        Ok(bytes(Status::OK, &vec![b'x'; BODY_LENGTH]))
    }
    let app = route(app_default(()), Method::GET, "/", large).unwrap();
    let mut server = Server::new(
        app,
        send_timeout(options(1024, 1000, 1000, 1200).unwrap(), 150).unwrap(),
    )
    .await;
    let mut blocked = server.connect().await;
    send(&mut blocked, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;

    // Receiving the headers proves that the response and its send deadline are
    // active. Keep the much larger body unread so socket backpressure remains.
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut line = String::new();
        blocked.read_line(&mut line).await.unwrap();
        assert!(line.starts_with("HTTP/1.1 200 "));
        let mut length = None;
        loop {
            line.clear();
            assert_ne!(blocked.read_line(&mut line).await.unwrap(), 0);
            if line == "\r\n" {
                break;
            }
            let (name, value) = line.split_once(':').unwrap();
            if name.eq_ignore_ascii_case("content-length") {
                length = Some(value.trim().parse::<usize>().unwrap());
            }
        }
        assert_eq!(length, Some(BODY_LENGTH));
    })
    .await
    .unwrap();

    server.stopped.take().unwrap().send(()).unwrap();
    // Allow scheduling jitter around the 150ms send deadline, but distinguish
    // it from falling back to the entire 1200ms server shutdown deadline.
    tokio::time::timeout(Duration::from_millis(600), &mut server.task)
        .await
        .expect("graceful shutdown lost the stalled response send deadline")
        .unwrap()
        .unwrap();
    assert!(closed(&mut blocked).await.len() < BODY_LENGTH);
}
