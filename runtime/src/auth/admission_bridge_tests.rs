//! Private Grant/Lease oracle; native client queue and DB-backed Session remain separate.
use super::*;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Barrier,
    },
    time::Duration,
};

fn bearer_grant(request_end: Instant, credential_end: Instant) -> (LeaseOwner, Grant<()>) {
    let owner = LeaseOwner::new(request_end).unwrap();
    let identity = VerifiedIdentity::from_verified(7, credential_end).unwrap();
    let scope = AuthScope::bind(identity, Arc::clone(&owner.lease)).unwrap();
    let grant = Grant::from_authorized(scope, 9).unwrap();
    (owner, grant)
}

#[test]
fn private_end_is_original_request_when_it_is_shortest() {
    let now = Instant::now();
    let request_end = now + Duration::from_secs(5);
    let credential_end = now + Duration::from_secs(30);
    let whole_end = now + Duration::from_secs(50);
    let (_owner, grant) = bearer_grant(request_end, credential_end);
    let answer = grant.submit_private(17, whole_end, |subject, target, end, slot| {
        (subject, target, end, slot)
    });
    assert_eq!(answer.unwrap(), (7, 9, request_end, 17));
}

#[test]
fn private_end_is_fixed_whole_ceiling_when_it_is_shortest() {
    let now = Instant::now();
    let request_end = now + Duration::from_secs(30);
    let credential_end = now + Duration::from_secs(40);
    let whole_end = now + Duration::from_secs(2);
    let (_owner, grant) = bearer_grant(request_end, credential_end);
    let answer = grant.submit_private(17, whole_end, |subject, target, end, slot| {
        (subject, target, end, slot)
    });
    assert_eq!(answer.unwrap(), (7, 9, whole_end, 17));
}

#[test]
fn expired_whole_ceiling_rejects_and_drops_reservation_without_callback() {
    struct Reservation(Arc<AtomicUsize>);
    impl Drop for Reservation {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let now = Instant::now();
    let (_owner, grant) =
        bearer_grant(now + Duration::from_secs(30), now + Duration::from_secs(40));
    let dropped = Arc::new(AtomicUsize::new(0));
    let called = Arc::new(AtomicUsize::new(0));
    let callback = Arc::clone(&called);
    let result = grant.submit_private(Reservation(Arc::clone(&dropped)), now, move |_, _, _, _| {
        callback.fetch_add(1, Ordering::SeqCst);
    });
    assert_eq!(result.unwrap_err().kind(), FailureKind::Expired);
    assert_eq!(called.load(Ordering::SeqCst), 0);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
}

#[test]
fn invalidation_before_private_permit_rejects_without_enqueue() {
    let now = Instant::now();
    let (owner, grant) = bearer_grant(now + Duration::from_secs(30), now + Duration::from_secs(40));
    let gate = Arc::clone(&owner.lease);
    let started = Arc::new(Barrier::new(2));
    let worker_started = Arc::clone(&started);
    let called = Arc::new(AtomicUsize::new(0));
    let worker_called = Arc::clone(&called);
    let mut held = gate.gate.lock().unwrap();
    let worker = std::thread::spawn(move || {
        worker_started.wait();
        grant.submit_private((), now + Duration::from_secs(10), |_, _, _, ()| {
            worker_called.fetch_add(1, Ordering::SeqCst);
        })
    });
    started.wait();
    held.active = false;
    drop(held);
    drop(owner);
    assert_eq!(
        worker.join().unwrap().unwrap_err().kind(),
        FailureKind::Expired
    );
    assert_eq!(called.load(Ordering::SeqCst), 0);
}

#[test]
fn private_permit_before_revocation_stays_admitted_once() {
    let now = Instant::now();
    let (owner, grant) = bearer_grant(now + Duration::from_secs(30), now + Duration::from_secs(40));
    let whole_end = now + Duration::from_secs(10);
    let issued = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let worker_issued = Arc::clone(&issued);
    let worker_release = Arc::clone(&release);
    let worker = std::thread::spawn(move || {
        grant.submit_private(17, whole_end, |subject, target, end, slot| {
            worker_issued.wait();
            worker_release.wait();
            (subject, target, end, slot)
        })
    });
    issued.wait();
    drop(owner);
    release.wait();
    assert_eq!(worker.join().unwrap().unwrap(), (7, 9, whole_end, 17));
}

#[test]
fn cancelled_capacity_wait_does_not_issue_a_private_permit() {
    let now = Instant::now();
    let (owner, grant) = bearer_grant(now + Duration::from_secs(30), now + Duration::from_secs(40));
    let called = Arc::new(AtomicUsize::new(0));
    let callback = Arc::clone(&called);
    // A closed capacity reservation is a pre-permit failure. This is only a
    // local ordering fixture, not proof of the future native client's queue.
    let reserved: Result<(), ()> = Err(());
    let attempt = reserved.and_then(|slot| {
        grant
            .submit_private(slot, now + Duration::from_secs(10), move |_, _, _, ()| {
                callback.fetch_add(1, Ordering::SeqCst);
            })
            .map_err(|_| ())
    });
    assert!(attempt.is_err());
    assert_eq!(called.load(Ordering::SeqCst), 0);
    assert!(owner.lease.validate().is_ok());
}

#[test]
fn public_submit_signature_and_binding_still_use_same_issue_gate() {
    let now = Instant::now();
    let (owner, grant) = bearer_grant(now + Duration::from_secs(30), now + Duration::from_secs(40));
    assert_eq!(grant.submit(17, |s, r, v| (s, r, v)).unwrap(), (7, 9, 17));
    drop(owner);
}
