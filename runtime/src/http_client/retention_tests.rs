//! Small semantic fixtures. These are not allocation-size or process-RSS claims.
use super::*;
fn limits() -> RetentionLimits {
    crate::http_client::startup_tests::retention_limits()
}
fn catalog() -> StartupCatalog {
    StartupCatalog::new(2, limits()).unwrap()
}
fn store() -> RootCertStore {
    let mut subject = Vec::with_capacity(256);
    subject.extend_from_slice(&[1, 2]);
    RootCertStore {
        roots: vec![TrustAnchor {
            subject: Der::from(subject),
            subject_public_key_info: Der::from(vec![3, 4, 5]),
            name_constraints: Some(Der::from(Vec::new())),
        }],
    }
}
fn service(catalog: &StartupCatalog, roots: RootCertStore) -> Result<ServiceSpec, Failure> {
    ServiceSpec::checked(
        catalog,
        ServiceConfig {
            key: ServiceKey::checked(1, 2, 3, 4).unwrap(),
            canonical_dns_name: "api.example.test".into(),
            port: 443,
            resolver_endpoint: "127.0.0.1:53".parse().unwrap(),
            roots,
            max_candidates: 8,
        },
    )
}
fn failure<T>(result: Result<T, Failure>, expected: FailureKind) {
    let error = result.err().expect("startup refusal");
    assert_eq!(error.kind, expected);
    assert_eq!(error.outcome, Outcome::RequestNotStarted);
    assert!(error.auth.is_none());
}
#[test]
fn explicit_positive_limits_and_checked_arithmetic_without_large_allocations() {
    for selector in 0..3 {
        let mut value = limits();
        match selector {
            0 => value.max_root_anchors = 0,
            1 => value.max_root_bytes = 0,
            _ => value.max_config_bytes = 0,
        }
        failure(StartupCatalog::new(1, value), FailureKind::InvalidInput);
    }
    failure(sum(&[usize::MAX, 1]), FailureKind::InvalidInput);
    failure(
        slots::<TrustAnchor<'static>>(usize::MAX),
        FailureKind::InvalidInput,
    );
}
#[test]
fn all_three_exact_boundaries_aggregate_rollback_and_last_owner() {
    let budget = RetentionBudget::new(RetentionLimits {
        max_root_anchors: 2,
        max_root_bytes: 3,
        max_config_bytes: size_of::<RetentionBudget>() + 16,
    })
    .unwrap();
    let base = budget.used();
    let claim = Arc::new(
        budget
            .reserve(Retained {
                anchors: 2,
                der: 3,
                config: 16,
            })
            .unwrap(),
    );
    let full = budget.used();
    for extra in [
        Retained {
            anchors: 1,
            ..Retained::default()
        },
        Retained {
            der: 1,
            ..Retained::default()
        },
        Retained {
            config: 1,
            ..Retained::default()
        },
    ] {
        failure(budget.reserve(extra), FailureKind::Capacity);
        assert_eq!(budget.used(), full);
    }
    let last = Arc::clone(&claim);
    drop(claim);
    assert_eq!(budget.used(), full);
    drop(last);
    assert_eq!(budget.used(), base);
    drop(
        budget
            .reserve(Retained {
                anchors: 2,
                der: 3,
                config: 16,
            })
            .unwrap(),
    );
    assert_eq!(budget.used(), base);
}
#[test]
fn normalized_der_keeps_order_and_some_empty_but_discards_hidden_capacity() {
    let budget = RetentionBudget::new(limits()).unwrap();
    let mut claim = budget
        .reserve(Retained {
            anchors: 2,
            ..Retained::default()
        })
        .unwrap();
    let mut original = store();
    original.roots.push(original.roots[0].clone());
    let before = DER_COPIES.with(|c| c.get());
    preflight_roots(&original, &mut claim).unwrap();
    let out = normalized(&original, &mut claim).unwrap();
    let after = DER_COPIES.with(|c| c.get());
    assert_eq!(out.roots.len(), 2);
    for (input, output) in original.roots.iter().zip(&out.roots) {
        assert_eq!(input.subject.as_ref(), output.subject.as_ref());
        assert_eq!(
            input.subject_public_key_info.as_ref(),
            output.subject_public_key_info.as_ref()
        );
        assert_eq!(
            output.name_constraints.as_ref().map(|n| n.as_ref()),
            Some([].as_slice())
        );
        assert_ne!(
            input.subject.as_ref().as_ptr(),
            output.subject.as_ref().as_ptr()
        );
    }
    assert_eq!(after.0 - before.0, 6);
    assert_eq!(claim.amount.der, after.1 - before.1);
    assert_eq!(
        claim.amount.config,
        claim.amount.der + out.roots.capacity() * size_of::<TrustAnchor<'static>>()
    );
    // Observe the allocated output capacities; do not assume reserve_exact has
    // no spare capacity. The original 256-byte subject allocation is not moved.
    assert_eq!(claim.amount.anchors, out.roots.len());
}
#[test]
fn unregistered_services_count_individually_before_root_copy_and_refund_on_drop() {
    let mut configured = limits();
    configured.max_root_anchors = 1;
    let catalog = StartupCatalog::new(2, configured).unwrap();
    let base = catalog.budget.used();
    let first = service(&catalog, store()).unwrap();
    let full = catalog.budget.used();
    let before = DER_COPIES.with(|c| c.get());
    failure(service(&catalog, store()), FailureKind::Capacity);
    assert_eq!(DER_COPIES.with(|c| c.get()), before);
    assert_eq!(catalog.budget.used(), full);
    drop(first);
    assert_eq!(catalog.budget.used(), base);
    drop(service(&catalog, store()).unwrap());
    assert_eq!(catalog.budget.used(), base);
}
#[test]
fn native_header_clone_keeps_measured_capacity_and_claim_after_descriptor_drop() {
    let catalog = catalog();
    let base = catalog.budget.used();
    let mut bytes = Vec::with_capacity(64);
    bytes.push(b'a');
    let capacity = bytes.capacity();
    let pointer = bytes.as_ptr();
    let headers = FixedRequestHeaders::checked(&catalog, Some(bytes), None).unwrap();
    let value = headers.fixed.test_accept().unwrap();
    assert_eq!(value.as_bytes(), b"a");
    assert_eq!(value.as_bytes().as_ptr(), pointer);
    let mut native = hyper::HeaderMap::new();
    native.insert(hyper::header::ACCEPT, value);
    drop(headers);
    assert_eq!(
        catalog.budget.used().config,
        base.config + header_charge(capacity).unwrap()
    );
    let cloned = native[hyper::header::ACCEPT].clone();
    drop(native);
    assert_eq!(
        catalog.budget.used().config,
        base.config + header_charge(capacity).unwrap()
    );
    drop(cloned);
    assert_eq!(catalog.budget.used(), base);
}
#[test]
fn oversized_header_and_string_capacity_are_rejected_and_invalid_payloads_refund() {
    let catalog = catalog();
    let base = catalog.budget.used();
    let mut bytes = Vec::with_capacity(limits().max_config_bytes);
    bytes.push(b'a');
    failure(
        FixedRequestHeaders::checked(&catalog, Some(bytes), None),
        FailureKind::Capacity,
    );
    for bytes in [b"\r".to_vec(), b"\n".to_vec(), vec![127]] {
        failure(
            FixedRequestHeaders::checked(&catalog, Some(bytes), None),
            FailureKind::InvalidInput,
        );
        assert_eq!(catalog.budget.used(), base);
    }
    let mut name = String::with_capacity(limits().max_config_bytes);
    name.push_str("api.example.test");
    failure(
        ServiceSpec::checked(
            &catalog,
            ServiceConfig {
                key: ServiceKey::checked(1, 2, 3, 4).unwrap(),
                canonical_dns_name: name,
                port: 443,
                resolver_endpoint: "127.0.0.1:53".parse().unwrap(),
                roots: store(),
                max_candidates: 8,
            },
        ),
        FailureKind::Capacity,
    );
    assert_eq!(catalog.budget.used(), base);
}
#[test]
fn bearer_is_sensitive_budgeted_and_bound_to_catalog_service_and_operation() {
    let catalog = catalog();
    let other = self::catalog();
    let service = Arc::new(service(&catalog, store()).unwrap());
    let same_numbers = Arc::new(self::service(&catalog, store()).unwrap());
    assert!(!Arc::ptr_eq(&service.trust, &same_numbers.trust));
    assert_eq!(catalog.budget.used().anchors, 2);
    let base = catalog.budget.used();
    failure(
        ServiceCredential::bearer(&other, &service, 1, b"token".to_vec()),
        FailureKind::InvalidInput,
    );
    for bytes in [Vec::new(), b"bad token".to_vec(), b"x\r\ny".to_vec()] {
        failure(
            ServiceCredential::bearer(&catalog, &service, 1, bytes),
            FailureKind::InvalidInput,
        );
        assert_eq!(catalog.budget.used(), base);
    }
    let credential = ServiceCredential::bearer(&catalog, &service, 1, b"token".to_vec()).unwrap();
    assert_eq!(credential.value.as_bytes(), b"Bearer token");
    assert!(credential.value.is_sensitive());
    failure(
        ProtectedSpec::<()>::checked(
            Arc::clone(&same_numbers),
            1,
            Method::GET,
            crate::http_client::startup_tests::limits(),
            FixedRequestHeaders::checked(&catalog, None, None).unwrap(),
            Some(credential),
        ),
        FailureKind::InvalidInput,
    );
    assert_eq!(catalog.budget.used(), base);
    let credential = ServiceCredential::bearer(&catalog, &service, 1, b"token".to_vec()).unwrap();
    let native = credential.value.clone();
    drop(credential);
    assert!(catalog.budget.used().config > base.config);
    assert!(native.is_sensitive());
    drop(native);
    assert_eq!(catalog.budget.used(), base);
    failure(
        PublicSpec::checked(
            Arc::clone(&service),
            1,
            Method::GET,
            crate::http_client::startup_tests::limits(),
            FixedRequestHeaders::checked(&other, None, None).unwrap(),
        ),
        FailureKind::InvalidInput,
    );
}
#[test]
fn partial_allocation_failures_refund_after_accepted_partial_data_is_dropped() {
    let catalog = catalog();
    let base = catalog.budget.used();
    let mut injected = 0;
    for site in 0..16 {
        COPY_FAULT.with(|f| f.set(Some(site)));
        let result = service(&catalog, store());
        COPY_FAULT.with(|f| f.set(None));
        match result {
            Ok(value) => {
                drop(value);
                assert_eq!(catalog.budget.used(), base);
                break;
            }
            Err(error) => {
                failure::<()>(Err(error), FailureKind::Unavailable);
                injected += 1;
                assert_eq!(catalog.budget.used(), base);
            }
        }
    }
    assert!(
        injected >= 6,
        "root slots, DER, authority/name and ALPN failure sites"
    );
}
#[tokio::test]
async fn same_service_operations_share_one_trust_and_duplicate_registration_refunds() {
    let mut catalog = catalog();
    let budget = Arc::clone(&catalog.budget);
    let service = Arc::new(service(&catalog, store()).unwrap());
    let roots = budget.used();
    let make = |operation| {
        PublicSpec::checked(
            Arc::clone(&service),
            operation,
            Method::GET,
            crate::http_client::startup_tests::limits(),
            FixedRequestHeaders::checked(&catalog, None, None).unwrap(),
        )
        .unwrap()
    };
    let first_spec = make(1);
    let second_spec = make(2);
    let first = catalog.register(first_spec.spec, None).unwrap();
    let second = catalog.register(second_spec.spec, None).unwrap();
    assert!(Arc::ptr_eq(&first.profile.trust, &second.profile.trust));
    assert_eq!(budget.used().anchors, roots.anchors);
    assert_eq!(budget.used().der, roots.der);
    let baseline = budget.used();
    let duplicate = PublicSpec::checked(
        Arc::clone(&service),
        1,
        Method::GET,
        crate::http_client::startup_tests::limits(),
        FixedRequestHeaders::checked(&catalog, None, None).unwrap(),
    )
    .unwrap();
    failure(
        catalog.register(duplicate.spec, None),
        FailureKind::Capacity,
    );
    assert_eq!(budget.used(), baseline);
    let mut owner = catalog.freeze();
    assert_eq!(budget.used().anchors, roots.anchors);
    drop(service);
    drop(first);
    assert_eq!(budget.used().anchors, roots.anchors);
    drop(second);
    owner.shutdown().await;
    assert_eq!(budget.used().anchors, 0);
    assert_eq!(budget.used().der, 0);
    failure(budget.reserve(Retained::default()), FailureKind::Closing);
}

#[test]
fn root_byte_preflight_fails_before_der_copy_and_catalog_drop_closes_authority() {
    let mut configured = limits();
    configured.max_root_bytes = 4;
    let catalog = StartupCatalog::new(1, configured).unwrap();
    let base = catalog.budget.used();
    let copies = DER_COPIES.with(|c| c.get());
    failure(service(&catalog, store()), FailureKind::Capacity);
    assert_eq!(DER_COPIES.with(|c| c.get()), copies);
    assert_eq!(catalog.budget.used(), base);
    let budget = Arc::clone(&catalog.budget);
    drop(catalog);
    failure(budget.reserve(Retained::default()), FailureKind::Closing);
}
#[test]
fn unregistered_header_aggregate_uses_actual_capacity_and_refunds_rejected_constructor() {
    let catalog = catalog();
    let base = catalog.budget.used();
    let mut bytes = Vec::with_capacity(32);
    bytes.push(b'a');
    let capacity = bytes.capacity();
    let one = sum(&[
        size_of::<FixedRequestHeaders>(),
        size_of::<RetentionClaim>(),
        header_charge(capacity).unwrap(),
    ])
    .unwrap();
    // Fill only the ledger with a synthetic reservation. No allocation size or
    // repeated reserve_exact capacity is assumed by this boundary oracle.
    let padding = catalog
        .budget
        .reserve(Retained {
            config: catalog.budget.limits.max_config_bytes - base.config - one,
            ..Retained::default()
        })
        .unwrap();
    let before = catalog.budget.used();
    let first = FixedRequestHeaders::checked(&catalog, Some(bytes), None).unwrap();
    assert_eq!(catalog.budget.used().config, before.config + one);
    let full = catalog.budget.used();
    failure(
        FixedRequestHeaders::checked(&catalog, None, Some(vec![b'b'])),
        FailureKind::Capacity,
    );
    assert_eq!(catalog.budget.used(), full);
    drop(first);
    assert_eq!(catalog.budget.used(), before);
    drop(FixedRequestHeaders::checked(&catalog, Some(vec![b'c']), None).unwrap());
    assert_eq!(catalog.budget.used(), before);
    drop(padding);
    assert_eq!(catalog.budget.used(), base);
}

#[test]
fn parent_no_runtime_refusal_and_cross_catalog_registration_refund_all_partial_config() {
    let mut catalog = catalog();
    let other = self::catalog();
    let service = Arc::new(service(&catalog, store()).unwrap());
    let base = catalog.budget.used();
    let spec = PublicSpec::checked(
        Arc::clone(&service),
        1,
        Method::GET,
        crate::http_client::startup_tests::limits(),
        FixedRequestHeaders::checked(&catalog, None, None).unwrap(),
    )
    .unwrap();
    failure(catalog.register(spec.spec, None), FailureKind::Unavailable);
    assert!(catalog.identities.is_empty());
    assert!(catalog.parent.records.is_empty());
    assert_eq!(catalog.budget.used(), base);
    let wrong_service = Arc::new(self::service(&other, store()).unwrap());
    let other_base = other.budget.used();
    let spec = PublicSpec::checked(
        wrong_service.clone(),
        1,
        Method::GET,
        crate::http_client::startup_tests::limits(),
        FixedRequestHeaders::checked(&other, None, None).unwrap(),
    )
    .unwrap();
    failure(catalog.register(spec.spec, None), FailureKind::InvalidInput);
    assert!(catalog.identities.is_empty());
    assert_eq!(other.budget.used(), other_base);
    let credential = ServiceCredential::bearer(&catalog, &service, 1, b"token".to_vec()).unwrap();
    failure(
        ProtectedSpec::<()>::checked(
            Arc::clone(&service),
            2,
            Method::GET,
            crate::http_client::startup_tests::limits(),
            FixedRequestHeaders::checked(&catalog, None, None).unwrap(),
            Some(credential),
        ),
        FailureKind::InvalidInput,
    );
    assert_eq!(catalog.budget.used(), base);
}
