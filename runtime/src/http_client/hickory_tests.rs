//! Contract/owned-loopback fixtures: UNRUN until exact Hickory graph compile.
use super::*;
use hickory_resolver::{
    net::NoRecords,
    proto::{
        op::{Message, Query},
        rr::{
            rdata::{A, AAAA},
            Record,
        },
    },
};
use std::net::{Ipv4Addr, Ipv6Addr};
use tokio::net::UdpSocket;

#[test]
fn only_exact_matching_noerror_negative_is_nodata() {
    let name = Name::from_ascii("api.example.test.").unwrap();
    for (query_name, kind, rcode, expected) in [
        (name.clone(), RecordType::A, ResponseCode::NoError, true),
        (name.clone(), RecordType::A, ResponseCode::NXDomain, false),
        (name.clone(), RecordType::A, ResponseCode::ServFail, false),
        (name.clone(), RecordType::A, ResponseCode::Refused, false),
        (name.clone(), RecordType::AAAA, ResponseCode::NoError, false),
        (
            Name::from_ascii("other.example.test.").unwrap(),
            RecordType::A,
            ResponseCode::NoError,
            false,
        ),
    ] {
        let negative = NoRecords::new(Query::query(query_name, kind), rcode);
        assert_eq!(
            classify(&name, RecordType::A, Err(negative.into()), Instant::now()).is_ok(),
            expected
        );
    }
}
#[test]
fn full_final_answers_preserved_and_unrelated_additional_ignored() {
    let name = Name::from_ascii("api.example.test.").unwrap();
    let address = Ipv4Addr::new(192, 0, 2, 10);
    let mut lookup = Lookup::new_with_deadline(
        Query::query(name.clone(), RecordType::A),
        [Record::from_rdata(name.clone(), 30, RData::A(A(address)))],
        Instant::now() + Duration::from_secs(30),
    );
    lookup.extend_additionals(vec![Record::from_rdata(
        Name::from_ascii("unrelated.example.test.").unwrap(),
        30,
        RData::A(A(Ipv4Addr::LOCALHOST)),
    )]);
    match classify(&name, RecordType::A, Ok(lookup), Instant::now()).unwrap() {
        FamilyOutcome::Answers {
            lookup, addresses, ..
        } => {
            assert_eq!(addresses, vec![IpAddr::V4(address)]);
            assert_eq!(lookup.additionals().len(), 1);
        }
        _ => panic!("positive answer"),
    }
}
#[test]
fn unrelated_or_wrong_family_answer_is_not_silently_filtered() {
    let name = Name::from_ascii("api.example.test.").unwrap();
    for (owner, data) in [
        (
            Name::from_ascii("other.example.test.").unwrap(),
            RData::A(A(Ipv4Addr::LOCALHOST)),
        ),
        (name.clone(), RData::AAAA(AAAA(Ipv6Addr::LOCALHOST))),
    ] {
        let lookup = Lookup::new_with_deadline(
            Query::query(name.clone(), RecordType::A),
            [
                Record::from_rdata(name.clone(), 30, RData::A(A(Ipv4Addr::new(192, 0, 2, 10)))),
                Record::from_rdata(owner, 30, data),
            ],
            Instant::now() + Duration::from_secs(30),
        );
        assert!(classify(&name, RecordType::A, Ok(lookup), Instant::now()).is_err());
    }
}
#[test]
fn startup_rejects_local_shortcuts_literals_and_noncanonical_input() {
    let endpoint = "127.0.0.1:53".parse().unwrap();
    assert!(ResolverStartup::checked("api.example.test", endpoint).is_ok());
    let maximum = format!(
        "{}.{}.{}.{}",
        "a".repeat(63),
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(61)
    );
    assert_eq!(maximum.len(), 253);
    for host in [
        "api",
        "a-b.example.test",
        "xn--bcher-kva.example",
        maximum.as_str(),
    ] {
        let startup = ResolverStartup::checked(host, endpoint).unwrap();
        assert_eq!(startup.name.to_ascii(), format!("{host}."));
        assert!(matches!(url::Host::parse(host), Ok(url::Host::Domain(name)) if name == host));
        assert!(matches!(rustls::pki_types::ServerName::try_from(host),
            Ok(rustls::pki_types::ServerName::DnsName(name)) if name.as_ref() == host));
    }
    let oversized_label = format!("{}.example", "a".repeat(64));
    let oversized_name = format!("{maximum}a");
    for host in [
        "-api.example.test",
        "api-.example.test",
        "api..example.test",
        ".example.test",
        "2130706433",
        "127.1",
        "0x7f000001",
        "0177.0.0.1",
        "123",
        oversized_label.as_str(),
        oversized_name.as_str(),
    ] {
        assert!(
            matches!(
                ResolverStartup::checked(host, endpoint),
                Err(DnsFailure::Configuration)
            ),
            "sf06_canonical_startup_reject"
        );
    }
    for host in [
        "localhost",
        "foo.localhost",
        "foo.invalid",
        "foo.local",
        "foo.onion",
        "127.in-addr.arpa",
        "api.example.test.",
        "API.example.test",
        "127.0.0.1",
    ] {
        assert!(ResolverStartup::checked(host, endpoint).is_err(), "{host}");
    }
}

#[tokio::test]
async fn owned_udp_two_families_actual_tracked_resolver_and_join() {
    let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let endpoint = server.local_addr().unwrap();
    let mut actor = tokio::spawn(async move {
        let mut seen = Vec::new();
        let mut wire = [0u8; 4096];
        for _ in 0..2 {
            let (n, peer) =
                tokio::time::timeout(Duration::from_secs(2), server.recv_from(&mut wire))
                    .await
                    .unwrap()
                    .unwrap();
            let request = Message::from_vec(&wire[..n]).unwrap();
            assert_eq!(request.queries.len(), 1);
            let query = request.queries[0].clone();
            assert_eq!(
                query.name(),
                &Name::from_ascii("api.example.test.").unwrap()
            );
            let kind = query.query_type();
            seen.push(kind);
            let mut response = Message::response(request.metadata.id, request.metadata.op_code);
            response.add_query(query.clone());
            let data = match kind {
                RecordType::A => RData::A(A(Ipv4Addr::new(192, 0, 2, 10))),
                RecordType::AAAA => RData::AAAA(AAAA("2001:db8::10".parse().unwrap())),
                _ => panic!("only finite A/AAAA fixture"),
            };
            response.add_answer(Record::from_rdata(query.name().clone(), 30, data));
            let bytes = response.to_vec().unwrap();
            assert!(bytes.len() <= 4096);
            server.send_to(&bytes, peer).await.unwrap();
        }
        assert!(seen.contains(&RecordType::A) && seen.contains(&RecordType::AAAA));
    });
    let (owner, sender, id) = super::super::tests::fixture();
    let runner = super::super::tests::fixture_runner(&owner);
    let key = super::super::tests::fixture_key(&owner, id);
    let startup = ResolverStartup::checked("api.example.test", endpoint).unwrap();
    let result = resolve(
        &runner,
        &key,
        &startup,
        Instant::now() + Duration::from_secs(1),
    )
    .await;
    // Always join the owned fixture actor before asserting resolver success.
    let actor_result = match tokio::time::timeout(Duration::from_secs(3), &mut actor).await {
        Ok(result) => result,
        Err(_) => {
            actor.abort();
            actor.await
        }
    };
    super::super::tests::cleanup(&owner).await;
    let resolved = result.unwrap();
    assert!(matches!(resolved.a, FamilyOutcome::Answers { .. }));
    assert!(matches!(resolved.aaaa, FamilyOutcome::Answers { .. }));
    assert!(actor_result.is_ok());
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::DnsRoot), 0);
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::DnsChild), 0);
    assert_eq!(owner.lock().unwrap().used_tasks(TaskKind::JobRoot), 0);
    drop(sender);
}

// UNRUN exact Hickory fixture. This tests result shapes produced by both the
// same-response and cross-response primary CachingClient CNAME paths.
#[test]
fn original_and_terminal_query_require_same_anchored_chain_and_original_ttl() {
    use hickory_resolver::proto::rr::rdata::CNAME;
    let original = Name::from_ascii("api.example.test.").unwrap();
    let terminal = Name::from_ascii("edge.example.test.").unwrap();
    let unrelated = Name::from_ascii("other.example.test.").unwrap();
    let started = Instant::now() - Duration::from_secs(1);
    for query_name in [original.clone(), terminal.clone()] {
        let lookup = Lookup::new_with_deadline(
            Query::query(query_name, RecordType::A),
            [
                Record::from_rdata(original.clone(), 5, RData::CNAME(CNAME(terminal.clone()))),
                Record::from_rdata(
                    terminal.clone(),
                    30,
                    RData::A(A(Ipv4Addr::new(192, 0, 2, 10))),
                ),
            ],
            Instant::now() + Duration::from_secs(30),
        );
        match classify(&original, RecordType::A, Ok(lookup), started).unwrap() {
            FamilyOutcome::Answers {
                lookup,
                positive_until,
                ..
            } => {
                assert_eq!(lookup.answers().len(), 2);
                assert!(positive_until <= started + Duration::from_secs(5));
            }
            _ => panic!("anchored positive CNAME"),
        }
    }
    for (query_name, owner) in [
        (terminal.clone(), unrelated.clone()),
        (unrelated, original.clone()),
    ] {
        let lookup = Lookup::new_with_deadline(
            Query::query(query_name, RecordType::A),
            [
                Record::from_rdata(owner, 5, RData::CNAME(CNAME(terminal.clone()))),
                Record::from_rdata(
                    terminal.clone(),
                    30,
                    RData::A(A(Ipv4Addr::new(192, 0, 2, 10))),
                ),
            ],
            Instant::now() + Duration::from_secs(30),
        );
        assert!(classify(&original, RecordType::A, Ok(lookup), started).is_err());
    }
}

// Actual resolver's same-response and separately queried CNAME paths, not just
// an invented terminal Query. Both finite owned actors are joined before asserts.
#[tokio::test]
async fn owned_udp_cname_same_response_and_cross_response_keep_origin() {
    use hickory_resolver::proto::rr::rdata::CNAME;
    for same_response in [true, false] {
        let original = Name::from_ascii("api.example.test.").unwrap();
        let terminal = Name::from_ascii("edge.example.test.").unwrap();
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let endpoint = server.local_addr().unwrap();
        let expected = if same_response { 2 } else { 4 };
        let mut actor = tokio::spawn(async move {
            let mut names = Vec::new();
            let mut wire = [0; 4096];
            for _ in 0..expected {
                let (n, peer) =
                    tokio::time::timeout(Duration::from_secs(2), server.recv_from(&mut wire))
                        .await
                        .unwrap()
                        .unwrap();
                let request = Message::from_vec(&wire[..n]).unwrap();
                assert_eq!(request.queries.len(), 1);
                let query = request.queries[0].clone();
                let initial = query.name() == &original;
                assert!(initial || query.name() == &terminal);
                names.push(query.name().clone());
                let mut response = Message::response(request.metadata.id, request.metadata.op_code);
                response.add_query(query.clone());
                if initial {
                    response.add_answer(Record::from_rdata(
                        original.clone(),
                        0,
                        RData::CNAME(CNAME(terminal.clone())),
                    ));
                }
                if !initial || same_response {
                    let data = match query.query_type() {
                        RecordType::A => RData::A(A(Ipv4Addr::new(192, 0, 2, 10))),
                        RecordType::AAAA => RData::AAAA(AAAA("2001:db8::10".parse().unwrap())),
                        _ => panic!("only A/AAAA"),
                    };
                    response.add_answer(Record::from_rdata(terminal.clone(), 30, data));
                }
                let bytes = response.to_vec().unwrap();
                assert!(bytes.len() <= 4096);
                server.send_to(&bytes, peer).await.unwrap();
            }
            assert_eq!(names.iter().filter(|name| *name == &original).count(), 2);
            assert_eq!(
                names.iter().filter(|name| *name == &terminal).count(),
                if same_response { 0 } else { 2 }
            );
        });
        let (owner, sender, id) = super::super::tests::fixture();
        let runner = super::super::tests::fixture_runner(&owner);
        let key = super::super::tests::fixture_key(&owner, id);
        let startup = ResolverStartup::checked("api.example.test", endpoint).unwrap();
        let begun = Instant::now();
        let result = resolve(&runner, &key, &startup, begun + Duration::from_secs(1)).await;
        let actor_result = match tokio::time::timeout(Duration::from_secs(3), &mut actor).await {
            Ok(result) => result,
            Err(_) => {
                actor.abort();
                actor.await
            }
        };
        super::super::tests::cleanup(&owner).await;
        let resolution = result.unwrap();
        assert_eq!(
            resolution.name,
            Name::from_ascii("api.example.test.").unwrap()
        );
        for family in [&resolution.a, &resolution.aaaa] {
            match family {
                FamilyOutcome::Answers {
                    lookup,
                    positive_until,
                    ..
                } => {
                    assert_eq!(
                        lookup
                            .answers()
                            .iter()
                            .filter(|r| matches!(&r.data, RData::CNAME(_)))
                            .count(),
                        1
                    );
                    assert!(positive_until <= &Instant::now());
                }
                _ => panic!("CNAME positive expected"),
            }
        }
        assert!(actor_result.is_ok());
        drop(sender);
    }
}
