//! U2 semantic fixtures. Native/type execution is required after source review.
use super::super::wire::{FallbackLedger, FallbackLimits};
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn limits() -> Limits {
    Limits {
        body_bytes: 64,
        user_header_bytes: 128,
        user_header_count: 4,
        managed_header_bytes: 1024,
        managed_header_count: 16,
        serialization_writes: 64,
        live_responses: 1,
        retained_bytes: 1024 * 1024,
    }
}
fn producer() -> Producer {
    Producer::checked_attached(limits(), 1, 256).unwrap()
}
fn meta(status: StatusCode, head: bool, source: PolicySource) -> ResponseMeta {
    ResponseMeta {
        status,
        kind: BodyKind::Text,
        head,
        source,
        origin: CompletionOrigin::HandlerOk,
    }
}
fn prepared(p: &Producer, status: StatusCode, head: bool) -> PreparedResponse {
    PreparedResponse::new(
        p,
        p.text("body").unwrap(),
        meta(status, head, PolicySource::Public),
    )
    .unwrap()
}
fn fallback_limits(responses: usize) -> FallbackLimits {
    FallbackLimits {
        responses,
        retained_bytes: 1024 * 1024,
        managed_header_bytes: 1024,
        managed_header_count: 16,
    }
}

#[test]
fn security_fallback_catalog_and_last_body_view_keep_finite_credit() {
    use crate::auth::FailureKind;
    let cases = [
        (
            FailureKind::InvalidCredential,
            401,
            b"invalid credential".as_slice(),
        ),
        (FailureKind::Denied, 403, b"permission denied".as_slice()),
        (FailureKind::Expired, 401, b"authority expired".as_slice()),
        (
            FailureKind::InvalidRequest,
            400,
            b"invalid security request".as_slice(),
        ),
        (
            FailureKind::Unavailable,
            503,
            b"security service unavailable".as_slice(),
        ),
        (
            FailureKind::Internal,
            500,
            b"security service failure".as_slice(),
        ),
    ];
    assert_eq!(FallbackStatus::all().len(), 24);
    for (kind, code, body) in cases {
        assert!(FallbackStatus::all()
            .iter()
            .any(|r| matches!(r, FallbackStatus::Security(k) if *k == kind)));
        let reason = FallbackStatus::Security(kind);
        assert_eq!(reason.code(), code);
        assert_eq!(reason.body(), body);
        let planning = FallbackLedger::checked(fallback_limits(1), 1).unwrap();
        let (metadata, charge) = planning.accounting();
        drop(planning);
        let mut config = fallback_limits(1);
        config.retained_bytes = metadata + charge;
        let fallback = FallbackLedger::checked(config, 1).unwrap();
        let (status, wire) = fallback.produce(reason).into_response().unwrap();
        assert_eq!(status, code);
        let (headers, original) = wire.into_parts();
        assert_eq!(original.as_ref(), body);
        let last_view = original.clone();
        drop(headers);
        drop(original);
        let before = fallback.allocation_attempts();
        let (closed, meter) = crate::metrics::measure(|| fallback.produce(reason));
        assert!(closed.is_close());
        assert_eq!(meter.allocations, 0);
        assert_eq!(before, fallback.allocation_attempts());
        drop(last_view);
        let (_, wire) = fallback.produce(reason).into_response().unwrap();
        assert_eq!(wire.into_parts().1.as_ref(), body);
        let mut too_small = fallback_limits(1);
        too_small.retained_bytes = metadata + charge - 1;
        assert!(FallbackLedger::checked(too_small, 1).is_err());
    }
}
fn held(p: &Producer) {
    assert_eq!(p.empty().err(), Some(Diagnostic::Capacity));
}

#[test]
fn rejected_user_append_copies_nothing_and_keeps_prior_entries() {
    let p = producer();
    let mut r = p.empty().unwrap();
    r.append_user_header("x-ok", b"first", false).unwrap();
    for (name, value) in [
        ("bad name", b"x".as_slice()),
        ("x-bad", b"\r\n"),
        ("content-type", b"x"),
        ("Access-Control-X", b"x"),
        ("x-big", &[0; 129]),
    ] {
        let (result, meter) = crate::metrics::measure(|| r.append_user_header(name, value, false));
        assert_eq!(result, Err(Diagnostic::HeaderLimit));
        assert_eq!(meter.allocations, 0);
        assert_eq!(r.headers.entries.len(), 1);
    }
    r.append_user_header("x-ok", b"second", false).unwrap();
    assert_eq!(r.headers.entries.len(), 2);
    assert_eq!(
        r.append_user_header("content-encoding", b"gzip", true),
        Err(Diagnostic::HeaderLimit)
    );
}

#[test]
fn foreign_owner_refused_before_plan_allocation_and_original_refunded() {
    let a = producer();
    let b = producer();
    let produced = a.text("body").unwrap();
    let (result, meter) = crate::metrics::measure(|| {
        PreparedResponse::new(
            &b,
            produced,
            meta(StatusCode::OK, false, PolicySource::Public),
        )
    });
    assert_eq!(result.err(), Some(Diagnostic::Fault));
    assert_eq!(meter.allocations, 0);
    assert!(a.empty().is_ok());
    assert!(b.empty().is_ok());
    let r = prepared(&a, StatusCode::OK, false).finish().unwrap();
    assert!(r.is_owned_by(&a));
}

#[test]
fn ordinary_s_plus_q_exact_boundary_does_not_require_l_times_q() {
    let p = producer();
    let s = p.metadata_bytes();
    let q = p.response_charge();
    drop(p);
    let mut config = limits();
    config.live_responses = 100;
    config.retained_bytes = s + q;
    let exact = Producer::checked_attached(config, 1, 256).unwrap();
    let first = exact.empty().unwrap();
    held(&exact);
    drop(first);
    assert!(exact.empty().is_ok());
    let mut config = limits();
    config.retained_bytes = s + q - 1;
    assert_eq!(
        Producer::checked_attached(config, 1, 256).err(),
        Some(Diagnostic::InvalidLimits)
    );
    assert_eq!(
        Producer::checked_attached(limits(), usize::MAX, 256).err(),
        Some(Diagnostic::InvalidLimits)
    );
    let mut config = limits();
    config.managed_header_count = 1;
    assert_eq!(
        Producer::checked_attached(config, 1, 256).err(),
        Some(Diagnostic::InvalidLimits)
    );
}

#[test]
fn head_and_bodyless_statuses_preserve_header_claim_and_security() {
    for status in [
        StatusCode::OK,
        StatusCode::NO_CONTENT,
        StatusCode::RESET_CONTENT,
        StatusCode::NOT_MODIFIED,
    ] {
        let p = producer();
        let mut r = p.text("body").unwrap();
        r.append_user_header("content-encoding", b"gzip", false)
            .unwrap();
        let wire = PreparedResponse::new(&p, r, meta(status, true, PolicySource::Public))
            .unwrap()
            .finish()
            .unwrap();
        let (headers, body) = wire.into_parts();
        assert!(body.is_empty());
        drop(body);
        held(&p);
        assert_eq!(
            headers.value("x-content-type-options"),
            Some(b"nosniff".as_slice())
        );
        assert_eq!(
            headers.value("content-security-policy"),
            Some(CONTENT_SECURITY_POLICY.as_bytes())
        );
        if status == StatusCode::OK {
            assert_eq!(headers.value("content-length"), Some(b"4".as_slice()));
            assert_eq!(
                headers.value("content-type"),
                Some(b"text/plain; charset=utf-8".as_slice())
            );
        } else {
            assert_eq!(headers.value("content-type"), None);
            assert_eq!(headers.value("content-encoding"), None);
            assert_eq!(
                headers.value("content-length"),
                if status == StatusCode::RESET_CONTENT {
                    Some(b"0".as_slice())
                } else {
                    None
                }
            );
        }
        drop(headers);
        assert!(p.empty().is_ok());
    }
}

#[test]
fn auth_304_and_html_encoding_are_invariants_before_conversion() {
    let p = producer();
    assert_eq!(
        PreparedResponse::new(
            &p,
            p.empty().unwrap(),
            meta(StatusCode::NOT_MODIFIED, false, PolicySource::Session)
        )
        .err(),
        Some(Diagnostic::Fault)
    );
    let mut r = p.empty().unwrap();
    r.headers.append(b"content-encoding", b"gzip").unwrap();
    let mut m = meta(StatusCode::OK, false, PolicySource::Public);
    m.kind = BodyKind::Html;
    assert_eq!(
        PreparedResponse::new(&p, r, m).err(),
        Some(Diagnostic::Fault)
    );

    // A real typed document is already encoded once; the producer must copy
    // those exact bytes and refuse a smaller body budget before allocation.
    let policy = crate::html::policy("https://example.invalid").unwrap();
    let fragment = crate::html::text(&policy, "<&").unwrap();
    let document = crate::html::document(&policy, "t<&", fragment).unwrap();
    let expected = b"<!doctype html><html><head><meta charset=\"utf-8\"><title>t&lt;&amp;</title></head><body>&lt;&amp;</body></html>";
    let (refused, meter) = crate::metrics::measure(|| p.html(&document));
    assert_eq!(refused.err(), Some(Diagnostic::BodyLimit));
    assert_eq!(meter.allocations, 0);
    assert_eq!(meter.allocated_bytes, 0);
    assert!(p.empty().is_ok());
    let mut exact_limits = limits();
    exact_limits.body_bytes = expected.len();
    let exact = Producer::checked_attached(exact_limits, 1, 256).unwrap();
    let mut html_meta = meta(StatusCode::OK, false, PolicySource::Public);
    html_meta.kind = BodyKind::Html;
    let wire = PreparedResponse::new(&exact, exact.html(&document).unwrap(), html_meta)
        .unwrap()
        .finish()
        .unwrap();
    let (headers, body) = wire.into_parts();
    assert_eq!(body.as_ref(), expected);
    assert_eq!(
        headers.value("content-type"),
        Some(b"text/html; charset=utf-8".as_slice())
    );
    assert_eq!(
        headers.value("content-security-policy"),
        Some(CONTENT_SECURITY_POLICY.as_bytes())
    );
    assert_eq!(headers.value("content-encoding"), None);
    let last = hyper::body::Frame::data(body.clone());
    drop(body);
    drop(headers);
    held(&exact);
    drop(last);
    assert!(exact.empty().is_ok());
}

#[test]
fn finite_vary_union_cors_allow_and_preflight_are_planned_once() {
    let p = producer();
    let mut r = prepared(&p, StatusCode::NO_CONTENT, false);
    r.cors(
        Some(HeaderValue::from_static("https://example.invalid")),
        true,
    )
    .unwrap();
    r.allow(b"GET, HEAD").unwrap();
    r.preflight(b"GET", Some(b"X-Test"), Some(b"60")).unwrap();
    r.vary(VarySet::AUTH);
    r.vary(VarySet::AUTH);
    let wire = r.finish().unwrap();
    let (h, b) = wire.into_parts();
    assert!(b.is_empty());
    assert_eq!(h.value("allow"), Some(b"GET, HEAD".as_slice()));
    assert_eq!(
        h.value("access-control-allow-credentials"),
        Some(b"true".as_slice())
    );
    assert_eq!(
        h.value("access-control-allow-methods"),
        Some(b"GET".as_slice())
    );
    assert_eq!(h.value("cache-control"), Some(b"no-store".as_slice()));
    assert_eq!(h.value("vary"),Some(b"Access-Control-Request-Headers, Access-Control-Request-Method, Authorization, Cookie, Origin".as_slice()));
    assert_eq!(h.value("content-type"), None);
}

#[test]
fn complete_plan_refuses_before_native_conversion_and_preserves_peak_credit() {
    let p = producer();
    let mut r = prepared(&p, StatusCode::OK, false);
    r.vary(VarySet::BROWSER);
    r.plan.byte_limit = r.plan.bytes;
    let (result, meter) = crate::metrics::measure(|| r.finish());
    assert_eq!(result.err(), Some(Diagnostic::HeaderLimit));
    assert_eq!(meter.allocations, 0);
    let mut r = prepared(&p, StatusCode::OK, false);
    r.vary(VarySet::BROWSER);
    let peak = r.produced.wire.unwrap().peak_bytes;
    let (wire, meter) = crate::metrics::measure(|| r.finish().unwrap());
    assert!(meter.allocations > 0);
    assert!(usize::try_from(meter.allocated_bytes).unwrap() <= peak);
    assert_eq!(
        wire.into_parts().0.value("vary"),
        Some(b"Authorization, Content-Type, Cookie, Origin, X-CSRF-Token".as_slice())
    );
}

#[test]
fn fallback_sf_plus_c_qf_and_f_ge_c_are_independent_exact_boundaries() {
    let f = FallbackLedger::checked(fallback_limits(2), 2).unwrap();
    let (s, q) = f.accounting();
    drop(f);
    let mut config = fallback_limits(20);
    config.retained_bytes = s + 2 * q;
    let exact = FallbackLedger::checked(config, 2).unwrap();
    let one = exact
        .produce(FallbackStatus::Forbidden)
        .into_response()
        .unwrap();
    let two = exact
        .produce(FallbackStatus::Unauthorized)
        .into_response()
        .unwrap();
    let before = exact.allocation_attempts();
    let (closed, meter) = crate::metrics::measure(|| exact.produce(FallbackStatus::Capacity));
    assert!(closed.is_close());
    assert_eq!(meter.allocations, 0);
    assert_eq!(before, exact.allocation_attempts());
    drop(one);
    drop(two);
    assert!(!exact.produce(FallbackStatus::Internal).is_close());
    let mut config = fallback_limits(2);
    config.retained_bytes = s + 2 * q - 1;
    assert_eq!(
        FallbackLedger::checked(config, 2)
            .err()
            .map(|_| Diagnostic::InvalidLimits),
        Some(Diagnostic::InvalidLimits)
    );
    assert!(FallbackLedger::checked(fallback_limits(1), 2).is_err());
}

#[test]
fn every_terminal_reason_keeps_status_and_retry_zero_closes_without_allocation() {
    let f = FallbackLedger::checked(fallback_limits(1), 1).unwrap();
    for reason in FallbackStatus::all() {
        let (status, wire) = f
            .terminal(f.intent(reason).connection_close(true))
            .into_response()
            .unwrap();
        assert_eq!(status, reason.code());
        let (h, b) = wire.into_parts();
        assert_eq!(b.as_ref(), reason.body());
        assert_eq!(h.value("connection"), Some(b"close".as_slice()));
        if matches!(reason, FallbackStatus::Limited { .. }) {
            assert_eq!(
                h.value("retry-after"),
                Some(b"18446744073709551615".as_slice())
            );
        }
    }
    let (closed, meter) =
        crate::metrics::measure(|| f.produce(FallbackStatus::Limited { retry_seconds: 0 }));
    assert!(closed.is_close());
    assert_eq!(meter.allocations, 0);
}

#[test]
fn original_auth_refusal_deletion_and_cancellation_survive_fallback() {
    let f = FallbackLedger::checked(fallback_limits(1), 1).unwrap();
    for reason in [FallbackStatus::Unauthorized, FallbackStatus::Forbidden] {
        let intent = f
            .intent(reason)
            .context(PolicySource::Session, false, VarySet::GUARDED)
            .consumed_deletion(HeaderValue::from_static("session=; Max-Age=0"))
            .unwrap()
            .cors(HeaderValue::from_static("https://example.invalid"), true)
            .unwrap()
            .allow(HeaderValue::from_static("GET, HEAD"))
            .unwrap();
        let (status, wire) = f.terminal(intent).into_response().unwrap();
        assert_eq!(status, reason.code());
        let (h, _) = wire.into_parts();
        assert_eq!(
            h.value("set-cookie"),
            Some(b"session=; Max-Age=0".as_slice())
        );
        assert_eq!(
            h.value("www-authenticate"),
            if status == 401 {
                Some(b"Session".as_slice())
            } else {
                None
            }
        );
        assert_eq!(
            h.value("access-control-allow-origin"),
            Some(b"https://example.invalid".as_slice())
        );
        assert_eq!(
            h.value("access-control-allow-credentials"),
            Some(b"true".as_slice())
        );
        assert_eq!(h.value("allow"), Some(b"GET, HEAD".as_slice()));
        assert_eq!(
            h.value("vary"),
            Some(b"Authorization, Cookie, Origin, X-CSRF-Token".as_slice())
        );
    }
    let intent = f
        .intent(FallbackStatus::Capacity)
        .context(PolicySource::Session, true, VarySet::AUTH)
        .cancellation(FallbackStatus::Deadline);
    let (status, wire) = f.terminal(intent).into_response().unwrap();
    assert_eq!(status, 504);
    assert!(wire.into_parts().1.is_empty());
}

struct SecretOwner {
    bytes: [u8; 60],
    drops: Arc<AtomicUsize>,
}
impl AsRef<[u8]> for SecretOwner {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}
impl Drop for SecretOwner {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
fn secret(drops: &Arc<AtomicUsize>) -> Bytes {
    Bytes::from_owner(SecretOwner {
        bytes: [b'x'; 60],
        drops: Arc::clone(drops),
    })
}

#[test]
fn fixed_body_preserves_original_bytes_pointer_until_last_frame_owner() {
    let p = producer();
    let drops = Arc::new(AtomicUsize::new(0));
    let body = secret(&drops);
    let pointer = body.as_ptr();
    let ticket = FrameworkTicket::reserve(&p, FrameworkBody::Csrf, None)
        .unwrap()
        .cors(
            Some(HeaderValue::from_static("https://example.invalid")),
            true,
        )
        .unwrap()
        .dependencies(VarySet::GUARDED)
        .unwrap();
    let prepared = ticket.adopt(body).unwrap();
    let (headers, body) = prepared.finish().unwrap().into_parts();
    assert_eq!(body.as_ptr(), pointer);
    assert_eq!(headers.value("content-length"), Some(b"60".as_slice()));
    assert_eq!(
        headers.value("access-control-allow-origin"),
        Some(b"https://example.invalid".as_slice())
    );
    assert_eq!(
        headers.value("access-control-allow-credentials"),
        Some(b"true".as_slice())
    );
    assert_eq!(
        headers.value("vary"),
        Some(b"Authorization, Cookie, Origin, X-CSRF-Token".as_slice())
    );
    let last = hyper::body::Frame::data(body.clone());
    drop(body);
    drop(headers);
    held(&p);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(last);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(p.empty().is_ok());
}

#[test]
fn reserve_before_fake_take_failure_never_reissues_and_fallback_is_independent() {
    let p = producer();
    let take = AtomicUsize::new(0);
    let live = p.empty().unwrap();
    let result = FrameworkTicket::reserve(&p, FrameworkBody::Csrf, None).inspect(|_| {
        take.fetch_add(1, Ordering::SeqCst);
    });
    assert_eq!(result.err(), Some(Diagnostic::Capacity));
    assert_eq!(take.load(Ordering::SeqCst), 0);
    let f = FallbackLedger::checked(fallback_limits(1), 1).unwrap();
    let held = f.produce(FallbackStatus::Forbidden);
    let (closed, meter) = crate::metrics::measure(|| f.produce(FallbackStatus::Capacity));
    assert!(closed.is_close());
    assert_eq!(meter.allocations, 0);
    assert_eq!(take.load(Ordering::SeqCst), 0);
    drop(held);
    drop(live);
    let mut config = limits();
    config.body_bytes = 59;
    let too_small = Producer::checked_attached(config, 1, 256).unwrap();
    assert_eq!(
        FrameworkTicket::reserve(&too_small, FrameworkBody::Csrf, None).err(),
        Some(Diagnostic::BodyLimit)
    );
}

#[test]
fn prelogin_owned_cookie_moves_once_without_secret_copy() {
    let p = producer();
    let drops = Arc::new(AtomicUsize::new(0));
    let ticket = FrameworkTicket::reserve(&p, FrameworkBody::Prelogin, Some(512)).unwrap();
    let slot = ticket.cookie.unwrap();
    let mut prepared = ticket.adopt(secret(&drops)).unwrap();
    let bytes = Bytes::from_static(b"prelogin=opaque; HttpOnly");
    let pointer = bytes.as_ptr();
    let mut cookie = HeaderValue::from_maybe_shared(bytes).unwrap();
    cookie.set_sensitive(true);
    prepared.plan.fill_cookie(slot, cookie).unwrap();
    let (headers, body) = prepared.finish().unwrap().into_parts();
    assert_eq!(headers.value("set-cookie").unwrap().as_ptr(), pointer);
    drop(body);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    held(&p);
    drop(headers);
    assert!(p.empty().is_ok());
}

#[test]
fn private_finalizer_and_framework_ticket_keep_send_sync_layout() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<PreparedResponse>();
    send_sync::<FrameworkTicket>();
    send_sync::<ChargedWire>();
}

#[test]
fn hidden_vec_capacity_and_empty_work_are_checked_before_copy_or_headers() {
    use std::io::Write;
    let p = producer();
    let data = Vec::with_capacity(65);
    let charge = p.reserve().unwrap();
    let (result, meter) = crate::metrics::measure(|| p.finish(data, charge));
    assert_eq!(result.err(), Some(Diagnostic::Allocation));
    assert_eq!(meter.allocations, 0);
    let mut writer = FiniteWriter {
        producer: &p,
        data: Vec::with_capacity(65),
        writes: 0,
        failure: None,
    };
    assert!(writer.write(b"secret").is_err());
    assert_eq!(writer.failure, Some(Diagnostic::Allocation));
    assert!(writer.data.is_empty());
    let mut writer = FiniteWriter {
        producer: &p,
        data: Vec::new(),
        writes: 64,
        failure: None,
    };
    assert!(writer.write(b"").is_err());
    assert_eq!(writer.failure, Some(Diagnostic::WorkLimit));
    assert!(writer.data.is_empty());
}

#[test]
fn fallback_sf_includes_actual_quota_receipt_and_refuses_one_byte_over_budget() {
    let c = 2;
    let config = fallback_limits(c);
    let expected_sf = size_of::<FallbackLedger>()
        + c * (size_of::<TerminalIntent>() + config.managed_header_bytes)
        + size_of::<Producer>()
        + 2 * size_of::<usize>()
        + Quota::retained_catalog_minimum(2).unwrap();
    let initial = FallbackLedger::checked(config, c).unwrap();
    let (sf, qf) = initial.accounting();
    // Independent fixed-layout sum: reading Sf from accounting alone would
    // accept an implementation mutant that omitted the Quota receipt entirely.
    assert_eq!(sf, expected_sf);
    drop(initial);
    let mut exact = fallback_limits(c);
    exact.retained_bytes = expected_sf + c * qf;
    assert!(FallbackLedger::checked(exact, c).is_ok());
    let mut short = fallback_limits(c);
    short.retained_bytes = expected_sf + c * qf - 1;
    let (result, meter) = crate::metrics::measure(|| FallbackLedger::checked(short, c));
    assert_eq!(result.err(), Some(Diagnostic::InvalidLimits));
    assert_eq!(meter.allocations, 0);
    assert_eq!(meter.allocated_bytes, 0);
    let mut omitted = fallback_limits(c);
    omitted.retained_bytes = expected_sf - Quota::retained_catalog_minimum(2).unwrap() + c * qf;
    let (result, meter) = crate::metrics::measure(|| FallbackLedger::checked(omitted, c));
    assert_eq!(result.err(), Some(Diagnostic::InvalidLimits));
    assert_eq!(meter.allocations, 0);
}
