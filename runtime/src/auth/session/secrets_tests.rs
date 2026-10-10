//! Small owned component oracles. Never print secret values or crate Cookie Debug.
use super::*;
fn fixture_id() -> SessionId {
    SessionId::parse(&URL_SAFE_NO_PAD.encode([7u8; 32]))
        .unwrap_or_else(|_| panic!("fixture rejected"))
}
fn policy() -> CookiePolicy {
    CookiePolicy::new("__Host-owned", SameSite::Strict)
        .unwrap_or_else(|_| panic!("policy rejected"))
}
#[test]
fn entropy_failure_is_unavailable_without_fallback() {
    let r = fill_entropy(|_| Err(getrandom::Error::UNSUPPORTED));
    assert!(matches!(r, Err(FailureKind::Unavailable)));
    let generated = SessionId::generate();
    assert!(
        generated.is_ok(),
        "owned OS entropy call succeeds on this host"
    );
    assert!(generated.map(|id| id.0.len() == 43).unwrap_or(false));
}
#[test]
fn canonical_id_and_digest_have_closed_shape() {
    let id = fixture_id();
    let reparsed = SessionId::parse(&id.0).unwrap_or_else(|_| panic!("roundtrip failed"));
    assert!(id.digest().0 == reparsed.digest().0);
    let expected: [u8; 32] = Sha256::digest(id.0.as_bytes()).into();
    assert!(id.digest().0 == expected);
    let short = URL_SAFE_NO_PAD.encode([7u8; 31]);
    let long = URL_SAFE_NO_PAD.encode([7u8; 33]);
    let mut noncanonical_tail = id.0.clone();
    noncanonical_tail.pop();
    noncanonical_tail.push('d');
    assert!(matches!(
        SessionId::parse(&noncanonical_tail),
        Err(FailureKind::InvalidCredential)
    ));
    let non_ascii = format!("{}é", "a".repeat(41));
    assert!(matches!(
        SessionId::parse(&non_ascii),
        Err(FailureKind::InvalidCredential)
    ));
    let padded = format!("{}=", id.0);
    for value in [
        "",
        short.as_str(),
        long.as_str(),
        padded.as_str(),
        "ordinary-value",
    ] {
        assert!(matches!(
            SessionId::parse(value),
            Err(FailureKind::InvalidCredential)
        ));
    }
}
#[test]
fn cookie_policy_is_host_only_explicit_and_finite() {
    let p = policy();
    assert!(p.name() == "__Host-owned" && p.same_site_code() == 1);
    assert!(CookiePolicy::new("__Host-owned", SameSite::Lax).is_ok());
    for name in ["", "owned", "__Host-", "__Host-owned name"] {
        assert!(matches!(
            CookiePolicy::new(name, SameSite::Strict),
            Err(FailureKind::InvalidRequest)
        ));
    }
    let exact = format!("__Host-{}", "a".repeat(MAX_NAME_BYTES - 7));
    assert!(CookiePolicy::new(&exact, SameSite::Strict).is_ok());
    let larger = format!("{exact}a");
    assert!(matches!(
        CookiePolicy::new(&larger, SameSite::Strict),
        Err(FailureKind::InvalidRequest)
    ));
    assert!(matches!(
        CookiePolicy::new("__Host-owned", SameSite::None),
        Err(FailureKind::InvalidRequest)
    ));
}
#[test]
fn serialized_issuance_and_removal_keep_managed_attributes() {
    let p = policy();
    let delivery = p
        .issue(fixture_id())
        .unwrap_or_else(|_| panic!("issue failed"));
    let serialized = delivery
        .0
        .to_str()
        .unwrap_or_else(|_| panic!("header failed"));
    let c = Cookie::parse(serialized).unwrap_or_else(|_| panic!("cookie failed"));
    assert!(c.name() == "__Host-owned" && c.value().len() == 43);
    assert!(c.secure() == Some(true) && c.http_only() == Some(true));
    assert!(c.path() == Some("/") && c.domain().is_none());
    assert!(c.same_site() == Some(SameSite::Strict));
    assert!(c.max_age().is_none() && c.expires().is_none());
    assert!(serialized.len() <= MAX_SET_COOKIE_BYTES);
    let removal = p.remove().unwrap_or_else(|_| panic!("removal failed"));
    assert!(removal.0.is_sensitive());
    let c = Cookie::parse(removal.0.to_str().unwrap()).unwrap();
    assert!(c.name() == "__Host-owned" && c.value().is_empty());
    assert!(c.max_age() == Some(cookie::time::Duration::ZERO));
    assert!(c
        .expires_datetime()
        .is_some_and(|x| x.unix_timestamp() == 0));
    assert!(c.secure() == Some(true) && c.http_only() == Some(true));
    assert!(c.path() == Some("/") && c.domain().is_none());
    assert!(c.same_site() == Some(SameSite::Strict));
    let name = format!("__Host-{}", "a".repeat(MAX_NAME_BYTES - 7));
    let lax =
        CookiePolicy::new(&name, SameSite::Lax).unwrap_or_else(|_| panic!("lax policy failed"));
    let delivery = lax
        .issue(fixture_id())
        .unwrap_or_else(|_| panic!("lax issue failed"));
    let wire = delivery.0.to_str().unwrap();
    let c = Cookie::parse(wire).unwrap();
    assert!(c.same_site() == Some(SameSite::Lax));
    assert!(wire.len() <= MAX_SET_COOKIE_BYTES && c.name().len() == MAX_NAME_BYTES);
    assert!(lax
        .remove()
        .map(|x| x.0.as_bytes().len() <= MAX_SET_COOKIE_BYTES)
        .unwrap_or(false));
}
#[test]
fn every_pair_is_inspected_and_duplicate_or_mix_is_rejected() {
    let p = policy();
    let raw = format!("{}={}", p.name(), fixture_id().0);
    assert!(p.read(&[raw.as_str()], false).is_ok());
    assert!(matches!(
        p.read(&[raw.as_str(), raw.as_str()], false),
        Err(FailureKind::InvalidRequest)
    ));
    assert!(matches!(
        p.read(&[raw.as_str()], true),
        Err(FailureKind::InvalidRequest)
    ));
    assert!(matches!(
        p.read(&[raw.as_str(), "ordinary-without-equals"], false),
        Err(FailureKind::InvalidRequest)
    ));
    assert!(matches!(
        p.read(&[], false),
        Err(FailureKind::InvalidCredential)
    ));
    assert!(matches!(
        p.read(&["__Host-owned="], false),
        Err(FailureKind::InvalidCredential)
    ));
    assert!(
        matches!(
            p.read(&["__Host-owned=ordinary", "ordinary-without-equals"], false),
            Err(FailureKind::InvalidRequest)
        ),
        "whole pair classification precedes invalid credential"
    );
    let spaced = format!(" ; {raw} ; ");
    assert!(
        p.read(&[spaced.as_str()], false).is_ok(),
        "inherit crate skip/trim semantics"
    );
}
#[test]
fn cookie_input_budgets_are_preflighted_and_inclusive() {
    let p = policy();
    let raw = format!("{}={}", p.name(), fixture_id().0);
    let exact = format!("{}{}", raw, " ".repeat(MAX_COOKIE_BYTES - raw.len()));
    assert!(p.read(&[exact.as_str()], false).is_ok());
    let over = format!("{exact} ");
    assert!(matches!(
        p.read(&[over.as_str()], false),
        Err(FailureKind::InvalidRequest)
    ));
    let exact_pairs = format!("{}{}", "other=x;".repeat(MAX_COOKIE_PAIRS - 1), raw);
    assert!(p.read(&[exact_pairs.as_str()], false).is_ok());
    let over_pairs = format!("other=x;{exact_pairs}");
    assert!(matches!(
        p.read(&[over_pairs.as_str()], false),
        Err(FailureKind::InvalidRequest)
    ));
    let mut headers = vec![""; MAX_COOKIE_HEADERS];
    headers[0] = raw.as_str();
    assert!(p.read(&headers, false).is_ok());
    headers.push("");
    assert!(matches!(
        p.read(&headers, false),
        Err(FailureKind::InvalidRequest)
    ));
}
