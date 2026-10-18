//! Request-bound proofs for trusted credential and policy adapters.
//! Verifier assertions remain trusted; only the dispatcher creates leases.
#[path = "auth/csrf.rs"]
pub mod csrf;
#[path = "auth/session.rs"]
pub mod session;
#[cfg(nagi_auth_stage_diag)]
#[path = "auth/session_deadline_observer.rs"]
pub(crate) mod session_deadline_observer;
#[cfg(all(test, nagi_auth_stage_diag))]
#[path = "auth/auth_stage_observer.rs"]
mod stage_observer;
// Opt-in private failure witness for integration diagnostics; absent in stock builds.
#[cfg(nagi_browser_login_diag)]
#[path = "auth/browser_login_observer.rs"]
pub(crate) mod browser_login_observer;
macro_rules! browser_observe {
    ($phase:ident, $future:expr) => {{
        #[cfg(nagi_browser_login_diag)]
        {
            crate::auth::browser_login_observer::observe(
                crate::auth::browser_login_observer::Phase::$phase,
                $future,
            )
            .await
        }
        #[cfg(not(nagi_browser_login_diag))]
        {
            $future.await
        }
    }};
}
#[cfg(nagi_browser_login_diag)]
pub(crate) use browser_observe;
use std::{
    marker::PhantomData,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    InvalidCredential,
    Denied,
    Expired,
    InvalidRequest,
    Unavailable,
    Internal,
}
impl FailureKind {
    pub const INVALID_CREDENTIAL: Self = Self::InvalidCredential;
    pub const DENIED: Self = Self::Denied;
    pub const EXPIRED: Self = Self::Expired;
    pub const INVALID_REQUEST: Self = Self::InvalidRequest;
    pub const UNAVAILABLE: Self = Self::Unavailable;
    pub const INTERNAL: Self = Self::Internal;
    pub(crate) const fn message(self) -> &'static str {
        match self {
            Self::InvalidCredential => "invalid credential",
            Self::Denied => "permission denied",
            Self::Expired => "authority expired",
            Self::InvalidRequest => "invalid security request",
            Self::Unavailable => "security service unavailable",
            Self::Internal => "security service failure",
        }
    }
}
/// Stable public codes/messages, without credentials or internal causes.
#[derive(Debug)]
pub struct Failure {
    kind: FailureKind,
}
impl Failure {
    pub fn kind(&self) -> FailureKind {
        self.kind
    }
    pub fn message(&self) -> &str {
        self.kind.message()
    }
    pub fn invalid_credential() -> Self {
        Self {
            kind: FailureKind::InvalidCredential,
        }
    }
    pub fn denied() -> Self {
        Self {
            kind: FailureKind::Denied,
        }
    }
    pub fn expired() -> Self {
        Self {
            kind: FailureKind::Expired,
        }
    }
    pub fn invalid_request() -> Self {
        Self {
            kind: FailureKind::InvalidRequest,
        }
    }
    pub fn unavailable() -> Self {
        Self {
            kind: FailureKind::Unavailable,
        }
    }
    pub fn internal() -> Self {
        Self {
            kind: FailureKind::Internal,
        }
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}
impl std::error::Error for Failure {}

// Private immutable credential data. Neither the source nor the original deadline
// is a new proof or public conversion. The live request lease remains separate.
struct CredentialMetadata {
    original_expires_at: Instant,
    source: CredentialSource,
}
enum CredentialSource {
    Bearer,
    Session(session::Snapshot),
}
impl CredentialMetadata {
    // Admission keeps the original credential ceiling even when the request
    // lease has a shorter budget. No clock/authority extension or public getter.
    fn validate(&self) -> Result<(), Failure> {
        let now = Instant::now();
        if self.original_expires_at <= now {
            return Err(Failure::expired());
        }
        if let CredentialSource::Session(snapshot) = &self.source {
            snapshot.validate_admission(self.original_expires_at, now)?;
        }
        Ok(())
    }
}

/// Trusted verifier output, not an authorization proof. No Nagi factory.
pub struct VerifiedIdentity {
    pending_http_touch: Option<session::HttpTouch>,
    subject: i64,
    expires_at: Instant,
    credential: CredentialMetadata,
}
impl VerifiedIdentity {
    /// The adapter must first verify credentials, audience, expiry and revocation.
    /// This value alone cannot create a request lease or authorize an operation.
    pub fn from_verified(subject: i64, expires_at: Instant) -> Result<Self, Failure> {
        if expires_at <= Instant::now() {
            return Err(Failure::expired());
        }
        Ok(Self {
            pending_http_touch: None,
            subject,
            expires_at,
            credential: CredentialMetadata {
                original_expires_at: expires_at,
                source: CredentialSource::Bearer,
            },
        })
    }
}
struct Gate {
    // None belongs to existing trusted non-HTTP admission. Only the HTTP
    // dispatcher can create Pending; only its post-body rate gate admits it.
    http_admitted: Option<bool>,
    subject: Option<i64>,
    http_touch: Option<session::HttpTouch>,
    active: bool,
    expires_at: Instant,
}
pub(crate) struct Lease {
    gate: Mutex<Gate>,
    id: u64,
    delivery: session::delivery::DeliveryWeak,
}
pub(crate) struct LeaseOwner {
    pub(crate) lease: Arc<Lease>,
    delivery: session::delivery::DeliveryOwner,
}
static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);
impl LeaseOwner {
    pub(crate) fn new(expires_at: Instant) -> Result<Self, Failure> {
        let mut id = NEXT_REQUEST.load(Ordering::Relaxed);
        loop {
            let next = id.checked_add(1).ok_or_else(Failure::unavailable)?;
            match NEXT_REQUEST.compare_exchange_weak(id, next, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(actual) => id = actual,
            }
        }
        if expires_at <= Instant::now() {
            return Err(Failure::expired());
        }
        let delivery =
            session::delivery::DeliveryOwner::new(id).map_err(|_| Failure::unavailable())?;
        let weak = delivery.downgrade();
        Ok(Self {
            lease: Arc::new(Lease {
                gate: Mutex::new(Gate {
                    http_admitted: None,
                    subject: None,
                    http_touch: None,
                    active: true,
                    expires_at,
                }),
                id,
                delivery: weak,
            }),
            delivery,
        })
    }
    pub(crate) fn new_http(expires_at: Instant) -> Result<Self, Failure> {
        let owner = Self::new(expires_at)?;
        owner.lease.checked_gate()?.http_admitted = Some(false);
        Ok(owner)
    }
    // Closed synchronous operations only; this is not a host callback gateway.
    fn checked_delivery<'a>(
        &'a self,
        lease: &'a Arc<Lease>,
    ) -> Result<session::delivery::CheckedDelivery<'a>, Failure> {
        session::delivery::CheckedDelivery::for_lease(self, lease)
    }
}
impl Drop for LeaseOwner {
    fn drop(&mut self) {
        let mut gate = match self.lease.gate.lock() {
            Ok(value) => value,
            Err(error) => error.into_inner(),
        };
        gate.active = false;
        let pending_touch = gate.http_touch.take();
        // Keep the gate held through secret disposal: gate -> cell, no await.
        self.delivery.clear();
        drop(gate);
        drop(pending_touch);
    }
}
impl Lease {
    pub(crate) fn rate_subject(&self) -> Result<i64, Failure> {
        self.checked_gate()?.subject.ok_or_else(Failure::internal)
    }
    // Called only after the request's rate/CSRF/body gates have succeeded. No
    // callback or await holds Gate; HttpTouch rechecks after native capacity.
    pub(crate) async fn finish_http_auth(self: &Arc<Self>) -> Result<(), Failure> {
        let touch = { self.checked_gate()?.http_touch.take() };
        if let Some(touch) = touch {
            touch.apply(Arc::clone(self)).await?;
        }
        // Recheck original expiry/active owner under the same gate that guards
        // actual Session delivery. A late DB reply cannot resurrect admission.
        let mut gate = self.checked_gate()?;
        if gate.http_admitted == Some(true) {
            return Err(Failure::invalid_request());
        }
        if gate.http_admitted == Some(false) {
            gate.http_admitted = Some(true);
        }
        Ok(())
    }

    fn checked_session_gate(&self) -> Result<std::sync::MutexGuard<'_, Gate>, Failure> {
        let gate = self.checked_gate()?;
        if gate.http_admitted == Some(false) {
            return Err(Failure::denied());
        }
        Ok(gate)
    }

    pub(crate) fn validate(&self) -> Result<(), Failure> {
        drop(self.checked_gate()?);
        Ok(())
    }
    fn checked_gate(&self) -> Result<std::sync::MutexGuard<'_, Gate>, Failure> {
        let gate = self.gate.lock().map_err(|_| Failure::internal())?;
        if !gate.active || gate.expires_at <= Instant::now() {
            return Err(Failure::expired());
        }
        Ok(gate)
    }
}
// Conservative credit for actual per-request lease layout/control, including
// new subject/deferred-touch state. Existing host-captured heap is not measured.
pub(crate) fn rate_metadata_bytes(requests: usize) -> Option<usize> {
    std::mem::size_of::<Lease>()
        // The verified output temporarily owns deferred metadata before bind
        // moves it into Gate. Credit both complete actual layouts conservatively.
        .checked_add(std::mem::size_of::<VerifiedIdentity>())?
        .checked_add(2 * std::mem::size_of::<usize>())?
        .checked_mul(requests)
        .filter(|n| *n <= isize::MAX as usize)
}
/// Same-task, non-cloneable identity bound to a live dispatcher-owned request.
///
/// ```compile_fail
/// let scope = nagi_runtime::auth::AuthScope::from_verified_subject(7);
/// ```
pub struct AuthScope {
    subject: i64,
    lease: Arc<Lease>,
    credential: CredentialMetadata,
}
impl AuthScope {
    pub(crate) fn bind(mut identity: VerifiedIdentity, lease: Arc<Lease>) -> Result<Self, Failure> {
        let retired_touch = {
            let mut gate = lease.checked_gate()?;
            if identity.expires_at <= Instant::now() {
                return Err(Failure::expired());
            }
            if gate.http_admitted.is_some() && gate.subject.is_some() {
                return Err(Failure::invalid_request());
            }
            gate.expires_at = gate.expires_at.min(identity.expires_at);
            gate.subject = Some(identity.subject);
            let retired = gate.http_touch.take();
            gate.http_touch = identity.pending_http_touch.take();
            retired
        };
        // Existing trusted non-HTTP rebinding remains available; any old Store
        // owner must be disposed after the short authority lock is released.
        drop(retired_touch);
        Ok(Self {
            subject: identity.subject,
            lease,
            credential: identity.credential,
        })
    }
    pub fn subject(&self) -> i64 {
        self.subject
    }
    pub(crate) fn belongs_to(&self, lease: &Arc<Lease>) -> bool {
        self.lease.id == lease.id && Arc::ptr_eq(&self.lease, lease)
    }
    pub(crate) fn validate(&self) -> Result<(), Failure> {
        drop(self.lease.checked_gate()?);
        Ok(())
    }
}
/// A single operation's nominal permission and actual target, with request lease.
///
/// ```compile_fail
/// fn duplicate<P>(value:nagi_runtime::auth::Grant<P>) { let _=value.clone(); }
/// ```
/// ```compile_fail
/// fn unchecked<P>(value:nagi_runtime::auth::Grant<P>) { let _=value.into_authorized_parts(); }
/// ```
pub struct Grant<P> {
    subject: i64,
    resource: i64,
    lease: Arc<Lease>,
    credential: CredentialMetadata,
    permission: PhantomData<fn() -> P>,
}
struct ExecutionPermit<P, R> {
    subject: i64,
    resource: i64,
    reservation: R,
    permission: PhantomData<fn() -> P>,
}
impl<P> Grant<P> {
    /// Trusted boundary: call only after the named policy for P succeeded.
    /// Consumes the live scope. It cannot mint or prolong a lease.
    pub fn from_authorized(scope: AuthScope, resource: i64) -> Result<Self, Failure> {
        scope.validate()?;
        Ok(Self {
            subject: scope.subject,
            resource,
            lease: scope.lease,
            credential: scope.credential,
            permission: PhantomData,
        })
    }
    pub(crate) fn belongs_to(&self, lease: &Arc<Lease>) -> bool {
        self.lease.id == lease.id && Arc::ptr_eq(&self.lease, lease)
    }
    pub(crate) fn validate(&self) -> Result<(), Failure> {
        let _gate = self.lease.checked_gate()?;
        self.credential.validate()
    }
    /// Call AFTER awaiting bounded native capacity. Issuance under the lease gate
    /// is admission; merely reserving a slot is not. The reservation is consumed.
    /// The trusted callback synchronously submits the bound operation to that
    /// reservation, using the supplied target. It must not return deferred work
    /// as a substitute for submission. Arbitrary Rust callback code is not proven.
    /// After issuance, later request cancellation does not roll back admitted I/O.
    pub fn submit<R, T>(
        self,
        reservation: R,
        submit: impl FnOnce(i64, i64, R) -> T,
    ) -> Result<T, Failure> {
        let permit = {
            let _gate = self.lease.checked_gate()?;
            self.credential.validate()?;
            ExecutionPermit::<P, R> {
                subject: self.subject,
                resource: self.resource,
                reservation,
                permission: PhantomData,
            }
        };
        let ExecutionPermit {
            subject,
            resource,
            reservation,
            permission: _,
        } = permit;
        Ok(submit(subject, resource, reservation))
    }
}
pub fn subject(scope: &AuthScope) -> i64 {
    scope.subject()
}
pub fn kind(failure: &Failure) -> FailureKind {
    failure.kind()
}
pub fn message(failure: &Failure) -> &str {
    failure.message()
}
pub fn invalid_credential() -> Failure {
    Failure::invalid_credential()
}
pub fn denied() -> Failure {
    Failure::denied()
}
pub fn expired() -> Failure {
    Failure::expired()
}
pub fn invalid_request() -> Failure {
    Failure::invalid_request()
}
pub fn unavailable() -> Failure {
    Failure::unavailable()
}
pub fn internal() -> Failure {
    Failure::internal()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn make_scope() -> (LeaseOwner, AuthScope) {
        let deadline = Instant::now() + Duration::from_secs(30);
        let owner = LeaseOwner::new(deadline).unwrap();
        let proof = AuthScope::bind(
            VerifiedIdentity::from_verified(7, deadline).unwrap(),
            Arc::clone(&owner.lease),
        )
        .unwrap();
        (owner, proof)
    }
    #[test]
    fn end_rejects_grant_issue() {
        let (owner, scope) = make_scope();
        drop(owner);
        assert_eq!(
            Grant::<()>::from_authorized(scope, 9).err().unwrap().kind(),
            FailureKind::Expired
        );
    }
    #[test]
    fn end_rejects_native_admission_and_drops_reservation() {
        struct Reservation(Arc<AtomicU64>);
        impl Drop for Reservation {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let (owner, scope) = make_scope();
        let grant = Grant::<()>::from_authorized(scope, 9).unwrap();
        drop(owner);
        let dropped = Arc::new(AtomicU64::new(0));
        assert!(grant
            .submit(Reservation(Arc::clone(&dropped)), |_, _, _| panic!(
                "must not submit"
            ))
            .is_err());
        assert_eq!(dropped.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn admitted_operation_can_complete_after_invalidation() {
        let (owner, scope) = make_scope();
        let grant = Grant::<()>::from_authorized(scope, 9).unwrap();
        assert_eq!(
            grant
                .submit(17, |s, r, v| {
                    drop(owner);
                    (s, r, v)
                })
                .unwrap(),
            (7, 9, 17)
        );
    }
    #[test]
    fn wrong_request_is_distinct() {
        let (owner, scope) = make_scope();
        let (other, _) = make_scope();
        assert!(scope.belongs_to(&owner.lease));
        assert!(!scope.belongs_to(&other.lease));
    }
    #[test]
    fn expired_identity_and_request_are_rejected() {
        assert!(VerifiedIdentity::from_verified(7, Instant::now()).is_err());
        assert!(LeaseOwner::new(Instant::now()).is_err());
    }
    #[tokio::test]
    async fn invalidation_during_capacity_wait_prevents_submission() {
        let (owner, scope) = make_scope();
        let grant = Grant::<()>::from_authorized(scope, 9).unwrap();
        let (done, wait) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            wait.await.unwrap();
            grant.submit((), |_, _, _| panic!("must not submit"))
        });
        drop(owner);
        done.send(()).unwrap();
        assert!(task.await.unwrap().is_err());
    }
    #[test]
    fn poisoned_gate_fails_closed() {
        let (owner, scope) = make_scope();
        let lease = Arc::clone(&owner.lease);
        let _ = std::thread::spawn(move || {
            let _guard = lease.gate.lock().unwrap();
            panic!("fixture");
        })
        .join();
        assert_eq!(scope.validate().unwrap_err().kind(), FailureKind::Internal);
        drop(owner);
    }
    #[test]
    fn invalidation_wins_a_contended_gate_without_native_admission() {
        use std::sync::Barrier;
        let (owner, scope) = make_scope();
        let grant = Grant::<()>::from_authorized(scope, 9).unwrap();
        let started = Arc::new(Barrier::new(2));
        let worker_started = Arc::clone(&started);
        let submitted = Arc::new(AtomicU64::new(0));
        let calls = Arc::clone(&submitted);
        // Hold the same gate to force the native attempt behind invalidation. This
        // writes the exact owner-Drop transition, before releasing the contended lock.
        let mut gate = owner.lease.gate.lock().unwrap();
        let worker = std::thread::spawn(move || {
            worker_started.wait();
            grant.submit((), |_, _, ()| {
                calls.fetch_add(1, Ordering::SeqCst);
            })
        });
        started.wait();
        gate.active = false;
        drop(gate);
        drop(owner);
        assert_eq!(
            worker.join().unwrap().unwrap_err().kind(),
            FailureKind::Expired
        );
        assert_eq!(submitted.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn permit_wins_before_concurrent_owner_drop_and_admits_once() {
        use std::sync::Barrier;
        let (owner, scope) = make_scope();
        let grant = Grant::<()>::from_authorized(scope, 9).unwrap();
        let issued = Arc::new(Barrier::new(2));
        let finish = Arc::new(Barrier::new(2));
        let issued_worker = Arc::clone(&issued);
        let finish_worker = Arc::clone(&finish);
        let worker = std::thread::spawn(move || {
            grant.submit(17, |subject, target, reservation| {
                issued_worker.wait();
                finish_worker.wait();
                (subject, target, reservation)
            })
        });
        issued.wait();
        drop(owner);
        finish.wait();
        assert_eq!(worker.join().unwrap().unwrap(), (7, 9, 17));
    }
}

#[cfg(test)]
#[path = "auth/credential_metadata_tests.rs"]
mod credential_metadata_tests;

// Checked startup/bootstrap; BrowserLogin/CSRF/compiler remain deferred.
pub mod prelogin;
