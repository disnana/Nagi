//! Closed request-owned delivery, registered with the actual LeaseOwner/Lease.
//! Typed bridge operations hold the checked live lease gate before the cell.
//! SQLite admission/commit and HTTP finalization remain separate integration gates.
use super::secrets::SetCookie;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

#[derive(Debug, PartialEq, Eq)]
pub(in crate::auth) enum Error {
    InvalidState,
    InvalidBinding,
    Unavailable,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Vacant,
    Pending,
    Admitted,
    Ready,
    Applied,
    Terminal,
}
struct Cell {
    request_id: u64,
    next_serial: u64,
    serial: u64,
    phase: Phase,
    material: Option<SetCookie>,
}
impl Cell {
    fn retire(&mut self) {
        self.phase = Phase::Terminal;
        self.material.take();
    }
    fn matches(&self, claim: &Claim) -> bool {
        self.request_id == claim.request_id && self.serial == claim.serial
    }
}
fn checked(cell: &Mutex<Cell>) -> Result<MutexGuard<'_, Cell>, Error> {
    match cell.lock() {
        Ok(value) => Ok(value),
        Err(error) => {
            error.into_inner().retire();
            Err(Error::Unavailable)
        }
    }
}
// Only the request owner keeps a strong cell. Intent/reservation drops use Weak.
pub(in crate::auth) struct DeliveryOwner {
    cell: Arc<Mutex<Cell>>,
}
// A Lease may outlive its dispatcher owner but cannot retain secret material.
pub(in crate::auth) struct DeliveryWeak(Weak<Mutex<Cell>>);
impl DeliveryWeak {
    fn belongs_to(&self, owner: &DeliveryOwner) -> bool {
        Weak::ptr_eq(&self.0, &Arc::downgrade(&owner.cell))
    }
}
// No callbacks or raw owner/cell getters. The borrow also prevents owner Drop
// until this guard has released the actual request gate.
pub(in crate::auth) struct CheckedDelivery<'a> {
    _gate: MutexGuard<'a, crate::auth::Gate>,
    owner: &'a DeliveryOwner,
}
impl<'a> CheckedDelivery<'a> {
    pub(in crate::auth) fn for_lease(
        owner: &'a crate::auth::LeaseOwner,
        lease: &'a Arc<crate::auth::Lease>,
    ) -> Result<Self, crate::auth::Failure> {
        if owner.lease.id != lease.id || !Arc::ptr_eq(&owner.lease, lease) {
            return Err(crate::auth::Failure::invalid_request());
        }
        let gate = lease.checked_gate()?;
        if !lease.delivery.belongs_to(&owner.delivery) {
            return Err(crate::auth::Failure::internal());
        }
        Ok(Self {
            _gate: gate,
            owner: &owner.delivery,
        })
    }
    pub(super) fn stage(&self, material: SetCookie) -> Result<Pending, Error> {
        self.owner.stage(material)
    }
    pub(super) fn admit(&self, pending: Pending) -> Result<Admitted, Error> {
        if !Weak::ptr_eq(&pending.0.cell, &Arc::downgrade(&self.owner.cell)) {
            self.owner.clear();
            return Err(Error::InvalidBinding);
        }
        pending.admit()
    }
    // State mark only: a later caller must confirm the actual Tx commit first.
    pub(super) fn confirm_commit(&self, admitted: Admitted) -> Result<ReadyClaim, Error> {
        if !Weak::ptr_eq(&admitted.0.cell, &Arc::downgrade(&self.owner.cell)) {
            self.owner.clear();
            return Err(Error::InvalidBinding);
        }
        admitted.confirm_commit()
    }
    pub(super) fn apply(&self, right: ReadyClaim) -> Result<Seal, Error> {
        self.owner.apply(right)
    }
    pub(super) fn take_applied(&self, seal: Seal) -> Result<SetCookie, Error> {
        self.owner.take_applied(seal)
    }
    pub(super) fn retire(&self) {
        self.owner.clear();
    }
}
fn stage_cell(owner: &Arc<Mutex<Cell>>, material: SetCookie) -> Result<Pending, Error> {
    let mut cell = checked(owner)?;
    if cell.phase != Phase::Vacant {
        return Err(Error::InvalidState);
    }
    let Some(serial) = cell.next_serial.checked_add(1) else {
        cell.retire();
        return Err(Error::Unavailable);
    };
    cell.next_serial = serial;
    cell.serial = serial;
    cell.material = Some(material);
    cell.phase = Phase::Pending;
    Ok(Pending(Claim {
        cell: Arc::downgrade(owner),
        request_id: cell.request_id,
        serial,
    }))
}

// The dispatcher remains the only enduring strong owner. This upgrade exists
// only while holding the real live gate, and is never retained across await.
pub(super) struct LeaseDelivery<'a> {
    _gate: MutexGuard<'a, crate::auth::Gate>,
    cell: Arc<Mutex<Cell>>,
}
impl<'a> LeaseDelivery<'a> {
    pub(super) fn for_lease(lease: &'a crate::auth::Lease) -> Result<Self, crate::auth::Failure> {
        let gate = lease.checked_gate()?;
        let cell = lease
            .delivery
            .0
            .upgrade()
            .ok_or_else(crate::auth::Failure::expired)?;
        {
            let value = checked(&cell).map_err(|_| crate::auth::Failure::unavailable())?;
            if value.request_id != lease.id {
                return Err(crate::auth::Failure::invalid_request());
            }
        }
        Ok(Self { _gate: gate, cell })
    }
    pub(super) fn stage(&self, material: SetCookie) -> Result<Pending, Error> {
        stage_cell(&self.cell, material)
    }
    pub(super) fn enqueue_logout(
        &self,
        pending: Pending,
        reservation: crate::sqlite::ExecReservation<'_>,
        plan: super::mutation::LogoutPlan,
        sent: &mut bool,
    ) -> Result<(Admitted, super::mutation::Reply), Error> {
        self.enqueue_mutation(
            pending,
            reservation,
            super::mutation::NativePlan::Logout(plan),
            sent,
        )
    }
    pub(super) fn enqueue_mutation(
        &self,
        pending: Pending,
        reservation: crate::sqlite::ExecReservation<'_>,
        plan: super::mutation::NativePlan,
        sent: &mut bool,
    ) -> Result<(Admitted, super::mutation::Reply), Error> {
        // Reject foreign/stale claims before business admission. Never hold the
        // cell lock while sending or dropping a claim (Drop locks that cell).
        {
            let cell = checked(&self.cell)?;
            if !Weak::ptr_eq(&pending.0.cell, &Arc::downgrade(&self.cell))
                || !cell.matches(&pending.0)
                || cell.phase != Phase::Pending
            {
                return Err(Error::InvalidBinding);
            }
        }
        let (query, parameters) = plan.into_native();
        let reply = reservation.enqueue(query, parameters);
        *sent = true; // bookkeeping for rollback; not a commit/delivery proof
        let admitted = pending.admit()?;
        Ok((admitted, Box::pin(reply)))
    }
    pub(super) fn apply_ready(&self, right: ReadyClaim) -> Result<Seal, Error> {
        apply_cell(&self.cell, right)
    }
    pub(super) fn confirm_committed(&self, admitted: Admitted) -> Result<ReadyClaim, Error> {
        if !Weak::ptr_eq(&admitted.0.cell, &Arc::downgrade(&self.cell)) {
            return Err(Error::InvalidBinding);
        }
        admitted.confirm_commit()
    }
}
#[cfg(test)]
pub(super) fn test_slot_status(lease: &crate::auth::Lease) -> Option<(bool, bool, bool, bool)> {
    let owner = lease.delivery.0.upgrade()?;
    let cell = checked(&owner).ok()?;
    Some((
        cell.phase == Phase::Vacant,
        cell.phase == Phase::Admitted,
        cell.phase == Phase::Ready,
        cell.material.is_some(),
    ))
}
struct Claim {
    cell: Weak<Mutex<Cell>>,
    request_id: u64,
    serial: u64,
}
pub(super) struct Pending(Claim);
pub(super) struct Admitted(Claim);
pub(super) struct ReadyClaim(Claim);
// Nonsecret, nonclone seal. It retains neither a cell nor a lease/proof.
pub(super) struct Seal(u64);
fn apply_cell(owner: &Arc<Mutex<Cell>>, right: ReadyClaim) -> Result<Seal, Error> {
    let mut claim = right.0;
    if !Weak::ptr_eq(&claim.cell, &Arc::downgrade(owner)) {
        checked(owner)?.retire();
        // The original Ready claim drops after receiver lock release.
        return Err(Error::InvalidBinding);
    }
    let result = {
        let mut cell = checked(owner)?;
        if !cell.matches(&claim) || cell.phase != Phase::Ready {
            cell.retire();
            Err(Error::InvalidState)
        } else {
            cell.phase = Phase::Applied;
            Ok(Seal(cell.request_id))
        }
    };
    if result.is_ok() {
        // Disarm before Drop; do not acquire the same lock recursively.
        claim.cell = Weak::new();
    }
    result
}

impl DeliveryOwner {
    pub(in crate::auth) fn downgrade(&self) -> DeliveryWeak {
        DeliveryWeak(Arc::downgrade(&self.cell))
    }
    pub(in crate::auth) fn new(request_id: u64) -> Result<Self, Error> {
        if request_id == 0 {
            return Err(Error::InvalidBinding);
        }
        Ok(Self {
            cell: Arc::new(Mutex::new(Cell {
                request_id,
                next_serial: 0,
                serial: 0,
                phase: Phase::Vacant,
                material: None,
            })),
        })
    }
    fn stage(&self, material: SetCookie) -> Result<Pending, Error> {
        stage_cell(&self.cell, material)
    }
    pub(in crate::auth) fn clear(&self) {
        match self.cell.lock() {
            Ok(mut value) => value.retire(),
            Err(error) => error.into_inner().retire(),
        }
    }
    fn apply(&self, right: ReadyClaim) -> Result<Seal, Error> {
        apply_cell(&self.cell, right)
    }
    // Destructive one-use terminal bridge, never a borrowed/raw secret getter.
    // A later checked finalizer may consume it into the native wire response.
    fn take_applied(&self, seal: Seal) -> Result<SetCookie, Error> {
        let result = {
            let mut cell = checked(&self.cell)?;
            if seal.0 != cell.request_id {
                cell.retire();
                Err(Error::InvalidBinding)
            } else if cell.phase != Phase::Applied {
                cell.retire();
                Err(Error::InvalidState)
            } else {
                let material = cell.material.take().ok_or(Error::Unavailable);
                cell.retire();
                material
            }
        };
        // The cell guard is gone before a later request-owner Drop clears it.
        result
    }
}
impl Drop for DeliveryOwner {
    fn drop(&mut self) {
        self.clear();
    }
}
impl Claim {
    fn transition(&self, from: Phase, to: Phase) -> Result<(), Error> {
        let owner = self.cell.upgrade().ok_or(Error::Unavailable)?;
        let mut cell = checked(&owner)?;
        if !cell.matches(self) || cell.phase != from {
            return Err(Error::InvalidState);
        }
        cell.phase = to;
        Ok(())
    }
}
impl Drop for Claim {
    fn drop(&mut self) {
        let Some(owner) = self.cell.upgrade() else {
            return;
        };
        let mut cell = match owner.lock() {
            Ok(value) => value,
            Err(error) => {
                error.into_inner().retire();
                return;
            }
        };
        if !cell.matches(self) {
            return;
        }
        match cell.phase {
            Phase::Pending => {
                cell.material.take();
                cell.phase = Phase::Vacant;
            }
            Phase::Admitted | Phase::Ready => cell.retire(),
            Phase::Vacant | Phase::Applied | Phase::Terminal => {}
        }
    }
}
impl Pending {
    // This typed state mark is not an authorization permit. Later parent code
    // invokes it only as part of the checked synchronous native enqueue.
    fn admit(self) -> Result<Admitted, Error> {
        self.0.transition(Phase::Pending, Phase::Admitted)?;
        Ok(Admitted(self.0))
    }
    fn abort(self) {
        drop(self);
    }
}
impl Admitted {
    // Only a later confirmed Tx finish may invoke this private transition.
    // No bool/Outcome argument can manufacture authorization or commit proof.
    fn confirm_commit(self) -> Result<ReadyClaim, Error> {
        self.0.transition(Phase::Admitted, Phase::Ready)?;
        Ok(ReadyClaim(self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::super::secrets::CookiePolicy;
    use super::*;
    fn material() -> SetCookie {
        match CookiePolicy::new("__Host-owned", cookie::SameSite::Strict) {
            Ok(policy) => match policy.remove() {
                Ok(value) => value,
                Err(_) => panic!("owned deletion material construction failed"),
            },
            Err(_) => panic!("owned cookie policy construction failed"),
        }
    }
    fn ready(owner: &DeliveryOwner) -> ReadyClaim {
        owner
            .stage(material())
            .unwrap()
            .admit()
            .unwrap()
            .confirm_commit()
            .unwrap()
    }
    fn state(owner: &DeliveryOwner, phase: Phase, has_material: bool) {
        let value = owner.cell.lock().unwrap();
        assert!(value.phase == phase);
        assert!(value.material.is_some() == has_material);
    }
    #[test]
    fn one_pending_pre_admission_abort_and_stale_drop() {
        let owner = DeliveryOwner::new(1).unwrap();
        let first = owner.stage(material()).unwrap();
        state(&owner, Phase::Pending, true);
        assert!(owner.stage(material()).is_err());
        let stale = Claim {
            cell: first.0.cell.clone(),
            request_id: first.0.request_id,
            serial: first.0.serial,
        };
        first.abort();
        state(&owner, Phase::Vacant, false);
        let replacement = owner.stage(material()).unwrap();
        drop(stale);
        state(&owner, Phase::Pending, true);
        replacement.abort();
        state(&owner, Phase::Vacant, false);
    }
    #[test]
    fn admitted_abort_is_terminal_without_reuse() {
        let owner = DeliveryOwner::new(1).unwrap();
        let admitted = owner.stage(material()).unwrap().admit().unwrap();
        state(&owner, Phase::Admitted, true);
        drop(admitted);
        state(&owner, Phase::Terminal, false);
        assert!(owner.stage(material()).is_err());
    }
    #[test]
    fn ready_claim_drop_discards_without_reuse() {
        let owner = DeliveryOwner::new(1).unwrap();
        let right = ready(&owner);
        state(&owner, Phase::Ready, true);
        drop(right);
        state(&owner, Phase::Terminal, false);
        assert!(owner.stage(material()).is_err());
    }
    #[test]
    fn one_use_apply_and_destructive_terminal_bridge() {
        let owner = DeliveryOwner::new(1).unwrap();
        let seal = owner.apply(ready(&owner)).unwrap();
        state(&owner, Phase::Applied, true);
        assert!(owner.stage(material()).is_err());
        let observer = Arc::clone(&owner.cell);
        let wire = owner.take_applied(seal).unwrap();
        let value = observer.lock().unwrap();
        assert!(value.phase == Phase::Terminal && value.material.is_none());
        drop(value);
        // Only boolean checks of deletion attributes, never a raw header artifact.
        let header = wire.into_header();
        assert!(header.is_sensitive());
        let text = header.to_str().unwrap();
        assert!(text.contains("Secure") && text.contains("HttpOnly") && text.contains("Path=/"));
        assert!(text.contains("Max-Age=0") && !text.contains("Domain="));
    }
    #[test]
    fn stale_ready_apply_fails_closed() {
        let owner = DeliveryOwner::new(1).unwrap();
        let right = ready(&owner);
        let stale = ReadyClaim(Claim {
            cell: right.0.cell.clone(),
            request_id: right.0.request_id,
            serial: right.0.serial,
        });
        let seal = owner.apply(right).unwrap();
        assert!(owner.apply(stale).is_err());
        state(&owner, Phase::Terminal, false);
        assert!(owner.take_applied(seal).is_err());
    }
    #[test]
    fn different_cells_even_with_same_id_reject_claim_and_clear_both() {
        let source = DeliveryOwner::new(1).unwrap();
        let other = DeliveryOwner::new(1).unwrap();
        let pending_other = other.stage(material()).unwrap();
        assert!(other.apply(ready(&source)).is_err());
        state(&source, Phase::Terminal, false);
        state(&other, Phase::Terminal, false);
        drop(pending_other);
        state(&other, Phase::Terminal, false);
    }
    #[test]
    fn mismatched_origin_seal_discards_current_material() {
        let owner = DeliveryOwner::new(1).unwrap();
        let original_seal = owner.apply(ready(&owner)).unwrap();
        let observer = Arc::clone(&owner.cell);
        assert!(owner.take_applied(Seal(2)).is_err());
        let value = observer.lock().unwrap();
        assert!(value.phase == Phase::Terminal && value.material.is_none());
        drop(value);
        drop(original_seal);
    }
    #[test]
    fn explicit_clear_retains_no_secret_in_every_active_phase() {
        for phase in 0..4 {
            let owner = DeliveryOwner::new(1).unwrap();
            let pending = owner.stage(material()).unwrap();
            let held: Box<dyn std::any::Any> = match phase {
                0 => Box::new(pending),
                1 => Box::new(pending.admit().unwrap()),
                2 => Box::new(pending.admit().unwrap().confirm_commit().unwrap()),
                _ => Box::new(
                    owner
                        .apply(pending.admit().unwrap().confirm_commit().unwrap())
                        .unwrap(),
                ),
            };
            owner.clear();
            state(&owner, Phase::Terminal, false);
            drop(held);
            state(&owner, Phase::Terminal, false);
        }
    }
    #[test]
    fn poison_redacts_and_never_reactivates() {
        let owner = DeliveryOwner::new(1).unwrap();
        let right = ready(&owner);
        let cell = Arc::clone(&owner.cell);
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _held = cell.lock().unwrap();
            panic!("owned poison fixture");
        }))
        .is_err());
        assert!(owner.apply(right).is_err());
        let value = match cell.lock() {
            Err(error) => error.into_inner(),
            Ok(_) => panic!("owned poison must remain observable"),
        };
        assert!(value.phase == Phase::Terminal && value.material.is_none());
        drop(value);
        assert!(owner.stage(material()).is_err());
        owner.clear();
    }
    #[test]
    fn owner_drop_releases_material_and_weak_claim() {
        let owner = DeliveryOwner::new(1).unwrap();
        let pending = owner.stage(material()).unwrap();
        let weak = Arc::downgrade(&owner.cell);
        drop(owner);
        assert!(weak.upgrade().is_none());
        assert!(pending.admit().is_err());
    }
    #[test]
    fn invalid_id_and_private_serial_overflow_fail_closed() {
        assert!(DeliveryOwner::new(0).is_err());
        let owner = DeliveryOwner::new(1).unwrap();
        owner.cell.lock().unwrap().next_serial = u64::MAX;
        assert!(owner.stage(material()).is_err());
        state(&owner, Phase::Terminal, false);
    }
}

#[cfg(test)]
#[path = "lease_delivery_tests.rs"]
mod lease_tests;

#[cfg(test)]
#[path = "wire_cell_tests.rs"]
mod wire_tests;
