use super::*;
use serde::ser::SerializeSeq;
use std::io::Write;

fn limits() -> Limits {
    // Deliberately tiny private numbers; not public defaults/support ceilings.
    Limits {
        body_bytes: 16,
        user_header_bytes: 32,
        user_header_count: 2,
        managed_header_bytes: 256,
        managed_header_count: 8,
        serialization_writes: 64,
        live_responses: 1,
        retained_bytes: 4096,
    }
}
fn error(result: Result<Produced, Diagnostic>) -> Diagnostic {
    result.err().expect("expected finite factory refusal")
}
fn attempts(producer: &Producer) -> usize {
    producer
        .allocation_attempts
        .load(std::sync::atomic::Ordering::Relaxed)
}

#[test]
fn known_oversized_input_is_refused_before_factory_allocation() {
    let producer = Producer::checked(limits()).unwrap();
    assert_eq!(error(producer.bytes(&[0; 17])), Diagnostic::BodyLimit);
    assert_eq!(
        error(producer.text("01234567890123456")),
        Diagnostic::BodyLimit
    );
    assert_eq!(attempts(&producer), 0);
    let exact = producer.bytes(&[0; 16]).unwrap();
    assert_eq!(exact.body().len(), 16);
}

#[test]
fn failed_capacity_check_does_not_allocate_or_mutate_existing_buffer() {
    let producer = Producer::checked(limits()).unwrap();
    let first = producer.text("first").unwrap();
    let allocated = attempts(&producer);
    assert_eq!(error(producer.text("second")), Diagnostic::Capacity);
    assert_eq!(attempts(&producer), allocated);
    assert_eq!(first.body(), b"first");
    drop(first);
    assert_eq!(producer.text("next").unwrap().body(), b"next");
}

#[test]
fn json_checks_serialized_expansion_not_input_string_length() {
    let producer = Producer::checked(limits()).unwrap();
    // Input is only three bytes, but JSON escaping plus quotes needs20 bytes.
    assert_eq!(error(producer.json(&"\0\0\0")), Diagnostic::BodyLimit);
    let exact = producer.json(&"01234567890123").unwrap();
    assert_eq!(exact.body(), b"\"01234567890123\"");
}

struct SecretFailure;
impl Serialize for SecretFailure {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("secret-token-and-private-path"))
    }
}
#[test]
fn serializer_failure_has_bounded_static_diagnostic_and_releases_charge() {
    let producer = Producer::checked(limits()).unwrap();
    let refusal = error(producer.json(&SecretFailure));
    assert_eq!(refusal, Diagnostic::Serialization);
    assert_eq!(refusal.message(), "response serialization failed");
    assert!(!refusal.message().contains("secret"));
    assert_eq!(producer.text("healthy").unwrap().body(), b"healthy");
}

struct PanickingSerializer;
impl Serialize for PanickingSerializer {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        panic!("private serializer panic control")
    }
}
#[test]
fn serializer_panic_keeps_unwind_and_releases_factory_charge() {
    let producer = Producer::checked(limits()).unwrap();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = producer.json(&PanickingSerializer);
    }));
    assert!(
        panic.is_err(),
        "arbitrary panic became a business diagnostic"
    );
    assert_eq!(producer.bytes(b"healthy").unwrap().body(), b"healthy");
}

struct ManyValues;
impl Serialize for ManyValues {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(4))?;
        for _ in 0..4 {
            seq.serialize_element(&0)?;
        }
        seq.end()
    }
}
#[test]
fn serializer_sink_work_bound_is_independent_of_body_size() {
    let mut config = limits();
    config.serialization_writes = 1;
    let producer = Producer::checked(config).unwrap();
    assert_eq!(error(producer.json(&ManyValues)), Diagnostic::WorkLimit);
    assert!(producer.bytes(b"healthy").is_ok());
}

#[test]
fn failed_and_empty_writes_cannot_reset_or_evade_work_bound() {
    let mut config = limits();
    config.serialization_writes = 2;
    let producer = Producer::checked(config).unwrap();
    let mut writer = FiniteWriter {
        producer: &producer,
        data: Vec::new(),
        writes: 0,
        failure: None,
    };
    assert_eq!(writer.write(b"").unwrap(), 0);
    assert_eq!(writer.write(b"").unwrap(), 0);
    assert!(writer.write(b"").is_err());
    assert!(writer.write(b"x").is_err());
    assert!(writer.flush().is_err());
    assert_eq!(writer.failure, Some(Diagnostic::WorkLimit));
    assert_eq!(attempts(&producer), 0);
    assert!(writer.data.is_empty());
}

#[test]
fn transport_frame_clone_and_header_owner_both_retain_core_charge() {
    let producer = Producer::checked(limits()).unwrap();
    let response = producer.bytes(b"owned").unwrap();
    let pointer = response.body().as_ptr();
    let (headers, frame) = response.into_parts();
    assert_eq!(pointer, frame.as_ptr());
    let held_frame = frame.clone();
    drop(frame);
    assert_eq!(error(producer.text("blocked")), Diagnostic::Capacity);
    drop(headers);
    assert_eq!(error(producer.text("blocked")), Diagnostic::Capacity);
    assert_eq!(held_frame.as_ref(), b"owned");
    drop(held_frame);
    assert!(producer.text("released").is_ok());

    let (headers, frame) = producer.bytes(b"reverse").unwrap().into_parts();
    drop(frame);
    assert_eq!(error(producer.text("blocked")), Diagnostic::Capacity);
    assert_eq!(Arc::strong_count(&headers.charge), 1);
    drop(headers);
    assert!(producer.text("released").is_ok());
}

#[test]
fn header_count_byte_bounds_are_checked_before_copy_and_failure_preserves_entries() {
    let producer = Producer::checked(limits()).unwrap();
    let mut response = producer.text("body").unwrap();
    response.headers.append(b"X-Test", b"a").unwrap();
    assert_eq!(response.headers.used_bytes, 11);
    assert_eq!(response.headers.entries[0].name, b"X-Test");
    assert_eq!(response.headers.entries[0].value, b"a");
    assert_eq!(
        response.headers.append(b"X-Huge", &[0; 32]),
        Err(Diagnostic::HeaderLimit)
    );
    assert_eq!(response.headers.entries.len(), 1);
    response.headers.append(b"X-Test", b"b").unwrap();
    assert_eq!(
        response.headers.append(b"X", b"c"),
        Err(Diagnostic::HeaderLimit)
    );
    assert_eq!(response.headers.entries.len(), 2);
    assert_eq!(response.headers.used_bytes, 22);
    assert_eq!(response.body(), b"body");
}

#[test]
fn invalid_zero_overflow_and_impossible_total_profiles_fail_before_publish() {
    let mut zero = limits();
    zero.body_bytes = 0;
    assert!(matches!(
        Producer::checked(zero),
        Err(Diagnostic::InvalidLimits)
    ));
    let mut overflow = limits();
    overflow.managed_header_count = isize::MAX as usize;
    assert!(matches!(
        Producer::checked(overflow),
        Err(Diagnostic::InvalidLimits)
    ));
    let mut total = limits();
    total.retained_bytes = 1;
    assert!(matches!(
        Producer::checked(total),
        Err(Diagnostic::InvalidLimits)
    ));
}
