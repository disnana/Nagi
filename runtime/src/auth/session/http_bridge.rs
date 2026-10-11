//! Closed Session-to-HTTP identity bridge; no raw credential getter.
use super::Store;
use crate::auth::{Failure, VerifiedIdentity};
use hyper::header::{HeaderMap, AUTHORIZATION, COOKIE};

impl Store {
    pub(crate) async fn lookup_http_identity(
        &self,
        headers: &HeaderMap,
    ) -> Result<VerifiedIdentity, Failure> {
        // The existing CookiePolicy allows at most64 headers. Retain at most65
        // borrowed values: the overflow sentinel is rejected by that same parser
        // before ID decoding/SQL. No header bytes or secrets are copied here.
        if headers.contains_key(AUTHORIZATION) {
            return Err(Failure::invalid_request());
        }
        let values = headers
            .get_all(COOKIE)
            .iter()
            .take(65)
            .map(|value| {
                std::str::from_utf8(value.as_bytes()).map_err(|_| Failure::invalid_request())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let snapshot = self
            .foundation
            .lookup_cookie(&values, false)
            .await
            .map_err(|failure| Failure { kind: failure.kind })?;
        snapshot.into_verified_identity()
    }
}
// Terminal disposal is allowed even after expiry/poison. Always gate -> cell.
// There is no callback, await, raw owner getter or extra strong cell owner.
pub(crate) fn retire_http_delivery(owner: &crate::auth::LeaseOwner) {
    let _gate = match owner.lease.gate.lock() {
        Ok(gate) => gate,
        Err(error) => error.into_inner(),
    };
    owner.delivery.clear();
}
pub(crate) fn take_http_cookie(
    owner: &crate::auth::LeaseOwner,
    seal: super::ResponseSeal,
) -> Result<hyper::header::HeaderValue, ()> {
    let access = owner.checked_delivery(&owner.lease).map_err(|_| ())?;
    access
        .take_applied(seal.0)
        .map(|material| material.into_header())
        .map_err(|_| ())
}

#[cfg(test)]
#[path = "http_bridge_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "delivery_wire_tests.rs"]
mod delivery_tests;
