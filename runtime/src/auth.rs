//! Request-bound proofs for trusted credential and policy adapters.
//! Verifier assertions remain trusted; only the dispatcher creates leases.
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
        match self.kind {
            FailureKind::InvalidCredential => "invalid credential",
            FailureKind::Denied => "permission denied",
            FailureKind::Expired => "authority expired",
            FailureKind::InvalidRequest => "invalid security request",
            FailureKind::Unavailable => "security service unavailable",
            FailureKind::Internal => "security service failure",
        }
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

/// Trusted verifier output, not an authorization proof. No Nagi factory.
pub struct VerifiedIdentity {
    subject: i64,
    expires_at: Instant,
}
impl VerifiedIdentity {
    /// The adapter must first verify credentials, audience, expiry and revocation.
    /// This value alone cannot create a request lease or authorize an operation.
    pub fn from_verified(subject: i64, expires_at: Instant) -> Result<Self, Failure> {
        if expires_at <= Instant::now() {
            return Err(Failure::expired());
        }
        Ok(Self {
            subject,
            expires_at,
        })
    }
}
struct Gate {
    active: bool,
    expires_at: Instant,
}
pub(crate) struct Lease {
    gate: Mutex<Gate>,
    id: u64,
}
pub(crate) struct LeaseOwner {
    pub(crate) lease: Arc<Lease>,
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
        Ok(Self {
            lease: Arc::new(Lease {
                gate: Mutex::new(Gate {
                    active: true,
                    expires_at,
                }),
                id,
            }),
        })
    }
}
impl Drop for LeaseOwner {
    fn drop(&mut self) {
        match self.lease.gate.lock() {
            Ok(mut g) => g.active = false,
            Err(e) => e.into_inner().active = false,
        }
    }
}
impl Lease {
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
/// Same-task, non-cloneable identity bound to a live dispatcher-owned request.
///
/// ```compile_fail
/// let scope = nagi_runtime::auth::AuthScope::from_verified_subject(7);
/// ```
pub struct AuthScope {
    subject: i64,
    lease: Arc<Lease>,
}
impl AuthScope {
    pub(crate) fn bind(identity: VerifiedIdentity, lease: Arc<Lease>) -> Result<Self, Failure> {
        {
            let mut gate = lease.checked_gate()?;
            if identity.expires_at <= Instant::now() {
                return Err(Failure::expired());
            }
            gate.expires_at = gate.expires_at.min(identity.expires_at);
        }
        Ok(Self {
            subject: identity.subject,
            lease,
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
            permission: PhantomData,
        })
    }
    pub(crate) fn belongs_to(&self, lease: &Arc<Lease>) -> bool {
        self.lease.id == lease.id && Arc::ptr_eq(&self.lease, lease)
    }
    pub(crate) fn validate(&self) -> Result<(), Failure> {
        drop(self.lease.checked_gate()?);
        Ok(())
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
