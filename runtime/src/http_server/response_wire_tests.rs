use super::*;

fn limits() -> Limits {
    Limits {
        body_bytes: 32,
        user_header_bytes: 128,
        user_header_count: 4,
        managed_header_bytes: 512,
        managed_header_count: 12,
        serialization_writes: 64,
        live_responses: 1,
        retained_bytes: 65536,
    }
}
fn producer() -> Producer {
    Producer::checked_wire(limits()).unwrap()
}
fn capacity(producer: &Producer) {
    assert_eq!(producer.text("held").err(), Some(Diagnostic::Capacity));
}
fn fallback_limits() -> FallbackLimits {
    FallbackLimits {
        responses: 1,
        retained_bytes: 65536,
        managed_header_bytes: 512,
        managed_header_count: 12,
    }
}
fn fallback_wire(outcome: FallbackOutcome, expected: u16) -> ChargedWire {
    match outcome {
        FallbackOutcome::Wire { status, wire } => {
            assert_eq!(status, expected);
            wire
        }
        FallbackOutcome::Close => panic!("expected one finite fallback"),
    }
}

#[test]
fn native_header_only_and_bytes_frame_clone_retain_one_original_claim() {
    let producer = producer();
    let mut response = producer.bytes(b"owned").unwrap();
    let pointer = response.body().as_ptr();
    response.headers.append(b"x-test", b"first").unwrap();
    response.headers.append(b"x-test", b"second").unwrap();
    assert!(producer.owns(&response));
    let wire = response.into_wire().unwrap();
    assert!(wire.is_owned_by(&producer));
    let (headers, frame) = wire.into_parts();
    assert_eq!(headers.map.get_all("x-test").iter().count(), 2);
    assert_eq!(headers.value("x-test"), Some(b"first".as_slice()));
    assert_eq!(pointer, frame.as_ptr());
    let cloned_frame = hyper::body::Frame::data(frame.clone());
    drop(frame);
    capacity(&producer);
    drop(headers);
    capacity(&producer);
    assert_eq!(cloned_frame.data_ref().unwrap().as_ref(), b"owned");
    drop(cloned_frame);
    assert!(producer.text("released").is_ok());

    let (headers, body) = producer
        .text("reverse")
        .unwrap()
        .into_wire()
        .unwrap()
        .into_parts();
    drop(body);
    assert_eq!(Arc::strong_count(&headers.charge), 1);
    capacity(&producer);
    drop(headers);
    assert!(producer.text("released").is_ok());
}

#[test]
fn head_empty_body_and_cleared_native_container_keep_header_charge() {
    let producer = producer();
    let mut wire = producer
        .text("representation")
        .unwrap()
        .into_wire()
        .unwrap();
    wire.headers_mut()
        .append_managed(ManagedHeader::ContentLength, b"14")
        .unwrap();
    wire.suppress_body_for_head();
    let (mut headers, body) = wire.into_parts();
    assert!(body.is_empty());
    drop(body);
    assert_eq!(headers.value("content-length"), Some(b"14".as_slice()));
    let capacity_before = headers.map.capacity();
    headers.map.clear(); // values/names gone; native container storage remains.
    assert_eq!(headers.map.capacity(), capacity_before);
    assert!(capacity_before > 0);
    capacity(&producer);
    drop(headers);
    assert!(producer.bytes(b"").is_ok());
    let (headers, body) = producer
        .bytes(b"")
        .unwrap()
        .into_wire()
        .unwrap()
        .into_parts();
    drop(body);
    capacity(&producer);
    drop(headers);
    assert!(producer.text("released").is_ok());
}

#[test]
fn native_transfer_rejects_unplanned_old_producer_and_foreign_owner() {
    let legacy = Producer::checked(limits()).unwrap();
    assert!(matches!(
        legacy.text("old").unwrap().into_wire(),
        Err(Diagnostic::InvalidLimits)
    ));
    assert!(legacy.text("released").is_ok());
    let first = producer();
    let second = producer();
    let made = first.text("first").unwrap();
    assert!(!second.owns(&made));
    let wire = made.into_wire().unwrap();
    assert!(!wire.is_owned_by(&second));
    capacity(&first);
    assert!(second.text("independent").is_ok());
}

#[test]
fn invalid_native_header_conversion_frees_reservation_without_partial_escape() {
    let producer = producer();
    let mut made = producer.text("body").unwrap();
    made.headers.append(b"invalid name", b"value").unwrap();
    assert!(matches!(made.into_wire(), Err(Diagnostic::HeaderLimit)));
    assert!(producer.text("released").is_ok());
}

#[test]
fn managed_and_user_header_budgets_remain_separate() {
    let producer = producer();
    let mut made = producer.text("body").unwrap();
    for _ in 0..4 {
        made.headers.append(b"x", b"user").unwrap();
    }
    assert_eq!(
        made.headers.append(b"x", b"fifth"),
        Err(Diagnostic::HeaderLimit)
    );
    let mut wire = made.into_wire().unwrap();
    wire.headers_mut()
        .append_managed(ManagedHeader::SetCookie, b"fixed=managed")
        .unwrap();
    wire.headers_mut()
        .append_managed(ManagedHeader::ContentSecurityPolicy, b"default-src 'none'")
        .unwrap();
    let count = wire.headers.map.len();
    assert_eq!(
        wire.headers_mut()
            .append_managed(ManagedHeader::AllowOrigin, &[b'x'; 513]),
        Err(Diagnostic::HeaderLimit)
    );
    assert_eq!(wire.headers.map.len(), count);
    capacity(&producer);
}

#[test]
fn framework_names_are_fixed_and_each_uses_managed_capacity() {
    let producer = producer();
    for name in [
        ManagedHeader::ContentType,
        ManagedHeader::ContentLength,
        ManagedHeader::CacheControl,
        ManagedHeader::Vary,
        ManagedHeader::WwwAuthenticate,
        ManagedHeader::SetCookie,
        ManagedHeader::ContentSecurityPolicy,
        ManagedHeader::Nosniff,
        ManagedHeader::AllowOrigin,
        ManagedHeader::AllowCredentials,
        ManagedHeader::AllowMethods,
        ManagedHeader::AllowHeaders,
        ManagedHeader::MaxAge,
        ManagedHeader::RetryAfter,
    ] {
        let mut wire = producer.bytes(b"").unwrap().into_wire().unwrap();
        wire.headers_mut().append_managed(name, b"fixed").unwrap();
        assert_eq!(wire.headers.managed_count, 1);
        assert_eq!(wire.headers.value(name.name()), Some(b"fixed".as_slice()));
        drop(wire);
    }
}

#[test]
fn native_layout_peak_is_reserved_before_copy_and_checked_before_publish() {
    let config = limits();
    let layout = WireLayout::checked(&config).unwrap();
    assert!(layout.peak_bytes > config.user_header_bytes + config.managed_header_bytes);
    let mut impossible = limits();
    impossible.retained_bytes = layout.peak_bytes;
    assert!(matches!(
        Producer::checked_wire(impossible),
        Err(Diagnostic::InvalidLimits)
    ));
    let mut overflow = limits();
    overflow.managed_header_count = usize::MAX;
    assert!(matches!(
        Producer::checked_wire(overflow),
        Err(Diagnostic::InvalidLimits)
    ));
    let mut native_max = limits();
    native_max.managed_header_count = 32768;
    assert!(matches!(
        Producer::checked_wire(native_max),
        Err(Diagnostic::InvalidLimits)
    ));
}

#[test]
fn native_container_capacity_stays_within_pinned_layout_on_duplicates_and_names() {
    let mut config = limits();
    config.user_header_count = 256;
    config.user_header_bytes = 16384;
    config.retained_bytes = 4 * 1024 * 1024;
    let producer = Producer::checked_wire(config).unwrap();
    let mut made = producer.text("body").unwrap();
    for index in 0..256 {
        let name = if index % 2 == 0 {
            "x-duplicate".to_owned()
        } else {
            format!("x-{index}")
        };
        made.headers.append(name.as_bytes(), b"value").unwrap();
    }
    let peak = made.wire.unwrap().peak_bytes;
    let (wire, allocations) = crate::metrics::measure(|| made.into_wire().unwrap());
    assert!(
        allocations.allocations > 0,
        "native allocation meter positive control"
    );
    // Cumulative allocator-requested native conversion bytes upper-bound live
    // requested bytes for this fixture. This is not RSS or a universal profile.
    assert!(usize::try_from(allocations.allocated_bytes).unwrap() <= peak);
    assert_eq!(wire.headers.map.len(), 256);
    assert!(wire.headers.map.capacity() <= wire.headers.layout.max_capacity);
    capacity(&producer);
    drop(wire);
    assert!(producer.text("released").is_ok());
}

#[test]
fn fallback_uses_independent_ledger_then_closes_without_another_allocation() {
    let ordinary = producer();
    let held = ordinary.text("held").unwrap().into_wire().unwrap();
    capacity(&ordinary);
    let fallback = FallbackLedger::checked(fallback_limits(), 1).unwrap();
    let wire = fallback_wire(fallback.produce(FallbackStatus::Capacity), 503);
    assert!(!wire.is_owned_by(&ordinary));
    assert_eq!(
        wire.headers.value("cache-control"),
        Some(b"no-store".as_slice())
    );
    let (headers, body) = wire.into_parts();
    let frame = hyper::body::Frame::data(body.clone());
    drop(body);
    let allocations = fallback
        .producer
        .allocation_attempts
        .load(std::sync::atomic::Ordering::Relaxed);
    assert!(matches!(
        fallback.produce(FallbackStatus::Capacity),
        FallbackOutcome::Close
    ));
    assert_eq!(
        fallback
            .producer
            .allocation_attempts
            .load(std::sync::atomic::Ordering::Relaxed),
        allocations
    );
    drop(headers);
    assert!(matches!(
        fallback.produce(FallbackStatus::Internal),
        FallbackOutcome::Close
    ));
    drop(frame);
    assert!(matches!(
        fallback.produce(FallbackStatus::Internal),
        FallbackOutcome::Wire { status: 500, .. }
    ));
    capacity(&ordinary);
    drop(held);
    assert!(ordinary.text("released").is_ok());
}

#[test]
fn fallback_preserves_statuses_and_positive_retry_without_ordinary_serializer() {
    let fallback = FallbackLedger::checked(fallback_limits(), 1).unwrap();
    for (status, code) in [
        (FallbackStatus::Unauthorized, 401),
        (FallbackStatus::Forbidden, 403),
        (FallbackStatus::BodyTimeout, 408),
        (FallbackStatus::BodyTooLarge, 413),
        (FallbackStatus::Deadline, 504),
        (FallbackStatus::Internal, 500),
        (FallbackStatus::Capacity, 503),
        (
            FallbackStatus::Limited {
                retry_seconds: u64::MAX,
            },
            429,
        ),
    ] {
        let wire = fallback_wire(fallback.produce(status), code);
        assert!(wire.body.len() <= 19);
        if code == 429 {
            assert_eq!(
                wire.headers.value("retry-after"),
                Some(b"18446744073709551615".as_slice())
            );
        }
        drop(wire);
    }
    assert!(matches!(
        fallback.produce(FallbackStatus::Limited { retry_seconds: 0 }),
        FallbackOutcome::Close
    ));
}

#[test]
fn fallback_startup_requires_connection_count_and_complete_minimum_bytes() {
    assert!(matches!(
        FallbackLedger::checked(fallback_limits(), 2),
        Err(Diagnostic::InvalidLimits)
    ));
    let one = FallbackLedger::checked(fallback_limits(), 1).unwrap();
    let mut insufficient = fallback_limits();
    insufficient.responses = 2;
    insufficient.retained_bytes = one.producer.limits.charge;
    assert!(matches!(
        FallbackLedger::checked(insufficient, 2),
        Err(Diagnostic::InvalidLimits)
    ));
    let mut invalid = fallback_limits();
    invalid.managed_header_bytes = 100;
    assert!(matches!(
        FallbackLedger::checked(invalid, 1),
        Err(Diagnostic::InvalidLimits)
    ));
}

#[test]
fn wire_native_owners_are_send_sync_without_clone_or_raw_map_extraction() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<ChargedNativeHeaders>();
    send_sync::<ChargedWire>();
    send_sync::<FallbackLedger>();
}
