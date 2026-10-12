use super::super::super::{check_literal, InternalPolicy};
use super::super::{
    connect, connect_inner, DialControl, ServiceIdentity, SocketError, SocketQuota,
};
use super::*;
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
    ServerConfig,
};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use tokio_rustls::TlsAcceptor;

// Test-only credentials are generated inside this process. No key or certificate
// bytes are stored in the repository or emitted in test output.
struct TestCertificates {
    ca: Vec<u8>,
    valid: Vec<u8>,
    expired: Vec<u8>,
    server_key: Vec<u8>,
}

static TEST_CERTIFICATES: OnceLock<TestCertificates> = OnceLock::new();

fn test_certificates() -> &'static TestCertificates {
    TEST_CERTIFICATES.get_or_init(|| {
        use rcgen::{
            date_time_ymd, BasicConstraints, CertificateParams, DistinguishedName, DnType,
            ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair, KeyUsagePurpose,
            PKCS_ECDSA_P256_SHA256,
        };

        let ca_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
        let mut ca_params = CertificateParams::default();
        ca_params.not_before = date_time_ymd(2020, 1, 1);
        ca_params.not_after = date_time_ymd(2090, 1, 1);
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        ca_params.distinguished_name = DistinguishedName::new();
        ca_params
            .distinguished_name
            .push(DnType::CommonName, "Nagi owned loopback test CA");
        let ca = ca_params.self_signed(&ca_key).unwrap();
        let issuer = Issuer::from_params(&ca_params, &ca_key);

        let server_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
        let mut leaf = CertificateParams::new(vec!["api.example.test".to_owned()]).unwrap();
        leaf.not_before = date_time_ymd(2020, 1, 1);
        leaf.not_after = date_time_ymd(2090, 1, 1);
        leaf.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        leaf.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let valid = leaf.signed_by(&server_key, &issuer).unwrap();
        leaf.not_after = date_time_ymd(2020, 1, 2);
        let expired = leaf.signed_by(&server_key, &issuer).unwrap();

        TestCertificates {
            ca: ca.der().to_vec(),
            valid: valid.der().to_vec(),
            expired: expired.der().to_vec(),
            server_key: server_key.serialize_der(),
        }
    })
}

#[derive(Default)]
struct Observed {
    application_bytes: AtomicUsize,
    sni: Mutex<Option<String>>,
    alpn: Mutex<Option<Vec<u8>>>,
    host: Mutex<Option<String>>,
}

fn identity() -> ServiceIdentity {
    ServiceIdentity {
        service: 71,
        purpose: 72,
        generation: 73,
        trust: 74,
    }
}

fn limits() -> TransportLimits {
    TransportLimits::checked(16, 4096, 8192, 1024, 4096, 1024).unwrap()
}

fn roots(trusted: bool) -> rustls::RootCertStore {
    let mut store = rustls::RootCertStore::empty();
    if trusted {
        store
            .add(CertificateDer::from(test_certificates().ca.clone()))
            .unwrap();
    }
    store
}

fn profile(
    port: u16,
    name: &str,
    trusted: bool,
    whole_end: Instant,
    tls_end: Instant,
) -> TlsProfile {
    TlsProfile::checked(
        identity(),
        url::Host::Domain(name.to_owned()),
        port,
        roots(trusted),
        whole_end,
        tls_end,
        limits(),
    )
    .unwrap()
}

fn snapshot(port: u16, end: Instant) -> super::super::CheckedSnapshot {
    // Test-only exact 127.0.0.1 exception belongs to this owned listener.
    let policy =
        InternalPolicy::checked(identity(), port, 1, vec![IpAddr::V4(Ipv4Addr::LOCALHOST)])
            .unwrap();
    check_literal(
        &policy,
        identity(),
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        end,
        end,
        Instant::now(),
    )
    .unwrap()
}

async fn tls_server(cert: &'static [u8]) -> (SocketAddr, Arc<Observed>, JoinHandle<()>) {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let observation = Arc::new(Observed::default());
    let seen = Arc::clone(&observation);
    let handle = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let mut config = ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(cert.to_vec())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
                    test_certificates().server_key.clone(),
                )),
            )
            .unwrap();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let Ok(mut tls) = acceptor.accept(tcp).await else {
            return;
        };
        *seen.sni.lock().unwrap() = tls.get_ref().1.server_name().map(str::to_owned);
        *seen.alpn.lock().unwrap() = tls.get_ref().1.alpn_protocol().map(|p| p.to_vec());
        let mut request = Vec::new();
        let mut buf = [0_u8; 256];
        while request.len() < 2048 && !request.windows(4).any(|v| v == b"\r\n\r\n") {
            let count = match tokio::time::timeout(Duration::from_secs(3), tls.read(&mut buf)).await
            {
                Ok(Ok(count)) => count,
                Ok(Err(_)) | Err(_) => return,
            };
            if count == 0 {
                return;
            }
            seen.application_bytes.fetch_add(count, Ordering::SeqCst);
            request.extend_from_slice(&buf[..count]);
        }
        assert!(
            request.windows(4).any(|v| v == b"\r\n\r\n"),
            "bounded HTTP request missing terminator"
        );
        let text = String::from_utf8_lossy(&request).to_ascii_lowercase();
        *seen.host.lock().unwrap() = text
            .lines()
            .find_map(|line| line.strip_prefix("host: "))
            .map(str::to_owned);
        tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK")
            .await
            .unwrap();
        let _ = tls.shutdown().await;
    });
    (address, observation, handle)
}

async fn join_server(handle: JoinHandle<()>) {
    tokio::time::timeout(Duration::from_secs(4), handle)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn trusted_ca_original_name_and_host_http1_then_driver_actual_join() {
    let (address, seen, server) = tls_server(&test_certificates().valid).await;
    let control = Arc::new(DialControl::new());
    let quota = SocketQuota::checked(1).unwrap();
    let end = Instant::now() + Duration::from_secs(4);
    let pinned = connect(snapshot(address.port(), end), end, &control, &quota)
        .await
        .unwrap();
    let profile = profile(address.port(), "api.example.test", true, end, end);
    let Http1Parts { client, driver } = establish(pinned, &profile, Arc::clone(&control))
        .await
        .unwrap();
    assert_eq!(
        quota.used(),
        1,
        "driver owns the pinned socket after handshake"
    );
    // Test-only owner. Production driver creation must go through the parent JobLedger gate.
    let driver_owner = tokio::spawn(async move { driver.await });
    let request = WireRequest::checked(
        hyper::Method::GET,
        "/probe".parse().unwrap(),
        Vec::new(),
        &limits(),
    )
    .unwrap();
    let result = client.send_once(request).await.unwrap();
    assert_eq!(result.status, hyper::StatusCode::OK);
    assert_eq!(result.body.as_slice(), b"OK");
    assert!(tokio::time::timeout(Duration::from_secs(4), driver_owner)
        .await
        .unwrap()
        .unwrap()
        .is_ok());
    assert_eq!(quota.used(), 0, "quota releases after actual driver join");
    join_server(server).await;
    assert_eq!(
        seen.sni.lock().unwrap().as_deref(),
        Some("api.example.test")
    );
    assert_eq!(
        seen.alpn.lock().unwrap().as_deref(),
        Some(b"http/1.1".as_slice())
    );
    let expected_host = format!("api.example.test:{}", address.port());
    assert_eq!(
        seen.host.lock().unwrap().as_deref(),
        Some(expected_host.as_str())
    );
    assert!(seen.application_bytes.load(Ordering::SeqCst) > 0);
}

async fn rejected_certificate_has_zero_http_bytes(cert: &'static [u8], name: &str, trusted: bool) {
    let (address, seen, server) = tls_server(cert).await;
    let control = Arc::new(DialControl::new());
    let quota = SocketQuota::checked(1).unwrap();
    let end = Instant::now() + Duration::from_secs(4);
    let pinned = connect(snapshot(address.port(), end), end, &control, &quota)
        .await
        .unwrap();
    let profile = profile(address.port(), name, trusted, end, end);
    assert!(matches!(
        establish(pinned, &profile, control).await,
        Err(TransportError::Tls)
    ));
    assert_eq!(quota.used(), 0);
    join_server(server).await;
    assert_eq!(seen.application_bytes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn wrong_hostname_is_rejected_before_http_application_bytes() {
    rejected_certificate_has_zero_http_bytes(
        &test_certificates().valid,
        "wrong.example.test",
        true,
    )
    .await;
}

#[tokio::test]
async fn untrusted_ca_is_rejected_before_http_application_bytes() {
    rejected_certificate_has_zero_http_bytes(&test_certificates().valid, "api.example.test", false)
        .await;
}

#[tokio::test]
async fn expired_certificate_is_rejected_before_http_application_bytes() {
    rejected_certificate_has_zero_http_bytes(
        &test_certificates().expired,
        "api.example.test",
        true,
    )
    .await;
}

#[tokio::test]
async fn wrong_numeric_peer_never_enters_tls() {
    let expected = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let other = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let other_address = other.local_addr().unwrap();
    let control = Arc::new(DialControl::new());
    let quota = SocketQuota::checked(1).unwrap();
    let end = Instant::now() + Duration::from_secs(4);
    let result = connect_inner(
        snapshot(expected.local_addr().unwrap().port(), end),
        end,
        &control,
        &quota,
        |_| TcpStream::connect(other_address),
    )
    .await;
    let (mut peer, _) = other.accept().await.unwrap();
    assert!(matches!(result, Err(SocketError::PeerMismatch)));
    let mut byte = [0_u8; 1];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), peer.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    assert_eq!(quota.used(), 0);
}

async fn raw_pending_server(listener: TcpListener, started: tokio::sync::oneshot::Sender<()>) {
    let (mut peer, _) = listener.accept().await.unwrap();
    let mut buf = [0_u8; 256];
    let first = tokio::time::timeout(Duration::from_secs(4), peer.read(&mut buf))
        .await
        .unwrap()
        .unwrap();
    assert!(
        first > 0,
        "TLS ClientHello must reach owned server before phase barrier"
    );
    started.send(()).unwrap();
    loop {
        let n = tokio::time::timeout(Duration::from_secs(4), peer.read(&mut buf))
            .await
            .unwrap()
            .unwrap();
        if n == 0 {
            break;
        }
        // TLS record bytes are allowed; this server never completes TLS or observes HTTP.
    }
}

#[tokio::test]
async fn tls_phase_deadline_drops_owned_socket_without_http() {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(raw_pending_server(listener, accepted_tx));
    let control = Arc::new(DialControl::new());
    let quota = SocketQuota::checked(1).unwrap();
    let end = Instant::now() + Duration::from_secs(4);
    let pinned = connect(snapshot(port, end), end, &control, &quota)
        .await
        .unwrap();
    let profile = profile(
        port,
        "api.example.test",
        true,
        end,
        Instant::now() + Duration::from_secs(1),
    );
    let (result, started) = tokio::join!(
        establish(pinned, &profile, control),
        tokio::time::timeout(Duration::from_secs(4), accepted_rx),
    );
    started.unwrap().unwrap();
    assert!(matches!(result, Err(TransportError::Deadline)));
    assert_eq!(quota.used(), 0);
    join_server(server).await;
}

#[tokio::test]
async fn close_during_tls_aborts_without_detached_socket() {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(raw_pending_server(listener, accepted_tx));
    let control = Arc::new(DialControl::new());
    let quota = SocketQuota::checked(1).unwrap();
    let end = Instant::now() + Duration::from_secs(4);
    let pinned = connect(snapshot(port, end), end, &control, &quota)
        .await
        .unwrap();
    let profile = profile(port, "api.example.test", true, end, end);
    let mut client = Box::pin(establish(pinned, &profile, Arc::clone(&control)));
    tokio::select! {
        started = tokio::time::timeout(Duration::from_secs(4), accepted_rx) => started.unwrap().unwrap(),
        _ = &mut client => panic!("TLS attempt finished before owned ClientHello barrier"),
    }
    control.close();
    assert!(matches!(
        client.await,
        Err(TransportError::Socket(SocketError::Closing))
    ));
    assert_eq!(quota.used(), 0);
    join_server(server).await;
}

#[test]
fn profile_limits_reject_unsafe_hyper_buffer_and_absolute_uri() {
    assert_eq!(
        TransportLimits::checked(16, 4096, 8191, 1024, 4096, 1024),
        Err(TransportError::InvalidProfile)
    );
    let request = WireRequest::checked(
        hyper::Method::GET,
        "https://other.example.test/path".parse().unwrap(),
        Vec::new(),
        &limits(),
    );
    assert!(matches!(request, Err(TransportError::InvalidProfile)));
}

#[tokio::test]
async fn ip_origin_requires_ip_san_and_never_guesses_dns_name() {
    let (address, seen, server) = tls_server(&test_certificates().valid).await;
    let control = Arc::new(DialControl::new());
    let quota = SocketQuota::checked(1).unwrap();
    let end = Instant::now() + Duration::from_secs(4);
    let pinned = connect(snapshot(address.port(), end), end, &control, &quota)
        .await
        .unwrap();
    let profile = TlsProfile::checked(
        identity(),
        url::Host::Ipv4(Ipv4Addr::LOCALHOST),
        address.port(),
        roots(true),
        end,
        end,
        limits(),
    )
    .unwrap();
    assert!(matches!(
        establish(pinned, &profile, control).await,
        Err(TransportError::Tls)
    ));
    assert_eq!(quota.used(), 0);
    join_server(server).await;
    assert_eq!(seen.application_bytes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn close_after_preparation_is_checked_inside_the_lazy_send_before_dispatch() {
    let (client_io, _server_io) = tokio::io::duplex(4096);
    let (mut sender, driver) = http1::handshake::<_, RequestBody>(TokioIo::new(client_io))
        .await
        .unwrap();
    let control = DialControl::new();
    let end = Instant::now() + Duration::from_secs(2);
    phase_gate(&control, end).unwrap();
    let request = WireRequest::checked(
        hyper::Method::GET,
        "/probe".parse().unwrap(),
        Vec::new(),
        &limits(),
    )
    .unwrap()
    .into_hyper(hyper::header::HeaderValue::from_static("api.example.test"))
    .unwrap();
    control.close();
    assert!(matches!(
        send_request_gated(&mut sender, request, &control, end).await,
        Err(TransportError::Socket(SocketError::Closing))
    ));
    drop(driver);
}

#[test]
fn call_time_enqueue_seam_never_invokes_side_effect_after_close_or_deadline() {
    let called = AtomicUsize::new(0);
    let control = DialControl::new();
    let end = Instant::now() + Duration::from_secs(2);
    phase_gate(&control, end).unwrap();
    control.close();
    assert!(matches!(
        enqueue_under_gate(&control, end, || called.fetch_add(1, Ordering::SeqCst)),
        Err(TransportError::Socket(SocketError::Closing))
    ));
    assert_eq!(called.load(Ordering::SeqCst), 0);

    let open = DialControl::new();
    assert_eq!(
        enqueue_under_gate(&open, Instant::now(), || called
            .fetch_add(1, Ordering::SeqCst)),
        Err(TransportError::Deadline)
    );
    assert_eq!(called.load(Ordering::SeqCst), 0);
    assert_eq!(
        enqueue_under_gate(&open, end, || called.fetch_add(1, Ordering::SeqCst)),
        Ok(0)
    );
    assert_eq!(called.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn exact_hyper_dispatches_at_send_request_call_before_future_poll() {
    let (client_io, mut server_io) = tokio::io::duplex(4096);
    let (mut sender, driver) = http1::handshake::<_, RequestBody>(TokioIo::new(client_io))
        .await
        .unwrap();
    let driver_owner = tokio::spawn(async move { driver.await });
    let request = WireRequest::checked(
        hyper::Method::GET,
        "/probe".parse().unwrap(),
        Vec::new(),
        &limits(),
    )
    .unwrap()
    .into_hyper(hyper::header::HeaderValue::from_static("api.example.test"))
    .unwrap();
    let response_future = sender.send_request(request);
    // Deliberately do not poll response_future yet. A read here demonstrates
    // that this exact Hyper version enqueues synchronously at method call.
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 256];
    while !bytes.windows(2).any(|part| part == b"\r\n") {
        let count = tokio::time::timeout(Duration::from_secs(2), server_io.read(&mut chunk))
            .await
            .unwrap()
            .unwrap();
        assert!(count > 0 && bytes.len() + count <= 512);
        bytes.extend_from_slice(&chunk[..count]);
    }
    assert!(bytes.starts_with(b"GET /probe HTTP/1.1\r\n"));
    server_io
        .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let response = tokio::time::timeout(Duration::from_secs(2), response_future)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(response.status(), hyper::StatusCode::NO_CONTENT);
    drop(response);
    drop(sender);
    drop(server_io);
    assert!(tokio::time::timeout(Duration::from_secs(2), driver_owner)
        .await
        .unwrap()
        .unwrap()
        .is_ok());
}
