use super::*;
use crate::auth;
use hickory_resolver::proto::{
    op::Message,
    rr::{rdata::A, RData, Record, RecordType},
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::net::{IpAddr, Ipv4Addr};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, UdpSocket},
};

fn limits() -> Limits {
    Limits {
        jobs: 1,
        queue: 1,
        job_roots: 1,
        dns_roots: 1,
        dns_children: 2,
        http_drivers: 1,
        waiters: 2,
        waiter_bytes: 256,
        senders: 2,
    }
}
fn freeze(value: Vec<u8>, limit: &InputLimits) -> Result<FrozenInput, Failure> {
    if value.len() > limit.max_bytes() {
        return Err(Failure::new(FailureKind::InvalidInput, false));
    }
    Ok(FrozenInput(value))
}
fn prepare(
    state: &(),
    bound: BoundValues,
    input: FrozenInput,
    limits: &RequestLimits,
) -> Result<PreparedRequest, Failure> {
    let _ = state;
    // Actual subject/target are the wire predicate; the input cannot replace them.
    let subject = bound.subject().to_string();
    let target = bound.target().to_string();
    PreparedRequest::protected(
        bound,
        &["subjects", &subject, "targets", &target],
        &[],
        input.into_bytes(),
        limits,
    )
}
fn grant(end: Instant) -> (auth::LeaseOwner, auth::Grant<()>) {
    let owner = auth::LeaseOwner::new(end).unwrap();
    let identity = auth::VerifiedIdentity::from_verified(7, end).unwrap();
    let scope = auth::AuthScope::bind(identity, Arc::clone(&owner.lease)).unwrap();
    (owner, auth::Grant::from_authorized(scope, 91).unwrap())
}

#[derive(Clone, Copy)]
enum Scenario {
    Completed,
    UnreceivedResult,
    CallerCancelled,
    OwnerClosed,
    Deadline,
    UnsupportedEncoding,
}

async fn https_case(scenario: Scenario) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let dns = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let endpoint = dns.local_addr().unwrap();
    let dns_actor = tokio::spawn(async move {
        let mut seen = Vec::new();
        let mut buffer = [0u8; 4096];
        for _ in 0..2 {
            let (n, peer) = dns.recv_from(&mut buffer).await.unwrap();
            let request = Message::from_vec(&buffer[..n]).unwrap();
            let query = request.queries[0].clone();
            seen.push(query.query_type());
            let mut reply = Message::response(request.metadata.id, request.metadata.op_code);
            reply.add_query(query.clone());
            if query.query_type() == RecordType::A {
                reply.add_answer(Record::from_rdata(
                    query.name().clone(),
                    0,
                    RData::A(A(Ipv4Addr::LOCALHOST)),
                ));
            }
            dns.send_to(&reply.to_vec().unwrap(), peer).await.unwrap();
        }
        assert!(seen.contains(&RecordType::A) && seen.contains(&RecordType::AAAA));
    });
    // Native runtime generated keys only; ordinary rustls verification remains on.
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
    let (wire_tx, wire_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut tls = acceptor.accept(socket).await.unwrap();
        let mut wire = Vec::new();
        let mut byte = [0u8; 1];
        while !wire.ends_with(b"\r\n\r\n") {
            tls.read_exact(&mut byte).await.unwrap();
            wire.push(byte[0]);
            assert!(wire.len() < 4096);
        }
        let wire = String::from_utf8(wire).unwrap();
        assert!(wire.starts_with("POST /subjects/7/targets/91 HTTP/1.1\r\n"));
        assert!(wire
            .to_ascii_lowercase()
            .contains("accept-encoding: identity\r\n"));
        assert!(!wire.to_ascii_lowercase().contains("cookie:"));
        let mut body = [0u8; 3];
        tls.read_exact(&mut body).await.unwrap();
        assert_eq!(&body, b"dto");
        let response = if matches!(scenario, Scenario::UnsupportedEncoding) {
            b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nContent-Encoding: gzip\r\nConnection: close\r\n\r\nraw".as_slice()
        } else {
            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 3\r\nX-Dup: a\r\nX-Dup: b\r\nConnection: close\r\n\r\nraw".as_slice()
        };
        tls.write_all(response).await.unwrap();
        tls.flush().await.unwrap();
        let _ = wire_tx.send(());
        tls.shutdown().await.unwrap();
    });
    let hold = Arc::new(Notify::new());
    let identity = ServiceIdentity::from_startup(1, 2, 3, 4).unwrap();
    let catalog =
        startup::StartupCatalog::new(1, crate::http_client::startup_tests::retention_limits())
            .unwrap();
    let trust = startup::FrozenTrust::for_test(
        &catalog,
        identity,
        url::Host::Domain("api.example.test".into()),
        port,
        roots,
    );
    let profile = NativeProfile {
        identity,
        policy: Arc::new(
            InternalPolicy::checked(identity, port, 8, vec![IpAddr::V4(Ipv4Addr::LOCALHOST)])
                .unwrap(),
        ),
        resolver: Arc::new(ResolverStartup::checked("api.example.test", endpoint).unwrap()),
        transport: TransportLimits::checked(16, 4096, 8192, 256, 4096, 1024).unwrap(),
        quota: SocketQuota::checked(1).unwrap(),
        whole: if matches!(scenario, Scenario::Deadline) {
            Duration::from_millis(200)
        } else {
            Duration::from_secs(5)
        },
        connect: Duration::from_secs(2),
        attempts: 2,
        input_bytes: 256,
        method: Method::POST,
        driver_hold: if matches!(scenario, Scenario::UnsupportedEncoding) {
            None
        } else {
            Some(Arc::clone(&hold))
        },
        trust,
        retained: catalog.test_retained(),
    };
    let mut parent = ParentDrain::new(1).unwrap();
    let operation = register_private_protected(
        &mut parent,
        limits(),
        profile,
        Arc::new(()),
        freeze,
        prepare,
    )
    .unwrap();
    let (_lease, proof) = grant(Instant::now() + Duration::from_secs(10));
    let mut execution = Box::pin(execute(&operation, proof, b"dto".to_vec()));
    tokio::select! { biased; result = wire_rx => result.unwrap(), result = &mut execution => panic!("unexpected pre-wire completion: {:?}", result.err()) }
    if matches!(scenario, Scenario::UnreceivedResult) {
        hold.notify_one();
        // Leave execute unpolled: only its retained native actors/monitor run.
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let joined = {
                    let owner = operation.core.runner.owner.lock().unwrap();
                    [
                        TaskKind::JobRoot,
                        TaskKind::DnsRoot,
                        TaskKind::DnsChild,
                        TaskKind::HttpDriver,
                    ]
                    .into_iter()
                    .all(|kind| owner.used_tasks(kind) == 0)
                };
                if joined {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(operation.core.profile.quota.used(), 0);
        let (_next_lease, next_proof) = grant(Instant::now() + Duration::from_secs(10));
        let mut next = Box::pin(operation.core.sender.submit(
            next_proof,
            vec![1],
            Instant::now() + Duration::from_secs(5),
        ));
        let blocked = tokio::time::timeout(Duration::from_millis(50), &mut next)
            .await
            .is_err();
        drop(next);
        let response = execution.as_mut().await.unwrap();
        assert_eq!(status(&response), 503);
        assert_eq!(body(&response), b"raw");
        let (_after_lease, after_proof) = grant(Instant::now() + Duration::from_secs(10));
        assert!(tokio::time::timeout(
            Duration::from_secs(1),
            operation.core.sender.submit(
                after_proof,
                vec![1],
                Instant::now() + Duration::from_secs(5)
            )
        )
        .await
        .unwrap()
        .is_ok());
        drop(execution);
        drop(response);
        drop(operation);
        parent.shutdown().await;
        dns_actor.await.unwrap();
        server.await.unwrap();
        assert!(
            blocked,
            "unreceived response released its retained core budget before handoff"
        );
        return;
    }
    if matches!(
        scenario,
        Scenario::CallerCancelled
            | Scenario::OwnerClosed
            | Scenario::Deadline
            | Scenario::UnsupportedEncoding
    ) {
        if matches!(scenario, Scenario::CallerCancelled) {
            drop(execution);
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if operation.core.runner.owner.lock().unwrap().jobs.is_empty() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        } else {
            if matches!(scenario, Scenario::OwnerClosed) {
                parent.request_close(parent.records[0].id).unwrap();
            }
            let failure = match tokio::time::timeout(Duration::from_secs(2), &mut execution)
                .await
                .unwrap()
            {
                Ok(_) => panic!(
                    "operation published a response despite cancellation/deadline/encoding refusal"
                ),
                Err(failure) => failure,
            };
            let expected = match scenario {
                Scenario::OwnerClosed => FailureKind::Closing,
                Scenario::Deadline => FailureKind::Deadline,
                Scenario::UnsupportedEncoding => FailureKind::UnsupportedEncoding,
                _ => unreachable!(),
            };
            assert_eq!(kind(&failure), expected);
            assert_eq!(outcome(&failure), Outcome::Unknown);
            drop(execution);
        }
        assert_eq!(operation.core.profile.quota.used(), 0);
        {
            let owner = operation.core.runner.owner.lock().unwrap();
            assert!(owner.jobs.is_empty());
            assert_eq!(owner.used_tasks(TaskKind::JobRoot), 0);
            assert_eq!(owner.used_tasks(TaskKind::DnsRoot), 0);
            assert_eq!(owner.used_tasks(TaskKind::DnsChild), 0);
            assert_eq!(owner.used_tasks(TaskKind::HttpDriver), 0);
        }
        drop(operation);
        parent.shutdown().await;
        dns_actor.await.unwrap();
        server.await.unwrap();
        return;
    }
    let first = tokio::time::timeout(Duration::from_millis(50), &mut execution).await;
    let withheld = first.is_err();
    hold.notify_one();
    let response = match first {
        Ok(result) => result.unwrap(),
        Err(_) => execution.as_mut().await.unwrap(),
    };
    assert_eq!(status(&response), 503);
    let temporary_name = String::from("x-DUP");
    let values = headers(&response, &temporary_name).unwrap();
    drop(temporary_name);
    assert_eq!(values, [b"a".as_slice(), b"b".as_slice()]);
    let ptr = body(&response).as_ptr();
    let moved = take_body(response);
    assert_eq!(moved, b"raw");
    assert_eq!(ptr, moved.as_ptr());
    drop(execution);
    drop(operation);
    parent.shutdown().await;
    dns_actor.await.unwrap();
    server.await.unwrap();
    assert!(
        withheld,
        "provisional response escaped before actual driver join"
    );
}

#[tokio::test]
async fn operation_withholds_real_https_response_until_actual_driver_join() {
    https_case(Scenario::Completed).await;
}
#[tokio::test]
async fn unreceived_response_keeps_core_budget_after_actual_native_joins() {
    https_case(Scenario::UnreceivedResult).await;
}
#[tokio::test]
async fn dropped_execute_retains_cleanup_until_actual_native_joins() {
    https_case(Scenario::CallerCancelled).await;
}
#[tokio::test]
async fn owner_close_refuses_publish_after_actual_native_joins() {
    https_case(Scenario::OwnerClosed).await;
}
#[tokio::test]
async fn original_deadline_survives_provisional_response_and_join_wait() {
    https_case(Scenario::Deadline).await;
}
#[tokio::test]
async fn unsupported_encoding_is_unknown_and_never_replayed() {
    https_case(Scenario::UnsupportedEncoding).await;
}

fn idle_profile(endpoint: std::net::SocketAddr) -> NativeProfile {
    let identity = ServiceIdentity::from_startup(1, 2, 3, 4).unwrap();
    let catalog =
        startup::StartupCatalog::new(1, crate::http_client::startup_tests::retention_limits())
            .unwrap();
    let trust = startup::FrozenTrust::for_test(
        &catalog,
        identity,
        url::Host::Domain("api.example.test".into()),
        443,
        rustls::RootCertStore::empty(),
    );
    NativeProfile {
        identity,
        policy: Arc::new(
            InternalPolicy::checked(identity, 443, 8, vec![IpAddr::V4(Ipv4Addr::LOCALHOST)])
                .unwrap(),
        ),
        resolver: Arc::new(ResolverStartup::checked("api.example.test", endpoint).unwrap()),
        transport: TransportLimits::checked(16, 4096, 8192, 256, 4096, 1024).unwrap(),
        quota: SocketQuota::checked(1).unwrap(),
        whole: Duration::from_secs(5),
        connect: Duration::from_secs(2),
        attempts: 2,
        input_bytes: 256,
        method: Method::POST,
        driver_hold: None,
        trust,
        retained: catalog.test_retained(),
    }
}

#[tokio::test]
async fn unpolled_execute_starts_no_admission_or_native_resolution() {
    let dns = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let mut parent = ParentDrain::new(1).unwrap();
    let operation = register_private_protected(
        &mut parent,
        limits(),
        idle_profile(dns.local_addr().unwrap()),
        Arc::new(()),
        freeze,
        prepare,
    )
    .unwrap();
    let (_lease, proof) = grant(Instant::now() + Duration::from_secs(10));
    drop(execute(&operation, proof, b"dto".to_vec()));
    assert!(operation.core.runner.owner.lock().unwrap().jobs.is_empty());
    assert_eq!(
        operation
            .core
            .sender
            .shared
            .waiters
            .lock()
            .unwrap()
            .records
            .len(),
        0
    );
    assert_eq!(operation.core.profile.quota.used(), 0);
    let mut bytes = [0u8; 4096];
    assert!(
        tokio::time::timeout(Duration::from_millis(20), dns.recv_from(&mut bytes))
            .await
            .is_err()
    );
    drop(operation);
    parent.shutdown().await;
}

#[tokio::test]
async fn adapter_panic_remains_fault_after_actual_root_join() {
    use futures_util::FutureExt;
    fn panic_prepare(
        _: &(),
        _: BoundValues,
        _: FrozenInput,
        _: &RequestLimits,
    ) -> Result<PreparedRequest, Failure> {
        panic!("owned adapter panic fixture");
    }
    let dns = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let mut parent = ParentDrain::new(1).unwrap();
    let operation = register_private_protected(
        &mut parent,
        limits(),
        idle_profile(dns.local_addr().unwrap()),
        Arc::new(()),
        freeze,
        panic_prepare,
    )
    .unwrap();
    let retained = Arc::clone(&operation.core.runner.owner);
    let (_lease, proof) = grant(Instant::now() + Duration::from_secs(10));
    let observed = std::panic::AssertUnwindSafe(execute(&operation, proof, b"dto".to_vec()))
        .catch_unwind()
        .await;
    assert!(
        observed.is_err(),
        "adapter panic was flattened into a business Result"
    );
    {
        let owner = retained.lock().unwrap();
        assert_eq!(owner.fault(), Some(Fault::TaskPanic));
        assert!(owner.jobs.is_empty());
        assert_eq!(owner.used_tasks(TaskKind::JobRoot), 0);
    }
    assert_eq!(operation.core.profile.quota.used(), 0);
    drop(operation);
    parent.shutdown().await;
}
