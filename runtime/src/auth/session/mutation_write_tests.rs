//! Finite owned business oracles: no cookie, raw ID, or private cause output.
use super::super::mutation::{
    self,
    write::{complete_write, enqueue_write, EntropySource, PreparedWrite},
};
use super::*;
use crate::auth::{AuthScope, LeaseOwner, VerifiedIdentity};
fn bearer(subject: i64, request_ms: u64, credential_ms: u64) -> (LeaseOwner, AuthScope) {
    let owner = LeaseOwner::new(authority(request_ms)).unwrap();
    let scope = AuthScope::bind(
        VerifiedIdentity::from_verified(subject, authority(credential_ms)).unwrap(),
        Arc::clone(&owner.lease),
    )
    .unwrap();
    (owner, scope)
}
fn session(snapshot: Snapshot) -> (LeaseOwner, AuthScope) {
    let owner = LeaseOwner::new(authority(30000)).unwrap();
    let scope = AuthScope::bind(
        snapshot.into_verified_identity().unwrap(),
        Arc::clone(&owner.lease),
    )
    .unwrap();
    (owner, scope)
}
fn status(owner: &LeaseOwner) -> (bool, bool, bool, bool) {
    super::super::delivery::test_slot_status(&owner.lease).unwrap()
}
fn finite(values: &[u8]) -> EntropySource {
    EntropySource::Finite(
        values
            .iter()
            .map(|v| Ok(secrets::SessionId::test_from_entropy([*v; 32])))
            .collect(),
    )
}
fn digest(v: u8) -> [u8; 32] {
    secrets::SessionId::test_from_entropy([v; 32])
        .digest()
        .into_bytes()
}
async fn rows(pool: &sqlite::Pool) -> Vec<super::super::SessionRow> {
    let tx = sqlite::begin(pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let rows=sqlite::all(&tx,sqlite::literal("SELECT lineage,subject,digest,generation,idle_ms,absolute_ms FROM __nagi_session_rows ORDER BY lineage"),sqlite::parameters()).await.unwrap();
    sqlite::rollback(tx).await.unwrap();
    rows
}
async fn hold_queue<'a, F: std::future::Future>(
    tx: &'a sqlite::Tx,
    mut attempt: std::pin::Pin<&mut F>,
    reached: tokio::sync::oneshot::Receiver<()>,
    resume: tokio::sync::oneshot::Sender<()>,
) -> Vec<sqlite::ExecReservation<'a>> {
    tokio::select! {r=reached=>r.unwrap(),_= &mut attempt=>panic!("attempt ended before real reserve")}
    let mut permits = Vec::new();
    for _ in 0..4 {
        permits.push(tx.reserve_exec().await.unwrap());
    }
    resume.send(()).unwrap();
    assert!(futures_util::poll!(attempt.as_mut()).is_pending());
    permits
}
#[tokio::test]
async fn issue_retains_original_credential_not_short_request_budget() {
    let f = Fixture::new().await;
    let s = f.store().await;
    let (owner, scope) = bearer(7, 1000, 90000);
    let intent = s.issue_delivery(scope).await.unwrap();
    let r = rows(&f.pool).await;
    assert!(
        r.len() == 1
            && r[0].subject == 7
            && r[0].generation == 0
            && r[0].idle_ms == 11000
            && r[0].absolute_ms > 80000
            && r[0].absolute_ms <= 91000
    );
    assert!(status(&owner).2 && status(&owner).3);
    drop(intent);
    assert!(!status(&owner).3);
    f.stop().await;
}
#[tokio::test]
async fn issue_clamps_original_short_credential_and_settings() {
    let f = Fixture::new().await;
    let s = f.store().await;
    let (_owner, scope) = bearer(7, 30000, 5000);
    let intent = s.issue_delivery(scope).await.unwrap();
    let r = rows(&f.pool).await;
    assert!(
        r.len() == 1
            && r[0].absolute_ms <= 6000
            && r[0].absolute_ms > 5000
            && r[0].idle_ms == r[0].absolute_ms
    );
    drop(intent);
    f.stop().await;
}
#[tokio::test]
async fn issue_real_queue_wait_samples_clock_after_capacity() {
    let f = Fixture::new().await;
    let s = f.store().await;
    let (owner, scope) = bearer(7, 30000, 90000);
    let PreparedWrite {
        tx,
        mut context,
        mut progress,
    } = s.prepare_issue(scope).await.unwrap();
    let (reached, resume) = context.pause_before_reserve();
    let result = {
        let attempt = enqueue_write(&s, &tx, context, &mut progress);
        tokio::pin!(attempt);
        let hold = hold_queue(&tx, attempt.as_mut(), reached, resume).await;
        f.wall.store(2000, Ordering::SeqCst);
        drop(hold);
        attempt.await
    };
    assert!(tx.enqueued_execs() == 1);
    let intent = complete_write(&s, tx, result, progress).await.unwrap();
    let r = rows(&f.pool).await;
    assert!(r.len() == 1 && r[0].idle_ms == 12000 && status(&owner).2);
    drop(intent);
    f.stop().await;
}
#[tokio::test]
async fn issue_owner_end_during_real_wait_sends_zero() {
    let f = Fixture::new().await;
    let s = f.store().await;
    let (owner, scope) = bearer(7, 30000, 90000);
    let PreparedWrite {
        tx,
        mut context,
        mut progress,
    } = s.prepare_issue(scope).await.unwrap();
    let (reached, resume) = context.pause_before_reserve();
    let result = {
        let attempt = enqueue_write(&s, &tx, context, &mut progress);
        tokio::pin!(attempt);
        let hold = hold_queue(&tx, attempt.as_mut(), reached, resume).await;
        drop(owner);
        drop(hold);
        attempt.await
    };
    assert!(result.is_err() && tx.enqueued_execs() == 0);
    assert!(complete_write(&s, tx, result, progress).await.is_err());
    assert!(rows(&f.pool).await.is_empty());
    f.stop().await;
}
#[tokio::test]
async fn issue_unadmitted_future_drop_releases_reservations_before_pool_join() {
    let f = Fixture::new().await;
    let s = f.store().await;
    let (owner, scope) = bearer(7, 30000, 90000);
    let PreparedWrite {
        tx,
        mut context,
        mut progress,
    } = s.prepare_issue(scope).await.unwrap();
    let (reached, resume) = context.pause_before_reserve();
    {
        let attempt = enqueue_write(&s, &tx, context, &mut progress);
        tokio::pin!(attempt);
        let hold = hold_queue(&tx, attempt.as_mut(), reached, resume).await;
        drop(attempt);
        drop(hold);
    }
    assert!(tx.enqueued_execs() == 0 && status(&owner).0 && !status(&owner).3);
    drop(tx);
    sqlite::close(&f.pool, 2000).await.unwrap();
    assert!(sqlite::begin(&f.pool, sqlite::BeginMode::Immediate)
        .await
        .is_err());
    f.stop().await;
}
#[tokio::test]
async fn admitted_issue_drop_is_terminal_then_actual_cleanup_and_join() {
    let f = Fixture::new().await;
    let s = f.store().await;
    let (owner, scope) = bearer(7, 30000, 90000);
    let PreparedWrite {
        tx,
        context,
        mut progress,
    } = s.prepare_issue(scope).await.unwrap();
    let sent = enqueue_write(&s, &tx, context, &mut progress)
        .await
        .unwrap();
    assert!(tx.enqueued_execs() == 1 && status(&owner).1);
    drop(sent);
    assert!(!status(&owner).0 && !status(&owner).2 && !status(&owner).3);
    drop(tx);
    sqlite::close(&f.pool, 2000).await.unwrap();
    let reopened = sqlite::open(
        f.directory.join("owned.sqlite").to_str().unwrap(),
        sqlite::options(1, 4, 1000, 1000).unwrap(),
    )
    .await
    .unwrap();
    assert!(rows(&reopened).await.is_empty());
    sqlite::close(&reopened, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn issue_full_capacity_refuses_without_business_enqueue_or_eviction() {
    let f = Fixture::new().await;
    let s = f.store().await;
    s.insert(7, [1; 32], authority(90000)).await.unwrap();
    s.insert(8, [2; 32], authority(90000)).await.unwrap();
    let (owner, scope) = bearer(9, 30000, 90000);
    let PreparedWrite {
        tx,
        context,
        mut progress,
    } = s.prepare_issue(scope).await.unwrap();
    let result = enqueue_write(&s, &tx, context, &mut progress).await;
    assert!(result.is_err() && tx.enqueued_execs() == 0);
    assert!(complete_write(&s, tx, result, progress).await.is_err());
    assert!(rows(&f.pool).await.len() == 2 && !status(&owner).3);
    f.stop().await;
}
#[tokio::test]
async fn issue_collision_regenerates_within_finite_budget() {
    let f = Fixture::new().await;
    let s = f.store().await;
    s.insert(7, digest(1), authority(90000)).await.unwrap();
    let (owner, scope) = bearer(8, 30000, 90000);
    let PreparedWrite {
        tx,
        context,
        mut progress,
    } = s.prepare_issue_with(scope, finite(&[1, 2])).await.unwrap();
    let result = enqueue_write(&s, &tx, context, &mut progress).await;
    assert!(tx.enqueued_execs() == 1);
    let intent = complete_write(&s, tx, result, progress).await.unwrap();
    let r = rows(&f.pool).await;
    assert!(r.len() == 2 && r[1].digest == digest(2) && status(&owner).2);
    drop(intent);
    f.stop().await;
}
#[tokio::test]
async fn issue_collision_budget_exhaustion_has_no_intent_or_send() {
    let f = Fixture::new().await;
    let s = f.store().await;
    s.insert(7, digest(1), authority(90000)).await.unwrap();
    let (owner, scope) = bearer(8, 30000, 90000);
    let PreparedWrite {
        tx,
        context,
        mut progress,
    } = s.prepare_issue_with(scope, finite(&[1, 1])).await.unwrap();
    let result = enqueue_write(&s, &tx, context, &mut progress).await;
    assert!(result.is_err() && tx.enqueued_execs() == 0);
    assert!(complete_write(&s, tx, result, progress).await.is_err());
    assert!(rows(&f.pool).await.len() == 1 && !status(&owner).3);
    f.stop().await;
}
#[tokio::test]
async fn issue_entropy_error_has_no_fallback_pending_or_business_work() {
    let f = Fixture::new().await;
    let s = f.store().await;
    let (owner, scope) = bearer(7, 30000, 90000);
    let source = EntropySource::Finite([Err(secrets::FailureKind::Unavailable)].into());
    assert!(s.prepare_issue_with(scope, source).await.is_err());
    assert!(status(&owner).0 && !status(&owner).3 && rows(&f.pool).await.is_empty());
    f.stop().await;
}
#[tokio::test]
async fn issue_actual_unknown_and_committed_cleanup_never_ready() {
    for fault in [
        sqlite::LogoutTestFault::Unknown,
        sqlite::LogoutTestFault::CommittedCleanup,
    ] {
        let f = Fixture::new().await;
        let mut s = f.store().await;
        let (owner, scope) = bearer(7, 30000, 90000);
        let p = sqlite::logout_test_pool(f.directory.join("owned.sqlite"), fault);
        s.pool = sqlite::clone_pool(&p);
        let e = s.issue_delivery(scope).await.err().unwrap();
        assert!(
            outcome(&e)
                == match fault {
                    sqlite::LogoutTestFault::Unknown => sqlite::Outcome::Unknown,
                    _ => sqlite::Outcome::Committed,
                }
        );
        assert!(!status(&owner).2 && !status(&owner).3);
        assert!(sqlite::close(&p, 2000).await.is_err());
        f.stop().await;
    }
}
#[tokio::test]
async fn rotation_cas_one_winner_old_digest_invalid_absolute_unchanged() {
    let f = Fixture::new().await;
    let s = f.store().await;
    let old = s.insert(7, [1; 32], authority(90000)).await.unwrap();
    let lineage = old.lineage;
    let absolute = old.absolute_ms;
    let (one, a) = session(old.private_copy());
    let (two, b) = session(old);
    let barrier = tokio::sync::Barrier::new(2);
    let (a, b) = tokio::join!(
        async {
            barrier.wait().await;
            s.rotate_delivery(a).await
        },
        async {
            barrier.wait().await;
            s.rotate_delivery(b).await
        }
    );
    assert!(a.is_ok() != b.is_ok());
    let r = rows(&f.pool).await;
    assert!(
        r.len() == 1
            && r[0].lineage == lineage
            && r[0].generation == 1
            && r[0].absolute_ms == absolute
            && s.lookup([1; 32]).await.is_err()
    );
    assert!(status(&one).2 != status(&two).2);
    drop((a, b));
    f.stop().await;
}
#[tokio::test]
async fn rotation_real_wait_cannot_reactivate_immutable_lookup() {
    let f = Fixture::new().await;
    let s = f.store().await;
    let (owner, scope) = session(s.insert(7, [1; 32], authority(90000)).await.unwrap());
    let PreparedWrite {
        tx,
        mut context,
        mut progress,
    } = s.prepare_rotate(scope).await.unwrap();
    let (reached, resume) = context.pause_before_reserve();
    let result = {
        let attempt = enqueue_write(&s, &tx, context, &mut progress);
        tokio::pin!(attempt);
        let hold = hold_queue(&tx, attempt.as_mut(), reached, resume).await;
        f.wall.store(20000, Ordering::SeqCst);
        drop(hold);
        attempt.await
    };
    assert!(result.is_err() && tx.enqueued_execs() == 0);
    assert!(complete_write(&s, tx, result, progress).await.is_err());
    let r = rows(&f.pool).await;
    assert!(r.len() == 1 && r[0].generation == 0 && !status(&owner).3);
    f.stop().await;
}
#[tokio::test]
async fn rotation_foreign_store_incarnation_refuses_before_slot() {
    let a = Fixture::new().await;
    let b = Fixture::new().await;
    let s = a.store().await;
    let other =
        Foundation::initialize(&b.pool, settings(), "__Host-example", 1, [4; 32], b.clock())
            .await
            .unwrap();
    let (owner, scope) = session(s.insert(7, [1; 32], authority(90000)).await.unwrap());
    assert!(other.rotate_delivery(scope).await.is_err());
    assert!(status(&owner).0 && !status(&owner).3 && rows(&b.pool).await.is_empty());
    a.stop().await;
    b.stop().await;
}
#[tokio::test]
async fn committed_logout_wins_over_later_rotation_cas() {
    let f = Fixture::new().await;
    let s = f.store().await;
    let old = s.insert(7, [1; 32], authority(90000)).await.unwrap();
    let (_a, logout) = session(old.private_copy());
    let (owner, rotate) = session(old);
    let intent = s.logout_delivery(logout).await.unwrap();
    assert!(s.rotate_delivery(rotate).await.is_err());
    assert!(rows(&f.pool).await.is_empty() && !status(&owner).3);
    drop(intent);
    f.stop().await;
}
#[tokio::test]
async fn rotation_actual_unknown_and_committed_cleanup_never_ready() {
    for fault in [
        sqlite::LogoutTestFault::Unknown,
        sqlite::LogoutTestFault::CommittedCleanup,
    ] {
        let f = Fixture::new().await;
        let mut s = f.store().await;
        let (owner, scope) = session(s.insert(7, [1; 32], authority(90000)).await.unwrap());
        let p = sqlite::logout_test_pool(f.directory.join("owned.sqlite"), fault);
        s.pool = sqlite::clone_pool(&p);
        let e = s.rotate_delivery(scope).await.err().unwrap();
        assert!(
            outcome(&e)
                == match fault {
                    sqlite::LogoutTestFault::Unknown => sqlite::Outcome::Unknown,
                    _ => sqlite::Outcome::Committed,
                }
        );
        assert!(!status(&owner).2 && !status(&owner).3);
        assert!(sqlite::close(&p, 2000).await.is_err());
        f.stop().await;
    }
}
#[tokio::test]
async fn logout_after_rotation_preserves_same_subject_other_lineage() {
    let f = Fixture::new().await;
    let s = f.store().await;
    let old = s.insert(7, [1; 32], authority(90000)).await.unwrap();
    s.insert(7, [2; 32], authority(90000)).await.unwrap();
    let (_a, rotate) = session(old.private_copy());
    let (_b, logout) = session(old);
    let rotated = s.rotate_delivery(rotate).await.unwrap();
    let removed = s.logout_delivery(logout).await.unwrap();
    assert!(
        rows(&f.pool).await.len() == 1
            && s.lookup([2; 32]).await.is_ok()
            && s.lookup([1; 32]).await.is_err()
    );
    drop((rotated, removed));
    f.stop().await;
}
#[tokio::test]
async fn confirmed_absent_logout_is_idempotent_only_after_actual_commit() {
    let f = Fixture::new().await;
    let s = f.store().await;
    let old = s.insert(7, [1; 32], authority(90000)).await.unwrap();
    let (_a, first) = session(old.private_copy());
    let (owner, again) = session(old);
    let intent = s.logout_delivery(first).await.unwrap();
    drop(intent);
    let mutation::PreparedLogout {
        tx,
        context,
        mut progress,
    } = s.prepare_logout(again).await.unwrap();
    let sent = mutation::enqueue_logout(&s, &tx, context, &mut progress)
        .await
        .unwrap();
    assert!(sent.expected == 0 && tx.enqueued_execs() == 1);
    let intent = mutation::complete_logout(&s, tx, Ok(sent), progress)
        .await
        .unwrap();
    assert!(rows(&f.pool).await.is_empty() && status(&owner).2);
    drop(intent);
    f.stop().await;
}
