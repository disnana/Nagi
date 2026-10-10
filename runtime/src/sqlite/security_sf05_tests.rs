//! Native admission and predicates of a reviewed trusted adapter, not arbitrary
//! SQL authorization. Tiny owned SQLite fixtures; no external target or load.
use super::*;
use crate::auth::{self, AuthScope, Grant, LeaseOwner, VerifiedIdentity};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
struct Edit;
const SEED: &str = "CREATE TABLE protected(id INTEGER PRIMARY KEY, owner INTEGER, value INTEGER); INSERT INTO protected VALUES(9,7,100),(10,8,200); CREATE TABLE capacity(n); INSERT INTO capacity VALUES(0);";
fn request(subject: i64) -> (LeaseOwner, AuthScope) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let owner = LeaseOwner::new(deadline).unwrap();
    let scope = AuthScope::bind(
        VerifiedIdentity::from_verified(subject, deadline).unwrap(),
        Arc::clone(&owner.lease),
    )
    .unwrap();
    (owner, scope)
}
// Reviewed adapter: both real Grant values are bound; an unguarded UPDATE or
// client-supplied replacement for subject/target would fail the native oracle.
fn enqueue_reviewed(
    reservation: ExecReservation<'_>,
    subject: i64,
    target: i64,
    value: i64,
) -> impl std::future::Future<Output = Result<i64, Failure>> + Send + 'static {
    reservation.enqueue(
        literal("UPDATE protected SET value=? WHERE owner=? AND id=?"),
        bind_i64(bind_i64(bind_i64(parameters(), value), subject), target),
    )
}
#[derive(Debug, PartialEq)]
struct ValueRow(i64);
impl FromRow for ValueRow {
    fn columns() -> &'static [&'static str] {
        &["value"]
    }
    fn read(row: &rusqlite::Row<'_>, indices: &[usize]) -> rusqlite::Result<Self> {
        Ok(Self(row.get(indices[0])?))
    }
}
async fn value(tx: &Tx, id: i64) -> i64 {
    query::<ValueRow>(
        tx,
        literal("SELECT value FROM protected WHERE id=?"),
        bind_i64(parameters(), id),
    )
    .await
    .unwrap()
    .unwrap()
    .0
}
#[tokio::test]
async fn actual_grant_subject_and_target_are_bound_to_native_owner_predicate() {
    let driver = Driver::open(Config {
        seed: SEED,
        ..Default::default()
    });
    let tx = driver.begin().await.unwrap();
    for (subject, target, expected) in [(7, 9, 1), (8, 9, 0), (7, 10, 0)] {
        let (_owner, scope) = request(subject);
        let grant = Grant::<Edit>::from_authorized(scope, target).unwrap();
        let reservation = tx.reserve_exec().await.unwrap();
        let before = tx.enqueued_execs();
        let reply = grant
            .submit(reservation, |s, t, res| enqueue_reviewed(res, s, t, 101))
            .unwrap();
        assert_eq!(
            tx.enqueued_execs(),
            before + 1,
            "enqueue must be synchronous before reply poll"
        );
        assert_eq!(reply.await.unwrap(), expected);
        assert_eq!(value(&tx, 9).await, 101);
        assert_eq!(
            value(&tx, 10).await,
            200,
            "other tenant/target must remain untouched"
        );
    }
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}
#[tokio::test]
async fn revoke_before_real_queue_admission_enqueues_zero_commands() {
    let driver = Driver::open(Config {
        seed: SEED,
        ..Default::default()
    });
    let tx = driver.begin().await.unwrap();
    let (owner, scope) = request(7);
    let grant = Grant::<Edit>::from_authorized(scope, 9).unwrap();
    let reservation = tx.reserve_exec().await.unwrap();
    assert_eq!(tx.available_capacity(), 0);
    drop(owner);
    let error = grant
        .submit(reservation, |s, t, res| enqueue_reviewed(res, s, t, 999))
        .err()
        .unwrap();
    assert_eq!(error.kind(), auth::FailureKind::Expired);
    assert_eq!(tx.enqueued_execs(), 0);
    assert_eq!(
        tx.available_capacity(),
        1,
        "denied permit must release reserved capacity"
    );
    assert_eq!(value(&tx, 9).await, 100);
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}
#[tokio::test]
async fn real_queue_admission_before_revoke_is_already_accepted() {
    let gate = Arc::new(Gate::new());
    let driver = Driver::open(Config {
        seed: SEED,
        command_gate: Some(Arc::clone(&gate)),
        ..Default::default()
    });
    let tx = driver.begin().await.unwrap();
    let (owner, scope) = request(7);
    let grant = Grant::<Edit>::from_authorized(scope, 9).unwrap();
    let reservation = tx.reserve_exec().await.unwrap();
    let reply = grant
        .submit(reservation, |s, t, res| enqueue_reviewed(res, s, t, 101))
        .unwrap();
    assert_eq!(tx.enqueued_execs(), 1);
    gate.wait().await; // command accepted; native step has not yet run.
    drop(owner);
    gate.release();
    assert_eq!(reply.await.unwrap(), 1);
    assert_eq!(value(&tx, 9).await, 101);
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}
#[tokio::test]
async fn issued_permit_before_revoke_still_synchronously_enqueues_once() {
    let driver = Driver::open(Config {
        seed: SEED,
        ..Default::default()
    });
    let tx = driver.begin().await.unwrap();
    let (owner, scope) = request(7);
    let grant = Grant::<Edit>::from_authorized(scope, 9).unwrap();
    let reservation = tx.reserve_exec().await.unwrap();
    let reply = grant
        .submit(reservation, |s, t, res| {
            // SF01 admission has already won under the gate. Invalidation here,
            // before the actual send, must not deadlock or retract that permit.
            assert_eq!(tx.enqueued_execs(), 0);
            drop(owner);
            enqueue_reviewed(res, s, t, 101)
        })
        .unwrap();
    assert_eq!(tx.enqueued_execs(), 1);
    assert_eq!(reply.await.unwrap(), 1);
    assert_eq!(value(&tx, 9).await, 101);
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}
#[tokio::test]
async fn revoke_during_bounded_queue_capacity_wait_prevents_protected_enqueue() {
    use std::{future::Future, task::Poll};
    let gate = Arc::new(Gate::new());
    let driver = Driver::open(Config {
        seed: SEED,
        command_gate: Some(Arc::clone(&gate)),
        queue_capacity: Some(1),
        ..Default::default()
    });
    let tx = driver.begin().await.unwrap();
    let first = tx
        .reserve_exec()
        .await
        .unwrap()
        .enqueue(literal("UPDATE capacity SET n=n+1"), parameters());
    gate.wait().await;
    let second = tx
        .reserve_exec()
        .await
        .unwrap()
        .enqueue(literal("UPDATE capacity SET n=n+1"), parameters());
    assert_eq!(tx.available_capacity(), 0);
    let (owner, scope) = request(7);
    let grant = Grant::<Edit>::from_authorized(scope, 9).unwrap();
    let mut waiting = Box::pin(tx.reserve_exec());
    std::future::poll_fn(|cx| {
        assert!(matches!(waiting.as_mut().poll(cx), Poll::Pending));
        Poll::Ready(())
    })
    .await;
    drop(owner);
    gate.release();
    let reservation = tokio::time::timeout(Duration::from_secs(2), waiting)
        .await
        .unwrap()
        .unwrap();
    let error = grant
        .submit(reservation, |s, t, res| enqueue_reviewed(res, s, t, 999))
        .err()
        .unwrap();
    assert_eq!(error.kind(), auth::FailureKind::Expired);
    assert_eq!(
        tx.enqueued_execs(),
        2,
        "only the two capacity fixture commands were enqueued"
    );
    assert_eq!(first.await.unwrap(), 1);
    assert_eq!(second.await.unwrap(), 1);
    assert_eq!(value(&tx, 9).await, 100);
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}
#[tokio::test]
async fn unused_native_reservation_releases_capacity_without_admission() {
    let driver = Driver::open(Config {
        seed: SEED,
        ..Default::default()
    });
    let tx = driver.begin().await.unwrap();
    let reservation = tx.reserve_exec().await.unwrap();
    assert_eq!(tx.available_capacity(), 0);
    drop(reservation);
    assert_eq!(tx.available_capacity(), 1);
    assert_eq!(tx.enqueued_execs(), 0);
    assert_eq!(value(&tx, 9).await, 100);
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}
