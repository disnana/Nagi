//! Test-network-admission seam through canonical public exports.
//! TLS, Grant, owner and joins remain real; numbers are private test profiles.
use crate::{auth, http_client as client, http_server as http};
use client::startup::*;
use hickory_resolver::proto::{
    op::Message,
    rr::{rdata::A, RData, Record, RecordType},
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::{
    net::{IpAddr, Ipv4Addr},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::oneshot,
    task::JoinHandle,
};

// Explicit small-fixture settings, never host defaults or a measured support profile.
pub(crate) fn retention_limits() -> RetentionLimits {
    RetentionLimits {
        max_root_anchors: 8,
        max_root_bytes: 16 * 1024,
        max_config_bytes: 128 * 1024,
    }
}
pub(crate) fn limits() -> OperationLimits {
    OperationLimits {
        jobs: 1,
        queue: 1,
        job_roots: 1,
        dns_roots: 1,
        dns_children: 2,
        http_drivers: 1,
        waiters: 2,
        waiter_bytes: 256,
        senders: 2,
        whole: Duration::from_secs(5),
        connect: Duration::from_secs(2),
        attempts: 2,
        input_bytes: 256,
        sockets: 1,
        max_headers: 16,
        max_header_bytes: 4096,
        max_http_buffer_bytes: 8192,
        max_request_bytes: 256,
        max_response_bytes: 4096,
        max_target_bytes: 1024,
    }
}
struct Input {
    echo_target: i64,
    bytes: Vec<u8>,
}
// Actual C need not be Clone or Debug; the handle physically retains its Arc.
struct State {
    preparations: AtomicUsize,
}
fn freeze(input: Input, limits: &InputLimits) -> Result<FrozenInput, client::Failure> {
    let mut bytes = input.echo_target.to_be_bytes().to_vec();
    bytes.extend(input.bytes);
    FrozenInput::checked(bytes, limits)
}
fn protected_prepare(
    state: &State,
    bound: BoundValues,
    input: FrozenInput,
    limits: &RequestLimits,
) -> Result<PreparedRequest, client::Failure> {
    state.preparations.fetch_add(1, Ordering::SeqCst);
    let bytes = input.into_bytes();
    let echo = bytes
        .get(..8)
        .and_then(|bytes| bytes.try_into().ok())
        .map(i64::from_be_bytes)
        .ok_or_else(client::Failure::invalid_input)?;
    if echo != bound.target() {
        return Err(client::Failure::invalid_input());
    }
    let subject = bound.subject().to_string();
    let target = bound.target().to_string();
    PreparedRequest::protected(
        bound,
        &["subjects", &subject, "targets", &target],
        &[],
        bytes[8..].to_vec(),
        limits,
    )
}
fn public_prepare(
    state: &State,
    input: FrozenInput,
    limits: &RequestLimits,
) -> Result<PreparedRequest, client::Failure> {
    state.preparations.fetch_add(1, Ordering::SeqCst);
    let bytes = input.into_bytes();
    PreparedRequest::public(
        &["items", "a/b"],
        &[("name", "x&y")],
        bytes[8..].to_vec(),
        limits,
    )
}
pub(crate) fn fixed_headers(catalog: &StartupCatalog) -> FixedRequestHeaders {
    FixedRequestHeaders::checked(
        catalog,
        Some(b"application/octet-stream".to_vec()),
        Some(b"application/octet-stream".to_vec()),
    )
    .unwrap()
}
pub(crate) struct Fixture {
    pub(crate) service: Arc<ServiceSpec>,
    pub(crate) credential: Option<ServiceCredential>,
    pub(crate) wire_ready: Arc<tokio::sync::Notify>,
    pub(crate) dns_calls: Arc<AtomicUsize>,
    pub(crate) http_calls: Arc<AtomicUsize>,
    dns_stop: oneshot::Sender<()>,
    http_stop: oneshot::Sender<()>,
    dns: JoinHandle<()>,
    server: JoinHandle<()>,
}
impl Fixture {
    pub(crate) async fn new(catalog: &StartupCatalog, protected: bool) -> Self {
        Self::with_hold(catalog, protected, None).await
    }
    pub(crate) async fn with_hold(
        catalog: &StartupCatalog,
        protected: bool,
        hold: Option<Arc<tokio::sync::Notify>>,
    ) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let dns = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let endpoint = dns.local_addr().unwrap();
        let key = rcgen::KeyPair::generate().unwrap();
        let mut params = rcgen::CertificateParams::new(vec!["api.example.test".into()]).unwrap();
        params.not_before = rcgen::date_time_ymd(2020, 1, 1);
        params.not_after = rcgen::date_time_ymd(2090, 1, 1);
        let cert = params.self_signed(&key).unwrap();
        let der = CertificateDer::from(cert.der().to_vec());
        let mut roots = rustls::RootCertStore::empty();
        roots.add(der.clone()).unwrap();
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![der],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der())),
        )
        .unwrap();
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let dns_calls = Arc::new(AtomicUsize::new(0));
        let counted_dns = Arc::clone(&dns_calls);
        let (dns_stop, mut stop_dns) = oneshot::channel();
        let dns_actor = tokio::spawn(async move {
            let mut bytes = [0u8; 4096];
            loop {
                let (n, peer) = tokio::select! {biased; _=&mut stop_dns=>break,result=dns.recv_from(&mut bytes)=>result.unwrap()};
                counted_dns.fetch_add(1, Ordering::SeqCst);
                let request = Message::from_vec(&bytes[..n]).unwrap();
                let query = request.queries[0].clone();
                let mut response = Message::response(request.metadata.id, request.metadata.op_code);
                response.add_query(query.clone());
                match query.query_type() {
                    RecordType::A => {
                        response.add_answer(Record::from_rdata(
                            query.name().clone(),
                            0,
                            RData::A(A(Ipv4Addr::LOCALHOST)),
                        ));
                    }
                    RecordType::AAAA => {}
                    _ => panic!("only finite A/AAAA fixture"),
                }
                dns.send_to(&response.to_vec().unwrap(), peer)
                    .await
                    .unwrap();
            }
        });
        let http_calls = Arc::new(AtomicUsize::new(0));
        let counted_http = Arc::clone(&http_calls);
        let (http_stop, mut stop_http) = oneshot::channel();
        let wire_ready = Arc::new(tokio::sync::Notify::new());
        let notify_wire = Arc::clone(&wire_ready);
        let server = tokio::spawn(async move {
            loop {
                let (socket, _) = tokio::select! {biased; _=&mut stop_http=>break,result=listener.accept()=>result.unwrap()};
                counted_http.fetch_add(1, Ordering::SeqCst);
                let mut tls = acceptor.accept(socket).await.unwrap();
                let mut wire = Vec::new();
                let mut byte = [0u8; 1];
                while !wire.ends_with(b"\r\n\r\n") {
                    tls.read_exact(&mut byte).await.unwrap();
                    wire.push(byte[0]);
                    assert!(wire.len() < 4096);
                }
                let headers = String::from_utf8(wire).unwrap();
                let lower = headers.to_ascii_lowercase();
                assert!(lower.contains(&format!("host: api.example.test:{port}\r\n")));
                assert!(lower.contains("accept-encoding: identity\r\n"));
                assert!(lower.contains("connection: close\r\n"));
                assert!(lower.contains("content-length: 3\r\n"));
                assert!(!lower.contains("cookie:"));
                assert!(!lower.contains("proxy-"));
                if protected {
                    assert!(headers.starts_with("POST /subjects/7/targets/91 HTTP/1.1\r\n"));
                    assert!(lower.contains("authorization: bearer owned-fixture-token\r\n"));
                } else {
                    assert!(headers.starts_with("POST /items/a%2Fb?name=x%26y HTTP/1.1\r\n"));
                    assert!(!lower.contains("authorization:"));
                }
                let mut body = [0u8; 3];
                tls.read_exact(&mut body).await.unwrap();
                assert_eq!(&body, b"dto");
                notify_wire.notify_one();
                if let Some(hold) = &hold {
                    hold.notified().await;
                }
                let response = if protected {
                    b"HTTP/1.1 403 Forbidden\r\n".as_slice()
                } else {
                    b"HTTP/1.1 302 Found\r\n".as_slice()
                };
                tls.write_all(response).await.unwrap();
                tls.write_all(b"Content-Length: 3\r\nX-Dup: a\r\nX-Dup: b\r\nX-Raw: \xff\r\nSet-Cookie: untrusted=data\r\nLocation: https://elsewhere.invalid/\r\nConnection: close\r\n\r\n\0\xffx").await.unwrap();
                tls.shutdown().await.unwrap();
            }
        });
        let service_key = ServiceKey::checked(1, 2, 3, 4).unwrap();
        let service = Arc::new(
            ServiceSpec::checked_for_test_network(
                catalog,
                ServiceConfig {
                    key: service_key,
                    canonical_dns_name: "api.example.test".into(),
                    port,
                    resolver_endpoint: endpoint,
                    roots,
                    max_candidates: 8,
                },
                vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            )
            .unwrap(),
        );
        let credential = if protected {
            Some(
                ServiceCredential::bearer(catalog, &service, 1, b"owned-fixture-token".to_vec())
                    .unwrap(),
            )
        } else {
            None
        };
        Self {
            service,
            credential,
            wire_ready,
            dns_calls,
            http_calls,
            dns_stop,
            http_stop,
            dns: dns_actor,
            server,
        }
    }
    pub(crate) async fn finish(self) {
        let _ = self.dns_stop.send(());
        let _ = self.http_stop.send(());
        self.dns.await.unwrap();
        self.server.await.unwrap();
    }
}
fn inspect_response(response: client::Response, status: i64) {
    assert_eq!(client::status(&response), status);
    let name = String::from("X-DUP");
    let values = client::headers(&response, &name).unwrap();
    drop(name);
    assert_eq!(values, [b"a".as_slice(), b"b".as_slice()]);
    assert_eq!(
        client::headers(&response, "x-raw").unwrap(),
        [b"\xff".as_slice()]
    );
    assert!(client::headers(&response, "bad\nname").is_err());
    let ptr = client::body(&response).as_ptr();
    let bytes = client::take_body(response);
    assert_eq!(bytes, [0, 255, b'x']);
    assert_eq!(ptr, bytes.as_ptr());
}

#[tokio::test]
async fn public_registered_operation_uses_verified_https_without_grant_or_redirect() {
    let mut catalog = StartupCatalog::new(1, retention_limits()).unwrap();
    let fixture = Fixture::new(&catalog, false).await;
    let state = Arc::new(State {
        preparations: AtomicUsize::new(0),
    });
    let spec = PublicSpec::checked(
        Arc::clone(&fixture.service),
        1,
        hyper::Method::POST,
        limits(),
        fixed_headers(&catalog),
    )
    .unwrap();
    let operation = register_public(
        &mut catalog,
        spec,
        Arc::clone(&state),
        freeze,
        public_prepare,
    )
    .unwrap();
    assert_eq!(format!("{operation:?}"), "PublicOperation");
    let refusal = client::execute_public(
        &operation,
        Input {
            echo_target: 0,
            bytes: b"dto".to_vec(),
        },
    )
    .await
    .err()
    .unwrap();
    assert_eq!(
        client::outcome(&refusal),
        client::Outcome::RequestNotStarted
    );
    assert_eq!(state.preparations.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.dns_calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.http_calls.load(Ordering::SeqCst), 0);
    let mut owner = catalog.freeze();
    let response = client::execute_public(
        &operation,
        Input {
            echo_target: 0,
            bytes: b"dto".to_vec(),
        },
    )
    .await
    .unwrap();
    inspect_response(response, 302);
    assert_eq!(state.preparations.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.dns_calls.load(Ordering::SeqCst), 2);
    assert_eq!(fixture.http_calls.load(Ordering::SeqCst), 1);
    owner.close();
    let closed = client::execute_public(
        &operation,
        Input {
            echo_target: 0,
            bytes: b"dto".to_vec(),
        },
    )
    .await
    .err()
    .unwrap();
    assert_eq!(client::kind(&closed), client::FailureKind::Closing);
    assert_eq!(client::outcome(&closed), client::Outcome::RequestNotStarted);
    assert_eq!(state.preparations.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.dns_calls.load(Ordering::SeqCst), 2);
    assert_eq!(fixture.http_calls.load(Ordering::SeqCst), 1);
    drop(operation);
    owner.shutdown().await;
    fixture.finish().await;
}

struct AppState {
    operation: client::ProtectedOperation<State, (), Input>,
}
async fn verify(
    _: http::Request,
    _: Arc<AppState>,
) -> Result<auth::VerifiedIdentity, auth::Failure> {
    auth::VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(5))
}
async fn authorize(
    scope: auth::AuthScope,
    _: http::Request,
    _: Arc<AppState>,
) -> Result<auth::Grant<()>, auth::Failure> {
    auth::Grant::from_authorized(scope, 91)
}
async fn handler(
    _: http::Request,
    state: Arc<AppState>,
    grant: auth::Grant<()>,
) -> Result<http::Response, crate::Error> {
    let response = client::execute(
        &state.operation,
        grant,
        Input {
            echo_target: 91,
            bytes: b"dto".to_vec(),
        },
    )
    .await
    .map_err(|_| crate::Error::internal("outbound operation failed"))?;
    inspect_response(response, 403);
    Ok(http::text(http::Status::OK, "bound"))
}
async fn conflicting_target(
    _: http::Request,
    state: Arc<AppState>,
    grant: auth::Grant<()>,
) -> Result<http::Response, crate::Error> {
    let failure = client::execute(
        &state.operation,
        grant,
        Input {
            echo_target: 92,
            bytes: b"dto".to_vec(),
        },
    )
    .await
    .err()
    .unwrap();
    assert_eq!(client::kind(&failure), client::FailureKind::InvalidInput);
    assert_eq!(
        client::outcome(&failure),
        client::Outcome::RequestNotStarted
    );
    Ok(http::text(http::Status::BAD_REQUEST, "refused"))
}
async fn incoming(endpoint: std::net::SocketAddr, wire: &[u8]) -> Vec<u8> {
    let mut stream = TcpStream::connect(endpoint).await.unwrap();
    stream.write_all(wire).await.unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut response))
        .await
        .unwrap()
        .unwrap();
    response
}
#[tokio::test]
async fn protected_registered_operation_preserves_actual_request_grant_target_and_startup_credential(
) {
    let mut catalog = StartupCatalog::new(1, retention_limits()).unwrap();
    let mut fixture = Fixture::new(&catalog, true).await;
    let state = Arc::new(State {
        preparations: AtomicUsize::new(0),
    });
    let spec = ProtectedSpec::checked(
        Arc::clone(&fixture.service),
        1,
        hyper::Method::POST,
        limits(),
        fixed_headers(&catalog),
        fixture.credential.take(),
    )
    .unwrap();
    let operation = register_protected(
        &mut catalog,
        spec,
        Arc::clone(&state),
        freeze,
        protected_prepare,
    )
    .unwrap();
    let mut owner = catalog.freeze();
    let app = http::app_default(AppState { operation });
    let app = http::route(
        app,
        http::Method::GET,
        "/",
        http::authorized_policy(verify, authorize),
        handler,
    )
    .unwrap();
    let app = http::route(
        app,
        http::Method::GET,
        "/conflict",
        http::authorized_policy(verify, authorize),
        conflicting_target,
    )
    .unwrap();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let endpoint = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel();
    let server = tokio::spawn(http::serve_listener(
        listener,
        app,
        http::authority(
            http::default_options(),
            "https://localhost",
            vec!["localhost".into()],
            1,
            128,
        )
        .unwrap(),
        async {
            let _ = stopped.await;
        },
    ));
    // Existing Auth rejects mixed bearer+Cookie before verifier/authorizer. Keep
    // that contract; outbound execution never receives raw incoming headers.
    let mixed=incoming(endpoint,b"GET / HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer incoming-fixture\r\nCookie: incoming=never-forwarded\r\nConnection: close\r\n\r\n").await;
    assert!(mixed.starts_with(b"HTTP/1.1 400"));
    assert_eq!(state.preparations.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.dns_calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.http_calls.load(Ordering::SeqCst), 0);
    let conflict=incoming(endpoint,b"GET /conflict HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer incoming-fixture\r\nConnection: close\r\n\r\n").await;
    assert!(conflict.starts_with(b"HTTP/1.1 400"));
    assert_eq!(state.preparations.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.dns_calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.http_calls.load(Ordering::SeqCst), 0);
    let response=incoming(endpoint,b"GET / HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer incoming-fixture\r\nConnection: close\r\n\r\n").await;
    assert!(response.starts_with(b"HTTP/1.1 200"));
    assert!(response.ends_with(b"bound"));
    assert_eq!(state.preparations.load(Ordering::SeqCst), 2);
    assert_eq!(fixture.dns_calls.load(Ordering::SeqCst), 2);
    assert_eq!(fixture.http_calls.load(Ordering::SeqCst), 1);
    let _ = stop.send(());
    server.await.unwrap().unwrap();
    owner.shutdown().await;
    fixture.finish().await;
}

#[tokio::test]
async fn startup_refuses_duplicate_operation_wrong_credential_binding_and_invalid_limits_before_native_work(
) {
    let mut catalog = StartupCatalog::new(2, retention_limits()).unwrap();
    // Exercise the production constructor, without the test-network exception.
    // Constructor acceptance is not production DNS/socket/TLS acceptance.
    for host in [
        "-api.example.test",
        "api-.example.test",
        "api..example.test",
        "127.1",
        "2130706433",
        "0x7f000001",
    ] {
        let failure = ServiceSpec::checked(
            &catalog,
            ServiceConfig {
                key: ServiceKey::checked(1, 2, 3, 4).unwrap(),
                canonical_dns_name: host.to_owned(),
                port: 443,
                resolver_endpoint: "127.0.0.1:53".parse().unwrap(),
                roots: rustls::RootCertStore::empty(),
                max_candidates: 8,
            },
        )
        .err()
        .expect("sf06_public_canonical_startup_reject");
        assert_eq!(client::kind(&failure), client::FailureKind::InvalidInput);
        assert_eq!(
            client::outcome(&failure),
            client::Outcome::RequestNotStarted
        );
    }
    assert!(ServiceSpec::checked(
        &catalog,
        ServiceConfig {
            key: ServiceKey::checked(1, 2, 3, 4).unwrap(),
            canonical_dns_name: "api.example.test".to_owned(),
            port: 443,
            resolver_endpoint: "127.0.0.1:53".parse().unwrap(),
            roots: rustls::RootCertStore::empty(),
            max_candidates: 8,
        }
    )
    .is_ok());
    let mut first = Fixture::new(&catalog, true).await;
    let second = Fixture::new(&catalog, false).await;
    let wrong = ProtectedSpec::<()>::checked(
        Arc::clone(&second.service),
        1,
        hyper::Method::POST,
        limits(),
        fixed_headers(&catalog),
        first.credential.take(),
    );
    assert_eq!(
        client::kind(&wrong.err().unwrap()),
        client::FailureKind::InvalidInput
    );
    let mut invalid = limits();
    invalid.whole = Duration::ZERO;
    assert_eq!(
        client::kind(
            &PublicSpec::checked(
                Arc::clone(&first.service),
                1,
                hyper::Method::POST,
                invalid,
                fixed_headers(&catalog)
            )
            .err()
            .unwrap()
        ),
        client::FailureKind::InvalidInput
    );
    let state = Arc::new(State {
        preparations: AtomicUsize::new(0),
    });
    let spec = PublicSpec::checked(
        Arc::clone(&first.service),
        1,
        hyper::Method::POST,
        limits(),
        fixed_headers(&catalog),
    )
    .unwrap();
    let operation = register_public(
        &mut catalog,
        spec,
        Arc::clone(&state),
        freeze,
        public_prepare,
    )
    .unwrap();
    let duplicate = PublicSpec::checked(
        Arc::clone(&first.service),
        1,
        hyper::Method::POST,
        limits(),
        fixed_headers(&catalog),
    )
    .unwrap();
    let failure = register_public(
        &mut catalog,
        duplicate,
        Arc::clone(&state),
        freeze,
        public_prepare,
    )
    .err()
    .unwrap();
    assert_eq!(client::kind(&failure), client::FailureKind::Capacity);
    assert_eq!(first.dns_calls.load(Ordering::SeqCst), 0);
    assert_eq!(first.http_calls.load(Ordering::SeqCst), 0);
    assert_eq!(second.dns_calls.load(Ordering::SeqCst), 0);
    assert_eq!(second.http_calls.load(Ordering::SeqCst), 0);
    let mut owner = catalog.freeze();
    drop(operation);
    owner.shutdown().await;
    first.finish().await;
    second.finish().await;
}

#[tokio::test]
async fn live_native_socket_quota_refusal_is_capacity_before_http_submission() {
    let mut catalog = StartupCatalog::new(1, retention_limits()).unwrap();
    let hold = Arc::new(tokio::sync::Notify::new());
    let fixture = Fixture::with_hold(&catalog, false, Some(Arc::clone(&hold))).await;
    let state = Arc::new(State {
        preparations: AtomicUsize::new(0),
    });
    let mut config = limits();
    config.jobs = 2;
    config.queue = 2;
    config.job_roots = 2;
    config.dns_roots = 2;
    config.dns_children = 4;
    config.http_drivers = 2;
    let spec = PublicSpec::checked(
        Arc::clone(&fixture.service),
        1,
        hyper::Method::POST,
        config,
        fixed_headers(&catalog),
    )
    .unwrap();
    let operation = register_public(
        &mut catalog,
        spec,
        Arc::clone(&state),
        freeze,
        public_prepare,
    )
    .unwrap();
    let mut owner = catalog.freeze();
    let mut first = Box::pin(client::execute_public(
        &operation,
        Input {
            echo_target: 0,
            bytes: b"dto".to_vec(),
        },
    ));
    tokio::select! {biased; _=fixture.wire_ready.notified()=>{},result=&mut first=>panic!("first request completed before server release: {:?}",result.err())}
    let refusal = client::execute_public(
        &operation,
        Input {
            echo_target: 0,
            bytes: b"dto".to_vec(),
        },
    )
    .await
    .err()
    .unwrap();
    let classified = client::kind(&refusal);
    let not_started = client::outcome(&refusal);
    hold.notify_one();
    let response = first.as_mut().await.unwrap();
    inspect_response(response, 302);
    drop(first);
    assert_eq!(state.preparations.load(Ordering::SeqCst), 2);
    assert_eq!(fixture.http_calls.load(Ordering::SeqCst), 1);
    drop(operation);
    owner.shutdown().await;
    fixture.finish().await;
    assert_eq!(
        classified,
        client::FailureKind::Capacity,
        "local quota failure was classified as a remote TCP failure"
    );
    assert_eq!(not_started, client::Outcome::RequestNotStarted);
}
