//! Explicit Linux owned-network acceptance, never an Internet probe.
//! The strict namespace gate executes these ignored cases individually. The
//! linked runtime is productioncfg: no test-network admission seam is available.
#![cfg(target_os = "linux")]

use hickory_resolver::proto::{
    op::Message,
    rr::{
        rdata::{A, AAAA},
        RData, Record, RecordType,
    },
};
use nagi_runtime::{
    auth,
    http_client::{self as client, startup::*},
    http_server as http,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::{
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
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

const OWNED_IP: Ipv4Addr = Ipv4Addr::new(11, 0, 0, 2);
const HOST: &str = "api.example.test";
const POSITIVE: &str = "production_owned_dns_tls_grant_and_shutdown";
const MIXED: &str = "production_owned_dns_rejects_mixed_unsafe_family";

// Existing explicit private fixture values; no defaults or recommendations.
fn retention_limits() -> RetentionLimits {
    RetentionLimits {
        max_root_anchors: 8,
        max_root_bytes: 16 * 1024,
        max_config_bytes: 128 * 1024,
    }
}
fn limits() -> OperationLimits {
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
#[derive(Clone, Copy)]
enum Profile {
    Ordinary,
    MixedUnsafe,
}
struct Input {
    target: i64,
    bytes: Vec<u8>,
}
struct PrepareState {
    calls: AtomicUsize,
}
fn freeze(input: Input, limits: &InputLimits) -> Result<FrozenInput, client::Failure> {
    let mut bytes = input.target.to_be_bytes().to_vec();
    bytes.extend(input.bytes);
    FrozenInput::checked(bytes, limits)
}
fn prepare(
    state: &PrepareState,
    bound: BoundValues,
    input: FrozenInput,
    limits: &RequestLimits,
) -> Result<PreparedRequest, client::Failure> {
    state.calls.fetch_add(1, Ordering::SeqCst);
    let bytes = input.into_bytes();
    let target = bytes
        .get(..8)
        .and_then(|n| n.try_into().ok())
        .map(i64::from_be_bytes)
        .ok_or_else(client::Failure::invalid_input)?;
    if target != bound.target() {
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

struct Network {
    service: Arc<ServiceSpec>,
    credential: Option<ServiceCredential>,
    dns_stop: oneshot::Sender<()>,
    tcp_stop: oneshot::Sender<()>,
    dns: JoinHandle<(usize, usize)>,
    tcp: JoinHandle<usize>,
}
impl Network {
    async fn start(catalog: &StartupCatalog, profile: Profile) -> Self {
        // Only the isolated namespace may own this ordinary unicast address.
        // No fallback to loopback, private IP, public service, or exception mode.
        let listener = TcpListener::bind((OWNED_IP, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let dns = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let endpoint = dns.local_addr().unwrap();

        // Same pinned rcgen/Rustls CA-and-leaf recipe as tls_http1_tests.rs.
        use rcgen::{
            BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
            KeyUsagePurpose, PKCS_ECDSA_P256_SHA256,
        };
        let ca_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
        let mut ca_params = CertificateParams::default();
        ca_params.not_before = rcgen::date_time_ymd(2020, 1, 1);
        ca_params.not_after = rcgen::date_time_ymd(2090, 1, 1);
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        ca_params.distinguished_name = rcgen::DistinguishedName::new();
        ca_params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "Nagi owned namespace test CA");
        let ca = ca_params.self_signed(&ca_key).unwrap();
        let issuer = Issuer::from_params(&ca_params, &ca_key);
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
        let mut leaf = CertificateParams::new(vec![HOST.to_owned()]).unwrap();
        leaf.not_before = rcgen::date_time_ymd(2020, 1, 1);
        leaf.not_after = rcgen::date_time_ymd(2090, 1, 1);
        leaf.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        leaf.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let leaf = leaf.signed_by(&key, &issuer).unwrap();
        let mut roots = rustls::RootCertStore::empty();
        roots.add(CertificateDer::from(ca.der().to_vec())).unwrap();
        let mut tls = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(leaf.der().to_vec())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der())),
        )
        .unwrap();
        tls.alpn_protocols = vec![b"http/1.1".to_vec()];
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls));

        let (dns_stop, mut stopped) = oneshot::channel();
        let dns_actor = tokio::spawn(async move {
            let mut counts = (0, 0);
            let mut bytes = [0; 4096];
            loop {
                let (n, peer) = tokio::select! { biased;
                    _ = &mut stopped => break,
                    result = dns.recv_from(&mut bytes) => result.unwrap(),
                };
                let request = Message::from_vec(&bytes[..n]).unwrap();
                assert_eq!(request.queries.len(), 1);
                let query = request.queries[0].clone();
                assert_eq!(query.name().to_ascii(), format!("{HOST}."));
                let mut response = Message::response(request.metadata.id, request.metadata.op_code);
                response.add_query(query.clone());
                match query.query_type() {
                    RecordType::A => {
                        counts.0 += 1;
                        response.add_answer(Record::from_rdata(
                            query.name().clone(),
                            0,
                            RData::A(A(OWNED_IP)),
                        ));
                    }
                    RecordType::AAAA => {
                        counts.1 += 1;
                        if matches!(profile, Profile::MixedUnsafe) {
                            response.add_answer(Record::from_rdata(
                                query.name().clone(),
                                0,
                                RData::AAAA(AAAA(Ipv6Addr::LOCALHOST)),
                            ));
                        }
                    }
                    _ => panic!("only A/AAAA are part of the owned fixture"),
                }
                dns.send_to(&response.to_vec().unwrap(), peer)
                    .await
                    .unwrap();
            }
            counts
        });
        let (tcp_stop, mut stopped) = oneshot::channel();
        let tcp_actor = tokio::spawn(async move {
            let mut accepts = 0;
            loop {
                // Drain queued accepts before stop so a forbidden dial cannot
                // be hidden by the final negative assertion/fixture shutdown.
                let (socket, _) = tokio::select! { biased;
                    result = listener.accept() => result.unwrap(),
                    _ = &mut stopped => break,
                };
                accepts += 1;
                assert_eq!(socket.local_addr().unwrap().ip(), OWNED_IP);
                if matches!(profile, Profile::MixedUnsafe) {
                    drop(socket);
                    continue;
                }
                tokio::time::timeout(limits().whole, async {
                    let mut socket = acceptor.accept(socket).await.unwrap();
                    assert_eq!(socket.get_ref().1.server_name(), Some(HOST));
                    assert_eq!(socket.get_ref().1.alpn_protocol(), Some(b"http/1.1".as_slice()));
                    let mut bytes = Vec::new();
                    let mut byte = [0; 1];
                    while !bytes.ends_with(b"\r\n\r\n") {
                        socket.read_exact(&mut byte).await.unwrap();
                        bytes.push(byte[0]);
                        assert!(bytes.len() < 4096);
                    }
                    let wire = String::from_utf8(bytes).unwrap();
                    let headers = wire.to_ascii_lowercase();
                    assert!(wire.starts_with("POST /subjects/7/targets/91 HTTP/1.1\r\n"));
                    assert!(headers.contains(&format!("host: {HOST}:{port}\r\n")));
                    assert!(headers.contains("authorization: bearer owned-fixture-token\r\n"));
                    assert!(headers.contains("content-length: 3\r\n"));
                    assert!(headers.contains("accept-encoding: identity\r\n"));
                    assert!(headers.contains("connection: close\r\n"));
                    assert!(!headers.contains("cookie:"));
                    assert!(!headers.contains("proxy-"));
                    let mut body = [0; 3];
                    socket.read_exact(&mut body).await.unwrap();
                    assert_eq!(&body, b"dto");
                    socket.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 3\r\nX-Dup: a\r\nX-Dup: b\r\nSet-Cookie: untrusted=data\r\nConnection: close\r\n\r\ndto").await.unwrap();
                    socket.shutdown().await.unwrap();
                }).await.unwrap();
            }
            accepts
        });
        let service = Arc::new(
            ServiceSpec::checked(
                catalog,
                ServiceConfig {
                    key: ServiceKey::checked(1, 2, 3, 4).unwrap(),
                    canonical_dns_name: HOST.to_owned(),
                    port,
                    resolver_endpoint: endpoint,
                    roots,
                    max_candidates: 8,
                },
            )
            .unwrap(),
        );
        let credential = Some(
            ServiceCredential::bearer(catalog, &service, 1, b"owned-fixture-token".to_vec())
                .unwrap(),
        );
        Self {
            service,
            credential,
            dns_stop,
            tcp_stop,
            dns: dns_actor,
            tcp: tcp_actor,
        }
    }
    async fn shutdown(self) -> (usize, usize, usize) {
        let _ = self.dns_stop.send(());
        let _ = self.tcp_stop.send(());
        let dns = self.dns.await;
        let tcp = self.tcp.await;
        let (a, aaaa) = dns.unwrap();
        let accepts = tcp.unwrap();
        (a, aaaa, accepts)
    }
}
struct AppState {
    operation: client::ProtectedOperation<PrepareState, (), Input>,
    profile: Profile,
}
async fn verify(
    _: http::Request,
    _: Arc<AppState>,
) -> Result<auth::VerifiedIdentity, auth::Failure> {
    auth::VerifiedIdentity::from_verified(7, Instant::now() + limits().whole)
}
async fn authorize(
    scope: auth::AuthScope,
    _: http::Request,
    _: Arc<AppState>,
) -> Result<auth::Grant<()>, auth::Failure> {
    auth::Grant::from_authorized(scope, 91)
}
async fn handler(
    request: http::Request,
    state: Arc<AppState>,
    grant: auth::Grant<()>,
) -> Result<http::Response, nagi_runtime::Error> {
    let result = client::execute(
        &state.operation,
        grant,
        Input {
            target: 91,
            bytes: b"dto".to_vec(),
        },
    )
    .await;
    match state.profile {
        Profile::Ordinary => {
            let response =
                result.map_err(|_| nagi_runtime::Error::internal("owned operation failed"))?;
            assert_eq!(client::status(&response), 403);
            assert_eq!(
                client::headers(&response, "x-dup").unwrap(),
                [b"a".as_slice(), b"b".as_slice()]
            );
            let pointer = client::body(&response).as_ptr();
            let body = client::take_body(response);
            assert_eq!(body, b"dto");
            assert_eq!(pointer, body.as_ptr());
        }
        Profile::MixedUnsafe => {
            let Err(failure) = result else {
                panic!("mixed unsafe family must refuse the whole snapshot");
            };
            assert_eq!(
                client::kind(&failure),
                client::FailureKind::DestinationDenied
            );
            assert_eq!(
                client::outcome(&failure),
                client::Outcome::RequestNotStarted
            );
        }
    }
    http::text(http::response_context(&request), http::Status::OK, "bound")
}
async fn conflict(
    request: http::Request,
    state: Arc<AppState>,
    grant: auth::Grant<()>,
) -> Result<http::Response, nagi_runtime::Error> {
    let Err(failure) = client::execute(
        &state.operation,
        grant,
        Input {
            target: 92,
            bytes: b"dto".to_vec(),
        },
    )
    .await
    else {
        panic!("conflicting target must be refused before DNS");
    };
    assert_eq!(client::kind(&failure), client::FailureKind::InvalidInput);
    assert_eq!(
        client::outcome(&failure),
        client::Outcome::RequestNotStarted
    );
    http::text(
        http::response_context(&request),
        http::Status::BAD_REQUEST,
        "refused",
    )
}
async fn incoming(endpoint: SocketAddr, wire: &[u8]) -> Vec<u8> {
    let mut socket = TcpStream::connect(endpoint).await.unwrap();
    socket.write_all(wire).await.unwrap();
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut chunk = [0; 1024];
        loop {
            let n = socket.read(&mut chunk).await.unwrap();
            if n == 0 {
                break;
            }
            assert!(bytes.len() + n <= 8192);
            bytes.extend_from_slice(&chunk[..n]);
        }
    })
    .await
    .unwrap();
    bytes
}
async fn run(profile: Profile, name: &'static str) {
    let begun = Instant::now();
    let mut catalog = StartupCatalog::new(1, retention_limits()).unwrap();
    let mut network = Network::start(&catalog, profile).await;
    let state = Arc::new(PrepareState {
        calls: AtomicUsize::new(0),
    });
    let headers = FixedRequestHeaders::checked(
        &catalog,
        Some(b"application/octet-stream".to_vec()),
        Some(b"application/octet-stream".to_vec()),
    )
    .unwrap();
    let spec = ProtectedSpec::checked(
        Arc::clone(&network.service),
        1,
        hyper::Method::POST,
        limits(),
        headers,
        network.credential.take(),
    )
    .unwrap();
    let operation =
        register_protected(&mut catalog, spec, Arc::clone(&state), freeze, prepare).unwrap();
    let mut owner = catalog.freeze();
    let app = http::app_default(AppState { operation, profile });
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
        conflict,
    )
    .unwrap();
    let app = http::rate_route(
        app,
        http::Method::GET,
        "/",
        http::rate_policy(10, 1000).unwrap(),
    )
    .unwrap();
    let app = http::rate_route(
        app,
        http::Method::GET,
        "/conflict",
        http::rate_policy(10, 1000).unwrap(),
    )
    .unwrap();
    let options = http::capacity(http::default_options(), 3, 3).unwrap();
    let options = http::with_response_limits(
        options,
        http::response_limits(1024, 128, 4, 512, 12, 128, 3, 393216, 3, 393216, 512, 12).unwrap(),
    )
    .unwrap();
    let options =
        http::with_rate_limits(options, http::rate_limits(4, 1, 2, 131072, 256).unwrap()).unwrap();
    let options = http::authority(
        options,
        "https://localhost",
        vec!["localhost".into()],
        1,
        128,
    )
    .unwrap();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let endpoint = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel();
    let server = tokio::spawn(http::serve_listener(listener, app, options, async {
        let _ = stopped.await;
    }));
    let mixed = incoming(endpoint, b"GET / HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer inbound\r\nCookie: incoming=never-forwarded\r\nConnection: close\r\n\r\n").await;
    assert!(mixed.starts_with(b"HTTP/1.1 400"));
    assert_eq!(state.calls.load(Ordering::SeqCst), 0);
    let refused = incoming(endpoint, b"GET /conflict HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer inbound\r\nConnection: close\r\n\r\n").await;
    assert!(refused.starts_with(b"HTTP/1.1 400"));
    assert_eq!(state.calls.load(Ordering::SeqCst), 1);
    let response = incoming(endpoint, b"GET / HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer inbound\r\nConnection: close\r\n\r\n").await;
    assert!(response.starts_with(b"HTTP/1.1 200"));
    assert!(response.ends_with(b"bound"));
    assert_eq!(state.calls.load(Ordering::SeqCst), 2);
    let _ = stop.send(());
    server.await.unwrap().unwrap();
    owner.close();
    owner.shutdown().await;
    let (a, aaaa, http_calls) = network.shutdown().await;
    assert_eq!((a, aaaa), (1, 1));
    assert_eq!(
        http_calls,
        if matches!(profile, Profile::Ordinary) {
            1
        } else {
            0
        }
    );
    // Only these three fixture JoinHandles are counted. Owner.shutdown above is
    // separately awaited; this is not an inferred count of internal child tasks.
    let configured = limits();
    let retained = retention_limits();
    eprintln!(
        "sf06_production_profile {}",
        serde_json::json!({
            "schema_version": 1, "case": name, "elapsed_us": begun.elapsed().as_micros(),
            "counts": {"dns_a": a, "dns_aaaa": aaaa, "http": http_calls, "joins": 3},
            "configured_limits": {
                "root_anchors": retained.max_root_anchors, "root_bytes": retained.max_root_bytes,
                "config_bytes": retained.max_config_bytes,
                "jobs": configured.jobs, "queue": configured.queue, "job_roots": configured.job_roots,
                "dns_roots": configured.dns_roots, "dns_children": configured.dns_children,
                "http_drivers": configured.http_drivers, "waiters": configured.waiters,
                "waiter_bytes": configured.waiter_bytes, "senders": configured.senders,
                "whole_ms": configured.whole.as_millis(), "connect_ms": configured.connect.as_millis(),
                "attempts": configured.attempts, "input_bytes": configured.input_bytes,
                "sockets": configured.sockets, "max_headers": configured.max_headers,
                "max_header_bytes": configured.max_header_bytes,
                "max_http_buffer_bytes": configured.max_http_buffer_bytes,
                "max_request_bytes": configured.max_request_bytes,
                "max_response_bytes": configured.max_response_bytes,
                "max_target_bytes": configured.max_target_bytes
            }
        })
    );
}

#[tokio::test]
#[ignore = "requires isolated owned network; strict Linux CI gate executes"]
async fn production_owned_dns_tls_grant_and_shutdown() {
    run(Profile::Ordinary, POSITIVE).await;
}

#[tokio::test]
#[ignore = "requires isolated owned network; strict Linux CI gate executes"]
async fn production_owned_dns_rejects_mixed_unsafe_family() {
    run(Profile::MixedUnsafe, MIXED).await;
}
