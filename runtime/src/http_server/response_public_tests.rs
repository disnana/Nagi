//! Explicit finite unit-fixture inputs. No production defaults or Context mint.
use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::oneshot,
};

type SecurityRefusalCase = (fn() -> Failure, u16, &'static [u8]);
fn stable_security_refusal_cases() -> [SecurityRefusalCase; 6] {
    [
        (Failure::invalid_credential, 401, b"invalid credential"),
        (Failure::denied, 403, b"permission denied"),
        (Failure::expired, 401, b"authority expired"),
        (Failure::invalid_request, 400, b"invalid security request"),
        (Failure::unavailable, 503, b"security service unavailable"),
        (Failure::internal, 500, b"security service failure"),
    ]
}

#[test]
fn security_refusals_keep_six_stable_bodies_head_cors_and_challenges() {
    for (failure, status, body) in stable_security_refusal_cases() {
        assert_eq!(failure().message().as_bytes(), body);
        for source in [PolicySource::Bearer, PolicySource::Session] {
            for head in [false, true] {
                for fault in [false, true] {
                    let mut limits = limits_fixture(256, 1);
                    limits.fallback_responses = 2;
                    let options =
                        with_response_limits(capacity(default_options(), 1, 1).unwrap(), limits)
                            .unwrap();
                    let runtime = RuntimeResponses::checked(&options, 0).unwrap();
                    let mut context = runtime.context();
                    context.profile.origin =
                        Some(HeaderValue::from_static("https://app.example.invalid"));
                    context.profile.credentials = true;
                    // Early auth denial may precede source-profile attachment.
                    context.profile.dependencies = vary::Dependencies::ORIGIN;
                    let mut response = security_response(&context, failure(), source, head);
                    if fault {
                        response
                            .headers
                            .as_mut()
                            .unwrap()
                            .inject_fault(Diagnostic::Fault);
                    }
                    // An actual new internal fault still classifies unsealed
                    // 400/503 as500. Established401/403 keep their prior refusal.
                    let (expected_status, expected_body) = if fault && matches!(status, 400 | 503) {
                        (500, b"internal error".as_slice())
                    } else {
                        (status, body)
                    };
                    let ledger = response_send::SendLedger::new(
                        Arc::clone(&runtime.producer),
                        Arc::clone(&runtime.fallback),
                    );
                    let native = response.transfer_native(&ledger, head).unwrap();
                    assert_eq!(native.status().as_u16(), expected_status);
                    assert_eq!(
                        native.body().bytes.as_ref(),
                        if head { b"".as_slice() } else { expected_body }
                    );
                    assert_eq!(
                        native.headers()[names::CONTENT_LENGTH].to_str().unwrap(),
                        expected_body.len().to_string()
                    );
                    assert_eq!(
                        native.headers()[names::CONTENT_TYPE],
                        "text/plain; charset=utf-8"
                    );
                    assert_eq!(native.headers()[names::CACHE_CONTROL], "no-store");
                    assert_eq!(
                        native.headers()[names::CONTENT_SECURITY_POLICY],
                        CONTENT_SECURITY_POLICY
                    );
                    assert_eq!(native.headers()[names::X_CONTENT_TYPE_OPTIONS], "nosniff");
                    assert_eq!(native.headers()[names::CONNECTION], "close");
                    assert_eq!(
                        native.headers()[names::ACCESS_CONTROL_ALLOW_ORIGIN],
                        "https://app.example.invalid"
                    );
                    assert_eq!(
                        native.headers()[names::ACCESS_CONTROL_ALLOW_CREDENTIALS],
                        "true"
                    );
                    assert_eq!(
                        native.headers()[names::VARY],
                        "Authorization, Cookie, Origin"
                    );
                    assert_eq!(
                        native
                            .headers()
                            .get(names::WWW_AUTHENTICATE)
                            .map(|v| v.as_bytes()),
                        if expected_status == 401 {
                            Some(if source == PolicySource::Session {
                                b"Session".as_slice()
                            } else {
                                b"Bearer".as_slice()
                            })
                        } else {
                            None
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn security_replacement_retains_typed_body_and_generic_status_fallback_stays_generic() {
    for (failure, status, body) in stable_security_refusal_cases() {
        let mut limits = limits_fixture(256, 1);
        limits.fallback_responses = 2;
        let options =
            with_response_limits(capacity(default_options(), 1, 1).unwrap(), limits).unwrap();
        let runtime = RuntimeResponses::checked(&options, 0).unwrap();
        let context = runtime.context();
        let response = security_response(&context, failure(), PolicySource::Session, false);
        let replacement =
            response.replacement(Status(StatusCode::from_u16(status).unwrap()), false, true);
        drop(response);
        let ledger = response_send::SendLedger::new(
            Arc::clone(&runtime.producer),
            Arc::clone(&runtime.fallback),
        );
        let native = replacement.transfer_native(&ledger, false).unwrap();
        assert_eq!(native.status().as_u16(), status);
        assert_eq!(native.body().bytes.as_ref(), body);
    }
    for (status, body) in [
        (Status::UNAUTHORIZED, b"unauthorized".as_slice()),
        (Status::BAD_REQUEST, b"Bad Request".as_slice()),
        (
            Status::SERVICE_UNAVAILABLE,
            b"service unavailable".as_slice(),
        ),
    ] {
        let request = request_fixture(256);
        let context = response_context(&request);
        let response = HttpResponse::terminal(Arc::clone(&context.fallback), status, false, true);
        assert_eq!(response.body().bytes.as_ref(), body);
    }
}

pub(in crate::http_server) fn headers_fixture(map: HeaderMap, body_bound: usize) -> RequestHeaders {
    let body = i64::try_from(body_bound).unwrap();
    let retained = body
        .checked_mul(8)
        .and_then(|n| n.checked_add(16 * 1024 * 1024))
        .unwrap();
    let options = capacity(default_options(), 1, 1).unwrap();
    let options = with_response_limits(
        options,
        response_limits(
            body,
            65536,
            256,
            4096,
            32,
            131072,
            8,
            retained,
            1,
            4 * 1024 * 1024,
            4096,
            32,
        )
        .unwrap(),
    )
    .unwrap();
    let runtime = RuntimeResponses::checked(&options, 0).unwrap();
    let context = runtime.context();
    let mut headers = RequestHeaders::from_received(request_headers::ReceivedHeaders::configured(
        map, None, context,
    ));
    headers.rate(rate_public::fixture_request_rate()).unwrap();
    headers
        .admit(Arc::clone(&runtime.requests).try_acquire_owned().unwrap())
        .unwrap();
    headers
}
pub(crate) fn request_fixture(body_bound: usize) -> Request {
    Request {
        method: Method::GET,
        uri: Uri::from_static("/"),
        body: Bytes::new(),
        headers: headers_fixture(HeaderMap::new(), body_bound),
    }
}
pub(in crate::http_server) fn adopt_bytes_fixture(
    context: &ResponseContext,
    status: Status,
    body: Bytes,
    kind: BodyKind,
) -> Result<Response, Error> {
    representation(context, status, kind, |producer| {
        producer.adopt_fixture_bytes(body)
    })
}
fn limits_fixture(body: i64, live: i64) -> ResponseLimits {
    response_limits(
        body,
        512,
        16,
        4096,
        32,
        128,
        live,
        16 * 1024 * 1024,
        1,
        4 * 1024 * 1024,
        4096,
        32,
    )
    .unwrap()
}
#[test]
fn missing_explicit_response_settings_and_later_incompatible_capacity_reject() {
    assert!(RuntimeResponses::checked(&default_options(), 0).is_err());
    assert!(response_limits(0, 512, 16, 4096, 32, 128, 1, 16777216, 1, 4194304, 4096, 32).is_err());
    let options = with_response_limits(
        capacity(default_options(), 1, 1).unwrap(),
        limits_fixture(256, 1),
    )
    .unwrap();
    assert!(
        capacity(options, 2, 1).is_err(),
        "one fallback slot cannot admit two native connections"
    );
}
#[test]
fn public_context_is_request_owned_and_json_has_distinct_body_and_work_limits() {
    let options = with_response_limits(
        capacity(default_options(), 1, 1).unwrap(),
        limits_fixture(256, 1),
    )
    .unwrap();
    let runtime = RuntimeResponses::checked(&options, 0).unwrap();
    let mut headers = RequestHeaders::from_received(request_headers::ReceivedHeaders::configured(
        HeaderMap::new(),
        None,
        runtime.context(),
    ));
    headers
        .admit(Arc::clone(&runtime.requests).try_acquire_owned().unwrap())
        .unwrap();
    let request = Request {
        method: Method::GET,
        uri: Uri::from_static("/"),
        body: Bytes::new(),
        headers,
    };
    let view = request.security_snapshot();
    assert_eq!(runtime.requests.available_permits(), 0);
    let context = response_context(&request);
    let body_error = json(context, Status::OK, &format!("{}\\", "x".repeat(254))).unwrap_err();
    assert_eq!(body_error.message, "response body limit");
    let work_error = json(context, Status::OK, &"\\".repeat(200)).unwrap_err();
    assert_eq!(work_error.message, "response serialization limit");
    let response = text(context, Status::OK, "kept").unwrap();
    assert!(empty(context, Status::OK).is_err());
    drop(response);
    assert!(empty(context, Status::OK).is_ok());
    drop(request);
    assert_eq!(
        runtime.requests.available_permits(),
        0,
        "shared header owner retains the same receipt"
    );
    drop(view);
    assert_eq!(runtime.requests.available_permits(), 1);
}
#[tokio::test]
async fn public_mapper_failure_and_business_error_remain_distinct_completion_kinds() {
    struct BusinessError;
    fn broken(_: &ResponseContext, _: BusinessError) -> Result<Response, Error> {
        Err(Error::internal("closed mapper failure"))
    }
    fn mapped(context: &ResponseContext, _: BusinessError) -> Result<Response, Error> {
        text(context, Status::FORBIDDEN, "business refusal")
    }
    let owner =
        LeaseOwner::new(Instant::now().checked_add(Duration::from_secs(2)).unwrap()).unwrap();
    let handler = Arc::new(|_: Request, _: Arc<()>, ()| async {
        Err::<Response, BusinessError>(BusinessError)
    });
    let prepared = prepared_handler(
        Arc::clone(&handler),
        broken,
        (),
        Arc::clone(&owner.lease),
        PolicySource::Public,
    );
    assert!(matches!(
        prepared(request_fixture(256), Arc::new(())).await,
        PreparedCompletion::MapperFailure(_)
    ));
    let prepared = prepared_handler(
        handler,
        mapped,
        (),
        Arc::clone(&owner.lease),
        PolicySource::Public,
    );
    match prepared(request_fixture(256), Arc::new(())).await {
        PreparedCompletion::Replacement(response) => {
            assert_eq!(response.status(), Status::FORBIDDEN);
            assert_eq!(response.body(), b"business refusal");
        }
        _ => panic!("business Err with a successful mapper must remain a replacement"),
    }
    assert_eq!(
        refusal_status(Status::FORBIDDEN, Diagnostic::HeaderLimit),
        Status::FORBIDDEN
    );
    assert_eq!(
        refusal_status(Status::OK, Diagnostic::HeaderLimit),
        Status::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        refusal_status(Status::FORBIDDEN, Diagnostic::Fault),
        Status::INTERNAL_SERVER_ERROR
    );
}
#[test]
fn managed_header_refusal_keeps_denial_and_independent_fallback_exhaustion_closes() {
    let options = with_response_limits(
        capacity(default_options(), 1, 1).unwrap(),
        limits_fixture(256, 1),
    )
    .unwrap();
    let runtime = RuntimeResponses::checked(&options, 0).unwrap();
    let context = runtime.context();
    let mut response = text(&context, Status::FORBIDDEN, "denied")
        .unwrap()
        .into_http(false);
    response.headers_mut().insert(
        names::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_str(&"x".repeat(4097)).unwrap(),
    );
    let ledger = response_send::SendLedger::new(
        Arc::clone(&runtime.producer),
        Arc::clone(&runtime.fallback),
    );
    let native = response.transfer_native(&ledger, false).unwrap();
    assert_eq!(native.status(), StatusCode::FORBIDDEN);
    let denied = transport(&context, Status::FORBIDDEN, false, true);
    assert!(
        denied.transfer_native(&ledger, false).is_err(),
        "occupied finite fallback cannot become uncharged response"
    );
    drop(native);
}
#[tokio::test]
async fn public_first_request_header_owner_survives_native_join_and_permit_reuse() {
    const WAIT: Duration = Duration::from_secs(2);
    struct State {
        calls: AtomicUsize,
        held: Mutex<Option<RequestHeaders>>,
    }
    let state = Arc::new(State {
        calls: AtomicUsize::new(0),
        held: Mutex::new(None),
    });
    let handler_state = Arc::clone(&state);
    let app = route(
        app((), default_error),
        Method::GET,
        "/",
        public_policy(),
        move |request: Request, _, ()| {
            let state = Arc::clone(&handler_state);
            async move {
                state.calls.fetch_add(1, Ordering::SeqCst);
                let response = empty(response_context(&request), Status::OK)?;
                // Retain precisely the public shared-header owner, not prior response
                // bytes. The first request on a fresh connection has no prior claim.
                let mut held = state.held.lock().unwrap();
                if held.is_none() {
                    *held = Some(request.headers.share());
                }
                Ok::<_, Error>(response)
            }
        },
    )
    .unwrap();
    let app = rate_route(app, Method::GET, "/", rate_policy(64, 60_000).unwrap()).unwrap();
    let probe = Arc::new(lifecycle_probe::Probe::default());
    let mut options = with_response_limits(
        capacity(default_options(), 1, 1).unwrap(),
        limits_fixture(256, 1),
    )
    .unwrap();
    options = with_rate_limits(
        options,
        rate_limits(1, 1, 1, 4 * 1024 * 1024, 10_000).unwrap(),
    )
    .unwrap();
    options = authority(
        options,
        "https://localhost",
        vec!["localhost".into()],
        1,
        128,
    )
    .unwrap();
    options.lifecycle_probe = Some(Arc::clone(&probe));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel();
    let server = tokio::spawn(serve_listener(listener, app, options, async {
        let _ = stopped.await;
    }));
    for turn in 0..3 {
        if turn == 2 {
            drop(state.held.lock().unwrap().take());
        }
        let mut client = TcpStream::connect(address).await.unwrap();
        client
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut response = Vec::new();
        tokio::time::timeout(WAIT, client.read_to_end(&mut response))
            .await
            .unwrap()
            .unwrap();
        let status = std::str::from_utf8(&response)
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse::<u16>()
            .unwrap();
        assert_eq!(status, if turn == 1 { 503 } else { 200 });
        tokio::time::timeout(
            WAIT,
            probe.wait(|probe| probe.joins.load(Ordering::SeqCst) > turn),
        )
        .await
        .unwrap();
        assert_eq!(
            state.calls.load(Ordering::SeqCst),
            if turn < 2 { 1 } else { 2 }
        );
    }
    stop.send(()).unwrap();
    tokio::time::timeout(WAIT, server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[test]
fn native_fault_classification_distinguishes_user_status_from_established_refusal() {
    for (status, error, established, expected) in [
        (Status::OK, Diagnostic::Fault, false, 500),
        (Status::FORBIDDEN, Diagnostic::Fault, false, 500),
        (Status::FORBIDDEN, Diagnostic::InvalidLimits, false, 500),
        (Status::FORBIDDEN, Diagnostic::HeaderLimit, false, 403),
        (Status::OK, Diagnostic::HeaderLimit, false, 503),
        (Status::UNAUTHORIZED, Diagnostic::Fault, true, 401),
        (Status::FORBIDDEN, Diagnostic::Fault, true, 403),
        (Status::REQUEST_TIMEOUT, Diagnostic::Fault, true, 408),
        (Status::CONTENT_TOO_LARGE, Diagnostic::Fault, true, 413),
        (Status::GATEWAY_TIMEOUT, Diagnostic::Fault, true, 504),
    ] {
        let mut limits = limits_fixture(256, 1);
        limits.fallback_responses = 2;
        let options =
            with_response_limits(capacity(default_options(), 1, 1).unwrap(), limits).unwrap();
        let runtime = RuntimeResponses::checked(&options, 0).unwrap();
        let context = runtime.context();
        let mut response = if established {
            match status {
                Status::UNAUTHORIZED => security_response(
                    &context,
                    Failure::invalid_credential(),
                    PolicySource::Session,
                    false,
                ),
                Status::FORBIDDEN => {
                    security_response(&context, Failure::denied(), PolicySource::Session, false)
                }
                _ => transport(&context, status, false, true),
            }
        } else {
            text(&context, status, "ordinary handler")
                .unwrap()
                .into_http(false)
        };
        assert_eq!(response.runtime_refusal.is_some(), established);
        response.headers.as_mut().unwrap().inject_fault(error);
        let ledger = response_send::SendLedger::new(
            Arc::clone(&runtime.producer),
            Arc::clone(&runtime.fallback),
        );
        let native = response.transfer_native(&ledger, false).unwrap();
        assert_eq!(native.status().as_u16(), expected);
        if established && expected == 401 {
            assert_eq!(
                native.headers().get(names::WWW_AUTHENTICATE).unwrap(),
                "Session"
            );
        }
    }
}

#[test]
fn allow_serialization_preflights_before_allocation_and_keeps_405_close() {
    let options = with_response_limits(
        capacity(default_options(), 1, 1).unwrap(),
        limits_fixture(256, 1),
    )
    .unwrap();
    let runtime = RuntimeResponses::checked(&options, 0).unwrap();
    let context = runtime.context();
    let mut response = transport(&context, Status::METHOD_NOT_ALLOWED, false, true);
    assert_eq!(
        response.allow_methods(["PATCH", "GET", "POST"].into_iter()),
        Ok(())
    );
    assert_eq!(
        response.headers().get(names::ALLOW).unwrap(),
        "GET, HEAD, PATCH, POST"
    );
    assert_eq!(response.allow_allocations, 1);
    drop(response);
    let mut response = transport(&context, Status::METHOD_NOT_ALLOWED, false, true);
    let too_long = "X".repeat(4097);
    assert_eq!(
        response.allow_methods([too_long.as_str()].into_iter()),
        Err(Diagnostic::HeaderLimit)
    );
    assert_eq!(
        response.allow_allocations, 0,
        "no Vec/join/value allocation before full length admission"
    );
    let response = response.close_before_native();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    let ledger = response_send::SendLedger::new(
        Arc::clone(&runtime.producer),
        Arc::clone(&runtime.fallback),
    );
    assert!(response.transfer_native(&ledger, false).is_err());
}

#[test]
fn retained_response_never_restores_old_cors_when_current_headers_or_fallback_are_full() {
    for occupy_fallback in [false, true] {
        let options = with_response_limits(
            capacity(default_options(), 1, 1).unwrap(),
            limits_fixture(256, 1),
        )
        .unwrap();
        let runtime = RuntimeResponses::checked(&options, 0).unwrap();
        let mut old = runtime.context();
        old.profile.origin = Some(HeaderValue::from_static("https://old.example"));
        old.profile.credentials = true;
        let response = Mutex::new(Some(text(&old, Status::OK, "retained").unwrap()));
        let mut current = runtime.context();
        current.profile.origin = Some(HeaderValue::from_str(&"x".repeat(4097)).unwrap());
        let held =
            occupy_fallback.then(|| transport(&current, Status::SERVICE_UNAVAILABLE, false, true));
        let wire = response
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .into_replacement_http(&current, false);
        assert!(wire
            .headers()
            .get(names::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none());
        assert!(wire
            .headers()
            .get(names::ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .is_none());
        let ledger = response_send::SendLedger::new(
            Arc::clone(&runtime.producer),
            Arc::clone(&runtime.fallback),
        );
        if occupy_fallback {
            assert!(wire.transfer_native(&ledger, false).is_err());
        } else {
            assert_eq!(
                wire.transfer_native(&ledger, false).unwrap().status(),
                StatusCode::SERVICE_UNAVAILABLE
            );
        }
        drop(held);
    }
}

#[test]
fn prebuilt_fixture_adoption_keeps_same_claim_and_actual_bytes_owner_without_copy() {
    let options = with_response_limits(
        capacity(default_options(), 1, 1).unwrap(),
        limits_fixture(256, 1),
    )
    .unwrap();
    let runtime = RuntimeResponses::checked(&options, 0).unwrap();
    let context = runtime.context();
    let input = Bytes::from(vec![b'x'; 256]);
    let pointer = input.as_ptr();
    let response = adopt_bytes_fixture(&context, Status::OK, input, BodyKind::Binary).unwrap();
    assert_eq!(response.body().as_ptr(), pointer);
    let ledger = response_send::SendLedger::new(
        Arc::clone(&runtime.producer),
        Arc::clone(&runtime.fallback),
    );
    let native = response
        .into_http(false)
        .transfer_native(&ledger, false)
        .unwrap();
    let frame = native.body().bytes.clone();
    assert_eq!(frame.as_ptr(), pointer);
    drop(native);
    drop(ledger.take_prior().unwrap());
    assert!(
        runtime.producer.empty().is_err(),
        "actual frame retains original claim"
    );
    drop(frame);
    assert!(runtime.producer.empty().is_ok());
    assert!(adopt_bytes_fixture(
        &context,
        Status::OK,
        Bytes::from(vec![0; 257]),
        BodyKind::Binary
    )
    .is_err());
    assert!(runtime.producer.empty().is_ok());
}

#[tokio::test]
async fn retained_same_producer_response_uses_current_request_cors_on_real_wire() {
    const WAIT: Duration = Duration::from_secs(2);
    let retained = Arc::new(Mutex::new(None::<Response>));
    let store = Arc::clone(&retained);
    let mut app = route(
        app((), default_error),
        Method::GET,
        "/store",
        public_policy(),
        move |request: Request, _, ()| {
            let store = Arc::clone(&store);
            async move {
                let context = response_context(&request);
                *store.lock().unwrap() = Some(text(context, Status::OK, "stored")?);
                empty(context, Status::OK)
            }
        },
    )
    .unwrap();
    for path in ["/plain", "/denied", "/other"] {
        let retained = Arc::clone(&retained);
        app = route(
            app,
            Method::GET,
            path,
            public_policy(),
            move |_: Request, _, ()| {
                let retained = Arc::clone(&retained);
                async move {
                    retained
                        .lock()
                        .unwrap()
                        .take()
                        .ok_or_else(|| Error::internal("fixture Response missing"))
                }
            },
        )
        .unwrap();
    }
    let policy = |origin: &str, credentials| {
        cors_policy(
            vec![origin.into()],
            Vec::new(),
            credentials,
            0,
            1,
            128,
            1,
            128,
        )
        .unwrap()
    };
    app = cors_route(
        app,
        Method::GET,
        "/store",
        policy("https://a.example", CorsCredentials::Allow),
    )
    .unwrap();
    app = cors_route(
        app,
        Method::GET,
        "/denied",
        policy("https://a.example", CorsCredentials::Allow),
    )
    .unwrap();
    app = cors_route(
        app,
        Method::GET,
        "/other",
        policy("https://c.example", CorsCredentials::Omit),
    )
    .unwrap();
    for routes in app.routes.values_mut() {
        for route in routes {
            route.rate = Some(rate_policy(64, 60_000).unwrap());
        }
    }
    let mut options = with_response_limits(
        capacity(default_options(), 1, 1).unwrap(),
        limits_fixture(256, 4),
    )
    .unwrap();
    options = with_rate_limits(
        options,
        rate_limits(4, 1, 1, 4 * 1024 * 1024, 10_000).unwrap(),
    )
    .unwrap();
    options = authority(
        options,
        "https://localhost",
        vec!["localhost".into()],
        1,
        128,
    )
    .unwrap();
    let probe = Arc::new(lifecycle_probe::Probe::default());
    options.lifecycle_probe = Some(Arc::clone(&probe));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel();
    let server = tokio::spawn(serve_listener(listener, app, options, async {
        let _ = stopped.await;
    }));
    let mut turn = 0;
    for (target, origin, expected) in [
        ("/plain", "https://b.example", None),
        ("/denied", "https://b.example", None),
        ("/other", "https://c.example", Some("https://c.example")),
    ] {
        for (path, origin) in [("/store", "https://a.example"), (target, origin)] {
            let mut client = TcpStream::connect(address).await.unwrap();
            let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nOrigin: {origin}\r\nConnection: close\r\n\r\n");
            client.write_all(request.as_bytes()).await.unwrap();
            let mut wire = Vec::new();
            tokio::time::timeout(WAIT, client.read_to_end(&mut wire))
                .await
                .unwrap()
                .unwrap();
            let wire = std::str::from_utf8(&wire).unwrap();
            assert_eq!(wire.split_whitespace().nth(1), Some("200"));
            let origin_header = wire
                .lines()
                .find_map(|line| line.strip_prefix("access-control-allow-origin: "));
            let credentials = wire
                .lines()
                .find_map(|line| line.strip_prefix("access-control-allow-credentials: "));
            if path == "/store" {
                assert_eq!(origin_header, Some("https://a.example"));
                assert_eq!(credentials, Some("true"));
            } else {
                assert_eq!(origin_header, expected);
                assert_eq!(credentials, None);
                assert!(wire.ends_with("stored"));
            }
            turn += 1;
            tokio::time::timeout(WAIT, probe.wait(|p| p.joins.load(Ordering::SeqCst) >= turn))
                .await
                .unwrap();
        }
    }
    stop.send(()).unwrap();
    tokio::time::timeout(WAIT, server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
