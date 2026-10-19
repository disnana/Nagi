use super::*;
use axum::body::Body;
use std::io::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    sync::{oneshot, Notify},
};

pub(super) fn with_response_fixture(options: Options) -> Options {
    if options.response_limits.is_none() {
        // Explicit finite test-only response profile: enough for the
        // existing 16 MiB stalled-send fixture and no production default.
        let body_bytes = options.body_bytes.max(STALLED_BODY_LENGTH);
        let retained_bytes = body_bytes
            .checked_mul(8)
            .and_then(|bytes| bytes.checked_add(16 * 1024 * 1024))
            .expect("test response retained bound fits usize");
        let fallback_retained_bytes = options
            .connections
            .checked_mul(262_144)
            .and_then(|bytes| bytes.checked_add(16 * 1024 * 1024))
            .expect("test fallback retained bound fits usize");
        let limits = response_limits(
            i64::try_from(body_bytes).expect("test body bound fits i64"),
            65_536,
            256,
            4096,
            32,
            131_072,
            i64::try_from(options.requests).expect("test request count fits i64"),
            i64::try_from(retained_bytes).expect("test retained bound fits i64"),
            i64::try_from(options.connections).expect("test connection count fits i64"),
            i64::try_from(fallback_retained_bytes).expect("test fallback retained bound fits i64"),
            4096,
            32,
        )
        .expect("fixed test response limits are valid");
        with_response_limits(options, limits).expect("fixed test response profile validates")
    } else {
        options
    }
}

// Explicit finite profile for old lifecycle fixtures only. Startup-refusal and
// rate tests use the public API directly and never pass through this helper.
pub(crate) fn with_rate_fixture<S, E>(
    mut app: App<S, E>,
    options: Options,
) -> (App<S, E>, Options) {
    for route in app.routes.values_mut().flatten() {
        if route.rate.is_none() {
            route.rate = Some(rate_policy(1_000_000, 1000).expect("finite test route policy"));
        }
    }
    let options = if options.rate_limits.is_none() {
        with_rate_limits(
            options,
            rate_limits(64, 64, 4096, 4 * 1024 * 1024, 65_536).unwrap(),
        )
        .expect("finite test rate metadata fits")
    } else {
        options
    };
    (app, options)
}

fn response_fixture_request(body_bound: usize) -> Request {
    super::response_public::tests::request_fixture(body_bound)
}

#[path = "authority_tests.rs"]
mod authority_tests;
#[path = "panic_tests.rs"]
mod panic_tests;

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
        Self::new_with_send_buffer(app, options, None).await
    }
    async fn new_with_send_buffer<S, E>(
        app: App<S, E>,
        options: Options,
        send_buffer_bytes: Option<u32>,
    ) -> Self
    where
        S: Send + Sync + 'static,
        E: 'static,
    {
        // Existing lifecycle fixtures explicitly accept their localhost Host;
        // public serve/serve_listener do not supply an authority default.
        let options = if options.authority.is_none() {
            authority(
                options,
                "https://localhost",
                vec!["localhost".to_owned()],
                1,
                128,
            )
            .unwrap()
        } else {
            options
        };
        let options = with_response_fixture(options);
        let (app, options) = with_rate_fixture(app, options);
        let listener = if let Some(bytes) = send_buffer_bytes {
            // Only stalled-send fixtures constrain native buffering. The
            // accepted socket inherits this listener setting; actual write
            // Pending, expiry and join remain the evidence below.
            let socket = tokio::net::TcpSocket::new_v4().unwrap();
            #[cfg(not(windows))]
            socket.set_reuseaddr(true).unwrap();
            socket.set_send_buffer_size(bytes).unwrap();
            socket.bind("127.0.0.1:0".parse().unwrap()).unwrap();
            socket.listen(128).unwrap()
        } else {
            TcpListener::bind("127.0.0.1:0").await.unwrap()
        };
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
#[derive(Clone, Copy, Debug)]
enum TestIoPhase {
    RequestWrite,
    ResponseStatus,
    ResponseHeaders,
    ResponseBody,
    ConnectionEof,
}
fn test_io<T>(phase: TestIoPhase, result: std::io::Result<T>) -> T {
    result.unwrap_or_else(|error| {
        panic!(
            "HTTP test I/O failed: phase={phase:?} kind={:?}",
            error.kind()
        )
    })
}
async fn send(socket: &mut BufReader<TcpStream>, data: &[u8]) {
    test_io(
        TestIoPhase::RequestWrite,
        socket.get_mut().write_all(data).await,
    );
}
async fn response(socket: &mut BufReader<TcpStream>, head: bool) -> Received {
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut line = String::new();
        test_io(
            TestIoPhase::ResponseStatus,
            socket.read_line(&mut line).await,
        );
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
            assert_ne!(
                test_io(
                    TestIoPhase::ResponseHeaders,
                    socket.read_line(&mut line).await
                ),
                0
            );
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
                    test_io(TestIoPhase::ResponseBody, socket.read_line(&mut line).await);
                    let length = usize::from_str_radix(line.trim(), 16).unwrap();
                    if length == 0 {
                        test_io(TestIoPhase::ResponseBody, socket.read_line(&mut line).await);
                        break;
                    }
                    let start = body.len();
                    body.resize(start + length, 0);
                    test_io(
                        TestIoPhase::ResponseBody,
                        socket.read_exact(&mut body[start..]).await,
                    );
                    let mut end = [0; 2];
                    test_io(TestIoPhase::ResponseBody, socket.read_exact(&mut end).await);
                    assert_eq!(&end, b"\r\n");
                }
            } else {
                body.resize(length, 0);
                test_io(
                    TestIoPhase::ResponseBody,
                    socket.read_exact(&mut body).await,
                );
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
    test_io(
        TestIoPhase::ConnectionEof,
        tokio::time::timeout(Duration::from_secs(3), socket.read_to_end(&mut rest))
            .await
            .unwrap(),
    );
    rest
}
// An intentional parser rejection or expired send closes the transport without
// promising FIN. This helper is never used to accept a failed normal response.
async fn terminal_close_after_abort(
    socket: &mut BufReader<TcpStream>,
    max_bytes: usize,
) -> Vec<u8> {
    let mut rest = Vec::new();
    let result = tokio::time::timeout(
        Duration::from_secs(3),
        (&mut *socket)
            .take(max_bytes as u64 + 1)
            .read_to_end(&mut rest),
    )
    .await
    .unwrap();
    match result {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
        Err(error) => test_io(TestIoPhase::ConnectionEof, Err::<(), _>(error)),
    }
    assert!(
        rest.len() <= max_bytes,
        "unexpected bytes after terminal close"
    );
    rest
}

fn basic_request() -> Request {
    Request {
        method: Method::GET,
        uri: "/users/7?q=yes".parse().unwrap(),
        headers: HeaderMap::new().into(),
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

#[tokio::test]
async fn heterogeneous_route_erasure_adds_exactly_one_future_allocation() {
    let app = route(
        app_default("state".to_owned()),
        Method::GET,
        "/",
        public_policy(),
        hello,
    )
    .unwrap();
    let owner = LeaseOwner::new(Instant::now() + Duration::from_secs(30)).unwrap();
    let route = &app.routes["/"][0];
    let RouteKind::Ordinary(prepare) = &route.kind else {
        panic!("expected ordinary route");
    };
    let prepared = prepare(
        basic_request().security_snapshot(),
        Arc::clone(&app.state),
        Arc::clone(&owner.lease),
    )
    .await
    .unwrap();
    let request = basic_request();
    let (future, count) = crate::metrics::measure(|| prepared(request, Arc::clone(&app.state)));
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
fn response_headers_reject_managed_values_and_keep_ordinary_duplicates() {
    let request = response_fixture_request(16);
    let context = response_context(&request);
    for (name, value) in [
        ("bad name", b"v".as_slice()),
        ("valid", b"a\r\nb"),
        ("Content-Length", b"100"),
        ("TRANSFER-ENCODING", b"chunked"),
        ("Content-Type", b"text/html"),
        ("Set-Cookie", b"session=active"),
        ("Cache-Control", b"public"),
        ("Vary", b"Origin"),
        ("Access-Control-Allow-Origin", b"*"),
        ("Content-Security-Policy", b"default-src 'self'"),
        ("X-Content-Type-Options", b"nosniff"),
    ] {
        assert!(append_header(empty(context, Status::OK).unwrap(), name, value).is_err());
    }
    assert!(append_header_text(empty(context, Status::OK).unwrap(), "x-value", "a\r\nb").is_err());
    // SF01 intentionally removes the raw-HTML activation path and user-managed
    // Set-Cookie output. HTML bytes remain an inert binary representation;
    // ordinary duplicate application headers remain available.
    let inactive_html = bytes(context, Status::CREATED, b"<p>ok</p>").unwrap();
    assert!(append_header_text(inactive_html, "Content-Type", "text/html").is_err());
    let result = append_header(
        append_header(
            bytes(context, Status::CREATED, b"ok").unwrap(),
            "X-Trace",
            b"a=1",
        )
        .unwrap(),
        "X-Trace",
        b"b=2",
    )
    .unwrap()
    .into_http(false);
    assert_eq!(
        result
            .headers()
            .get_all("x-trace")
            .iter()
            .map(HeaderValue::as_bytes)
            .collect::<Vec<_>>(),
        [b"a=1".as_slice(), b"b=2".as_slice()]
    );
    assert_eq!(result.status(), 201);
    assert_eq!(
        result.headers()[names::CONTENT_TYPE],
        "application/octet-stream"
    );
    assert!(!result.headers().contains_key(names::CONTENT_LENGTH));
}

#[test]
fn ordinary_response_framing_uses_the_native_body_length_without_allocating_a_header_value() {
    let request = response_fixture_request(16);
    let context = response_context(&request);
    let normal_response = text(context, Status::OK, "hello").unwrap();
    let head_response = text(context, Status::OK, "hello").unwrap();
    let (_, normal) = crate::metrics::measure(|| normal_response.into_http(false));
    let (_, head) = crate::metrics::measure(|| head_response.into_http(true));
    let (_, framing) = crate::metrics::measure(|| HeaderValue::from_bytes(b"5").unwrap());
    // The managed plan uses a stack decimal buffer and copies only the final
    // digits into HeaderValue. Ordinary frames omit Content-Length entirely;
    // Hyper writes their exact length itself.
    // The bounded response path now also prepares its managed plan and
    // charged native map here. The former constructor's three allocations
    // are not the current handoff baseline. Preserve the exact HEAD delta and
    // byte/realloc checks against the same primitive used by the managed plan.
    assert_eq!(normal.allocations, 7);
    assert_eq!(framing.allocations, 1);
    assert_eq!(head.allocations, normal.allocations + framing.allocations);
    assert_eq!(
        head.allocated_bytes,
        normal.allocated_bytes + framing.allocated_bytes
    );
    assert_eq!(framing.allocated_bytes, 1);
    assert_eq!(normal.reallocations, 0);
    assert_eq!(head.reallocations, 0);
}

#[test]
fn json_encoding_is_fallible_and_default_internal_mapping_hides_details() {
    let request = response_fixture_request(256);
    let context = response_context(&request);
    struct Fails;
    impl Serialize for Fails {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("private serializer failure"))
        }
    }
    let error = json(context, Status::OK, &Fails).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::Internal));
    let response = default_error(context, error).unwrap();
    assert_eq!(response.status(), Status::INTERNAL_SERVER_ERROR);
    assert_eq!(response.body(), b"internal error");
    let response = json(context, Status::CREATED, &vec![1, 2]).unwrap();
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

async fn hello(request: Request, state: Arc<String>, _: ()) -> Result<Response, Error> {
    text(response_context(&request), Status::OK, &state)
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
            route(
                app_default(String::new()),
                Method::GET,
                path,
                public_policy(),
                hello
            )
            .is_err(),
            "{path}"
        );
    }
    let app = route(
        app_default(String::new()),
        Method::GET,
        "/users/{id}",
        public_policy(),
        hello,
    )
    .unwrap();
    assert!(route(app, Method::GET, "/users/{name}", public_policy(), hello).is_err());
    let app = route(
        app_default(String::new()),
        Method::GET,
        "/",
        public_policy(),
        hello,
    )
    .unwrap();
    assert!(route(app, Method::GET, "/", public_policy(), hello).is_err());
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
    let (mut body, allocations) = crate::metrics::measure(|| BufferedBody {
        bytes,
        sending: None,
    });
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
        public_policy(),
        hello,
    )
    .unwrap();
    let app = route(
        app,
        method("CUSTOM").unwrap(),
        "/users/{id}",
        public_policy(),
        hello,
    )
    .unwrap();
    let first = Server::new(app, default_options()).await;
    let second = Server::new(
        route(
            app_default("second".to_owned()),
            Method::GET,
            "/users/{id}",
            public_policy(),
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
        public_policy(),
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
    let app = route(
        app_default("ok".to_owned()),
        Method::GET,
        "/",
        public_policy(),
        hello,
    )
    .unwrap();
    let server = Server::new(app, header_limits(default_options(), 8192, 4).unwrap()).await;
    let mut socket = server.connect().await;
    let request = format!(
        "GET / HTTP/1.1\r\nHost: localhost\r\nX-Large: {}\r\n\r\n",
        "x".repeat(16 * 1024)
    );
    send(&mut socket, request.as_bytes()).await;
    let result = response(&mut socket, false).await;
    assert_eq!(result.status, 431);
    assert!(terminal_close_after_abort(&mut socket, 0).await.is_empty());
    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"GET / HTTP/1.1\r\nHost: localhost\r\nX-A: a\r\nX-B: b\r\nX-C: c\r\nX-D: d\r\n\r\n",
    )
    .await;
    assert_eq!(response(&mut socket, false).await.status, 431);
    assert!(terminal_close_after_abort(&mut socket, 0).await.is_empty());
    server.stop().await;
    let app = route(
        app_default("ok".to_owned()),
        Method::GET,
        "/",
        public_policy(),
        hello,
    )
    .unwrap();
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
    assert!(terminal_close_after_abort(&mut socket, 0).await.is_empty());
    server.stop().await;
}

#[tokio::test]
async fn head_uses_get_representation_length_explicit_head_wins_and_allow_is_correct() {
    async fn head_only(request: Request, _: Arc<String>, _: ()) -> Result<Response, Error> {
        text(response_context(&request), Status::CREATED, "explicit")
    }
    let app = route(
        app_default("hello".to_owned()),
        Method::GET,
        "/",
        public_policy(),
        hello,
    )
    .unwrap();
    let app = route(app, Method::GET, "/explicit", public_policy(), hello).unwrap();
    let app = route(app, Method::HEAD, "/explicit", public_policy(), head_only).unwrap();
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
    Factory(Error),
}
fn business(context: &ResponseContext, error: Business) -> Result<Response, Error> {
    let status = match error {
        Business::Factory(error) => return Err(error),
        Business::Auth => Status::UNAUTHORIZED,
        Business::Conflict => Status::CONFLICT,
        Business::Limited => Status::TOO_MANY_REQUESTS,
    };
    text(context, status, "default")
}
fn override_business(context: &ResponseContext, error: Business) -> Result<Response, Error> {
    if let Business::Factory(error) = error {
        return Err(error);
    }
    text(context, Status::FORBIDDEN, "override")
}
#[tokio::test]
async fn async_handlers_use_app_mapping_route_override_and_leave_success_alone() {
    async fn auth(_: Request, _: Arc<()>, _: ()) -> Result<Response, Business> {
        tokio::task::yield_now().await;
        Err(Business::Auth)
    }
    async fn conflict(_: Request, _: Arc<()>, _: ()) -> Result<Response, Business> {
        Err(Business::Conflict)
    }
    async fn limited(_: Request, _: Arc<()>, _: ()) -> Result<Response, Business> {
        Err(Business::Limited)
    }
    async fn success(request: Request, _: Arc<()>, _: ()) -> Result<Response, Business> {
        text(response_context(&request), Status::CREATED, "created").map_err(Business::Factory)
    }
    async fn independent(_: Request, _: Arc<()>, _: ()) -> Result<Response, i64> {
        Err(42)
    }
    fn map_number(context: &ResponseContext, error: i64) -> Result<Response, Error> {
        assert_eq!(error, 42);
        text(context, Status::CONFLICT, "independent")
    }
    let app = route(
        app((), business),
        Method::GET,
        "/auth",
        public_policy(),
        auth,
    )
    .unwrap();
    let app = route_mapped(
        app,
        Method::GET,
        "/override",
        public_policy(),
        auth,
        override_business,
    )
    .unwrap();
    let app = route(app, Method::GET, "/conflict", public_policy(), conflict).unwrap();
    let app = route(app, Method::GET, "/limited", public_policy(), limited).unwrap();
    let app = route_mapped(
        app,
        Method::GET,
        "/success",
        public_policy(),
        success,
        override_business,
    )
    .unwrap();
    let app = route_mapped(
        app,
        Method::GET,
        "/independent",
        public_policy(),
        independent,
        map_number,
    )
    .unwrap();
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
    async fn suppress(request: Request, _: Arc<()>, _: ()) -> Result<Response, Error> {
        let code: i64 = request.path().trim_start_matches('/').parse().unwrap();
        text(
            response_context(&request),
            status(code).unwrap(),
            "must not be transmitted",
        )
    }
    let mut app = app_default(());
    for code in [204, 205, 304] {
        app = route(
            app,
            Method::GET,
            &format!("/{code}"),
            public_policy(),
            suppress,
        )
        .unwrap();
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
async fn sockets_preserve_raw_duplicate_headers_and_strict_authorization() {
    async fn inspect(request: Request, _: Arc<()>, _: ()) -> Result<Response, Error> {
        if header_text(&request, "authorization")?.is_none() {
            return empty(response_context(&request), Status::UNAUTHORIZED);
        }
        let raw = headers(&request, "x-many")?;
        let result = bytes(response_context(&request), Status::OK, &raw.concat())?;
        append_header_text(
            append_header_text(result, "x-trace", "a=1")?,
            "x-trace",
            "b=2",
        )
    }
    let server = Server::new(
        route(app_default(()), Method::GET, "/", public_policy(), inspect).unwrap(),
        default_options(),
    )
    .await;
    let mut socket = server.connect().await;
    send(&mut socket, b"GET / HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer ok\r\nX-Many: one\r\nX-Many: two\r\n\r\n").await;
    let result = response(&mut socket, false).await;
    assert_eq!(result.body, b"onetwo");
    assert_eq!(result.all("x-trace"), ["a=1", "b=2"]);
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
    async fn echo(request: Request, _: Arc<()>, _: ()) -> Result<Response, Error> {
        bytes(response_context(&request), Status::OK, request.body())
    }
    let server = Server::new(
        route(app_default(()), Method::POST, "/", public_policy(), echo).unwrap(),
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

struct OversizedBodyState {
    calls: Arc<AtomicUsize>,
}

async fn oversized_body_server() -> (Server, Arc<AtomicUsize>) {
    async fn counted(
        request: Request,
        state: Arc<OversizedBodyState>,
        _: (),
    ) -> Result<Response, Error> {
        state.calls.fetch_add(1, Ordering::SeqCst);
        text(response_context(&request), Status::OK, "accepted")
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let state = OversizedBodyState {
        calls: Arc::clone(&calls),
    };
    let app = route(
        app_default(state),
        Method::POST,
        "/",
        public_policy(),
        counted,
    )
    .unwrap();
    let app = route(app, Method::GET, "/", public_policy(), counted).unwrap();
    let server = Server::new(
        app,
        capacity(options(4096, 200, 1000, 1000).unwrap(), 1, 1).unwrap(),
    )
    .await;
    (server, calls)
}

async fn oversized_body_assert_response(socket: &mut BufReader<TcpStream>, expected: u16) {
    let result = response(socket, false).await;
    assert_eq!(result.status, expected);
    assert_eq!(result.all("connection"), ["close"]);
    assert_eq!(
        result.body,
        if expected == 413 {
            b"Payload Too Large".as_slice()
        } else {
            b"Request Timeout".as_slice()
        }
    );
    // A TCP reset after the complete 413 message still closes the connection.
    // It must not conceal a reset while reading its status, headers or body.
    let mut rest = Vec::new();
    let closing = tokio::time::timeout(Duration::from_secs(3), socket.read_to_end(&mut rest))
        .await
        .expect("rejected upload connection did not close");
    match closing {
        Ok(_) => {}
        Err(error) if expected == 413 && error.kind() == std::io::ErrorKind::ConnectionReset => {}
        Err(error) => panic!("reading rejected upload closure: {error}"),
    }
    assert!(rest.is_empty(), "rejected upload produced another response");
}

async fn oversized_body_assert_capacity_released(server: &Server, calls: &AtomicUsize) {
    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    )
    .await;
    assert_eq!(response(&mut socket, false).await.status, 200);
    assert!(closed(&mut socket).await.is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

// Run these real TCP regressions on all supported operating systems. An
// in-memory Body cannot expose the reset caused by closing an unread socket.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_body_content_length_delivers_413_without_calling_handler() {
    for split_headers in [false, true] {
        let (server, calls) = oversized_body_server().await;
        let mut socket = server.connect().await;
        let headers = b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5120\r\n\r\n";
        let body = vec![b'x'; 5120];
        if split_headers {
            send(&mut socket, headers).await;
            // A concurrent server may reject the declared length before this
            // second write. Still inspect its complete final response.
            let _ = socket.get_mut().write_all(&body).await;
        } else {
            let mut request = headers.to_vec();
            request.extend_from_slice(&body);
            send(&mut socket, &request).await;
        }
        oversized_body_assert_response(&mut socket, 413).await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        oversized_body_assert_capacity_released(&server, &calls).await;
        server.stop().await;
    }
}

// This upload mirrors http.client's separate header/body writes with NODELAY.
// The public close-without-drain contract permits a reset before a full 413;
// record receipt separately from the required rejection and capacity recovery.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_body_nodelay_split_writes_observes_receipt_and_recovery() {
    let (server, calls) = oversized_body_server().await;
    let mut socket = server.connect().await;
    socket.get_mut().set_nodelay(true).unwrap();
    let began = Instant::now();
    send(
        &mut socket,
        b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 4097\r\n\r\n",
    )
    .await;
    let sending = tokio::time::timeout(
        Duration::from_secs(3),
        socket.get_mut().write_all(&[b' '; 4097]),
    )
    .await;
    let mut wire = Vec::new();
    let closing = tokio::time::timeout(Duration::from_secs(3), async {
        let mut chunk = [0; 1024];
        loop {
            let length = socket.read(&mut chunk).await?;
            if length == 0 {
                return Ok::<_, std::io::Error>(());
            }
            if wire.len() + length > 8192 {
                return Err(std::io::Error::other("HTTP observation exceeds 8192 bytes"));
            }
            wire.extend_from_slice(&chunk[..length]);
        }
    })
    .await;
    let headers_end = wire.windows(4).position(|part| part == b"\r\n\r\n");
    let complete_response = headers_end.is_some_and(|end| wire.len() >= end + 4 + 17);
    let mut report = serde_json::json!({
        "case": "oversized_body_nodelay_split_writes",
        "os": std::env::consts::OS,
        "body_bytes": 4097,
        "body_limit": 4096,
        "nodelay": true,
        "separate_writes": true,
        "send_result": format!("{sending:?}"),
        "close_result": format!("{closing:?}"),
        "elapsed_ms": began.elapsed().as_millis(),
        "complete_headers_received": headers_end.is_some(),
        "complete_response_received": complete_response,
        "received_bytes": wire.len(),
        "wire": String::from_utf8_lossy(&wire),
        "handler_calls_before_recovery": calls.load(Ordering::SeqCst),
        "recovery_verified": false,
    });
    let directory = std::env::var_os("NAGI_FAILURE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("nagi-http-observations"));
    std::fs::create_dir_all(&directory).unwrap();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = directory.join(format!(
        "http413-nodelay-{}-{timestamp}.json",
        std::process::id()
    ));
    let mut artifact = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    std::io::Write::write_all(
        &mut artifact,
        serde_json::to_string_pretty(&report).unwrap().as_bytes(),
    )
    .unwrap();
    println!(
        "HTTP split-write receipt observation: {report}; artifact: {}",
        path.display()
    );

    let sending = sending.expect("bounded oversized write timed out");
    if let Err(error) = sending {
        assert!(matches!(
            error.kind(),
            std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::BrokenPipe
        ));
    }
    if let Err(error) = closing.expect("rejected split-write connection did not close") {
        assert!(matches!(
            error.kind(),
            std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
        ));
    }
    // Complete received fields retain their strict oracle. A partial response
    // is observable evidence, not proof of successful 413 delivery.
    if let Some(end) = wire.windows(2).position(|part| part == b"\r\n") {
        assert_eq!(
            std::str::from_utf8(&wire[..end])
                .unwrap()
                .split_whitespace()
                .nth(1),
            Some("413")
        );
    }
    if let Some(end) = headers_end {
        let headers = std::str::from_utf8(&wire[..end])
            .unwrap()
            .to_ascii_lowercase();
        assert!(headers.lines().any(|line| line == "connection: close"));
        assert!(headers.lines().any(|line| line == "content-length: 17"));
        let body = &wire[end + 4..];
        assert!(b"Payload Too Large".starts_with(body));
        if complete_response {
            assert_eq!(body, b"Payload Too Large");
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    oversized_body_assert_capacity_released(&server, &calls).await;
    server.stop().await;
    report["recovery_verified"] = serde_json::json!(true);
    std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_body_declared_length_rejects_headers_and_incomplete_upload_promptly() {
    for body in [b"".as_slice(), b"unfinished"] {
        let (server, calls) = oversized_body_server().await;
        let mut socket = server.connect().await;
        let mut request =
            b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5120\r\n\r\n".to_vec();
        request.extend_from_slice(body);
        let began = Instant::now();
        send(&mut socket, &request).await;
        oversized_body_assert_response(&mut socket, 413).await;
        assert!(began.elapsed() < Duration::from_secs(1));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        oversized_body_assert_capacity_released(&server, &calls).await;
        server.stop().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_body_chunked_delivers_413_with_or_without_final_chunk() {
    for complete in [false, true] {
        let (server, calls) = oversized_body_server().await;
        let mut socket = server.connect().await;
        let mut request =
            b"POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n1400\r\n"
                .to_vec();
        request.extend_from_slice(&vec![b'x'; 5120]);
        if complete {
            request.extend_from_slice(b"\r\n0\r\n\r\n");
        }
        let began = Instant::now();
        send(&mut socket, &request).await;
        oversized_body_assert_response(&mut socket, 413).await;
        assert!(began.elapsed() < Duration::from_secs(1));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        oversized_body_assert_capacity_released(&server, &calls).await;
        server.stop().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_body_rejects_keep_alive_upload_and_never_dispatches_pipeline() {
    let (server, calls) = oversized_body_server().await;
    let mut socket = server.connect().await;
    send(&mut socket, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    assert_eq!(response(&mut socket, false).await.status, 200);
    calls.store(0, Ordering::SeqCst);
    let mut request =
        b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5120\r\n\r\n".to_vec();
    request.extend_from_slice(&vec![b'x'; 5120]);
    request.extend_from_slice(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
    send(&mut socket, &request).await;
    oversized_body_assert_response(&mut socket, 413).await;
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    oversized_body_assert_capacity_released(&server, &calls).await;
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_body_boundary_accepts_4096_and_incomplete_upload_keeps_body_deadline() {
    let (server, calls) = oversized_body_server().await;
    let mut socket = server.connect().await;
    let mut request =
        b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 4096\r\n\r\n".to_vec();
    request.extend_from_slice(&vec![b'x'; 4096]);
    send(&mut socket, &request).await;
    assert_eq!(response(&mut socket, false).await.status, 200);
    calls.store(0, Ordering::SeqCst);
    let began = Instant::now();
    send(
        &mut socket,
        b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 4096\r\n\r\nx",
    )
    .await;
    oversized_body_assert_response(&mut socket, 408).await;
    assert!(began.elapsed() >= Duration::from_millis(150));
    assert!(began.elapsed() < Duration::from_secs(1));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    oversized_body_assert_capacity_released(&server, &calls).await;
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
async fn wait(request: Request, state: Arc<Gate>, _: ()) -> Result<Response, Error> {
    let _dropped = Dropped(Arc::clone(&state.cancelled));
    state.started.notify_one();
    state.released.notified().await;
    text(response_context(&request), Status::OK, "released")
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
    let app = route(app_default(state), Method::POST, "/", public_policy(), wait).unwrap();
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
    let app = route(app_default(state), Method::GET, "/", public_policy(), wait).unwrap();
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handler_timeout_rejects_a_ready_response_after_blocking_poll() {
    async fn slow(request: Request, _: Arc<()>, _: ()) -> Result<Response, Error> {
        // A timer cannot preempt this poll. Once it returns, the server must
        // still reject a response completed after the handler deadline.
        std::thread::sleep(Duration::from_millis(80));
        text(response_context(&request), Status::OK, "late")
    }
    async fn fast(request: Request, _: Arc<()>, _: ()) -> Result<Response, Error> {
        text(response_context(&request), Status::OK, "fast")
    }
    let app = route(app_default(()), Method::GET, "/slow", public_policy(), slow).unwrap();
    let app = route(app, Method::GET, "/fast", public_policy(), fast).unwrap();
    let server = Server::new(
        app,
        capacity(options(1024, 1000, 10, 1000).unwrap(), 1, 1).unwrap(),
    )
    .await;
    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"GET /slow HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    assert_eq!(response(&mut socket, false).await.status, 504);
    send(
        &mut socket,
        b"GET /fast HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    let result = response(&mut socket, false).await;
    assert_eq!(result.status, 200);
    assert_eq!(result.body, b"fast");
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handler_panic_after_deadline_returns_500_and_releases_admission() {
    async fn late_panic(_: Request, _: Arc<()>, _: ()) -> Result<Response, Error> {
        // This poll cannot be preempted. If it unwinds after the deadline,
        // preserve the panic boundary (500 and close), rather than the 504
        // used for a response that completed too late.
        std::thread::sleep(Duration::from_millis(80));
        panic!("private late handler failure");
    }
    async fn healthy(request: Request, _: Arc<()>, _: ()) -> Result<Response, Error> {
        text(response_context(&request), Status::OK, "healthy")
    }
    let app = route(
        app_default(()),
        Method::GET,
        "/panic",
        public_policy(),
        late_panic,
    )
    .unwrap();
    let app = route(app, Method::GET, "/healthy", public_policy(), healthy).unwrap();
    let server = Server::new(
        app,
        capacity(options(1024, 1000, 10, 1000).unwrap(), 1, 1).unwrap(),
    )
    .await;
    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"GET /panic HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    let failed = response(&mut socket, false).await;
    assert_eq!(failed.status, 500);
    assert_eq!(failed.all("connection"), ["close"]);
    assert_eq!(failed.body, b"Internal Server Error");
    assert!(closed(&mut socket).await.is_empty());

    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"GET /healthy HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    let healthy = response(&mut socket, false).await;
    assert_eq!(healthy.status, 200);
    assert_eq!(healthy.body, b"healthy");
    server.stop().await;
}

#[tokio::test]
async fn connection_capacity_and_shutdown_deadline_leave_no_handler_tasks() {
    let (state, started, _, cancelled) = gate();
    let app = route(app_default(state), Method::GET, "/", public_policy(), wait).unwrap();
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
    let probe = Arc::new(lifecycle_probe::Probe::default());
    let app = route(app_default(state), Method::GET, "/", public_policy(), wait).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopping) = oneshot::channel();
    let (observed, shutdown_observed) = oneshot::channel();
    let mut config = authority(
        options(1024, 1000, 1000, 1200).unwrap(),
        "https://localhost",
        vec!["localhost".to_owned()],
        1,
        128,
    )
    .unwrap();
    config.lifecycle_probe = Some(Arc::clone(&probe));
    let config = with_response_fixture(config);
    let (app, config) = with_rate_fixture(app, config);
    let task = tokio::spawn(serve_listener(listener, app, config, async {
        stopping.await.unwrap();
        observed.send(()).unwrap();
    }));
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
    assert!(probe.listener_closed.load(Ordering::SeqCst));
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
    assert_eq!(probe.joins.load(Ordering::SeqCst), 1);
    // Closed-listener connect rejection can take substantial platform time.
    // It must not consume the in-flight handler/shutdown budget before release.
    assert!(TcpStream::connect(address).await.is_err());
}

#[tokio::test]
async fn header_idle_deadline_is_independent_of_send_deadline_after_flush() {
    let server = Server::new(
        route(
            app_default("ok".to_owned()),
            Method::GET,
            "/",
            public_policy(),
            hello,
        )
        .unwrap(),
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
async fn header_deadline_expires_silent_dripped_and_buffered_partial_requests() {
    let app = route(
        app_default("ok".to_owned()),
        Method::GET,
        "/",
        public_policy(),
        hello,
    )
    .unwrap();
    let server = Server::new(app, header_timeout(default_options(), 400).unwrap()).await;

    let mut silent = server.connect().await;
    let expired = tokio::time::timeout(Duration::from_secs(2), closed(&mut silent))
        .await
        .unwrap();
    assert!(
        expired.is_empty() || expired.starts_with(b"HTTP/1.1 408"),
        "{expired:?}"
    );

    let mut dripped = server.connect().await;
    send(&mut dripped, b"GET / HTTP/1.1\r\nHost:").await;
    for _ in 0..3 {
        tokio::time::sleep(Duration::from_millis(70)).await;
        send(&mut dripped, b"x").await;
    }
    // The deadline is absolute: drips do not grant a fresh interval. If each
    // byte reset the timer, this read would still be waiting after 300ms.
    let expired = tokio::time::timeout(Duration::from_millis(300), closed(&mut dripped))
        .await
        .unwrap();
    assert!(
        expired.is_empty() || expired.starts_with(b"HTTP/1.1 408"),
        "{expired:?}"
    );

    let mut buffered = server.connect().await;
    send(
        &mut buffered,
        b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\nGET / HTTP/1.1\r\nHost: localhost\r\n\r\nGET / HTTP/1.1\r\nHost:",
    )
    .await;
    for _ in 0..2 {
        let result = response(&mut buffered, false).await;
        assert_eq!(result.status, 200);
        assert_eq!(result.body, b"ok");
    }
    let expired = tokio::time::timeout(Duration::from_secs(2), closed(&mut buffered))
        .await
        .unwrap();
    assert!(
        expired.is_empty() || expired.starts_with(b"HTTP/1.1 408"),
        "{expired:?}"
    );
    server.stop().await;
}

#[tokio::test]
async fn connection_capacity_supports_more_than_128_idle_sockets_and_shutdown_closes_them() {
    let app = route(
        app_default("ok".to_owned()),
        Method::GET,
        "/",
        public_policy(),
        hello,
    )
    .unwrap();
    let server = Server::new(app, capacity(default_options(), 160, 1).unwrap()).await;
    let mut idle = Vec::with_capacity(129);
    for _ in 0..129 {
        idle.push(server.connect().await);
    }

    let mut active = server.connect().await;
    send(&mut active, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    let result = response(&mut active, false).await;
    assert_eq!(result.status, 200);
    assert_eq!(result.body, b"ok");

    server.stop().await;
    for socket in &mut idle {
        assert!(closed(socket).await.is_empty());
    }
}

const STALLED_BODY_LENGTH: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy)]
enum StalledFixtureCase {
    Capacity,
    Graceful,
}
impl StalledFixtureCase {
    fn label(self) -> &'static str {
        match self {
            Self::Capacity => "Capacity",
            Self::Graceful => "Graceful",
        }
    }
}

#[derive(Clone, Copy)]
enum StalledFixturePhase {
    Setup,
    AwaitPending,
    ArmedStatus,
    ArmedBody,
    AwaitJoin,
    Expired,
    EndUnchanged,
    FreshConnect,
    FreshRequestWrite,
    FreshResponse,
    FreshStatus,
    FreshBody,
    StopJoin,
    ShutdownSignal,
    AwaitShutdown,
    GraceBeforeEnd,
    JoinCount,
    TerminalRead,
    TerminalPartial,
}
impl StalledFixturePhase {
    fn label(self) -> &'static str {
        match self {
            Self::Setup => "Setup",
            Self::AwaitPending => "AwaitPending",
            Self::ArmedStatus => "ArmedStatus",
            Self::ArmedBody => "ArmedBody",
            Self::AwaitJoin => "AwaitJoin",
            Self::Expired => "Expired",
            Self::EndUnchanged => "EndUnchanged",
            Self::FreshConnect => "FreshConnect",
            Self::FreshRequestWrite => "FreshRequestWrite",
            Self::FreshResponse => "FreshResponse",
            Self::FreshStatus => "FreshStatus",
            Self::FreshBody => "FreshBody",
            Self::StopJoin => "StopJoin",
            Self::ShutdownSignal => "ShutdownSignal",
            Self::AwaitShutdown => "AwaitShutdown",
            Self::GraceBeforeEnd => "GraceBeforeEnd",
            Self::JoinCount => "JoinCount",
            Self::TerminalRead => "TerminalRead",
            Self::TerminalPartial => "TerminalPartial",
        }
    }
}

struct StalledFixtureFailureMarker {
    case: StalledFixtureCase,
    phase: StalledFixturePhase,
    probe: Arc<lifecycle_probe::Probe>,
    end_changed: Option<bool>,
}
impl StalledFixtureFailureMarker {
    fn new(case: StalledFixtureCase, probe: Arc<lifecycle_probe::Probe>) -> Self {
        Self {
            case,
            phase: StalledFixturePhase::Setup,
            probe,
            end_changed: None,
        }
    }

    fn at(&mut self, phase: StalledFixturePhase) {
        self.phase = phase;
    }

    fn end_changed(&mut self, changed: bool) {
        self.end_changed = Some(changed);
    }
}
impl Drop for StalledFixtureFailureMarker {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            return;
        }
        let armed = self.probe.first.get();
        let status = armed.map(|value| value.status);
        let body_bytes = armed
            .filter(|value| value.body_bytes <= STALLED_BODY_LENGTH)
            .map(|value| value.body_bytes);
        let joins = u32::try_from(self.probe.joins.load(Ordering::SeqCst)).ok();
        let _ = writeln!(
            std::io::stderr().lock(),
            "http_stalled_fixture case={} phase={} armed={} pending={} expired={} grace_before_end={} joins={:?} status={:?} body_bytes={:?} end_changed={:?}",
            self.case.label(),
            self.phase.label(),
            armed.is_some(),
            self.probe.pending.load(Ordering::SeqCst),
            self.probe.expired.load(Ordering::SeqCst),
            self.probe.graceful_before_end.load(Ordering::SeqCst),
            joins,
            status,
            body_bytes,
            self.end_changed,
        );
    }
}

struct PreparedLarge {
    body: Bytes,
}
fn prepared_large() -> PreparedLarge {
    // Allocate once before any request/response timer. Handler work is then a
    // bounded Bytes clone, rather than 16MiB allocation plus a second copy.
    PreparedLarge {
        body: Bytes::from(vec![b'x'; STALLED_BODY_LENGTH]),
    }
}
async fn prepared_large_response(
    request: Request,
    state: Arc<PreparedLarge>,
    _: (),
) -> Result<Response, Error> {
    response_public::tests::adopt_bytes_fixture(
        response_context(&request),
        Status::OK,
        state.body.clone(),
        BodyKind::Binary,
    )
}
async fn prepared_healthy_response(
    request: Request,
    _: Arc<PreparedLarge>,
    _: (),
) -> Result<Response, Error> {
    text(response_context(&request), Status::OK, "ok")
}
async fn backpressured_socket(server: &Server) -> BufReader<TcpStream> {
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    socket.set_recv_buffer_size(4096).unwrap();
    BufReader::new(socket.connect(server.address).await.unwrap())
}

#[tokio::test]
async fn stalled_response_send_expires_and_releases_connection_permit() {
    let probe = Arc::new(lifecycle_probe::Probe::default());
    let mut failure_marker =
        StalledFixtureFailureMarker::new(StalledFixtureCase::Capacity, Arc::clone(&probe));
    let app = route(
        app_default(prepared_large()),
        Method::GET,
        "/large",
        public_policy(),
        prepared_large_response,
    )
    .unwrap();
    let app = route(
        app,
        Method::GET,
        "/",
        public_policy(),
        prepared_healthy_response,
    )
    .unwrap();
    let mut config = send_timeout(capacity(default_options(), 1, 1).unwrap(), 80).unwrap();
    config.lifecycle_probe = Some(Arc::clone(&probe));
    // Windows uses the reviewed bounded-prefix native schedule. Actual native
    // Pending is still required; this is not a natural full-buffer OS premise.
    #[cfg(windows)]
    {
        config.windows_stalled_fixture = true;
    }
    let server = Server::new_with_send_buffer(app, config, Some(4096)).await;
    let mut blocked = backpressured_socket(&server).await;
    send(
        &mut blocked,
        b"GET /large HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    failure_marker.at(StalledFixturePhase::AwaitPending);
    tokio::time::timeout(
        Duration::from_secs(2),
        probe.wait(|p| p.pending.load(Ordering::SeqCst)),
    )
    .await
    .unwrap();
    failure_marker.at(StalledFixturePhase::ArmedStatus);
    let armed = *probe.first.get().unwrap();
    assert_eq!(armed.status, 200);
    failure_marker.at(StalledFixturePhase::ArmedBody);
    assert_eq!(armed.body_bytes, STALLED_BODY_LENGTH);
    // Native write Pending proves backpressure. JoinSet observation, rather
    // than sleep or child Drop, proves the original socket task has finished.
    failure_marker.at(StalledFixturePhase::AwaitJoin);
    tokio::time::timeout(
        Duration::from_secs(2),
        probe.wait(|p| p.joins.load(Ordering::SeqCst) == 1),
    )
    .await
    .unwrap();
    failure_marker.at(StalledFixturePhase::Expired);
    assert!(probe.expired.load(Ordering::SeqCst));
    failure_marker.at(StalledFixturePhase::EndUnchanged);
    failure_marker.end_changed(probe.first.get().unwrap().end != armed.end);
    assert_eq!(probe.first.get().unwrap().end, armed.end);
    failure_marker.at(StalledFixturePhase::FreshConnect);
    let mut next = server.connect().await;
    failure_marker.at(StalledFixturePhase::FreshRequestWrite);
    send(&mut next, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    failure_marker.at(StalledFixturePhase::FreshResponse);
    let healthy = response(&mut next, false).await;
    failure_marker.at(StalledFixturePhase::FreshStatus);
    assert_eq!(healthy.status, 200);
    failure_marker.at(StalledFixturePhase::FreshBody);
    assert_eq!(healthy.body, b"ok");
    drop(blocked);
    failure_marker.at(StalledFixturePhase::StopJoin);
    server.stop().await;
}

#[tokio::test]
async fn graceful_shutdown_preserves_stalled_response_send_deadline() {
    let probe = Arc::new(lifecycle_probe::Probe::default());
    let mut failure_marker =
        StalledFixtureFailureMarker::new(StalledFixtureCase::Graceful, Arc::clone(&probe));
    let app = route(
        app_default(prepared_large()),
        Method::GET,
        "/",
        public_policy(),
        prepared_large_response,
    )
    .unwrap();
    let mut config = send_timeout(options(1024, 1000, 1000, 1200).unwrap(), 150).unwrap();
    config.lifecycle_probe = Some(Arc::clone(&probe));
    // Windows uses the reviewed bounded-prefix native schedule. Actual native
    // Pending is still required; this is not a natural full-buffer OS premise.
    #[cfg(windows)]
    {
        config.windows_stalled_fixture = true;
    }
    let mut server = Server::new_with_send_buffer(app, config, Some(4096)).await;
    let mut blocked = backpressured_socket(&server).await;
    send(&mut blocked, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    failure_marker.at(StalledFixturePhase::AwaitPending);
    tokio::time::timeout(
        Duration::from_secs(2),
        probe.wait(|p| p.pending.load(Ordering::SeqCst)),
    )
    .await
    .unwrap();
    failure_marker.at(StalledFixturePhase::ArmedStatus);
    let armed = *probe.first.get().unwrap();
    assert_eq!(armed.status, 200);
    failure_marker.at(StalledFixturePhase::ArmedBody);
    assert_eq!(armed.body_bytes, STALLED_BODY_LENGTH);
    failure_marker.at(StalledFixturePhase::ShutdownSignal);
    server.stopped.take().unwrap().send(()).unwrap();
    failure_marker.at(StalledFixturePhase::AwaitShutdown);
    tokio::time::timeout(Duration::from_millis(600), &mut server.task)
        .await
        .expect("graceful shutdown lost the stalled response send deadline")
        .unwrap()
        .unwrap();
    failure_marker.at(StalledFixturePhase::GraceBeforeEnd);
    assert!(probe.graceful_before_end.load(Ordering::SeqCst));
    failure_marker.at(StalledFixturePhase::Expired);
    assert!(probe.expired.load(Ordering::SeqCst));
    failure_marker.at(StalledFixturePhase::EndUnchanged);
    failure_marker.end_changed(probe.first.get().unwrap().end != armed.end);
    assert_eq!(probe.first.get().unwrap().end, armed.end);
    failure_marker.at(StalledFixturePhase::JoinCount);
    assert_eq!(probe.joins.load(Ordering::SeqCst), 1);
    // An expired partial send cannot promise header delivery/FIN on every OS.
    // The complete healthy 200 remains asserted in the capacity and graceful
    // in-flight tests; this socket proves terminal partial-send disposition.
    failure_marker.at(StalledFixturePhase::TerminalRead);
    let terminal = terminal_close_after_abort(&mut blocked, STALLED_BODY_LENGTH - 1).await;
    failure_marker.at(StalledFixturePhase::TerminalPartial);
    assert!(terminal.len() < STALLED_BODY_LENGTH);
}

#[path = "security_tests.rs"]
mod security_tests;

#[path = "unsafe_session_tests.rs"]
mod unsafe_session_tests;

#[path = "transport_lifecycle_tests.rs"]
mod transport_lifecycle_tests;

#[path = "send_deadline_tests.rs"]
mod send_deadline_tests;
