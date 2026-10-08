use std::{sync::Arc, time::{Duration, Instant}};

use nagi_runtime::{auth::{Failure, VerifiedIdentity}, http_server as http};

// Demo/test-only verifier: it compares one configured fixture credential and
// does not verify a production token format, signature, audience or revocation.
pub async fn verify(
    request: http::Request,
    state: Arc<super::State>,
) -> Result<VerifiedIdentity, Failure> {
    let authorization = http::header_text(&request, "authorization")
        .map_err(|_| Failure::invalid_request())?
        .ok_or_else(Failure::invalid_credential)?;
    if authorization != state.authorization {
        return Err(Failure::invalid_credential());
    }
    VerifiedIdentity::from_verified(
        state.subject,
        Instant::now() + Duration::from_secs(60),
    )
}
