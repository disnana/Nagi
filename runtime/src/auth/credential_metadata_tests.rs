//! Private actual-type oracles. Missing metadata is a definition gate, not purpose RED.
use super::*;
use std::time::Duration;
#[test]
fn bearer_original_expiry_survives_short_request_and_grant_move() {
    let now = Instant::now();
    let request = now + Duration::from_secs(5);
    let original = now + Duration::from_secs(60);
    let owner = LeaseOwner::new(request).unwrap();
    let identity = VerifiedIdentity::from_verified(7, original).unwrap();
    assert!(identity.credential.original_expires_at == original);
    let scope = AuthScope::bind(identity, Arc::clone(&owner.lease)).unwrap();
    assert!(scope.lease.checked_gate().unwrap().expires_at == request);
    assert!(scope.credential.original_expires_at == original);
    assert!(matches!(&scope.credential.source, CredentialSource::Bearer));
    let grant = Grant::<()>::from_authorized(scope, 9).unwrap();
    assert!(grant.credential.original_expires_at == original);
    assert!(matches!(&grant.credential.source, CredentialSource::Bearer));
    assert!(grant.lease.checked_gate().unwrap().expires_at == request);
    assert!(
        grant
            .submit(17, |subject, target, value| (subject, target, value))
            .unwrap()
            == (7, 9, 17)
    );
    drop(owner);
}
#[test]
fn earlier_credential_deadline_bounds_long_request_without_extending_authority() {
    let now = Instant::now();
    let request = now + Duration::from_secs(60);
    let original = now + Duration::from_secs(5);
    let owner = LeaseOwner::new(request).unwrap();
    let scope = AuthScope::bind(
        VerifiedIdentity::from_verified(7, original).unwrap(),
        Arc::clone(&owner.lease),
    )
    .unwrap();
    assert!(scope.lease.checked_gate().unwrap().expires_at == original);
    let grant = Grant::<()>::from_authorized(scope, 9).unwrap();
    assert!(grant.credential.original_expires_at == original);
    assert!(grant.lease.checked_gate().unwrap().expires_at == original);
    drop(owner);
    assert!(grant.validate().unwrap_err().kind() == FailureKind::Expired);
}
