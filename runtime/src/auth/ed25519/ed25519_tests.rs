//! Owned ephemeral Ed25519 fixtures: no token, key or signature is persisted.
use super::*;
use crate::auth::{AuthScope, CredentialSource, FailureKind, Grant, LeaseOwner};
use aws_lc_rs::signature::{Ed25519KeyPair, KeyPair};
struct Fixture {
    key: Ed25519KeyPair,
    verifier: Arc<Verifier>,
}
fn limits() -> Limits {
    Limits {
        token_bytes: 2048,
        header_bytes: 256,
        claim_bytes: 768,
        depth: 8,
        audiences: 3,
        kid_bytes: 16,
        keys: 2,
        max_ttl: Duration::from_secs(30),
    }
}
fn startup(keys: Vec<(String, Vec<u8>)>) -> Startup {
    Startup {
        issuer: "owned-issuer".into(),
        audience: "owned-audience".into(),
        keys,
        limits: limits(),
    }
}
impl Fixture {
    fn new() -> Self {
        let key = Ed25519KeyPair::generate().unwrap();
        let verifier = Verifier::checked(
            "owned-issuer".into(),
            "owned-audience".into(),
            vec![(
                "primary".into(),
                key.public_key().as_ref().try_into().unwrap(),
            )],
            limits(),
        )
        .ok()
        .unwrap();
        Self { key, verifier }
    }
    fn token(&self, raw_header: &str, raw_claims: &str) -> String {
        // All authentic fixtures use JWT11 public encode: no private JWS framing.
        let values: serde_json::Value = serde_json::from_str(raw_header).unwrap();
        let mut header = jsonwebtoken::Header::new(Algorithm::EdDSA);
        header.typ = None;
        for (name, value) in values.as_object().unwrap() {
            if name != "alg" {
                header.extras.insert(name, value);
            } else if value.as_str() != Some("EdDSA") {
                // HS256 is a deliberately rejected input, never a verifier fallback.
                header.alg = Algorithm::HS256;
            }
        }
        let claims: serde_json::Value = serde_json::from_str(raw_claims).unwrap();
        let document = self.key.to_pkcs8().unwrap();
        let key = if header.alg == Algorithm::EdDSA {
            jsonwebtoken::EncodingKey::from_ed_der(document.as_ref())
        } else {
            jsonwebtoken::EncodingKey::from_secret(b"owned-rejected-algorithm-fixture")
        };
        jsonwebtoken::encode(&header, &claims, &key).unwrap()
    }
    fn raw_preflight(&self, header: &str, claims: &str) -> Result<DecodingKey, Failure> {
        // This is an unauthenticated rejection-only JSON fixture, not signing input.
        let token = format!(
            "{}.{}.{}",
            URL_SAFE_NO_PAD.encode(header),
            URL_SAFE_NO_PAD.encode(claims),
            URL_SAFE_NO_PAD.encode([0_u8; 64])
        );
        self.verifier.preflight(&token).cloned()
    }
    fn valid(&self) -> String {
        self.token(header(), claims())
    }
    fn verify(&self, t: &str) -> Result<VerifiedIdentity, Failure> {
        self.verifier
            .verify_with_clock(t, || Ok(sample(Duration::from_secs(1000))))
    }
    fn rejects(&self, h: &str, c: &str) {
        assert!(self.verify(&self.token(h, c)).is_err());
    }
}
fn header() -> &'static str {
    r#"{"alg":"EdDSA","typ":"JWT","kid":"primary"}"#
}
fn claims() -> &'static str {
    r#"{"exp":1010,"iss":"owned-issuer","aud":"owned-audience","sub":"7"}"#
}
fn sample(utc: Duration) -> ClockSample {
    let t = Instant::now();
    ClockSample {
        before: t,
        after: t,
        utc,
    }
}
#[test]
fn actual_ed25519_preserves_original_deadline_through_short_request_grant() {
    let f = Fixture::new();
    let t = f.valid();
    let live_exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 10;
    let live = f.token(header(), &claims().replace("1010", &live_exp.to_string()));
    assert!(f.verifier.verify(&live).is_ok());
    let before = Instant::now();
    let identity = f.verify(&t).ok().unwrap();
    assert!(
        identity.subject == 7 && matches!(identity.credential.source, CredentialSource::Bearer)
    );
    assert!(
        identity.expires_at >= before + Duration::from_secs(10)
            && identity.expires_at == identity.credential.original_expires_at
    );
    let original = identity.credential.original_expires_at;
    let owner = LeaseOwner::new(Instant::now() + Duration::from_secs(1)).unwrap();
    let scope = AuthScope::bind(identity, Arc::clone(&owner.lease)).unwrap();
    assert!(
        scope.credential.original_expires_at == original
            && owner.lease.checked_gate().unwrap().expires_at < original
    );
    let grant: Grant<()> = Grant::from_authorized(scope, 41).unwrap();
    assert!(
        grant.credential.original_expires_at == original
            && matches!(grant.credential.source, CredentialSource::Bearer)
    );
}
#[test]
fn wrong_key_and_changed_signature_are_rejected() {
    let f = Fixture::new();
    let other = Fixture::new();
    assert!(other.verify(&f.valid()).is_err());
    let mut token = f.valid();
    let at = token.rfind('.').unwrap() + 1;
    let mut signature = URL_SAFE_NO_PAD.decode(&token[at..]).unwrap();
    signature[0] ^= 1;
    token.truncate(at);
    token.push_str(&URL_SAFE_NO_PAD.encode(signature));
    assert!(f.verify(&token).is_err());
}
#[test]
fn decoded_duplicate_unknown_nested_member_is_rejected() {
    let f = Fixture::new();
    let c = r#"{"exp":1010,"iss":"owned-issuer","aud":"owned-audience","sub":"7","unused":{"name":1,"na\u006de":2}}"#;
    assert!(f.raw_preflight(header(), c).is_err());
}
#[test]
fn strict_header_fixed_algorithm_type_key_and_duplicates() {
    let f = Fixture::new();
    for h in [
        r#"{"alg":"HS256"}"#,
        r#"{"alg":"EdDSA","typ":null}"#,
        r#"{"alg":"EdDSA","typ":"other"}"#,
        r#"{"alg":"EdDSA","kid":null}"#,
        r#"{"alg":"EdDSA","kid":"missing"}"#,
        r#"{"alg":"EdDSA","jku":"unused"}"#,
        r#"{"alg":"EdDSA","crit":[]}"#,
        r#"{"alg":"EdDSA","b64":true}"#,
    ] {
        f.rejects(h, claims());
    }
    assert!(f
        .raw_preflight(r#"{"alg":"EdDSA","al\u0067":"EdDSA"}"#, claims())
        .is_err());
    assert!(f.verify(&f.token(r#"{"alg":"EdDSA"}"#, claims())).is_ok());
    let second = Ed25519KeyPair::generate().unwrap();
    let v = Verifier::checked_startup(startup(vec![
        ("primary".into(), f.key.public_key().as_ref().to_vec()),
        ("second".into(), second.public_key().as_ref().to_vec()),
    ]))
    .ok()
    .unwrap();
    assert!(v
        .verify_with_clock(&f.token(r#"{"alg":"EdDSA"}"#, claims()), || Ok(sample(
            Duration::from_secs(1000)
        )))
        .is_err());
    assert!(v
        .verify_with_clock(&f.valid(), || Ok(sample(Duration::from_secs(1000))))
        .is_ok());
}
#[test]
fn required_integer_dates_exact_issuer_and_canonical_subject() {
    let f = Fixture::new();
    for replacement in ["null", "1010.0", "-1", "18446744073709551616"] {
        f.rejects(header(), &claims().replace("1010", replacement));
    }
    for replacement in ["+7", "07", "-0", " 7", "9223372036854775808"] {
        f.rejects(
            header(),
            &claims().replace("\"7\"", &format!("\"{}\"", replacement)),
        );
    }
    for replacement in ["0", "-7", "9223372036854775807", "-9223372036854775808"] {
        assert!(f
            .verify(&f.token(
                header(),
                &claims().replace("\"7\"", &format!("\"{}\"", replacement))
            ))
            .is_ok());
    }
    f.rejects(header(), &claims().replace("owned-issuer", "other-issuer"));
    for required in ["exp", "iss", "aud", "sub"] {
        let mut value: serde_json::Value = serde_json::from_str(claims()).unwrap();
        value.as_object_mut().unwrap().remove(required);
        f.rejects(header(), &serde_json::to_string(&value).unwrap());
    }
    f.rejects(
        header(),
        r#"{"iss":"owned-issuer","aud":"owned-audience","sub":"7"}"#,
    );
    assert!(f
        .raw_preflight(
            header(),
            &format!("{},\"exp\":1010}}", claims().trim_end_matches('}'))
        )
        .is_err());
    for date in ["null", "1001", "1000.0", "-1"] {
        f.rejects(
            header(),
            &format!("{},\"nbf\":{}}}", claims().trim_end_matches('}'), date),
        );
    }
    assert!(f
        .verify(&f.token(
            header(),
            &format!("{},\"nbf\":1000}}", claims().trim_end_matches('}'))
        ))
        .is_ok());
}
#[test]
fn finite_audience_arrays_and_unknown_values() {
    let f = Fixture::new();
    for a in [
        r#"[]"#,
        r#"[null,"owned-audience"]"#,
        r#"["one","two","three","owned-audience"]"#,
        r#""other""#,
        r#"null"#,
    ] {
        f.rejects(header(), &claims().replace("\"owned-audience\"", a));
    }
    assert!(f
        .verify(&f.token(
            header(),
            &claims().replace(
                "\"owned-audience\"",
                r#"["other","owned-audience","third"]"#
            )
        ))
        .is_ok());
    assert!(f
        .verify(&f.token(
            header(),
            &format!(
                "{},\"unused\":[true,null,1.5,{{\"nested\":0}}]}}",
                claims().trim_end_matches('}')
            )
        ))
        .is_ok());
}
#[test]
fn exact_expiry_fractional_remaining_and_leeway_zero() {
    let f = Fixture::new();
    let t = f.valid();
    assert!(f
        .verifier
        .verify_with_clock(&t, || Ok(sample(Duration::from_secs(1010))))
        .is_err());
    let before = Instant::now();
    let i = f
        .verifier
        .verify_with_clock(&t, || Ok(sample(Duration::new(1009, 500_000_000))))
        .ok()
        .unwrap();
    assert!(
        i.expires_at >= before + Duration::from_millis(500)
            && i.expires_at < Instant::now() + Duration::from_millis(501)
    );
    assert!(f
        .verifier
        .verify_with_clock(&t, || Ok(sample(Duration::from_secs(979))))
        .is_err());
    assert!(f
        .verifier
        .verify_with_clock(&t, || Ok(sample(Duration::from_secs(980))))
        .is_ok());
}
#[test]
fn clock_bracket_projection_does_not_extend_and_return_race_expires() {
    let f = Fixture::new();
    let t = f.valid();
    let after = Instant::now();
    let before = after - Duration::from_millis(100);
    let i = f
        .verifier
        .verify_with_clock(&t, || {
            Ok(ClockSample {
                before,
                after,
                utc: Duration::from_secs(1000),
            })
        })
        .ok()
        .unwrap();
    assert!(
        i.expires_at == before + Duration::from_secs(10)
            && i.expires_at < after + Duration::from_secs(10)
    );
    assert!(
        f.verifier
            .verify_with_clock(&t, || Ok(ClockSample {
                before: after,
                after: before,
                utc: Duration::from_secs(1000)
            }))
            .err()
            .unwrap()
            .kind()
            == FailureKind::Unavailable
    );
    let e = f
        .verifier
        .verify_with_clock(&t, || {
            Ok(ClockSample {
                before: after - Duration::from_secs(20),
                after,
                utc: Duration::from_secs(1000),
            })
        })
        .err()
        .unwrap();
    assert!(e.kind() == FailureKind::Expired);
    assert!(
        f.verifier
            .verify_with_clock(&t, || Err(Failure::unavailable()))
            .err()
            .unwrap()
            .kind()
            == FailureKind::Unavailable
    );
}
#[test]
fn encoded_decoded_byte_depth_and_canonical_framing_limits() {
    let f = Fixture::new();
    let token = f.valid();
    let mut s = startup(vec![(
        "primary".into(),
        f.key.public_key().as_ref().to_vec(),
    )]);
    s.limits.token_bytes = token.len();
    s.limits.header_bytes = header().len();
    s.limits.claim_bytes = claims().len();
    let v = Verifier::checked_startup(s).ok().unwrap();
    assert!(v
        .verify_with_clock(&token, || Ok(sample(Duration::from_secs(1000))))
        .is_ok());
    assert!(v
        .verify_with_clock(&(token + "x"), || Ok(sample(Duration::from_secs(1000))))
        .is_err());
    let c = format!("{},\"unused\":0}}", claims().trim_end_matches('}'));
    assert!(v
        .verify_with_clock(&f.token(header(), &c), || Ok(sample(Duration::from_secs(
            1000
        ))))
        .is_err());
    let h = format!("{} ", header());
    let raw = format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(&h),
        URL_SAFE_NO_PAD.encode(claims()),
        URL_SAFE_NO_PAD.encode([0_u8; 64])
    );
    assert!(v.preflight(&raw).is_err());
    let c = format!(
        "{},\"unused\":{}0{}}}",
        claims().trim_end_matches('}'),
        "[".repeat(8),
        "]".repeat(8)
    );
    f.rejects(header(), &c);
    let c = format!(
        "{},\"unused\":{}0{}}}",
        claims().trim_end_matches('}'),
        "[".repeat(7),
        "]".repeat(7)
    );
    assert!(f.verify(&f.token(header(), &c)).is_ok());
    assert!(f
        .raw_preflight(header(), &format!("{} true", claims()))
        .is_err());
    assert!(f.verify(&(f.valid() + ".")).is_err());
    let padded = f.valid().replacen('.', "=.", 1);
    assert!(f.verify(&padded).is_err());
}
#[test]
fn startup_key_and_budget_validation_is_fail_closed() {
    let f = Fixture::new();
    let raw = f.key.public_key().as_ref().to_vec();
    for bytes in [vec![], vec![0; 31], vec![0; 33]] {
        assert!(Verifier::checked_startup(startup(vec![("primary".into(), bytes)])).is_err());
    }
    assert!(Verifier::checked_startup(startup(vec![
        ("primary".into(), raw.clone()),
        ("primary".into(), raw.clone())
    ]))
    .is_err());
    assert!(Verifier::checked_startup(startup(vec![("x".repeat(17), raw.clone())])).is_err());
    assert!(Verifier::checked_startup(startup(vec![])).is_err());
    assert!(Verifier::checked_startup(startup(vec![
        ("one".into(), raw.clone()),
        ("two".into(), raw.clone()),
        ("three".into(), raw.clone()),
    ]))
    .is_err());
    for n in 0..10 {
        let mut s = startup(vec![("primary".into(), raw.clone())]);
        match n {
            0 => s.limits.token_bytes = 0,
            1 => s.limits.header_bytes = 0,
            2 => s.limits.claim_bytes = 0,
            3 => s.limits.depth = 129,
            4 => s.limits.audiences = 0,
            5 => s.limits.kid_bytes = 0,
            6 => s.limits.keys = 0,
            7 => s.limits.token_bytes = usize::MAX,
            8 => s.limits.max_ttl = Duration::ZERO,
            _ => s.limits.max_ttl = Duration::MAX,
        }
        assert!(Verifier::checked_startup(s).is_err());
    }
}
#[test]
fn prepared_config_is_send_sync_and_failures_are_redacted() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Verifier>();
    fn check_future(verifier: Arc<Verifier>, request: crate::http_server::Request) {
        fn require_send<T: Send>(_: T) {}
        require_send(async move { verifier.verify_request(&request) });
    }
    let _: fn(Arc<Verifier>, crate::http_server::Request) = check_future;
    let f = Fixture::new();
    let error = f.verify("not-a-credential").err().unwrap();
    assert_eq!(error.kind(), FailureKind::InvalidCredential);
    assert_eq!(error.message(), "invalid credential");
    assert_eq!(format!("{error:?}"), "Failure { kind: InvalidCredential }");
}
#[test]
fn signed_duplicates_and_unsupported_null_members_cannot_pass() {
    use serde::ser::SerializeMap;
    struct DuplicateClaims;
    impl serde::Serialize for DuplicateClaims {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let mut map = s.serialize_map(Some(5))?;
            map.serialize_entry("exp", &1010_u64)?;
            map.serialize_entry("exp", &1010_u64)?;
            map.serialize_entry("iss", "owned-issuer")?;
            map.serialize_entry("aud", "owned-audience")?;
            map.serialize_entry("sub", "7")?;
            map.end()
        }
    }
    let f = Fixture::new();
    let document = f.key.to_pkcs8().unwrap();
    let key = jsonwebtoken::EncodingKey::from_ed_der(document.as_ref());
    let h = jsonwebtoken::Header::new(Algorithm::EdDSA);
    let token = jsonwebtoken::encode(&h, &DuplicateClaims, &key).unwrap();
    assert!(f.verify(&token).is_err());
    // Direct typed signed-claims parser independently rejects duplicate input.
    assert!(serde_json::from_str::<StrictSignedClaims>(
        r#"{"exp":1010,"exp":1010,"iss":"owned-issuer","aud":"owned-audience","sub":"7"}"#
    )
    .is_err());
    for name in [
        "crit", "jku", "jwk", "x5u", "x5c", "x5t", "x5t#S256", "cty", "b64", "other",
    ] {
        let raw = format!("{{\"alg\":\"EdDSA\",\"{name}\":null}}");
        f.rejects(&raw, claims());
    }
}
