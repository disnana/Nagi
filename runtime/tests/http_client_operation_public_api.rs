//! Productioncfg default-deny proof, without an internal-address exception seam.
//! Private test numbers do not establish a supported positive production profile.
use hickory_resolver::proto::{
    op::Message,
    rr::{rdata::A, RData, Record, RecordType},
};
use nagi_runtime::http_client::{self as client, startup::*};
use std::{
    net::Ipv4Addr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    net::{TcpListener, UdpSocket},
    sync::oneshot,
};
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

struct State {
    preparations: AtomicUsize,
}
fn freeze(input: Vec<u8>, limits: &InputLimits) -> Result<FrozenInput, client::Failure> {
    FrozenInput::checked(input, limits)
}
fn public_prepare(
    state: &State,
    input: FrozenInput,
    limits: &RequestLimits,
) -> Result<PreparedRequest, client::Failure> {
    state.preparations.fetch_add(1, Ordering::SeqCst);
    PreparedRequest::public(&["items"], &[], input.into_bytes(), limits)
}
fn protected_prepare(
    _: &State,
    bound: BoundValues,
    input: FrozenInput,
    limits: &RequestLimits,
) -> Result<PreparedRequest, client::Failure> {
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
#[tokio::test]
async fn production_public_handle_rejects_loopback_before_tcp_or_http() {
    // This external crate cannot call the cfg(test) native-network admission seam.
    let _protected_callback: fn(
        &State,
        BoundValues,
        FrozenInput,
        &RequestLimits,
    ) -> Result<PreparedRequest, client::Failure> = protected_prepare;
    let _protected_execution = client::execute::<State, (), Vec<u8>>;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tcp_stop, mut stop_tcp) = oneshot::channel();
    let tcp_actor = tokio::spawn(async move {
        let mut count = 0;
        loop {
            // Poll accept first even when stop is ready, so queued connections
            // cannot be hidden by shutdown after the operation has actually joined.
            tokio::select! {biased;
                accepted = listener.accept() => { let (socket, _) = accepted.unwrap(); count += 1; drop(socket); }
                _ = &mut stop_tcp => break,
            }
        }
        count
    });
    let dns = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let endpoint = dns.local_addr().unwrap();
    let (dns_stop, mut stop_dns) = oneshot::channel();
    let dns_actor = tokio::spawn(async move {
        let mut count = 0;
        let mut bytes = [0; 4096];
        loop {
            let (n, peer) = tokio::select! {biased; _ = &mut stop_dns => break, result = dns.recv_from(&mut bytes) => result.unwrap()};
            count += 1;
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
        count
    });
    let mut catalog = StartupCatalog::new(
        1,
        RetentionLimits {
            max_root_anchors: 8,
            max_root_bytes: 16 * 1024,
            max_config_bytes: 128 * 1024,
        },
    )
    .unwrap();
    let service = Arc::new(
        ServiceSpec::checked(
            &catalog,
            ServiceConfig {
                key: ServiceKey::checked(1, 2, 3, 4).unwrap(),
                canonical_dns_name: "api.example.test".into(),
                port,
                resolver_endpoint: endpoint,
                roots: rustls::RootCertStore::empty(),
                max_candidates: 8,
            },
        )
        .unwrap(),
    );
    let state = Arc::new(State {
        preparations: AtomicUsize::new(0),
    });
    // PublicSpec has no credential argument and no caller-auth/header surface.
    let spec = PublicSpec::checked(
        service,
        1,
        hyper::Method::POST,
        limits(),
        FixedRequestHeaders::checked(&catalog, None, None).unwrap(),
    )
    .unwrap();
    let operation: client::PublicOperation<State, Vec<u8>> = register_public(
        &mut catalog,
        spec,
        Arc::clone(&state),
        freeze,
        public_prepare,
    )
    .unwrap();
    let mut owner = catalog.freeze();
    let refusal = client::execute_public(&operation, b"dto".to_vec())
        .await
        .err()
        .unwrap();
    drop(operation);
    owner.shutdown().await;
    dns_stop.send(()).unwrap();
    tcp_stop.send(()).unwrap();
    let dns_calls = dns_actor.await.unwrap();
    let tcp_calls = tcp_actor.await.unwrap();
    assert_eq!(
        client::kind(&refusal),
        client::FailureKind::DestinationDenied
    );
    assert_eq!(
        client::outcome(&refusal),
        client::Outcome::RequestNotStarted
    );
    assert_eq!(state.preparations.load(Ordering::SeqCst), 1);
    assert_eq!(dns_calls, 2);
    assert_eq!(
        tcp_calls, 0,
        "production loopback refusal started a TCP connection"
    );
}
