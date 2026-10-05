//! Sealed proof values for an explicitly trusted Rust adapter.
//!
//! These types carry facts asserted by that adapter; they do not implement a
//! credential verifier or a policy engine. Nagi cannot construct, deserialize,
//! clone, or share them. The Rust issuer remains trusted and must validate
//! credentials, expiry, audience, revocation, and policy before minting.
use std::marker::PhantomData;

/// Authentication asserted by a trusted Rust credential verifier.
///
/// The numeric subject is an application identifier, not a secret or a token.
///
/// ```compile_fail
/// let forged = nagi_runtime::auth::Principal { subject: 7 };
/// ```
/// ```compile_fail
/// let proof = nagi_runtime::auth::Principal::from_verified_subject(7);
/// let duplicate = proof.clone();
/// ```
/// ```compile_fail
/// let proof = nagi_runtime::serde_json::from_str::<nagi_runtime::auth::Principal>("{}");
/// ```
pub struct Principal {
    subject: i64,
}

impl Principal {
    /// Trust boundary: the caller asserts that verification has succeeded.
    /// This is intentionally not registered as a Nagi standard operation.
    pub fn from_verified_subject(subject: i64) -> Self {
        Self { subject }
    }

    /// Read the verified identifier inside a trusted adapter.
    /// Returning this number to Nagi does not make it an authentication proof.
    pub fn subject(&self) -> i64 {
        self.subject
    }
}

/// One owned authorization for a nominal permission marker `P` and resource.
///
/// Moving the value into a protected operation prevents Nagi reuse. Ordinary
/// async delegation is allowed; this type has no request lifetime, expiry, or
/// automatic revocation contract. Protected adapters must use the bound
/// resource rather than take a second, independently supplied identifier.
///
/// ```compile_fail
/// let grant = nagi_runtime::auth::Grant::<()>::from_authorized(
///     &nagi_runtime::auth::Principal::from_verified_subject(7), 9);
/// let duplicate = grant.clone();
/// ```
/// ```compile_fail
/// let grant = nagi_runtime::serde_json::from_str::<nagi_runtime::auth::Grant<()>>("{}");
/// ```
pub struct Grant<P> {
    subject: i64,
    resource: i64,
    permission: PhantomData<fn() -> P>,
}

impl<P> Grant<P> {
    /// Trust boundary: the caller asserts that the policy associated with `P`
    /// authorized this principal and resource. A named Nagi policy may make
    /// that decision; the adapter must await its success before calling here.
    /// This method is not a public factory in the Nagi standard module.
    pub fn from_authorized(principal: &Principal, resource: i64) -> Self {
        Self {
            subject: principal.subject,
            resource,
            permission: PhantomData,
        }
    }

    /// Consume the proof at a trusted protected operation's boundary.
    /// The returned pair is `(subject, resource)` for that operation's binds.
    pub fn into_authorized_parts(self) -> (i64, i64) {
        (self.subject, self.resource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Read;

    #[test]
    fn grant_binds_identity_and_resource_without_payload_allocation() {
        let principal = Principal::from_verified_subject(7);
        let grant = Grant::<Read>::from_authorized(&principal, 9);
        assert_eq!(grant.into_authorized_parts(), (7, 9));
        assert_eq!(std::mem::size_of::<Principal>(), std::mem::size_of::<i64>());
        assert_eq!(
            std::mem::size_of::<Grant<Read>>(),
            2 * std::mem::size_of::<i64>()
        );
    }

    #[test]
    fn owned_proof_is_send_sync_without_requiring_marker_payload_traits() {
        struct LocalMarker(std::marker::PhantomData<std::rc::Rc<()>>);
        fn requires_send_sync<T: Send + Sync>() {}
        requires_send_sync::<Principal>();
        requires_send_sync::<Grant<LocalMarker>>();
    }
}
