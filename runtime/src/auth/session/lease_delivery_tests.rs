use super::*;
use crate::auth::{AuthScope, FailureKind, Grant, LeaseOwner, VerifiedIdentity};
use std::time::{Duration, Instant};
fn owner() -> LeaseOwner {
    LeaseOwner::new(Instant::now() + Duration::from_secs(30)).unwrap()
}
fn material() -> SetCookie {
    match super::super::secrets::CookiePolicy::new("__Host-owned", cookie::SameSite::Strict) {
        Ok(policy) => match policy.remove() {
            Ok(value) => value,
            Err(_) => panic!("owned deletion material failed"),
        },
        Err(_) => panic!("owned policy failed"),
    }
}
#[test]
fn checked_owner_uses_existing_unique_request_identity() {
    let first = owner();
    let second = owner();
    assert!(first.lease.id != 0 && first.lease.id != second.lease.id);
    assert!(first.delivery.cell.lock().unwrap().request_id == first.lease.id);
    assert!(second.delivery.cell.lock().unwrap().request_id == second.lease.id);
    assert!(first.lease.delivery.belongs_to(&first.delivery));
    assert!(!first.lease.delivery.belongs_to(&second.delivery));
    let checked = first.checked_delivery(&first.lease).unwrap();
    assert!(matches!(
        first.lease.gate.try_lock(),
        Err(std::sync::TryLockError::WouldBlock)
    ));
    let pending = checked.stage(material()).unwrap();
    let admitted = checked.admit(pending).unwrap();
    let ready = checked.confirm_commit(admitted).unwrap();
    let _seal = checked.apply(ready).unwrap();
    checked.retire();
    assert!(first.delivery.cell.lock().unwrap().material.is_none());
}
#[test]
fn retained_scope_and_grant_do_not_keep_delivery_alive() {
    let owner = owner();
    let deadline = Instant::now() + Duration::from_secs(30);
    let scope = AuthScope::bind(
        VerifiedIdentity::from_verified(7, deadline).unwrap(),
        Arc::clone(&owner.lease),
    )
    .unwrap();
    let grant_scope = AuthScope::bind(
        VerifiedIdentity::from_verified(7, deadline).unwrap(),
        Arc::clone(&owner.lease),
    )
    .unwrap();
    let grant = Grant::<()>::from_authorized(grant_scope, 9).unwrap();
    let weak = Arc::downgrade(&owner.delivery.cell);
    let pending = owner
        .checked_delivery(&owner.lease)
        .unwrap()
        .stage(material())
        .unwrap();
    assert!(Arc::strong_count(&owner.delivery.cell) == 1);
    drop(owner);
    assert!(weak.upgrade().is_none());
    assert!(scope.validate().unwrap_err().kind() == FailureKind::Expired);
    assert!(
        grant
            .submit((), |_, _, _| panic!("must not submit"))
            .err()
            .unwrap()
            .kind()
            == FailureKind::Expired
    );
    assert!(pending.admit().is_err());
}
#[test]
fn wrong_request_expiry_and_wrong_cell_reject_checked_bridge() {
    let mut first = owner();
    let second = owner();
    assert!(
        first.checked_delivery(&second.lease).err().unwrap().kind() == FailureKind::InvalidRequest
    );
    first.lease.gate.lock().unwrap().expires_at = Instant::now();
    assert!(first.checked_delivery(&first.lease).err().unwrap().kind() == FailureKind::Expired);
    first.lease.gate.lock().unwrap().expires_at = Instant::now() + Duration::from_secs(30);
    Arc::get_mut(&mut first.lease).unwrap().delivery = second.delivery.downgrade();
    assert!(first.checked_delivery(&first.lease).err().unwrap().kind() == FailureKind::Internal);
    assert!(first.delivery.cell.lock().unwrap().phase == Phase::Vacant);
}
#[test]
fn owner_end_clears_ready_even_with_poison() {
    for poison in 0..3 {
        let owner = owner();
        let ready = {
            let checked = owner.checked_delivery(&owner.lease).unwrap();
            let admitted = checked.admit(checked.stage(material()).unwrap()).unwrap();
            checked.confirm_commit(admitted).unwrap()
        };
        // Test-only strong observer; production Lease/Scope/Grant keep Weak only.
        let observed = Arc::clone(&owner.delivery.cell);
        let lease = Arc::clone(&owner.lease);
        if poison == 1 {
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = observed.lock().unwrap();
                panic!("owned cell poison fixture");
            }))
            .is_err());
        } else if poison == 2 {
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = lease.gate.lock().unwrap();
                panic!("owned gate poison fixture");
            }))
            .is_err());
        }
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(owner))).is_ok());
        let gate = match lease.gate.lock() {
            Ok(value) => value,
            Err(error) => error.into_inner(),
        };
        assert!(!gate.active);
        drop(gate);
        let cell = match observed.lock() {
            Ok(value) => value,
            Err(error) => error.into_inner(),
        };
        assert!(cell.phase == Phase::Terminal && cell.material.is_none());
        drop(cell);
        drop(ready);
    }
}
#[test]
fn delayed_pending_drop_cannot_reopen_ended_owner() {
    let owner = owner();
    let observed = Arc::clone(&owner.delivery.cell);
    let pending = owner
        .checked_delivery(&owner.lease)
        .unwrap()
        .stage(material())
        .unwrap();
    drop(owner);
    drop(pending);
    let cell = observed.lock().unwrap();
    assert!(cell.phase == Phase::Terminal && cell.material.is_none());
}
#[test]
fn closed_admit_and_confirm_reject_foreign_claims() {
    let first = owner();
    let second = owner();
    let pending = first
        .checked_delivery(&first.lease)
        .unwrap()
        .stage(material())
        .unwrap();
    assert!(second
        .checked_delivery(&second.lease)
        .unwrap()
        .admit(pending)
        .is_err());
    assert!(first.delivery.cell.lock().unwrap().phase == Phase::Vacant);
    assert!(second.delivery.cell.lock().unwrap().phase == Phase::Terminal);
    let third = owner();
    let fourth = owner();
    let admitted = {
        let checked = third.checked_delivery(&third.lease).unwrap();
        checked.admit(checked.stage(material()).unwrap()).unwrap()
    };
    assert!(fourth
        .checked_delivery(&fourth.lease)
        .unwrap()
        .confirm_commit(admitted)
        .is_err());
    assert!(third.delivery.cell.lock().unwrap().phase == Phase::Terminal);
    assert!(fourth.delivery.cell.lock().unwrap().phase == Phase::Terminal);
}
