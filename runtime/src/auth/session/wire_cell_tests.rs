//! Actual public producer plus private guarded wire take; no secret output.
use super::*;
use crate::{
    auth::session,
    auth::{AuthScope, LeaseOwner, VerifiedIdentity},
    sqlite,
};
use std::time::{Duration, Instant};
async fn fixture() -> (std::path::PathBuf, sqlite::Pool, session::Store) {
    let root = std::env::temp_dir();
    let directory = loop {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = root.join(format!("owned-wire-cell-{}-{n}", std::process::id()));
        match std::fs::create_dir(&path) {
            Ok(()) => break path,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("owned fixture: {e}"),
        }
    };
    let pool = sqlite::open(
        directory.join("owned.sqlite").to_str().unwrap(),
        sqlite::options(2, 4, 1000, 1000).unwrap(),
    )
    .await
    .unwrap();
    let store = session::open(
        &pool,
        session::options(4, 5, 72, 1, 10000, 100000, 5000, 2).unwrap(),
        session::cookie_options("__Host-owned", session::SameSite::Strict).unwrap(),
    )
    .await
    .unwrap();
    (directory, pool, store)
}
fn scope(owner: &LeaseOwner) -> AuthScope {
    AuthScope::bind(
        VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(60)).unwrap(),
        Arc::clone(&owner.lease),
    )
    .unwrap()
}
async fn seal(store: &session::Store, owner: &LeaseOwner) -> Seal {
    let response = session::issue(store, scope(owner)).await.unwrap();
    response.intent.apply().unwrap()
}
async fn stop(directory: std::path::PathBuf, pool: sqlite::Pool, store: session::Store) {
    drop(store);
    sqlite::close(&pool, 2000).await.unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
#[tokio::test]
async fn actual_committed_guarded_take_rejects_replay_and_foreign_owner() {
    let (dir, pool, store) = fixture().await;
    let owner = LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap();
    let one = seal(&store, &owner).await;
    // Only this private oracle can duplicate the NONSECRET seal descriptor.
    // Production Seal/ResponseSeal have no Clone/copy/getter/constructor API.
    let replay = Seal(one.0);
    let header = session::take_http_cookie(&owner, session::ResponseSeal(one)).unwrap();
    assert!(header.is_sensitive());
    drop(header);
    assert!(session::take_http_cookie(&owner, session::ResponseSeal(replay)).is_err());
    assert!(!test_slot_status(&owner.lease).unwrap().3);
    let origin = LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap();
    let other = LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap();
    let foreign = seal(&store, &origin).await;
    assert!(session::take_http_cookie(&other, session::ResponseSeal(foreign)).is_err());
    assert!(!test_slot_status(&other.lease).unwrap().3);
    let lease = Arc::clone(&origin.lease);
    drop(origin);
    assert!(test_slot_status(&lease).is_none());
    drop(owner);
    drop(other);
    stop(dir, pool, store).await;
}
#[tokio::test]
async fn already_sealed_response_rejects_second_ready_right_and_poison_is_redacted() {
    let (dir, pool, store) = fixture().await;
    let first = LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap();
    let second = LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap();
    let response = session::apply(
        crate::http_server::text(crate::http_server::Status::OK, "owned"),
        session::issue(&store, scope(&first)).await.unwrap(),
    )
    .unwrap();
    let second_intent = session::issue(&store, scope(&second)).await.unwrap();
    let failure = session::apply(response, second_intent).err().unwrap();
    assert_eq!(session::kind(&failure), crate::auth::FailureKind::Internal);
    assert!(!test_slot_status(&second.lease).unwrap().3);
    session::retire_http_delivery(&first);
    assert!(!test_slot_status(&first.lease).unwrap().3);
    let poisoned = LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap();
    let one = seal(&store, &poisoned).await;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = poisoned.delivery.cell.lock().unwrap();
        panic!("owned cell poison");
    }));
    assert!(result.is_err());
    assert!(session::take_http_cookie(&poisoned, session::ResponseSeal(one)).is_err());
    session::retire_http_delivery(&poisoned); // poison-safe gate -> cell disposal
    match poisoned.delivery.cell.lock() {
        Err(error) => assert!(error.into_inner().material.is_none()),
        Ok(_) => panic!("poison not observed"),
    }
    drop(first);
    drop(second);
    drop(poisoned);
    stop(dir, pool, store).await;
}
