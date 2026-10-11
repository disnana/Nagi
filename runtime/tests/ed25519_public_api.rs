use nagi_runtime::auth::ed25519::{Limits, Verifier};
use nagi_runtime::auth::{AuthScope, Failure, FailureKind, VerifiedIdentity};
use nagi_runtime::http_server::{authenticated_policy, Policy, Request};
use std::{sync::Arc, time::Duration};

type PrepareVerifier =
    fn(String, String, Vec<(String, [u8; 32])>, Limits) -> Result<Arc<Verifier>, Failure>;

fn trusted_named_native_adapter(verifier: Arc<Verifier>) -> Policy<(), AuthScope> {
    authenticated_policy(move |request: Request, _state: Arc<()>| {
        let verifier = Arc::clone(&verifier);
        async move { verifier.verify_request(&request) }
    })
}

#[test]
fn trusted_adapter_facade_has_fixed_key_shape_and_finite_limits() {
    let _: fn(Arc<Verifier>) -> Policy<(), AuthScope> = trusted_named_native_adapter;
    let prepare: PrepareVerifier = Verifier::checked;
    let verify: fn(&Verifier, &Request) -> Result<VerifiedIdentity, Failure> =
        Verifier::verify_request;
    let _: fn(&Verifier, &Request) -> Result<VerifiedIdentity, Failure> = verify;
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Verifier>();

    let limits = Limits {
        token_bytes: 0,
        header_bytes: 256,
        claim_bytes: 768,
        depth: 8,
        audiences: 3,
        kid_bytes: 16,
        keys: 2,
        max_ttl: Duration::from_secs(30),
    };
    let failure = prepare(
        "owned-issuer".into(),
        "owned-audience".into(),
        vec![("primary".into(), [1; 32])],
        limits,
    )
    .err()
    .expect("zero token bound must fail before provider installation");
    assert_eq!(failure.kind(), FailureKind::InvalidRequest);
}
